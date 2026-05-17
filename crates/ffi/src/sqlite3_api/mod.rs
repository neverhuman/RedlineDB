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

// Extended SQLite-compatible surfaces added by FFI workstream B1-B5.
pub mod blob;
pub mod collation;
pub mod context;
pub mod hooks;
pub mod hooks_fire;
pub mod result;
pub mod udf;
pub mod value;

pub use blob::*;
pub use collation::*;
pub use context::*;
pub use hooks::*;
pub use hooks_fire::{
    __test_fire_authorizer, __test_fire_busy, __test_fire_commit, __test_fire_profile,
    __test_fire_rollback, __test_fire_trace, __test_fire_update,
};
pub use result::*;
pub use udf::*;
pub use value::*;
