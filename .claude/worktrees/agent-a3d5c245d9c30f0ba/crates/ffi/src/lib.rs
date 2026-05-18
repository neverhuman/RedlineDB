//! Redline DB C ABI: cdylib + staticlib surface.
//!
//! This crate exposes two coexisting C ABIs from the same library:
//!
//! 1. **Native Redline (`rldb_*`)** — the canonical surface. Each function
//!    has its definition (with `#[unsafe(no_mangle)]` and `extern "C"`) in
//!    one of the per-area submodules below. The cdylib's exported symbols
//!    must include every `rldb_*` listed in those modules.
//! 2. **SQLite-compat (`sqlite3_*`)** — drop-in replacement shims defined
//!    in [`sqlite3_compat`]. Each shim delegates to its `rldb_*`
//!    counterpart and layers on the status-recording side-effects callers
//!    of libsqlite3 expect.
//!
//! Refactor invariant: all `extern "C"` definitions live in submodules, but
//! the cdylib still exports them because each definition carries
//! `#[unsafe(no_mangle)]`. The `pub use` re-exports below keep the legacy
//! flat path (`redlinedb::rldb_open` etc.) for any in-crate consumer
//! (notably the test module).

#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod backup;
pub mod bind;
pub mod column;
pub mod config;
pub mod error;
pub mod exec;
pub mod lifecycle;
pub mod sqlite3_compat;
pub mod stmt;
pub mod types;
mod util;

#[cfg(test)]
mod tests;

// ---- Public re-exports ------------------------------------------------------
//
// `#[unsafe(no_mangle)]` lives on the actual definition, not on these
// re-exports — they are purely for Rust callers (tests, doctests, future
// internal users). The cdylib's exported C symbols are unaffected by
// whether or not these re-exports exist.

pub use backup::{
    rldb_backup_close, rldb_backup_finish, rldb_backup_init, rldb_backup_pagecount,
    rldb_backup_remaining, rldb_backup_step,
};
pub use bind::{
    rldb_bind_blob, rldb_bind_double, rldb_bind_int64, rldb_bind_null, rldb_bind_parameter_index,
    rldb_bind_text, rldb_parameter_count,
};
pub use column::{
    rldb_column_blob, rldb_column_bytes, rldb_column_count, rldb_column_double, rldb_column_int64,
    rldb_column_name, rldb_column_text, rldb_column_type,
};
pub use config::{
    rldb_busy_timeout, rldb_changes, rldb_checkpoint, rldb_last_insert_rowid, rldb_stats_json,
    rldb_vacuum,
};
pub use error::{rldb_errcode, rldb_errmsg, rldb_free, rldb_interrupt};
pub use exec::rldb_exec;
pub use lifecycle::{rldb_close, rldb_close_v2, rldb_open, rldb_open_v2};
pub use stmt::{rldb_clear_bindings, rldb_finalize, rldb_prepare_v2, rldb_reset, rldb_step};
pub use types::{rldb, rldb_backup, rldb_config, rldb_stmt, sqlite3, sqlite3_backup, sqlite3_stmt};
