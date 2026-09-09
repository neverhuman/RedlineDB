//! Per-session connection handle and transaction scaffolding.

use std::cell::Cell;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Duration;

use redlinedb_sql::BeginMode;

use crate::error::{Error, ErrorCode, Result};
use crate::iter::{FromRow, OwnedStep, Step};
use crate::options::{CommitStats, ConnectionStats, ExecuteSummary, FunctionArity, FunctionFlags};
use crate::params::Params;
use crate::statement::{OwnedStatement, Rows, Statement};
use crate::value::{Value, ValueRef};

/// Per-session connection handle.
///
/// `Connection` is `Send` but not `Sync`. Move it between threads if you
/// need to, but do not share a single handle for concurrent mutation;
/// create one connection per thread or guard it with external locking.
pub struct Connection {
    pub(crate) inner: Arc<redlinedb_sql::Connection>,
    pub(crate) read_only: bool,
    pub(crate) busy_timeout: Duration,
    pub(crate) interrupted: Arc<AtomicBool>,
    pub(crate) _sync_marker: Cell<()>,
}

/// Scoped transaction wrapper that auto-rolls-back on drop.
pub struct Transaction<'conn> {
    conn: &'conn mut Connection,
    committed: bool,
}

/// Cloneable trip-wire that can interrupt the parent connection.
#[derive(Clone)]
pub struct InterruptHandle {
    flag: Arc<AtomicBool>,
}

