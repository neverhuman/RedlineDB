//! SQLite-compatible C ABI surface.
//!
//! These entry points preserve the standard `sqlite3_*` symbol names so
//! callers linked against libsqlite3 can swap in libredlinedb at runtime.
//! They delegate to the corresponding `rldb_*` implementation, layering on
//! the status-recording semantics expected by the SQLite ABI.

#[path = "sqlite3_api/bind.rs"]
mod bind;
#[path = "sqlite3_api/column.rs"]
mod column;
#[path = "sqlite3_api/core.rs"]
mod core;
#[path = "sqlite3_api/exec.rs"]
mod exec;
mod meta;
#[path = "sqlite3_api/stmt.rs"]
mod stmt;

pub use bind::*;
pub use column::*;
pub use core::*;
pub use exec::*;
#[allow(unused_imports)]
pub(crate) use meta::*;
pub use stmt::*;
