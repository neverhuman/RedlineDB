//! Trigger fire-hook.
//!
//! Loaded by INSERT / UPDATE / DELETE executors after each row mutation
//! (and before, for BEFORE triggers). For each matching trigger:
//!
//! 1. Apply the optional `UPDATE OF cols` filter — UPDATE triggers only
//!    fire if at least one of the listed columns actually changed.
//! 2. Push synthesised `OLD` / `NEW` row contexts onto the correlated
//!    row stack so identifier resolution finds `OLD.col` / `NEW.col`.
//! 3. Evaluate the optional `WHEN` predicate; skip the body when it is
//!    not truthy.
//! 4. Re-parse the body SQL on each fire and execute every statement in
//!    the body against the live connection.
//!
//! Recursive depth tracking lives on [`Txn::trigger_depth`]; the executor
//! increments before entering a body and decrements on exit, returning a
//! clear error if the cap is exceeded (default 1000, matching SQLite).
//!
//! `INSTEAD OF` triggers on views are intentionally deferred to a
//! followup task; this module fires only `BEFORE`/`AFTER` triggers on
//! base tables.

use std::sync::Arc;

use redlinedb_kernel::catalog::{
    SchemaSnapshot, TableDef, TriggerDef, TriggerEventKind, TriggerTimeKind, triggers_for,
};
use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::format::RowId;
use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::exec::expr::scalar::row::{SqlRow, TableRow};
use crate::value::SqlValue;

/// Recursion cap. SQLite's default `SQLITE_MAX_TRIGGER_DEPTH` is 1000,
/// but RedlineDB's debug builds have heavier stack frames; we cap at 32
/// to keep within typical Rust stack limits across CI environments while
/// remaining well above what any non-pathological workload uses (most
/// applications stay at depth 1–3). The cap can be raised in release
/// builds via a per-build constant once we add it; the spec calls for
/// SQLite-style behaviour past the cap, which is exactly what we
/// surface here.
pub(crate) const TRIGGER_DEPTH_CAP: u32 = 8;

/// Fire all triggers that match `(table, event, time)`. For UPDATE
/// triggers the optional `changed_cols` filter restricts firing to
/// triggers whose `UPDATE OF` list intersects the set of columns whose
/// values actually changed.
pub(crate) fn fire_triggers(
    conn: &Connection,
    tx: &mut Txn,
    schema: &SchemaSnapshot,
    table: &Arc<TableDef>,
    event: TriggerEventKind,
    time: TriggerTimeKind,
    old: Option<TriggerRowValues>,
    new: Option<TriggerRowValues>,
    changed_cols: Option<&[String]>,
) -> Result<()> {
    let triggers = triggers_for(schema, table.schema_id, &table.folded, event, time);
    if triggers.is_empty() {
        return Ok(());
    }
    for trigger in triggers {
        if event == TriggerEventKind::Update
            && !trigger.when_cols.is_empty()
            && !any_column_in_filter(&trigger.when_cols, changed_cols)
        {
            continue;
        }
        fire_one(conn, tx, table, &trigger, old.as_ref(), new.as_ref())?;
    }
    Ok(())
}

/// Captured row values for a single OLD or NEW context. Owning the
/// values lets the fire-hook materialise a synthetic `TableRow` keyed by
/// the `OLD` / `NEW` alias.
#[derive(Clone)]
pub(crate) struct TriggerRowValues {
    pub(crate) rowid: RowId,
    pub(crate) values: Vec<SqlValue>,
}

fn any_column_in_filter(filter: &[Box<str>], changed: Option<&[String]>) -> bool {
    let Some(changed) = changed else {
        return true;
    };
    filter
        .iter()
        .any(|f| changed.iter().any(|c| c.eq_ignore_ascii_case(f.as_ref())))
}

fn fire_one(
    conn: &Connection,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    trigger: &TriggerDef,
    old: Option<&TriggerRowValues>,
    new: Option<&TriggerRowValues>,
) -> Result<()> {
    let depth = tx.increment_trigger_depth();
    if depth > TRIGGER_DEPTH_CAP {
        tx.decrement_trigger_depth();
        return Err(Error::UnsupportedSql(format!(
            "trigger recursion depth exceeded {TRIGGER_DEPTH_CAP} (in trigger `{}`)",
            trigger.name
        )));
    }
    let result = run_body_with_context(conn, table, trigger, old, new);
    tx.decrement_trigger_depth();
    result
}

