//! RedlineDB transactional storage kernel primitives.
//!
//! This crate starts with the correctness-critical foundation: typed storage IDs,
//! explicit on-disk page/WAL encodings, checksums, and MVCC visibility rules.

/// A26: process-wide cached `std::thread::available_parallelism()`.
///
/// `available_parallelism()` on Linux walks the cgroup hierarchy looking for
/// CPU limits — each call does 5+ statx/openat syscalls reading
/// `/proc/self/cgroup` + walking up `/sys/fs/cgroup/.../cpu.max` ancestors.
/// `EngineConfig::default()` and `BufferPool::new()` BOTH call it, so every
/// `:memory:` open paid the probe twice. The parity corpus opens 1127 fresh
/// processes — that's > 10000 wasted syscalls and a real chunk of the
/// startup tax the profiling agent identified.
///
/// Cached in a process-wide `OnceLock`. CPU limits don't change during a
/// process's lifetime so caching is safe; first call still does the probe,
/// subsequent calls are an atomic load.
pub fn cached_available_parallelism() -> usize {
    use std::sync::OnceLock;
    static CACHED: OnceLock<usize> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(4)
    })
}

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
