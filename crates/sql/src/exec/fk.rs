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
