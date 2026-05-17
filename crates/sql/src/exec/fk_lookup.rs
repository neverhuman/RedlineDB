//! FK parent-table resolution + key-equality helpers.
//!
//! Kept narrow: every function here is either a `(schema, fk)` resolver
//! or a value-level comparison. The cascade driver (`fk_cascade.rs`) is
//! the only consumer; SQLite semantics for NULL components (`MATCH
//! SIMPLE` — NULL exempts the row from the FK check) live here so the
//! driver stays declarative.

use std::sync::Arc;

use redlinedb_kernel::catalog::{ForeignKeyDef, SchemaSnapshot, TableDef};
use redlinedb_kernel::engine::{Engine, Txn};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::value::{SqlValue, compare_values};

use super::super::*;

/// Resolve the parent table referenced by `fk` from the current schema
/// snapshot. Errors with `ConstraintViolation` when the parent is missing,
/// matching SQLite's behaviour for an unresolved REFERENCES target.
pub(super) fn lookup_parent(
    schema: &SchemaSnapshot,
    fk: &ForeignKeyDef,
) -> Result<Arc<TableDef>> {
    match schema
        .tables
        .iter()
        .find(|t| t.folded.eq_ignore_ascii_case(&fk.parent_table))
        .cloned()
    {
        Some(t) => Ok(t),
        None => Err(Error::ConstraintViolation(format!(
            "FOREIGN KEY constraint refers to unknown table {}",
            fk.parent_table
        ))),
    }
}

/// Resolve the parent-column ordinals for `fk`. When the FK declaration
/// omitted the parent column list (SQLite-allowed shorthand), default to
/// the parent table's primary-key columns in declaration order.
pub(super) fn parent_column_ordinals(
    parent: &TableDef,
    fk: &ForeignKeyDef,
) -> Result<Vec<u16>> {
    if fk.parent_columns.is_empty() {
        let pk_index = match parent.indexes.iter().find(|ix| ix.primary) {
            Some(ix) => ix,
            None => return Err(Error::ConstraintViolation(format!(
                "parent table {} has no primary key for FOREIGN KEY",
                parent.name
            ))),
        };
        let mut ordinals = Vec::with_capacity(pk_index.keys.len());
        for key in &pk_index.keys {
            ordinals.push(key.ordinal);
        }
        return Ok(ordinals);
    }
    let mut ordinals = Vec::with_capacity(fk.parent_columns.len());
    for name in &fk.parent_columns {
        let folded = name.to_ascii_lowercase();
        let column = match parent
            .columns
            .iter()
            .find(|c| c.folded.as_ref() == folded.as_str())
        {
            Some(c) => c,
            None => return Err(Error::ConstraintViolation(format!(
                "FOREIGN KEY parent column '{name}' missing from {}",
                parent.name
            ))),
        };
        ordinals.push(column.ordinal);
    }
    Ok(ordinals)
}

pub(super) fn extract_child_key(fk: &ForeignKeyDef, values: &[SqlValue]) -> Vec<SqlValue> {
    fk.columns
        .iter()
        .map(|ord| {
            values
                .get(*ord as usize)
                .cloned()
                .unwrap_or(SqlValue::Null)
        })
        .collect()
}

pub(super) fn key_has_null(values: &[SqlValue]) -> bool {
    values.iter().any(|v| matches!(v, SqlValue::Null))
}

fn rows_match(
    child: &[SqlValue],
    parent_row: &[SqlValue],
    parent_ords: &[u16],
) -> bool {
    if child.len() != parent_ords.len() {
        return false;
    }
    for (cv, pord) in child.iter().zip(parent_ords.iter()) {
        let pv = parent_row
            .get(*pord as usize)
            .cloned()
            .unwrap_or(SqlValue::Null);
        if matches!(cv, SqlValue::Null) || matches!(pv, SqlValue::Null) {
            return false;
        }
        if compare_values(cv, &pv) != std::cmp::Ordering::Equal {
            return false;
        }
    }
    true
}

/// Returns true when a parent row carrying `key` exists in `parent`.
pub(super) fn parent_row_exists(
    engine: &Engine,
    tx: &mut Txn,
    parent: &Arc<TableDef>,
    parent_ords: &[u16],
    key: &[SqlValue],
) -> Result<bool> {
    let rowids = collect_table_rowids(engine, tx, parent)?;
    for rowid in rowids {
        if let Some(row) = load_table_row_by_rowid(engine, tx, parent, rowid)?
            && rows_match(key, &row.values, parent_ords)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Find every child table whose FK references `parent`. Returns
/// (child_table, fk-index-within-child) pairs.
pub(super) fn child_references(
    schema: &SchemaSnapshot,
    parent: &TableDef,
) -> Vec<(Arc<TableDef>, usize)> {
    let mut out = Vec::new();
    for child in &schema.tables {
        if child.table_id == parent.table_id {
            continue;
        }
        for (idx, fk) in child.foreign_keys.iter().enumerate() {
            if fk
                .parent_table
                .eq_ignore_ascii_case(&parent.folded)
                || fk.parent_table.eq_ignore_ascii_case(&parent.name)
            {
                out.push((Arc::clone(child), idx));
            }
        }
    }
    out
}

/// Find every child rowid whose FK columns equal `parent_key`. Used by
/// the cascade driver to decide which children participate in the
/// declared `ON DELETE` / `ON UPDATE` action.
pub(super) fn find_child_rows_matching(
    engine: &Engine,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    child_cols: &[u16],
    parent_key: &[SqlValue],
) -> Result<Vec<(redlinedb_kernel::format::RowId, Vec<SqlValue>)>> {
    let mut hits = Vec::new();
    let rowids = collect_table_rowids(engine, tx, child)?;
    for rowid in rowids {
        if let Some(row) = load_table_row_by_rowid(engine, tx, child, rowid)? {
            let mut matched = true;
            for (cord, pv) in child_cols.iter().zip(parent_key.iter()) {
                let cv = row
                    .values
                    .get(*cord as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null);
                if matches!(cv, SqlValue::Null) || matches!(pv, SqlValue::Null) {
                    matched = false;
                    break;
                }
                if compare_values(&cv, pv) != std::cmp::Ordering::Equal {
                    matched = false;
                    break;
                }
            }
            if matched {
                hits.push((row.rowid, row.values));
            }
        }
    }
    Ok(hits)
}

/// Public helper for tests / future tooling: report whether a child
/// row currently has a matching parent. Kept around so the FK harness
/// can sanity-check rows without driving DML mutations.
#[allow(dead_code)]
pub(crate) fn child_has_parent(
    conn: &Connection,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    fk_idx: usize,
    values: &[SqlValue],
) -> Result<bool> {
    let Some(fk) = child.foreign_keys.get(fk_idx) else {
        return Ok(true);
    };
    let key = extract_child_key(fk, values);
    if key_has_null(&key) {
        return Ok(true);
    }
    let schema = conn.engine().schema_snapshot();
    let parent = lookup_parent(&schema, fk)?;
    let parent_ords = parent_column_ordinals(&parent, fk)?;
    parent_row_exists(conn.engine(), tx, &parent, &parent_ords, &key)
}
