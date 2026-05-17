//! Deferred FK queue + COMMIT drain.
//!
//! `DEFERRABLE INITIALLY DEFERRED` constraints are buffered in
//! [`SessionState::deferred_fk_checks`] as they are observed at write
//! time. The buffer is drained either at:
//!   * autocommit boundary (`with_write_tx` in `exec/mod.rs`), or
//!   * explicit `COMMIT` (`Connection::commit` in `connection/session.rs`).
//!
//! A failed drain rolls the transaction back and surfaces the standard
//! `FOREIGN KEY constraint failed` error so the SQL surface stays
//! semantically identical to SQLite.

use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::format::RowId;

use crate::connection::Connection;
use crate::error::Result;
use crate::session::SessionState;

use super::super::*;

use super::lookup::{
    extract_child_key, key_has_null, lookup_parent, parent_column_ordinals, parent_row_exists,
};
use super::fk_violation_error;

/// Helper to drop all deferred checks (used by ROLLBACK).
pub(crate) fn clear_deferred_fk_checks(session: &mut SessionState) {
    session.deferred_fk_checks.clear();
}

/// Validate every buffered deferred FK check; called from COMMIT. Returns
/// an error on the first unresolved violation. SQLite's behaviour is to
/// surface a single `FOREIGN KEY constraint failed` and abort the commit;
/// we match that, dropping the remaining queue.
pub(crate) fn drain_deferred_fk_checks(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
) -> Result<()> {
    if !session.foreign_keys || session.deferred_fk_checks.is_empty() {
        session.deferred_fk_checks.clear();
        return Ok(());
    }
    let schema = conn.engine().schema_snapshot();
    let pending = std::mem::take(&mut session.deferred_fk_checks);
    for entry in pending {
        let Some(child) = schema.table_by_id(redlinedb_kernel::catalog::TableId(entry.child_table_id))
        else {
            continue;
        };
        let row = match load_table_row_by_rowid(
            conn.engine(),
            tx,
            &child,
            RowId(entry.child_rowid),
        )? {
            Some(row) => row,
            // Child row vanished (e.g. cascade deleted it): nothing to check.
            None => continue,
        };
        let Some(fk) = child.foreign_keys.get(entry.fk_index) else {
            continue;
        };
        let key = extract_child_key(fk, &row.values);
        if key_has_null(&key) {
            continue;
        }
        let parent = lookup_parent(&schema, fk)?;
        let parent_ords = parent_column_ordinals(&parent, fk)?;
        if !parent_row_exists(conn.engine(), tx, &parent, &parent_ords, &key)? {
            return Err(fk_violation_error(&child));
        }
    }
    Ok(())
}
