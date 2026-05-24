use std::cell::Cell;
use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

use redlinedb_kernel::catalog::{
    ColumnStats, ConstraintKind, EvalScratch, HistogramBucket, IndexStats, MostCommonValue,
    OwnedValue, RecordRef, RecordScratch, RowValueSource, SchemaSnapshot, SqliteSchemaRow,
    StatsEpoch, StatsSnapshot, TableDef, TableStats, ValueRef, apply_affinity, encode_record,
    eval_expr,
};
use redlinedb_kernel::engine::{CommitOutcome, Engine, Txn};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::txn::Isolation;
use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArgumentClause, FunctionArguments,
    OrderByExpr, SelectItem, UnaryOperator, Value,
};

use crate::batch::{
    ExecContext, ExecNode, ExecState, MaterializeNode, QueryMemoryBroker, RowBatch, RowLayout,
};
use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::planner::{self, ExplainMetrics};
use crate::session::SessionState;
use crate::statement::{
    AnalyzePlan, CreateTableAsSelectSpec, DmlValue, ExecutionResult, ExplainPlan, PragmaPlan,
    PreparedKind, PreparedTemplate, RuntimeState, SelectPlan, SelectRuntime, SelectRuntimeSource,
    SelectRuntimeTx, SelectSource,
};
use crate::value::{SqlValue, canonicalize, compare_values, is_truthy};

pub(crate) mod expr;
use expr::*;
pub(crate) mod index_access;
pub(crate) mod index_batch;
pub(crate) mod index_dml;
pub(crate) mod index_partial;
pub(crate) mod index_predicate;
pub(crate) mod policy;
mod tail;
use tail::*;
pub(crate) mod vec;

mod agg;
mod agg_eval;
pub(crate) mod intern;
use agg::*;
mod insert;
use insert::*;
mod select_top;
use select_top::*;
pub(crate) mod attach;
pub(crate) mod cross_db;
pub(crate) mod cte;
pub(crate) mod fk;
pub(crate) mod json_tv;
// Track K — SQL:2003 MERGE dispatch.
pub(crate) mod merge;
pub(crate) mod pragma_tv;
pub(crate) mod set_ops;
pub(crate) mod table_valued;
pub(crate) mod trigger;
pub(crate) mod view;
pub(crate) mod window;

thread_local! {
    static CURRENT_CONNECTION: Cell<*const Connection> = const { Cell::new(std::ptr::null()) };
    static CURRENT_TX: Cell<*mut Txn> = const { Cell::new(std::ptr::null_mut()) };
    /// Lane A5-triggers: pointer to the currently-locked SessionState.
    /// Set by [`with_write_tx`] inside `with_session`, cleared on exit.
    /// Re-entrant calls (e.g. trigger body fires) can borrow it directly
    /// instead of re-locking the session mutex (which would deadlock).
    static CURRENT_SESSION: Cell<*mut SessionState> = const { Cell::new(std::ptr::null_mut()) };
    /// Stack of *owned* `SqlRow` snapshots captured by enclosing query
    /// scopes. Inner-scope evaluators can walk this stack to resolve
    /// correlated identifiers (`outer_table.col`) when the immediate row
    /// context does not contain them.
    static OUTER_ROW_STACK: std::cell::RefCell<Vec<crate::exec::expr::scalar::row::SqlRow>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static CORRELATED_LOOKUP_USED: Cell<bool> = const { Cell::new(false) };
}

/// Set the per-thread current-session pointer for the duration of `f`.
/// Used by [`with_write_tx`] so re-entrant trigger fires can reuse the
/// session without deadlocking the mutex.
fn with_current_session<T>(ptr: *mut SessionState, f: impl FnOnce() -> T) -> T {
    CURRENT_SESSION.with(|cell| {
        let prev = cell.replace(ptr);
        let result = f();
        cell.set(prev);
        result
    })
}

pub(crate) fn current_session_ptr() -> Option<*mut SessionState> {
    with_current_session_ptr()
}

fn with_current_session_ptr() -> Option<*mut SessionState> {
    CURRENT_SESSION.with(|cell| {
        let p = cell.get();
        if p.is_null() { None } else { Some(p) }
    })
}

/// Run `f` against the connection's session state. Prefers the
/// thread-local current-session pointer when inside a re-entrant call
/// (e.g. a trigger body fired during a parent DML); otherwise acquires
/// the session mutex via `Connection::with_session`. Without this
/// shortcut, nested `with_session` calls deadlock on the non-re-entrant
/// `parking_lot::Mutex`.
fn with_session_reentrant<T>(
    conn: &Connection,
    f: impl FnOnce(&mut SessionState) -> Result<T>,
) -> Result<T> {
    if let Some(ptr) = with_current_session_ptr() {
        // SAFETY: the pointer was installed by an outer `with_write_tx`
        // and is valid for the duration of that closure. Re-entrant
        // trigger fires run strictly synchronously inside that scope.
        let session_ref: &mut SessionState = unsafe { &mut *ptr };
        return f(session_ref);
    }
    conn.with_session(f)
}