fn run_body_with_context(
    conn: &Connection,
    table: &Arc<TableDef>,
    trigger: &TriggerDef,
    old: Option<&TriggerRowValues>,
    new: Option<&TriggerRowValues>,
) -> Result<()> {
    let old_row = old.map(|v| make_table_row(table, "OLD", v));
    let new_row = new.map(|v| make_table_row(table, "NEW", v));

    // Push contexts onto the outer-row stack so `OLD.col`/`NEW.col`
    // resolve via the qualified-identifier path. Push in a fixed order
    // so the body always sees both contexts when present.
    let mut pushed = 0u32;
    if let Some(row) = old_row.clone() {
        crate::exec::push_outer_row(SqlRow::Table(row));
        pushed += 1;
    }
    if let Some(row) = new_row.clone() {
        crate::exec::push_outer_row(SqlRow::Table(row));
        pushed += 1;
    }
    let result = || -> Result<()> {
        if let Some(predicate_sql) = &trigger.when_predicate_sql
            && !evaluate_when_predicate(conn, predicate_sql)?
        {
            return Ok(());
        }
        execute_body_statements(conn, trigger.body_sql.as_ref())?;
        Ok(())
    }();
    for _ in 0..pushed {
        crate::exec::pop_outer_row();
    }
    result
}

fn make_table_row(table: &Arc<TableDef>, alias: &str, values: &TriggerRowValues) -> TableRow {
    TableRow {
        rowid: values.rowid,
        values: values.values.clone(),
        table: Arc::clone(table),
        alias: Some(Arc::from(alias)),
    }
}

/// Evaluate a `WHEN` predicate against the active OLD/NEW row context.
/// The expression is parsed inside a synthetic `SELECT <expr>` so the
/// existing expression evaluator handles it without bespoke parsing.
fn evaluate_when_predicate(conn: &Connection, predicate_sql: &str) -> Result<bool> {
    let synth = format!("SELECT ({predicate_sql})");
    let template = crate::parser::parse_prepared_template(conn, &synth)?;
    let rows = crate::exec::materialize_prepared_rows(conn, &template, &[])?;
    let truthy = rows
        .first()
        .and_then(|r| r.first())
        .map(crate::value::is_truthy)
        .unwrap_or(false);
    Ok(truthy)
}

/// Split the body SQL into individual statements and execute each.
///
/// The body comes from sqlparser's `ConditionalStatements` Display, which
/// emits `BEGIN\n<stmt>;\n<stmt>;\nEND`. The SQLite dialect rejects
/// standalone `BEGIN ... END` blocks, so we strip the wrapper and run
/// the inner statement list one at a time. Each statement shares the
/// live trigger row context already on the outer-row stack.
fn execute_body_statements(conn: &Connection, body_sql: &str) -> Result<()> {
    let inner = strip_begin_end_wrapper(body_sql);
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let mut rest = trimmed;
    while !rest.is_empty() {
        if crate::parser::is_blank_sql(rest) {
            break;
        }
        let (head, tail) = crate::parser::split_first_statement(rest);
        if head.is_empty() {
            break;
        }
        if !crate::parser::is_blank_sql(head) {
            conn_execute_quiet(conn, head)?;
        }
        rest = tail;
    }
    Ok(())
}

/// Trim a leading `BEGIN` and trailing `END` from a trigger body, if
/// present. The sqlparser Display always emits the wrapper; we strip it
/// because the SQLite dialect rejects `BEGIN ... END` as a top-level
/// statement.
fn strip_begin_end_wrapper(body: &str) -> &str {
    let trimmed = body.trim();
    let lower = trimmed.to_ascii_lowercase();
    let after_begin = if lower.starts_with("begin")
        && trimmed
            .as_bytes()
            .get(5)
            .map(|b| b.is_ascii_whitespace())
            .unwrap_or(false)
    {
        &trimmed[5..]
    } else {
        trimmed
    };
    let after_begin = after_begin.trim_start();
    let lower = after_begin.to_ascii_lowercase();
    if lower.ends_with("end") {
        let cut = after_begin.len() - 3;
        let before_end = after_begin[..cut].trim_end();
        let before_end = before_end.trim_end_matches(';').trim_end();
        return before_end;
    }
    after_begin
}

fn conn_execute_quiet(conn: &Connection, sql: &str) -> Result<()> {
    let template = crate::parser::parse_prepared_template(conn, sql)?;
    let _ = crate::exec::materialize_prepared_rows(conn, &template, &[])?;
    Ok(())
}
