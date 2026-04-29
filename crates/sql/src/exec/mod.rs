use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use redlinedb_kernel::catalog::{
    ColumnStats, ConstraintKind, EvalScratch, HistogramBucket, IndexStats, MostCommonValue,
    OwnedValue, RecordRef, RecordScratch, RowValueSource, SqliteSchemaRow, StatsEpoch,
    StatsSnapshot, TableDef, TableStats, ValueRef, apply_affinity, encode_record, eval_expr,
};
use redlinedb_kernel::engine::{CommitOutcome, Engine, Txn};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::txn::Isolation;
use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, OrderByExpr, SelectItem,
    UnaryOperator, Value,
};

use crate::batch::{
    ExecContext, ExecNode, ExecState, MaterializeNode, QueryMemoryBroker, RowBatch, RowLayout,
};
use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::planner::{self, ExplainMetrics};
use crate::session::SessionState;
use crate::statement::{
    AnalyzePlan, ExecutionResult, ExplainPlan, PragmaPlan, PreparedKind, PreparedTemplate,
    RuntimeState, SelectPlan, SelectRuntime, SelectRuntimeSource, SelectSource,
};
use crate::value::{SqlValue, canonicalize, compare_values, is_truthy};

pub(crate) mod expr;
use expr::*;
pub(crate) mod index_access;
pub(crate) mod index_dml;
mod tail;
use tail::*;
pub(crate) mod vec;

mod agg;
use agg::*;
mod insert;
use insert::*;
mod select_top;
use select_top::*;

thread_local! {
    static CURRENT_CONNECTION: Cell<*const Connection> = const { Cell::new(std::ptr::null()) };
}

pub(crate) fn with_current_connection<T>(conn: &Connection, f: impl FnOnce() -> T) -> T {
    CURRENT_CONNECTION.with(|cell| {
        let prev = cell.replace(conn as *const Connection);
        let result = f();
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

pub fn execute_prepared(
    conn: &Connection,
    template: &PreparedTemplate,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
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
            execute_pragma(conn, plan)?;
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
        PreparedKind::AlterTable(spec) => {
            with_write_tx(conn, |session, tx| {
                conn.engine().alter_table(tx, spec.clone())?;
                session.changes += 1;
                session.total_changes += 1;
                Ok(())
            })?;
            Ok(ExecutionResult {
                runtime: RuntimeState::Done,
                affected_rows: 1,
            })
        }
        PreparedKind::Insert(plan) => {
            let result = execute_insert(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                conn.with_session(|session| {
                    session.changes += result.affected_rows;
                    session.total_changes += result.affected_rows;
                    Ok(())
                })?;
            }
            Ok(result)
        }
        PreparedKind::Update(plan) => {
            let result = execute_update(conn, plan, bindings)?;
            if result.affected_rows > 0 {
                conn.with_session(|session| {
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
                conn.with_session(|session| {
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
    }
}

pub(crate) fn materialize_prepared_rows(
    conn: &Connection,
    template: &PreparedTemplate,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let result = execute_prepared(conn, template, bindings)?;
    let mut rows = Vec::new();
    if let RuntimeState::Select(mut runtime) = result.runtime {
        let mut current = None;
        loop {
            if step_select_runtime(conn, &mut runtime, bindings, &mut current)? {
                break;
            }
            rows.push(current.take().unwrap_or_default());
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

fn execute_pragma(conn: &Connection, plan: &PragmaPlan) -> Result<()> {
    match plan {
        PragmaPlan::SetForeignKeys(value) => {
            conn.set_foreign_keys(*value);
            Ok(())
        }
        PragmaPlan::SetUserVersion(value) => conn.set_user_version(*value),
    }
}

fn with_write_tx<T>(
    conn: &Connection,
    f: impl FnOnce(&mut SessionState, &mut Txn) -> Result<T>,
) -> Result<T> {
    conn.with_session(|session| {
        if session.failed {
            return Err(Error::TransactionState(
                "transaction is failed and must roll back",
            ));
        }
        if session.tx.is_some() {
            let mut tx = session.tx.take().expect("checked some");
            let result = f(session, &mut tx);
            session.tx = Some(tx);
            if result.is_err() {
                session.failed = true;
            }
            result
        } else {
            let mut tx = conn.engine().begin(Isolation::Snapshot)?;
            let result = f(session, &mut tx);
            match result {
                Ok(value) => match conn.engine().commit(tx) {
                    Ok(CommitOutcome::Committed(_)) => {
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        Ok(value)
                    }
                    Ok(CommitOutcome::MaybeCommitted) => {
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        Err(Error::CommitMaybeCommitted)
                    }
                    Ok(CommitOutcome::RolledBack) => {
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        Err(Error::TransactionState("transaction rolled back"))
                    }
                    Err(err) => {
                        session.kernel_unique_guards.clear();
                        session.unique_guards.clear();
                        Err(err.into())
                    }
                },
                Err(err) => {
                    let _ = conn.engine().rollback(tx);
                    session.kernel_unique_guards.clear();
                    session.unique_guards.clear();
                    Err(err)
                }
            }
        }
    })
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
            let row = batch
                .row(*cursor)
                .ok_or_else(|| Error::Bind("batch cursor out of range".to_owned()))?;
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
    if let Some(tx) = runtime.tx.take() {
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
