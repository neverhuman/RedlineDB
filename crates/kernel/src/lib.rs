//! RedlineDB transactional storage kernel primitives.
//!
//! This crate starts with the correctness-critical foundation: typed storage IDs,
//! explicit on-disk page/WAL encodings, checksums, and MVCC visibility rules.

pub mod catalog;
pub mod engine;
pub mod error;
#[macro_use]
pub mod failpoints;
pub mod format;
pub mod heap;
pub mod index;
pub mod integrity;
pub mod io;
pub mod json;
pub mod storage;
pub mod telemetry;
pub mod txn;
pub mod vector;
pub mod wal;

pub use error::{Error, Result};

/// Phase 5 WS-B6 — NUMA-aware shard pinning hooks. Re-exported here so
/// downstream crates can reach the helpers without `pub use`ing the
/// whole `storage::numa` module path. Off-feature the helpers are
/// trivial stubs (`numa_node_count() == 1`, `pin_current_thread_to_node`
/// returns `Ok(())`).
pub mod numa {
    pub use crate::storage::numa::*;
}

/// Internal re-export of the `fail` crate so the [`fail_point!`] macro can
/// resolve `fail::fail_point!` from any caller in the kernel without
/// requiring downstream crates to depend on `fail` directly.
#[cfg(feature = "failpoints")]
#[doc(hidden)]
pub use fail as __fail_reexport;
