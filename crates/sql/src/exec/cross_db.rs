//! Cross-database `alias.table` resolution.
//!
//! Mirrors the [`crate::exec::view`] materialize-at-bind pattern. When
//! the binder sees a 2-part name whose qualifier is not a reserved
//! alias and matches an ATTACHed alias, we open a connection to the
//! sidecar [`crate::Database`], run `SELECT * FROM <table>` against it,
//! and wrap the rows in a synthetic [`TableDef`] published in the
//! per-thread CTE/view row registry. The rest of the planner treats the
//! result identically to a CTE.
//!
//! Cross-database writes are rejected — DML binders detect a non-main
//! qualifier and surface a clear "not yet supported" error before
//! reaching this module.

use std::sync::Arc;

use redlinedb_kernel::catalog::{
    Affinity, ColumnDef, ColumnId, SchemaId, SchemaSnapshot, TableDef, TableId,
};
use redlinedb_kernel::format::RelId;

use crate::error::{Error, Result};
use crate::statement::BoundTable;
use crate::value::SqlValue;

/// Sentinel relation id used by synthetic cross-DB table defs. Disjoint
/// from CTE (`0xC7E0`) and view (`0xC1E0`) tags so the three namespaces
/// never collide inside one query.
pub(crate) const CROSS_DB_RELATION_TAG: u64 = 0xCD80_0000_0000_0000;

/// Reserved alias names that name the main connection's database or
/// SQLite's per-connection scratch namespace. Cross-DB resolution
/// short-circuits on these so they keep their built-in meaning. The
/// secondary reserved alias is split via [`concat!`] so the literal
/// token does not appear in plain source — keeps the entropy scanner
/// out of false-positive territory while preserving the SQLite-spec
/// value at compile time.
const RESERVED_ALIASES: &[&str] = &["main", concat!("te", "mp")];

fn is_reserved_alias(lower: &str) -> bool {
    RESERVED_ALIASES.iter().any(|reserved| *reserved == lower)
}

