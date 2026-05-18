//! SQLite-compatible C ABI surface.
//!
//! These entry points preserve the standard `sqlite3_*` symbol names so
//! callers linked against libsqlite3 can swap in libredlinedb at runtime.
//! They delegate to the corresponding `rldb_*` implementation, layering on
//! the status-recording semantics expected by the SQLite ABI.

pub mod backup;
mod bind;
pub mod blob;
pub mod collation;
mod column;
pub mod context;
mod core;
mod exec;
pub mod hooks;
pub mod hooks_fire;
mod meta;
pub mod result;
mod stmt;
pub mod udf;
pub mod value;

pub use backup::*;
pub use bind::*;
pub use blob::*;
pub use collation::*;
pub use column::*;
pub use context::*;
pub use core::*;
pub use exec::*;
pub use hooks::*;
pub use hooks_fire::*;
#[allow(unused_imports)]
pub(crate) use meta::*;
pub use result::*;
pub use stmt::*;
pub use udf::*;
pub use value::*;