/// Push a correlated-scope row onto the thread-local stack for the
/// duration of `f`. Used by subquery evaluators so that an outer
/// `Expr::CompoundIdentifier(a.id)` resolves against the caller's row when
/// the immediate scope does not bind the qualifier.
pub(crate) fn with_outer_row<T>(
    row: crate::exec::expr::scalar::row::SqlRow,
    f: impl FnOnce() -> T,
) -> T {
    OUTER_ROW_STACK.with(|cell| cell.borrow_mut().push(row));
    let result = f();
    OUTER_ROW_STACK.with(|cell| {
        cell.borrow_mut().pop();
    });
    result
}

/// Push a row onto the correlated-scope stack without an enclosing
/// closure. Pair with [`pop_outer_row`]. Used by the trigger fire-hook
/// because BEFORE/AFTER firing happens across multiple SQL statement
/// invocations that cannot share a single closure scope.
pub(crate) fn push_outer_row(row: crate::exec::expr::scalar::row::SqlRow) {
    OUTER_ROW_STACK.with(|cell| cell.borrow_mut().push(row));
}

/// Pop the most-recently pushed [`push_outer_row`] frame.
pub(crate) fn pop_outer_row() {
    OUTER_ROW_STACK.with(|cell| {
        cell.borrow_mut().pop();
    });
}

/// Walk the correlated-scope stack (from innermost to outermost) and
/// apply `f` to each frame's row context. Returns the first `Some` value.
pub(crate) fn lookup_correlated<T>(
    f: impl Fn(&crate::exec::expr::scalar::row::RowContext<'_>) -> Option<T>,
) -> Option<T> {
    OUTER_ROW_STACK.with(|cell| {
        let stack = cell.borrow();
        for row in stack.iter().rev() {
            let ctx = row.context();
            if let Some(v) = f(&ctx) {
                CORRELATED_LOOKUP_USED.with(|used| used.set(true));
                return Some(v);
            }
        }
        None
    })
}

pub(crate) fn with_correlated_lookup_tracking<T>(f: impl FnOnce() -> T) -> (T, bool) {
    CORRELATED_LOOKUP_USED.with(|used| {
        let prev = used.replace(false);
        let result = f();
        let current = used.get();
        used.set(prev || current);
        (result, current)
    })
}

pub(crate) fn with_current_connection<T>(conn: &Connection, f: impl FnOnce() -> T) -> T {
    CURRENT_CONNECTION.with(|cell| {
        let prev = cell.replace(conn as *const Connection);
        if prev.is_null() {
            expr::clear_subquery_template_cache();
            crate::json::scalar::clear_json_caches();
        }
        let result = f();
        if prev.is_null() {
            expr::clear_subquery_template_cache();
            crate::json::scalar::clear_json_caches();
        }
        cell.set(prev);
        result
    })
}

pub(crate) fn current_connection() -> Option<&'static Connection> {
    CURRENT_CONNECTION.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() {
            None
        } else {
            // SAFETY: the pointer is installed only for the duration of statement execution.
            unsafe { ptr.as_ref() }
        }
    })
}

pub(crate) fn with_current_tx<T>(tx: *mut Txn, f: impl FnOnce() -> T) -> T {
    CURRENT_TX.with(|cell| {
        let prev = cell.replace(tx);
        let result = f();
        cell.set(prev);
        result
    })
}

pub(crate) fn current_tx() -> Option<*mut Txn> {
    CURRENT_TX.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() { None } else { Some(ptr) }
    })
}

fn select_tx_ptr(tx: &mut SelectRuntimeTx) -> Option<*mut Txn> {
    tx.as_mut().map(|tx| tx as *mut Txn)
}

pub(crate) fn current_tx_schema_snapshot(conn: &Connection) -> Option<Arc<SchemaSnapshot>> {
    let tx_ptr = current_tx()?;
    if let Some(active) = current_connection()
        && !std::ptr::eq(active, conn)
    {
        return None;
    }
    // SAFETY: `tx_ptr` is installed by `with_write_tx` for a strictly
    // synchronous execution scope. Re-entrant trigger preparation can read
    // its pending schema snapshot without re-locking the session mutex.
    let tx_ref: &Txn = unsafe { &*tx_ptr };
    Some(conn.engine().schema_snapshot_for_tx(tx_ref))
}

