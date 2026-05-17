//! FK action driver: applies the declared `ON DELETE` / `ON UPDATE`
//! action (NO ACTION / RESTRICT / CASCADE / SET NULL / SET DEFAULT) to
//! a collection of affected child rows. Recurses through
//! [`super::cascade::enforce_fk_on_parent_change`] so transitive FK
//! propagation stays a single algorithm.

use std::sync::Arc;

use redlinedb_kernel::catalog::{FkAction, TableDef};
use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::format::RowId;

use crate::connection::Connection;
use crate::error::Result;
use crate::session::SessionState;
use crate::value::SqlValue;

use super::super::*;

use super::cascade::{enforce_fk_on_insert, enforce_fk_on_parent_change};
use super::lookup::{lookup_parent, parent_column_ordinals};
use super::fk_violation_error;

/// Apply the declared FK action to every affected child row. The driver
/// recurses through `enforce_fk_on_parent_change` so cascading deletes/
/// updates propagate to grand-children.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_parent_action(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    fk_idx: usize,
    affected: &[(RowId, Vec<SqlValue>)],
    new_parent: Option<&[SqlValue]>,
    action: FkAction,
    depth: usize,
) -> Result<()> {
    let fk = &child.foreign_keys[fk_idx];
    match action {
        FkAction::NoAction | FkAction::Restrict => Err(fk_violation_error(child)),
        FkAction::Cascade => {
            if new_parent.is_none() {
                for (rowid, values) in affected {
                    cascade_delete_child(conn, session, tx, child, *rowid, values, depth)?;
                }
            } else {
                let parent = lookup_parent(&conn.engine().schema_snapshot(), fk)?;
                let parent_ords = parent_column_ordinals(&parent, fk)?;
                let new_parent = new_parent.unwrap();
                let new_key: Vec<SqlValue> = parent_ords
                    .iter()
                    .map(|o| new_parent.get(*o as usize).cloned().unwrap_or(SqlValue::Null))
                    .collect();
                for (rowid, values) in affected {
                    update_child_columns(
                        conn,
                        session,
                        tx,
                        child,
                        *rowid,
                        values,
                        &fk.columns,
                        &new_key,
                        depth,
                    )?;
                }
            }
            Ok(())
        }
        FkAction::SetNull => {
            let nulls = vec![SqlValue::Null; fk.columns.len()];
            for (rowid, values) in affected {
                update_child_columns(
                    conn,
                    session,
                    tx,
                    child,
                    *rowid,
                    values,
                    &fk.columns,
                    &nulls,
                    depth,
                )?;
            }
            Ok(())
        }
        FkAction::SetDefault => {
            let mut defaults = Vec::with_capacity(fk.columns.len());
            for ord in &fk.columns {
                let col = child
                    .columns
                    .get(*ord as usize)
                    .ok_or_else(|| crate::error::Error::ConstraintViolation(format!(
                        "FOREIGN KEY child column ordinal {ord} missing"
                    )))?;
                defaults.push(
                    col.default_value
                        .as_ref()
                        .cloned()
                        .unwrap_or(SqlValue::Null),
                );
            }
            for (rowid, values) in affected {
                update_child_columns(
                    conn,
                    session,
                    tx,
                    child,
                    *rowid,
                    values,
                    &fk.columns,
                    &defaults,
                    depth,
                )?;
            }
            Ok(())
        }
    }
}

fn cascade_delete_child(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    rowid: RowId,
    values: &[SqlValue],
    depth: usize,
) -> Result<()> {
    let engine = conn.engine();
    engine.delete_for_relation(tx, child.relation_id, rowid)?;
    super::super::index_dml::maintain_indexes_on_delete(engine, tx, child, values, rowid)?;
    // Recurse: the deleted child may itself be a parent for grand-children.
    enforce_fk_on_parent_change(conn, session, tx, child, values, None, depth + 1)
}

#[allow(clippy::too_many_arguments)]
fn update_child_columns(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    child: &Arc<TableDef>,
    rowid: RowId,
    old_values: &[SqlValue],
    fk_cols: &[u16],
    new_key: &[SqlValue],
    depth: usize,
) -> Result<()> {
    let engine = conn.engine();
    let mut new_values = old_values.to_vec();
    for (slot, ord) in fk_cols.iter().enumerate() {
        if let Some(target) = new_values.get_mut(*ord as usize) {
            *target = new_key.get(slot).cloned().unwrap_or(SqlValue::Null);
        }
    }
    apply_constraints(child, &new_values)?;
    let payload = encode_sql_row(child.table_id.0, &new_values)?;
    engine.update_for_relation(tx, child.relation_id, rowid, payload)?;
    super::super::index_dml::maintain_indexes_on_update(
        engine, tx, child, old_values, &new_values, rowid, rowid,
    )?;
    // Re-validate every FK on the cascaded row. Mirrors SQLite's behaviour
    // for SET DEFAULT (and SET NULL with NOT NULL columns): if the new
    // key does not itself reference an existing parent, the cascade is
    // an immediate violation.
    enforce_fk_on_insert(conn, session, tx, child, &new_values, rowid)?;
    // Propagate: the child's own children may need cascading too.
    enforce_fk_on_parent_change(
        conn,
        session,
        tx,
        child,
        old_values,
        Some(&new_values),
        depth + 1,
    )
}
