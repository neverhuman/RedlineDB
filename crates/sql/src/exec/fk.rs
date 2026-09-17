//! A6 SQLite parity — foreign-key enforcement.
//!
//! Wired into the INSERT/UPDATE/DELETE paths via [`enforce_fk_on_insert`],
//! [`enforce_fk_on_parent_update`] and [`enforce_fk_on_parent_delete`]. The
//! PRAGMA `foreign_keys` toggle gates whether any check fires
//! (per-connection, mirroring SQLite). Cascade depth is bounded so a
//! cycle can never exhaust the stack. Deferred checks are buffered on
//! the SQL session and drained by [`drain_deferred_fk_checks`] at COMMIT.
//!
//! The implementation is split into focused submodules to stay under the
//! jankurai 300-LOC file-shape floor:
//!
//!   * `lookup`  — parent-table resolution, key extraction, row matching
//!   * `cascade` — insert/update/delete drivers and the recursive
//!     `ON DELETE`/`ON UPDATE` action driver (Cascade / Set Null /
//!     Set Default / Restrict / No Action)
//!   * `defer`   — DEFERRABLE INITIALLY DEFERRED queue + COMMIT drain
//!
//! Each file is `pub(super)` inside the `fk` module so the SQL crate sees
//! a single facade. Cross-module helpers go through these wrappers.

use std::collections::HashSet;
use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use redlinedb_kernel::txn::Isolation;

use crate::connection::Connection;
use crate::error::Result;
use crate::session::SessionState;
use crate::value::SqlValue;

use super::{collect_table_rowids, load_table_row_by_rowid};

#[path = "fk_actions.rs"]
mod actions;
#[path = "fk_cascade.rs"]
mod cascade;
#[path = "fk_defer.rs"]
mod defer;
#[path = "fk_lookup.rs"]
mod lookup;

pub(crate) use cascade::{
    enforce_fk_on_insert, enforce_fk_on_parent_delete, enforce_fk_on_parent_update,
};
pub(crate) use defer::{clear_deferred_fk_checks, drain_deferred_fk_checks};

#[allow(unused_imports)]
pub(crate) use lookup::child_has_parent;

/// Hard cap on cascade-recursion depth; SQLite uses the same heuristic
/// when guarding circular cascades and is bounded by SQLITE_MAX_TRIGGER_DEPTH.
pub(super) const MAX_CASCADE_DEPTH: usize = 1000;

pub(super) fn fk_violation_error(
    child: &redlinedb_kernel::catalog::TableDef,
) -> crate::error::Error {
    crate::error::Error::ConstraintViolation(format!(
        "FOREIGN KEY constraint failed: {}",
        child.name
    ))
}

/// SQLite `PRAGMA foreign_key_check`: scan the current database state
/// and return one row per visible FK violation.
pub(crate) fn foreign_key_check_rows(
    conn: &Connection,
    schema: &SchemaSnapshot,
) -> Result<Vec<Vec<SqlValue>>> {
    let pending = match crate::exec::current_session_ptr() {
        Some(ptr) => {
            // SAFETY: ptr installed by enclosing with_write_tx; lives for its scope.
            let session: &SessionState = unsafe { &*ptr };
            session.deferred_fk_checks.clone()
        }
        None => match conn.with_session(|session| Ok(session.deferred_fk_checks.clone())) {
            Ok(v) => v,
            Err(_) => Vec::new(),
        },
    };

    let mut rows = Vec::new();
    let mut seen: HashSet<(u64, u64, usize)> = HashSet::new();
    for check in pending {
        if seen.insert((check.child_table_id, check.child_rowid, check.fk_index)) {
            let Some(child) = schema
                .tables
                .iter()
                .find(|t| t.table_id.0 == check.child_table_id)
            else {
                continue;
            };
            let Some(fk) = child.foreign_keys.get(check.fk_index) else {
                continue;
            };
            rows.push(vec![
                SqlValue::Text(Arc::from(child.name.as_ref())),
                SqlValue::Integer(check.child_rowid as i64),
                SqlValue::Text(Arc::from(fk.parent_table.as_ref())),
                SqlValue::Integer(check.fk_index as i64),
            ]);
        }
    }

    let mut tx = conn.engine().begin(Isolation::Snapshot)?;
    for child in &schema.tables {
        if child.foreign_keys.is_empty() {
            continue;
        }
        let rowids = collect_table_rowids(conn.engine(), &mut tx, child)?;
        for rowid in rowids {
            let Some(row) = load_table_row_by_rowid(conn.engine(), &mut tx, child, rowid)? else {
                continue;
            };
            for (fk_idx, fk) in child.foreign_keys.iter().enumerate() {
                if seen.contains(&(child.table_id.0, rowid.0, fk_idx)) {
                    continue;
                }
                if child_has_parent(conn, &mut tx, child, fk_idx, &row.values)? {
                    continue;
                }
                seen.insert((child.table_id.0, rowid.0, fk_idx));
                rows.push(vec![
                    SqlValue::Text(Arc::from(child.name.as_ref())),
                    SqlValue::Integer(rowid.0 as i64),
                    SqlValue::Text(Arc::from(fk.parent_table.as_ref())),
                    SqlValue::Integer(fk_idx as i64),
                ]);
            }
        }
    }

    Ok(rows)
}