impl Connection {
    pub fn prepare<'c>(&'c mut self, sql: &str) -> Result<Statement<'c>> {
        Ok(Statement {
            inner: self.prepare_owned(sql)?,
            _conn: std::marker::PhantomData,
        })
    }

    pub fn prepare_owned(&mut self, sql: &str) -> Result<OwnedStatement> {
        self.check_interrupt()?;
        let stmt = self.inner.prepare(sql)?;
        if self.read_only && !stmt.is_readonly() {
            return Err(Error::new(ErrorCode::ReadOnly, "connection is read-only"));
        }
        Ok(OwnedStatement {
            inner: stmt,
            interrupted: Arc::clone(&self.interrupted),
            _marker: Rc::new(()),
        })
    }

    pub fn prepare_rql<'c>(
        &'c mut self,
        statement: &redlinedb_sql::RqlStatement,
    ) -> Result<Statement<'c>> {
        Ok(Statement {
            inner: self.prepare_rql_owned(statement)?,
            _conn: std::marker::PhantomData,
        })
    }

    pub fn prepare_rql_owned(
        &mut self,
        statement: &redlinedb_sql::RqlStatement,
    ) -> Result<OwnedStatement> {
        self.check_interrupt()?;
        let stmt = self.inner.prepare_rql(statement)?;
        if self.read_only && !stmt.is_readonly() {
            return Err(Error::new(ErrorCode::ReadOnly, "connection is read-only"));
        }
        Ok(OwnedStatement {
            inner: stmt,
            interrupted: Arc::clone(&self.interrupted),
            _marker: Rc::new(()),
        })
    }

    pub fn prepare_v2<'sql>(
        &mut self,
        sql: &'sql str,
    ) -> Result<(Option<OwnedStatement>, &'sql str)> {
        self.check_interrupt()?;
        let (stmt, tail) = self.inner.prepare_v2(sql)?;
        let Some(stmt) = stmt else {
            return Ok((None, tail));
        };
        if self.read_only && !stmt.is_readonly() {
            return Err(Error::new(ErrorCode::ReadOnly, "connection is read-only"));
        }
        Ok((
            Some(OwnedStatement {
                inner: stmt,
                interrupted: Arc::clone(&self.interrupted),
                _marker: Rc::new(()),
            }),
            tail,
        ))
    }

    pub fn prepare_cached<'c>(&'c mut self, sql: &str) -> Result<Statement<'c>> {
        self.prepare(sql)
    }

    pub fn query<'c, P: Params>(&'c mut self, sql: &str, params: P) -> Result<Rows<'c>> {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        Ok(Rows { stmt })
    }

    pub fn execute<P: Params>(&mut self, sql: &str, params: P) -> Result<ExecuteSummary> {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        let mut rows = 0_u64;
        while let Step::Row(_) = stmt.step()? {
            rows += 1;
        }
        Ok(ExecuteSummary {
            rows_affected: stmt.affected_rows() as u64,
            rows_returned: rows,
        })
    }

    /// Execute one or more SQL statements, discarding per-statement row counts.
    ///
    /// The order and transaction semantics follow the `Connection::execute`
    /// behavior in this crate, and execution stops on first error.
    pub fn execute_batch(&mut self, sql: &str) -> Result<()> {
        let mut rest = sql;
        loop {
            let (statement, tail) = self.prepare_v2(rest)?;
            if let Some(mut statement) = statement {
                while let OwnedStep::Row = statement.step()? {}
            }
            if tail.is_empty() {
                return Ok(());
            }
            rest = tail;
        }
    }

    pub fn execute_rql(&mut self, program: &redlinedb_sql::RqlProgram) -> Result<ExecuteSummary> {
        let mut rows_affected = 0_u64;
        let mut rows_returned = 0_u64;
        for statement in &program.statements {
            let mut stmt = self.prepare_rql(statement)?;
            while let Step::Row(_) = stmt.step()? {
                rows_returned += 1;
            }
            rows_affected += stmt.affected_rows() as u64;
        }
        Ok(ExecuteSummary {
            rows_affected,
            rows_returned,
        })
    }

    /// Fetch the first row of a query and map it via the `FromRow` trait.
    /// Returns `ErrorCode::NotFound` if the query produced zero rows.
    pub fn query_row<P, T>(&mut self, sql: &str, params: P) -> Result<T>
    where
        P: Params,
        T: FromRow,
    {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        match stmt.step()? {
            Step::Row(row) => T::from_row(&row),
            Step::Done => Err(Error::new(ErrorCode::NotFound, "query_row: no rows")),
        }
    }

    /// Fetch the first row of a query if any, mapped via `FromRow`.
    /// Returns `Ok(None)` if no rows were produced.
    pub fn query_row_opt<P, T>(&mut self, sql: &str, params: P) -> Result<Option<T>>
    where
        P: Params,
        T: FromRow,
    {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        match stmt.step()? {
            Step::Row(row) => T::from_row(&row).map(Some),
            Step::Done => Ok(None),
        }
    }

    pub fn begin(&mut self, mode: BeginMode) -> Result<()> {
        self.check_interrupt()?;
        self.inner.begin(mode)?;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<CommitStats> {
        self.check_interrupt()?;
        self.inner.commit()?;
        Ok(CommitStats {
            changes: self.inner.changes() as u64,
        })
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.inner.rollback()?;
        Ok(())
    }

    pub fn in_transaction(&self) -> bool {
        self.inner.in_transaction()
    }

    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        self.begin(BeginMode::Deferred)?;
        let mut tx = Transaction {
            conn: self,
            committed: false,
        };
        let result = f(&mut tx);
        if result.is_ok() && !tx.committed {
            tx.commit()?;
        } else if result.is_err() && !tx.committed {
            let _ = tx.rollback();
        }
        result
    }

    pub fn set_busy_timeout(&mut self, timeout: Duration) {
        self.busy_timeout = timeout;
        self.inner.set_busy_timeout(timeout);
    }

    pub fn interrupt_handle(&self) -> InterruptHandle {
        InterruptHandle {
            flag: Arc::clone(&self.interrupted),
        }
    }

    pub fn changes(&self) -> u64 {
        self.inner.changes() as u64
    }

    pub fn last_insert_rowid(&self) -> Option<i64> {
        self.inner.last_insert_rowid()
    }

    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            changes: self.changes(),
            last_insert_rowid: self.last_insert_rowid(),
            busy_timeout_ms: self.busy_timeout.as_millis() as u64,
            interrupted: self.interrupted.load(AtomicOrdering::Relaxed),
        }
    }

    pub fn create_scalar_function<F>(
        &mut self,
        _name: &str,
        _arity: FunctionArity,
        _flags: FunctionFlags,
        _f: F,
    ) -> Result<()>
    where
        F: Send + Sync + 'static + Fn(&[ValueRef<'_>]) -> Result<Value>,
    {
        Err(Error::unsupported(
            "scalar function hooks are reserved for a follow-on milestone",
        ))
    }

    pub fn create_collation<F>(&mut self, _name: &str, _cmp: F) -> Result<()>
    where
        F: Send + Sync + 'static + Fn(&str, &str) -> Ordering,
    {
        Err(Error::unsupported(
            "collation hooks are reserved for a follow-on milestone",
        ))
    }

    fn check_interrupt(&self) -> Result<()> {
        if self.interrupted.load(AtomicOrdering::Relaxed) {
            return Err(Error::new(ErrorCode::Interrupt, "interrupted"));
        }
        Ok(())
    }
}

impl<'conn> Transaction<'conn> {
    pub fn execute<P: Params>(&mut self, sql: &str, params: P) -> Result<ExecuteSummary> {
        self.conn.execute(sql, params)
    }

    pub fn execute_rql(&mut self, program: &redlinedb_sql::RqlProgram) -> Result<ExecuteSummary> {
        self.conn.execute_rql(program)
    }

    pub fn prepare<'a>(&'a mut self, sql: &str) -> Result<Statement<'a>> {
        self.conn.prepare(sql)
    }

    pub fn prepare_rql<'a>(
        &'a mut self,
        statement: &redlinedb_sql::RqlStatement,
    ) -> Result<Statement<'a>> {
        self.conn.prepare_rql(statement)
    }

    pub fn commit(&mut self) -> Result<()> {
        if !self.committed {
            let result = self.conn.commit();
            self.committed = true;
            result?;
        }
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.committed {
            self.conn.rollback()?;
            self.committed = true;
        }
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.rollback();
        }
    }
}

impl InterruptHandle {
    pub fn interrupt(&self) {
        self.flag.store(true, AtomicOrdering::Relaxed);
    }
}