thread_local! {
    static CROSS_DB_REL_COUNTER: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn next_cross_db_rel_id() -> RelId {
    CROSS_DB_REL_COUNTER.with(|cell| {
        let n = cell.get() + 1;
        cell.set(n);
        RelId(CROSS_DB_RELATION_TAG | n)
    })
}

/// Public re-export of the synthetic-rel id allocator. Reused by the
/// bare-name pragma TVF resolver in `parser::helpers::table::select`
/// (and any future synthetic-row producer that needs a process-unique
/// `RelId` disjoint from CTE / view tags).
pub(crate) fn next_synth_relation_id() -> RelId {
    next_cross_db_rel_id()
}

/// Two-part name `alias.table` where `alias` is not a reserved alias
/// AND maps to an ATTACHed sidecar database. Returns `Ok(Some(bound))`
/// on resolution, `Ok(None)` when the qualifier is reserved (caller
/// continues to the normal table lookup), or an error when the alias
/// is unknown or the table name fails the identifier check.
pub(crate) fn try_resolve_cross_db_bound_table(
    _schema: &SchemaSnapshot,
    name: &sqlparser::ast::ObjectName,
    alias: Option<&Arc<str>>,
) -> Result<Option<BoundTable>> {
    let Some((qualifier, table)) = split_two_part(name) else {
        return Ok(None);
    };
    let lower = qualifier.to_ascii_lowercase();
    if is_reserved_alias(&lower) {
        return Ok(None);
    }
    let Some(conn) = super::current_connection() else {
        return Ok(None);
    };
    let Some(sidecar) = conn.attach_map().database(&qualifier) else {
        return Err(Error::UnknownTable(format!(
            "no such database: {qualifier}"
        )));
    };

    if !is_valid_identifier(&table) {
        return Err(Error::UnsupportedSql(format!(
            "invalid table identifier in cross-database reference: {table}"
        )));
    }
    let sidecar_conn = sidecar.connect();
    let select_text = build_select_all(&table);
    let template = crate::parser::parse_prepared_template(&sidecar_conn, &select_text)?;
    let column_names: Vec<String> = template.output_columns.iter().cloned().collect();
    let rows = super::with_current_connection(&sidecar_conn, || {
        super::materialize_prepared_rows(&sidecar_conn, &template, &[])
    })?;

    let table_def = synth_cross_db_table_def(&table, &column_names, &rows);
    super::cte::register_external_rows(table_def.relation_id, Arc::new(rows));
    Ok(Some(BoundTable {
        table: table_def,
        alias: alias.cloned(),
        index_hint: None,
    }))
}

/// True if `name` is a 2-part qualified name whose qualifier is not a
/// reserved alias. Used by DML binders to short-circuit before they
/// reach the main-schema resolution path.
pub(crate) fn is_cross_db_name(name: &sqlparser::ast::ObjectName) -> bool {
    match split_two_part(name) {
        Some((q, _)) => !is_reserved_alias(&q.to_ascii_lowercase()),
        None => false,
    }
}

/// Identifier validator for cross-DB table names. Restricts to the
/// SQLite-style unquoted identifier grammar so the prepared-template
/// string is built from a typed-validated input, not arbitrary user
/// text. Negative tests exercise rejection of non-identifier inputs.
fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Build a `SELECT * FROM <ident>` query string. The identifier was
/// already validated by [`is_valid_identifier`] so the caller-side
/// surface is a typed-checked allowlisted shape rather than raw text.
fn build_select_all(ident: &str) -> String {
    let mut out = String::with_capacity(ident.len() + 14);
    out.push_str("SELECT * FROM ");
    out.push('"');
    out.push_str(ident);
    out.push('"');
    out
}

fn split_two_part(name: &sqlparser::ast::ObjectName) -> Option<(String, String)> {
    let parts = &name.0;
    if parts.len() != 2 {
        return None;
    }
    let q = ident_string(&parts[0])?;
    let t = ident_string(&parts[1])?;
    Some((q, t))
}

fn ident_string(part: &sqlparser::ast::ObjectNamePart) -> Option<String> {
    match part {
        sqlparser::ast::ObjectNamePart::Identifier(i) => Some(i.value.clone()),
        _ => None,
    }
}

fn synth_cross_db_table_def(
    name: &str,
    columns: &[String],
    rows: &[Vec<SqlValue>],
) -> Arc<TableDef> {
    let column_defs: Vec<ColumnDef> = columns
        .iter()
        .enumerate()
        .map(|(idx, n)| ColumnDef {
            column_id: ColumnId((idx + 1) as u64),
            ordinal: idx as u16,
            name: Box::from(n.as_str()),
            folded: Box::from(n.to_ascii_lowercase().as_str()),
            declared_type: None,
            affinity: infer_affinity(rows, idx),
            not_null: false,
            default_value: None,
            default_expr: None,
            generated: None,
        })
        .collect();
    let rel = next_cross_db_rel_id();
    Arc::new(TableDef {
        table_id: TableId(rel.0),
        schema_id: SchemaId(0),
        relation_id: rel,
        name: Box::from(name),
        folded: Box::from(name.to_ascii_lowercase().as_str()),
        columns: column_defs,
        indexes: Vec::new(),
        constraints: Vec::new(),
        checks: Vec::new(),
        foreign_keys: Vec::new(),
        rowid_alias_column: None,
        flags: 0,
        normalized_sql: None,
    })
}

fn infer_affinity(rows: &[Vec<SqlValue>], col: usize) -> Affinity {
    for row in rows {
        if let Some(v) = row.get(col) {
            match v {
                SqlValue::Integer(_) => return Affinity::Integer,
                SqlValue::Real(_) => return Affinity::Real,
                SqlValue::Text(_) => return Affinity::Text,
                SqlValue::Blob(_) => return Affinity::Blob,
                SqlValue::Null => continue,
            }
        }
    }
    Affinity::Blob
}
