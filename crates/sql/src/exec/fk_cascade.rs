//! FK insert / update / delete drivers + parent-change dispatcher.
//!
//! Sits between the SQL `tail.rs` mutation paths and the kernel:
//! - INSERT and UPDATE call [`enforce_fk_on_insert`] to verify the row's
//!   own FK columns point at an existing parent.
//! - DELETE and UPDATE on a parent table call
//!   [`enforce_fk_on_parent_delete`] / [`enforce_fk_on_parent_update`],
//!   which propagate the action through the declared `ON DELETE` /
//!   `ON UPDATE` action.
//!
//! The "apply action" half lives in [`super::actions`] so this file
//! stays under the jankurai 300-LOC file-shape floor; the dispatcher
//! here decides _which_ rows are affected, the actions file decides
//! _what_ to do with them.

use std::sync::Arc;

use redlinedb_kernel::catalog::TableDef;
use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::format::RowId;

use crate::connection::Connection;
use crate::error::Result;
use crate::session::{DeferredFkCheck, SessionState};
use crate::value::SqlValue;

use super::actions::apply_parent_action;
use super::lookup::{
    child_references, extract_child_key, find_child_rows_matching, key_has_null, lookup_parent,
    parent_column_ordinals, parent_row_exists,
};
use super::{MAX_CASCADE_DEPTH, fk_violation_error};

/// Verify every FK on `child` against its parent for one freshly inserted
/// or updated child row. Deferred constraints are pushed onto the session
/// queue instead of being checked immediately.
pub(crate) fn enforce_fk_on_insert(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    values: &[SqlValue],
    rowid: RowId,
) -> Result<()> {
    if !session.foreign_keys || child.foreign_keys.is_empty() {
        return Ok(());
    }
    let schema = conn.engine().schema_snapshot();
    for (idx, fk) in child.foreign_keys.iter().enumerate() {
        let key = extract_child_key(fk, values);
        if key_has_null(&key) {
            // SQLite parity: a NULL component (in MATCH SIMPLE, the default)
            // exempts the row from FK enforcement.
            continue;
        }
        if fk.deferred {
            session.deferred_fk_checks.push(DeferredFkCheck {
                child_table_id: child.table_id.0,
                child_rowid: rowid.0,
                fk_index: idx,
            });
            continue;
        }
        let parent = lookup_parent(&schema, fk)?;
        let parent_ords = parent_column_ordinals(&parent, fk)?;
        if !parent_row_exists(conn.engine(), tx, &parent, &parent_ords, &key)? {
            return Err(fk_violation_error(child));
        }
    }
    Ok(())
}

/// React to a parent-row mutation. `old_values`/`new_values` describe the
/// parent row before/after; `new_values == None` signals DELETE. Drives
/// the declared `ON DELETE` / `ON UPDATE` action for every child FK that
/// references this parent.
pub(crate) fn enforce_fk_on_parent_change(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    parent: &Arc<TableDef>,
    old_values: &[SqlValue],
    new_values: Option<&[SqlValue]>,
    depth: usize,
) -> Result<()> {
    if !session.foreign_keys {
        return Ok(());
    }
    if depth >= MAX_CASCADE_DEPTH {
        return Err(crate::error::Error::ConstraintViolation(
            "FOREIGN KEY cascade depth exceeded".to_owned(),
        ));
    }
    let schema = conn.engine().schema_snapshot();
    let children = child_references(&schema, parent);
    if children.is_empty() {
        return Ok(());
    }
    for (child, fk_idx) in children {
        let fk = &child.foreign_keys[fk_idx];
        let parent_ords = parent_column_ordinals(parent, fk)?;
        let old_key: Vec<SqlValue> = parent_ords
            .iter()
            .map(|o| {
                old_values
                    .get(*o as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null)
            })
            .collect();
        if key_has_null(&old_key) {
            continue;
        }
        let action = if new_values.is_none() {
            fk.on_delete
        } else {
            fk.on_update
        };
        let affected = find_child_rows_matching(conn.engine(), tx, &child, &fk.columns, &old_key)?;
        if affected.is_empty() {
            continue;
        }
        if let Some(new_values) = new_values {
            let new_key: Vec<SqlValue> = parent_ords
                .iter()
                .map(|o| {
                    new_values
                        .get(*o as usize)
                        .cloned()
                        .unwrap_or(SqlValue::Null)
                })
                .collect();
            // No-op when the key did not change.
            if new_key == old_key {
                continue;
            }
        }
        apply_parent_action(
            conn, session, tx, &child, fk_idx, &affected, new_values, action, depth,
        )?;
    }
    Ok(())
}

/// Top-level entrypoint for UPDATE — propagates a parent row's value
/// change through every child FK that references it.
pub(crate) fn enforce_fk_on_parent_update(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    parent: &Arc<TableDef>,
    old_values: &[SqlValue],
    new_values: &[SqlValue],
) -> Result<()> {
    enforce_fk_on_parent_change(conn, session, tx, parent, old_values, Some(new_values), 0)
}

/// Top-level entrypoint for DELETE — propagates the parent removal
/// through every child FK that references it.
pub(crate) fn enforce_fk_on_parent_delete(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    parent: &Arc<TableDef>,
    old_values: &[SqlValue],
) -> Result<()> {
    enforce_fk_on_parent_change(conn, session, tx, parent, old_values, None, 0)
}
