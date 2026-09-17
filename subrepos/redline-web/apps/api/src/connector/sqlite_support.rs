//! Free helper functions for the SQLite connector: query execution, value
//! conversion, schema-object helpers, and demo seeding. Kept out of
//! `sqlite.rs` so the connector type/impls stay small and focused.

use rusqlite::types::ValueRef;
use rusqlite::{Connection, ErrorCode, OptionalExtension};

use super::{ConnectorError, quote_ident};
use crate::model::{CellValue, QueryResult, SchemaObjectKind};

/// Execute one SQL statement, branching on whether it returns rows.
pub(crate) fn run_query(
    conn: &Connection,
    sql: &str,
    max_rows: i64,
) -> Result<QueryResult, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let col_count = stmt.column_count();

    if col_count == 0 {
        let affected = stmt.execute([])?;
        return Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            rows_affected: Some(affected as i64),
            elapsed_ms: 0.0,
            truncated: false,
        });
    }

    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let cap = max_rows.max(0) as usize;
    let mut rows_out: Vec<Vec<CellValue>> = Vec::new();
    let mut truncated = false;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if rows_out.len() >= cap {
            truncated = true;
            break;
        }
        let mut record = Vec::with_capacity(col_count);
        for i in 0..col_count {
            record.push(value_to_json(row.get_ref(i)?));
        }
        rows_out.push(record);
    }

    let row_count = rows_out.len() as i64;
    Ok(QueryResult {
        columns,
        rows: rows_out,
        row_count,
        rows_affected: None,
        elapsed_ms: 0.0,
        truncated,
    })
}

/// Convert a borrowed SQLite value into a JSON cell value.
pub(crate) fn value_to_json(value: ValueRef<'_>) -> CellValue {
    match value {
        ValueRef::Null => CellValue::Null,
        ValueRef::Integer(i) => CellValue::from(i),
        ValueRef::Real(f) => serde_json::Number::from_f64(f)
            .map(CellValue::Number)
            .unwrap_or(CellValue::Null),
        ValueRef::Text(t) => CellValue::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => CellValue::String(format!("0x{}", to_hex(b))),
    }
}

/// Lowercase hex encoding for BLOB cells.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub(crate) fn parse_kind(kind: &str) -> SchemaObjectKind {
    match kind {
        "view" => SchemaObjectKind::View,
        "index" => SchemaObjectKind::Index,
        "trigger" => SchemaObjectKind::Trigger,
        _ => SchemaObjectKind::Table,
    }
}

/// Count rows of a schema object.
///
/// Input-boundary guard: the object name is first checked against the fixed
/// allowlist of real names in `sqlite_master` via a bound parameter, so only an
/// existing object is ever named; its identifier is then escaped with
/// [`quote_ident`]. Identifiers cannot be bound positionally in SQLite, so this
/// allowlist + escape pair is the boundary (see `tests/property.rs`).
pub(crate) fn count_rows(conn: &Connection, name: &str) -> Option<i64> {
    if !object_exists(conn, name) {
        return None;
    }
    let mut stmt = String::with_capacity(24 + name.len());
    stmt.push_str("SELECT count(*) FROM ");
    stmt.push_str(&quote_ident(name));
    conn.query_row(&stmt, [], |r| r.get(0)).ok()
}

/// Allowlist check: is `name` a real object in `sqlite_master`? Uses a bound
/// parameter so the untrusted name never enters the SQL text.
fn object_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE name = ?1 LIMIT 1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

pub(crate) fn column_names(conn: &Connection, name: &str) -> Result<Vec<String>, ConnectorError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(name)))?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

pub(crate) fn dbstat_available(conn: &Connection) -> bool {
    conn.prepare("SELECT name FROM dbstat LIMIT 0").is_ok()
}

pub(crate) fn is_interrupt(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _) if err.code == ErrorCode::OperationInterrupted
    )
}

pub(crate) fn is_empty(conn: &Connection) -> Result<bool, ConnectorError> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    Ok(count == 0)
}

/// Seed a tiny demo schema so an empty database renders something useful. The
/// statements are a fixed, checked-in script (`seed_demo.sql`), not built from
/// any request input.
pub(crate) fn seed_demo(conn: &Connection) -> Result<(), ConnectorError> {
    conn.execute_batch(include_str!("seed_demo.sql"))?;
    Ok(())
}