pub fn execute_prepared(
    conn: &Connection,
    template: &PreparedTemplate,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    // PRAGMA query_only mirrors SQLite: prevent any write-side DDL/DML
    // while still allowing reads, transaction control, and PRAGMA setters
    // that read-only callers legitimately need.
    if template_writes(&template.kind)
        && with_session_reentrant(conn, |session| Ok(session.query_only))?
    {
        return Err(Error::UnsupportedSql(
            "attempt to write while PRAGMA query_only is set".to_owned(),
        ));
    }
    match &template.kind {
        PreparedKind::Begin(mode) => {
            conn.begin(*mode)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::Commit => {
            conn.commit()?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::Rollback => {
            conn.rollback()?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::Pragma(plan) => {
            // Several SQLite SET-style PRAGMAs echo the freshly assigned
            // value back as a single-row result set rather than returning
            // silently. The parser flags these by attaching a non-empty
            // `output_columns` list (e.g. `["journal_mode"]`). We honour
            // that flag here by routing the response through a static
            // row source whose payload is derived from the plan variant.
            if let Some(echo_value) = pragma_set_echo_value(plan) {
                execute_pragma(conn, plan)?;
                return Ok(ExecutionResult {
                    runtime: RuntimeState::Select(SelectRuntime {
                        tx: SelectRuntimeTx::Empty,
                        restore_tx: false,
                        source: SelectRuntimeSource::StaticRows {
                            rows: Arc::from(vec![vec![echo_value]]),
                            cursor: 0,
                        },
                        selection: None,
                        projection: Vec::new(),
                        limit: usize::MAX,
                        offset: 0,
                        seen: 0,
                        yielded: 0,
                        memory: QueryMemoryBroker::new(0, 0, None),
                    }),
                    affected_rows: 0,
                });
            }
            execute_pragma(conn, plan)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::Reindex => Ok(ExecutionResult {
            runtime: RuntimeState::Done,
            affected_rows: 0,
        }),
        PreparedKind::Vacuum => {
            conn.vacuum()?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::VacuumInto { path } => {
            vacuum_into_dir(conn, path.as_ref())?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::CreateTable(spec) => {
            with_write_tx(conn, |session, tx| {
                let table = conn.engine().create_table(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                session.last_insert_rowid = Some(table.table_id.0 as i64);
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::CreateTempTable(spec) => {
            with_write_tx(conn, |session, tx| {
                let table = conn.engine().create_table(tx, spec.clone())?;
                if !session
                    .temp_tables
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(spec.name.original()))
                {
                    session.temp_tables.push(spec.name.original().to_owned());
                }
                session.changes += 1;
                session.total_changes += 1;
                session.last_insert_rowid = Some(table.table_id.0 as i64);
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::CreateTableAsSelect(spec) => {
            execute_create_table_as_select(conn, spec, bindings)
        }
        PreparedKind::CreateIndex(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().create_index(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::CreateVirtualTable(plan) => {
            execute_create_virtual_table(conn, plan)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::DropTable(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().drop_table(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::DropIndex(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().drop_index(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::CreateView(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().create_view(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::DropView(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().drop_view(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::CreateTrigger(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().create_trigger(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::DropTrigger(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().drop_trigger(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::AlterTable(spec) => {
            // Surface `PRAGMA legacy_alter_table` to the kernel via the
            // per-thread flag the catalog ops module reads when
            // rewriting dependent view / trigger bodies after a column
            // / table rename. Snapshot the bit, install, run, restore.
            let prev_legacy =
                redlinedb_kernel::catalog::legacy_alter_table_active_for_tests();
            redlinedb_kernel::catalog::set_legacy_alter_table(conn.legacy_alter_table());
            let alter_result = with_write_tx(conn, |session, tx| {
                conn.engine().alter_table(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            });
            redlinedb_kernel::catalog::set_legacy_alter_table(prev_legacy);
            alter_result?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::Insert(plan) => {
            let result = execute_insert(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                with_session_reentrant(conn, |session| {
                    session.changes += result.affected_rows;
                    session.total_changes += result.affected_rows;
                    Ok(())
                })?;
            }
            Ok(result)
        }
        PreparedKind::InsertView(plan) => {
            let mut affected = 0usize;
            for row in &plan.rows {
                let values = row
                    .iter()
                    .map(|value| match value {
                        DmlValue::Expr(expr) => eval_scalar(expr, &RowContext::Empty, bindings),
                        DmlValue::Default => Ok(SqlValue::Null),
                    })
                    .collect::<Result<Vec<_>>>()?;
                trigger::fire_instead_of_insert(conn, &plan.view_name, &plan.columns, values)?;
                affected += 1;
            }
            if affected > 0 {
                with_session_reentrant(conn, |session| {
                    session.changes += affected;
                    session.total_changes += affected;
                    Ok(())
                })?;
            }
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: affected,
            })
        }
        PreparedKind::Update(plan) => {
            let result = execute_update(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                with_session_reentrant(conn, |session| {
                    session.changes += result.affected_rows;
                    session.total_changes += result.affected_rows;
                    Ok(())
                })?;
            }
            Ok(result)
        }
        PreparedKind::Delete(plan) => {
            let result = execute_delete(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                with_session_reentrant(conn, |session| {
                    session.changes += result.affected_rows;
                    session.total_changes += result.affected_rows;
                    Ok(())
                })?;
            }
            Ok(result)
        }
        PreparedKind::Merge(plan) => {
            let result = crate::exec::merge::execute_merge(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                with_session_reentrant(conn, |session| {
                    session.changes += result.affected_rows;
                    session.total_changes += result.affected_rows;
                    Ok(())
                })?;
            }
            Ok(result)
        }
        PreparedKind::Analyze(plan) => {
            analyze_database(conn, plan)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::Explain(plan) => {
            let runtime = execute_explain(conn, plan, bindings)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Select(runtime),
                affected_rows: 0,
            })
        }
        PreparedKind::Select(plan) => {
            let runtime = execute_select(conn, plan, bindings)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Select(runtime),
                affected_rows: 0,
            })
        }
        PreparedKind::Attach(plan) => {
            attach::apply_attach_plan(conn, plan)?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::CrossDbSql(plan) => {
            let Some(sidecar) = conn.attach_map().database(&plan.alias) else {
                return Err(Error::UnknownTable(format!(
                    "no such database: {}",
                    plan.alias
                )));
            };
            let sidecar_conn = sidecar.connect();
            sidecar_conn.execute(plan.sql.as_ref())?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        // Track J: register a Postgres-style schema name on the session.
        // SQLite has no schema layer, so this is purely a name-bookkeeping
        // operation that lets later `<schema>.<table>` references and
        // `pg_namespace` introspection see the freshly-registered name.
        PreparedKind::CreateSchema {
            name,
            if_not_exists,
        } => {
            let folded = name.to_ascii_lowercase();
            with_session_reentrant(conn, |session| {
                if !session.pg_schemas.insert(folded) && !if_not_exists {
                    return Err(Error::Kernel(redlinedb_kernel::Error::ObjectExists));
                }
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::DropSchema {
            name,
            if_exists,
            cascade: _,
        } => {
            let folded = name.to_ascii_lowercase();
            with_session_reentrant(conn, |session| {
                if !session.pg_schemas.remove(&folded) && !if_exists {
                    return Err(Error::Kernel(redlinedb_kernel::Error::ObjectNotFound));
                }
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::CreateSequence {
            name,
            if_not_exists,
            start_with,
            increment_by,
        } => {
            let folded = name.to_ascii_lowercase();
            let start = start_with.unwrap_or(1);
            let increment = increment_by.unwrap_or(1);
            with_session_reentrant(conn, |session| {
                if session.pg_sequences.contains_key(&folded) {
                    if *if_not_exists {
                        return Ok(());
                    }
                    return Err(Error::Kernel(redlinedb_kernel::Error::ObjectExists));
                }
                session
                    .pg_sequences
                    .insert(folded, crate::session::SequenceState::new(start, increment));
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        PreparedKind::DropSequence { name, if_exists } => {
            let folded = name.to_ascii_lowercase();
            with_session_reentrant(conn, |session| {
                if session.pg_sequences.remove(&folded).is_none() && !if_exists {
                    return Err(Error::Kernel(redlinedb_kernel::Error::ObjectNotFound));
                }
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        // Track J: SET TRANSACTION ISOLATION LEVEL — recall-only stash.
        PreparedKind::SetTransactionIsolation { level } => {
            with_session_reentrant(conn, |session| {
                session.transaction_isolation = *level;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 0,
            })
        }
        // Track J: SHOW <name>. Returns a single-row result with the recalled
        // session value for `transaction_isolation`; other names yield "".
        PreparedKind::ShowVariable { name } => {
            let value = if name.eq_ignore_ascii_case("transaction_isolation") {
                let iso = with_session_reentrant(conn, |session| {
                    Ok(session.transaction_isolation)
                })?;
                SqlValue::Text(Arc::from(iso.as_pg_str()))
            } else {
                SqlValue::Text(Arc::from(""))
            };
            Ok(ExecutionResult {
                runtime: RuntimeState::Select(SelectRuntime {
                    tx: SelectRuntimeTx::Empty,
                    restore_tx: false,
                    source: SelectRuntimeSource::StaticRows {
                        rows: Arc::from(vec![vec![value]]),
                        cursor: 0,
                    },
                    selection: None,
                    projection: Vec::new(),
                    limit: usize::MAX,
                    offset: 0,
                    seen: 0,
                    yielded: 0,
                    memory: QueryMemoryBroker::new(0, 0, None),
                }),
                affected_rows: 0,
            })
        }
        // Track J: ALTER INDEX <old> RENAME TO <new>.
        PreparedKind::AlterIndex { old_name, new_name } => {
            with_write_tx(conn, |session, tx| {
                let snapshot = conn.engine().schema_snapshot_for_tx(tx);
                let old_folded = old_name.to_ascii_lowercase();
                let new_folded = new_name.to_ascii_lowercase();
                let Some(schema_id) = snapshot.lookup_namespace("main") else {
                    return Err(Error::Kernel(
                        redlinedb_kernel::Error::ObjectNotFound,
                    ));
                };
                if snapshot.lookup_index(schema_id, &old_folded).is_none() {
                    return Err(Error::Kernel(
                        redlinedb_kernel::Error::ObjectNotFound,
                    ));
                }
                if snapshot.lookup_index(schema_id, &new_folded).is_some() {
                    return Err(Error::UnsupportedSql(format!(
                        "an index named {new_name} already exists"
                    )));
                }
                drop(snapshot);
                conn.engine().rename_index(tx, &old_folded, new_name)?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
    }
}

pub(crate) fn materialize_prepared_rows(
    conn: &Connection,
    template: &PreparedTemplate,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    materialize_prepared_rows_limited(conn, template, bindings, None)
}

pub(crate) fn materialize_prepared_rows_limited(
    conn: &Connection,
    template: &PreparedTemplate,
    bindings: &[Option<SqlValue>],
    max_rows: Option<usize>,
) -> Result<Vec<Vec<SqlValue>>> {
    let result = execute_prepared(conn, template, bindings)?;
    let mut rows = Vec::new();
    if let RuntimeState::Select(mut runtime) = result.runtime {
        let mut current = None;
        loop {
            if step_select_runtime(conn, &mut runtime, bindings, &mut current)? {
                break;
            }
            rows.push(match current.take() {
                Some(row) => row,
                None => Vec::new(),
            });
            if max_rows.is_some_and(|max| rows.len() >= max) {
                finish_select_runtime(conn, &mut runtime)?;
                break;
            }
        }
    }
    Ok(rows)
}

pub(crate) fn materialize_select_plan_rows(
    conn: &Connection,
    plan: &SelectPlan,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let template = PreparedTemplate {
        sql: Arc::from("<compound-branch>"),
        schema_epoch: conn.schema_epoch(),
        stats_epoch: conn.stats_epoch().0,
        optimizer_hash: conn.optimizer_hash(),
        param_layout: crate::statement::ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: true,
        kind: PreparedKind::Select(plan.clone()),
    };
    materialize_prepared_rows(conn, &template, bindings)
}

/// True when `kind` would mutate database or schema state. Used by the
/// `PRAGMA query_only` gate to reject writes without enumerating every
/// statement variant at the call site.
fn template_writes(kind: &PreparedKind) -> bool {
    match kind {
        PreparedKind::Begin(_)
        | PreparedKind::Commit
        | PreparedKind::Rollback
        | PreparedKind::Pragma(_)
        | PreparedKind::Reindex
        | PreparedKind::Vacuum
        | PreparedKind::VacuumInto { .. }
        | PreparedKind::Analyze(_)
        | PreparedKind::Explain(_)
        | PreparedKind::Select(_)
        | PreparedKind::Attach(_)
        | PreparedKind::SetTransactionIsolation { .. }
        | PreparedKind::ShowVariable { .. } => false,
        PreparedKind::CreateTable(_)
        | PreparedKind::CreateTempTable(_)
        | PreparedKind::CreateTableAsSelect(_)
        | PreparedKind::CreateIndex(_)
        | PreparedKind::CreateVirtualTable(_)
        | PreparedKind::CreateView(_)
        | PreparedKind::CreateTrigger(_)
        | PreparedKind::DropTable(_)
        | PreparedKind::DropIndex(_)
        | PreparedKind::DropView(_)
        | PreparedKind::DropTrigger(_)
        | PreparedKind::AlterTable(_)
        | PreparedKind::AlterIndex { .. }
        | PreparedKind::Insert(_)
        | PreparedKind::InsertView(_)
        | PreparedKind::Update(_)
        | PreparedKind::Delete(_)
        | PreparedKind::CrossDbSql(_)
        | PreparedKind::CreateSchema { .. }
        | PreparedKind::DropSchema { .. }
        | PreparedKind::CreateSequence { .. }
        | PreparedKind::DropSequence { .. } => true,
        | PreparedKind::Merge(_)
        | PreparedKind::CrossDbSql(_) => true,
    }
}

fn execute_create_virtual_table(
    conn: &Connection,
    plan: &crate::statement::CreateVirtualTablePlan,
) -> Result<()> {
    let cols = plan
        .columns
        .iter()
        .enumerate()
        .map(|(idx, col)| {
            let decl_type = if plan.module.eq_ignore_ascii_case("rtree") {
                if idx == 0 { "INTEGER" } else { "REAL" }
            } else {
                "TEXT"
            };
            format!("\"{col}\" {decl_type}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("CREATE TABLE \"{}\" ({cols})", plan.name);
    let template = crate::parser::parse_prepared_template(conn, &sql)?;
    let _ = materialize_prepared_rows(conn, &template, &[])?;
    if plan.module.eq_ignore_ascii_case("dbstat") {
        let sql = format!("INSERT INTO \"{}\" DEFAULT VALUES", plan.name);
        let template = crate::parser::parse_prepared_template(conn, &sql)?;
        let _ = materialize_prepared_rows(conn, &template, &[])?;
    }
    Ok(())
}

fn execute_create_table_as_select(
    conn: &Connection,
    spec: &CreateTableAsSelectSpec,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    let Some(select) = spec.select.as_ref() else {
        return Ok(ExecutionResult {
            runtime: RuntimeState::Done,
            affected_rows: 0,
        });
    };

    let inserted = with_write_tx(conn, |session, tx| {
        let table = conn.engine().create_table(tx, spec.table.clone())?;
        let mut runtime = execute_select(conn, select, bindings)?;
        let mut current_row = None;
        let mut inserted = 0usize;
        loop {
            if step_select_runtime(conn, &mut runtime, bindings, &mut current_row)? {
                break;
            }
            let values = current_row.take().unwrap_or_default();
            let values = apply_row_affinity(&table, values)?;
            let rowid = RowId::new((inserted + 1) as u64);
            let payload = encode_sql_row(table.table_id.0, &values)?;
            conn.engine()
                .insert_for_relation(tx, table.relation_id, rowid, payload)?;
            inserted += 1;
            session.last_insert_rowid = Some(rowid.0 as i64);
        }
        if inserted > 0 {
            session.changes += inserted;
            session.total_changes += inserted;
        }
        Ok(inserted)
    })?;

    Ok(ExecutionResult {
        runtime: RuntimeState::Done,
        affected_rows: inserted,
    })
}

/// Returns `Some(value)` when the given PRAGMA SET plan must echo a row
/// back to the caller (matching SQLite's surface for `journal_mode`,
/// `locking_mode`, `busy_timeout`, …). For SET pragmas that are silent
/// (e.g. `defer_foreign_keys=1`), returns `None`.
fn pragma_set_echo_value(plan: &PragmaPlan) -> Option<SqlValue> {
    match plan {
        PragmaPlan::SetJournalMode(value) => {
            Some(SqlValue::Text(Arc::from(value.as_str())))
        }
        PragmaPlan::SetLockingMode(value) => {
            Some(SqlValue::Text(Arc::from(value.as_str())))
        }
        PragmaPlan::SetBusyTimeout(value)
        | PragmaPlan::SetMaxPageCount(value)
        | PragmaPlan::SetThreads(value)
        | PragmaPlan::SetAnalysisLimit(value) => Some(SqlValue::Integer(*value)),
        PragmaPlan::SetSecureDelete(value) => {
            Some(SqlValue::Integer(if *value { 1 } else { 0 }))
        }
        _ => None,
    }
}

fn execute_pragma(conn: &Connection, plan: &PragmaPlan) -> Result<()> {
    match plan {
        PragmaPlan::SetForeignKeys(value) => {
            conn.set_foreign_keys(*value);
            Ok(())
        }
        PragmaPlan::SetUserVersion(value) => conn.set_user_version(*value),
        PragmaPlan::SetRecursiveTriggers(value) => {
            conn.set_recursive_triggers(*value);
            Ok(())
        }
        PragmaPlan::SetJournalMode(value) => {
            conn.set_journal_mode(*value);
            Ok(())
        }
        PragmaPlan::SetSynchronous(value) => {
            conn.set_synchronous(*value);
            Ok(())
        }
        PragmaPlan::SetTempStore(value) => {
            conn.set_temp_store(*value);
            Ok(())
        }
        PragmaPlan::SetCacheSize(value) => {
            conn.set_cache_size(*value);
            Ok(())
        }
        PragmaPlan::SetQueryOnly(value) => {
            conn.set_query_only(*value);
            Ok(())
        }
        PragmaPlan::SetCaseSensitiveLike(value) => {
            conn.set_case_sensitive_like(*value);
            Ok(())
        }
        PragmaPlan::WalCheckpoint => Ok(()),
        PragmaPlan::SetAnalysisLimit(value) => {
            conn.set_analysis_limit(*value);
            Ok(())
        }
        PragmaPlan::SetApplicationId(value) => {
            conn.set_application_id(*value);
            Ok(())
        }
        PragmaPlan::SetAutoVacuum(value) => {
            conn.set_auto_vacuum(*value);
            Ok(())
        }
        PragmaPlan::SetAutomaticIndex(value) => {
            conn.set_automatic_index(*value);
            Ok(())
        }
        PragmaPlan::SetBusyTimeout(value) => {
            conn.set_busy_timeout_ms(*value);
            Ok(())
        }
        PragmaPlan::SetCacheSpill(value) => {
            conn.set_cache_spill(*value);
            Ok(())
        }
        PragmaPlan::SetCheckpointFullfsync(value) => {
            conn.set_checkpoint_fullfsync(*value);
            Ok(())
        }
        PragmaPlan::SetDeferForeignKeys(value) => {
            conn.set_defer_foreign_keys(*value);
            Ok(())
        }
        PragmaPlan::SetFullfsync(value) => {
            conn.set_fullfsync(*value);
            Ok(())
        }
        PragmaPlan::SetHardHeapLimit(value) => {
            conn.set_hard_heap_limit(*value);
            Ok(())
        }
        PragmaPlan::SetIgnoreCheckConstraints(value) => {
            conn.set_ignore_check_constraints(*value);
            Ok(())
        }
        PragmaPlan::SetLegacyAlterTable(value) => {
            conn.set_legacy_alter_table(*value);
            Ok(())
        }
        PragmaPlan::SetLockingMode(value) => {
            conn.set_locking_mode(*value);
            Ok(())
        }
        PragmaPlan::SetMaxPageCount(value) => {
            conn.set_max_page_count(*value);
            Ok(())
        }
        PragmaPlan::SetMmapSize(value) => {
            conn.set_mmap_size(*value);
            Ok(())
        }
        PragmaPlan::SetReverseUnorderedSelects(value) => {
            conn.set_reverse_unordered_selects(*value);
            Ok(())
        }
        PragmaPlan::SetSecureDelete(value) => {
            conn.set_secure_delete(*value);
            Ok(())
        }
        PragmaPlan::SetSoftHeapLimit(value) => {
            conn.set_soft_heap_limit(*value);
            Ok(())
        }
        PragmaPlan::SetThreads(value) => {
            conn.set_threads(*value);
            Ok(())
        }
        PragmaPlan::SetTrustedSchema(value) => {
            conn.set_trusted_schema(*value);
            Ok(())
        }
        PragmaPlan::SetWritableSchema(value) => {
            conn.set_writable_schema(*value);
            Ok(())
        }
    }
}

fn vacuum_into_dir(conn: &Connection, dst: &str) -> Result<()> {
    let src = conn.database_path();
    let dst = Path::new(dst);
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|err| Error::Config(err.to_string()))?;
    }
    fs::create_dir_all(dst).map_err(|err| Error::Config(err.to_string()))?;
    copy_dir(src, dst)?;
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).map_err(|err| Error::Config(err.to_string()))? {
        let entry = entry.map_err(|err| Error::Config(err.to_string()))?;
        let name = entry.file_name();
        if name == "owner.lock" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let file_type = entry
            .file_type()
            .map_err(|err| Error::Config(err.to_string()))?;
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|err| Error::Config(err.to_string()))?;
            copy_dir(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|err| Error::Config(err.to_string()))?;
        }
    }
    Ok(())
}

fn with_write_tx<T>(
    conn: &Connection,
    mut f: impl FnMut(&mut SessionState, &mut Txn) -> Result<T>,
) -> Result<T> {
    const AUTOCOMMIT_WRITE_RETRY_LIMIT: usize = 4096;
    // Lane A5-triggers: re-entrant call from a trigger body fires here
    // while the parent DML's `with_session` lock is still held. In that
    // case the parent already owns the tx via thread-local; recurse
    // through it directly to avoid deadlocking on the session mutex.
    if let Some(tx_ptr) = current_tx()
        && let Some(session_ptr) = with_current_session_ptr()
    {
        // SAFETY: both pointers were installed by the parent
        // `with_write_tx` call below and live for the closure's
        // lifetime. The trigger fire-hook is strictly synchronous with
        // the parent — no other writer can observe these references.
        let session_ref: &mut SessionState = unsafe { &mut *session_ptr };
        let tx_ref: &mut Txn = unsafe { &mut *tx_ptr };
        return f(session_ref, tx_ref);
    }
    conn.with_session(|session| {
        if session.failed {
            return Err(Error::TransactionState(
                "transaction is failed and must roll back",
            ));
        }
        let session_ptr: *mut SessionState = session;
        if session.tx.is_some() {
            let mut tx = session.tx.take().expect("checked some");
            let tx_ptr: *mut Txn = &mut tx;
            let result = with_current_session(session_ptr, || {
                with_current_tx(tx_ptr, || {
                    // SAFETY: `tx_ptr` points at the `tx` local above for the
                    // duration of this closure, and no other mutable borrow is
                    // handed out while the closure runs.
                    let tx_ref = unsafe { &mut *tx_ptr };
                    f(session, tx_ref)
                })
            });
            session.tx = Some(tx);
            if result.is_err() {
                session.failed = true;
            }
            result
        } else {
            let mut attempts = 0_usize;
            loop {
                let mut tx = conn.engine().begin(Isolation::ReadCommitted)?;
                let tx_ptr: *mut Txn = &mut tx;
                let result = with_current_session(session_ptr, || {
                    with_current_tx(tx_ptr, || {
                        // SAFETY: same as the branch above; `tx` lives for the
                        // full duration of the closure and only one mutable
                        // reference is created at a time.
                        let tx_ref = unsafe { &mut *tx_ptr };
                        f(session, tx_ref)
                    })
                });
                match result {
                    Ok(value) => {
                        // A6 SQLite parity: drain deferred FK checks
                        // before this autocommit slice closes. If any
                        // entry violates referential integrity we roll
                        // the tx back and surface the violation.
                        let drain_result = with_current_tx(tx_ptr, || {
                            let tx_ref = unsafe { &mut *tx_ptr };
                            crate::exec::fk::drain_deferred_fk_checks(conn, session, tx_ref)
                        });
                        if let Err(err) = drain_result {
                            let _ = conn.engine().rollback(tx);
                            session.kernel_unique_guards.clear();
                            session.unique_guards.clear();
                            return Err(err);
                        }
                        match conn.engine().commit(tx) {
                            Ok(CommitOutcome::Committed(_)) => {
                                session.kernel_unique_guards.clear();
                                session.unique_guards.clear();
                                return Ok(value);
                            }
                            Ok(CommitOutcome::MaybeCommitted) => {
                                session.kernel_unique_guards.clear();
                                session.unique_guards.clear();
                                return Err(Error::CommitMaybeCommitted);
                            }
                            Ok(CommitOutcome::RolledBack) => {
                                session.kernel_unique_guards.clear();
                                session.unique_guards.clear();
                                return Err(Error::TransactionState("transaction rolled back"));
                            }
                            Err(err) => {
                                session.kernel_unique_guards.clear();
                                session.unique_guards.clear();
                                return Err(err.into());
                            }
                        }
                    }
                    Err(err)
                        if is_retryable_autocommit_write_error(&err)
                            && attempts < AUTOCOMMIT_WRITE_RETRY_LIMIT =>
                    {
                        attempts += 1;
                        let _ = conn.engine().rollback(tx);
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        std::thread::yield_now();
                        continue;
                    }
                    Err(err) => {
                        let _ = conn.engine().rollback(tx);
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        return Err(err);
                    }
                }
            }
        }
    })
}

fn is_retryable_autocommit_write_error(err: &Error) -> bool {
    matches!(
        err,
        Error::Kernel(
            redlinedb_kernel::Error::SerializationFailure
                | redlinedb_kernel::Error::WriteConflict
                | redlinedb_kernel::Error::LockTimeout
        )
    )
}

pub(crate) fn finalize_runtime(conn: &Connection, runtime: &mut RuntimeState) -> Result<()> {
    match runtime {
        RuntimeState::Select(select) => {
            finish_select_runtime(conn, select)?;
            *runtime = RuntimeState::Done;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn step_select_runtime(
    conn: &Connection,
    runtime: &mut SelectRuntime,
    bindings: &[Option<SqlValue>],
    current_row: &mut Option<Vec<SqlValue>>,
) -> Result<bool> {
    let tx_ptr = select_tx_ptr(&mut runtime.tx);
    if let Some(tx_ptr) = tx_ptr {
        return with_current_tx(tx_ptr, || {
            step_select_runtime_inner(conn, runtime, bindings, current_row)
        });
    }
    step_select_runtime_inner(conn, runtime, bindings, current_row)
}

fn step_select_runtime_inner(
    conn: &Connection,
    runtime: &mut SelectRuntime,
    bindings: &[Option<SqlValue>],
    current_row: &mut Option<Vec<SqlValue>>,
) -> Result<bool> {
    match &mut runtime.source {
        SelectRuntimeSource::Batched {
            node,
            ctx,
            batch,
            cursor,
        } => {
            if runtime.yielded >= runtime.limit {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            if *cursor >= batch.len {
                batch.clear();
                match node.next_batch(ctx, batch)? {
                    ExecState::Yield | ExecState::Done if batch.len > 0 => {
                        *cursor = 0;
                    }
                    ExecState::Done => {
                        finish_select_runtime(conn, runtime)?;
                        *current_row = None;
                        return Ok(true);
                    }
                    ExecState::Yield => {
                        *cursor = 0;
                    }
                }
            }
            if *cursor >= batch.len {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            let row = match batch.row(*cursor) {
                Some(r) => r,
                None => return Err(Error::Bind("batch cursor out of range".to_owned())),
            };
            *current_row = Some(row);
            *cursor += 1;
            runtime.yielded += 1;
            Ok(false)
        }
        SelectRuntimeSource::SqliteSchema { rows, cursor } => {
            while *cursor < rows.len() {
                let row = SqlRow::SqliteSchema(rows[*cursor].clone());
                *cursor += 1;
                if !selection_passes(&runtime.selection, &row, bindings)? {
                    continue;
                }
                runtime.seen += 1;
                if runtime.seen <= runtime.offset {
                    continue;
                }
                if runtime.yielded >= runtime.limit {
                    finish_select_runtime(conn, runtime)?;
                    *current_row = None;
                    return Ok(true);
                }
                *current_row = Some(project_row(&runtime.projection, &row, bindings)?);
                runtime.yielded += 1;
                return Ok(false);
            }
            finish_select_runtime(conn, runtime)?;
            *current_row = None;
            Ok(true)
        }
        SelectRuntimeSource::StaticRows { rows, cursor } => {
            if *cursor >= rows.len() {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            *current_row = Some(rows[*cursor].clone());
            *cursor += 1;
            runtime.yielded += 1;
            Ok(false)
        }
        SelectRuntimeSource::Table {
            table,
            rowids,
            cursor,
        } => {
            let tx = runtime
                .tx
                .as_mut()
                .ok_or(Error::TransactionState("transaction closed"))?;
            while *cursor < rowids.len() {
                let rowid = rowids[*cursor];
                *cursor += 1;
                if let Some(row) = load_table_row_by_rowid(conn.engine(), tx, table, rowid)? {
                    let row = SqlRow::Table(row);
                    if !selection_passes(&runtime.selection, &row, bindings)? {
                        continue;
                    }
                    runtime.seen += 1;
                    if runtime.seen <= runtime.offset {
                        continue;
                    }
                    if runtime.yielded >= runtime.limit {
                        finish_select_runtime(conn, runtime)?;
                        *current_row = None;
                        return Ok(true);
                    }
                    *current_row = Some(project_row(&runtime.projection, &row, bindings)?);
                    runtime.yielded += 1;
                    return Ok(false);
                }
            }
            finish_select_runtime(conn, runtime)?;
            *current_row = None;
            Ok(true)
        }
        SelectRuntimeSource::Empty => {
            if runtime.yielded > 0 || runtime.offset > 0 || runtime.limit == 0 {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            if !selection_passes(&runtime.selection, &SqlRow::Empty, bindings)? {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            runtime.seen = runtime.seen.saturating_add(1);
            if runtime.seen <= runtime.offset {
                finish_select_runtime(conn, runtime)?;
                *current_row = None;
                return Ok(true);
            }
            *current_row = Some(project_row(&runtime.projection, &SqlRow::Empty, bindings)?);
            runtime.yielded = 1;
            Ok(false)
        }
    }
}

fn finish_select_runtime(conn: &Connection, runtime: &mut SelectRuntime) -> Result<()> {
    if let Some(tx) = runtime.tx.take_owned() {
        if runtime.restore_tx {
            conn.with_session(|session| {
                if session.tx.is_some() {
                    return Err(Error::TransactionState("transaction already active"));
                }
                session.tx = Some(tx);
                Ok(())
            })?;
        } else {
            let _ = conn.engine().rollback(tx);
        }
    }
    runtime.source = SelectRuntimeSource::Empty;
    Ok(())
}
