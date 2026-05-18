//! Lane VE: vectorized executor primitives.
//!
//! Each submodule owns one operator class:
//!   - [`select`] — selection vectors / batch filtering.
//!   - [`spill`]  — temp-file format + lifecycle.
//!   - [`topk`]   — fixed-size heap for `ORDER BY ... LIMIT k`.
//!   - [`sort`]   — external merge-sort with spill.
//!   - [`hash_agg`] — hash-aggregation with spill on overflow.

// Public-API surface intentionally exposes all operator types so future
// callers (and the planner-side telemetry) can reach them without
// re-exporting through `exec.rs`. Tag items below as `dead_code` only
// while the initial wiring leaves them latent.
#![allow(dead_code, unused_imports)]

pub mod hash_agg;
pub mod select;
pub mod sort;
pub mod spill;
pub mod topk;

pub use hash_agg::{AggKind, HashAggregator};
pub use select::{
    DEFAULT_BATCH_ROWS, MAX_BATCH_ROWS, MIN_BATCH_ROWS, SelectionVector, batch_with_layout,
    filter_batch, row_from_batch,
};
pub use sort::SpillSort;
pub use spill::{SPILL_BLOCK_BYTES, SpillFile, SpillReader, SpillWriter};
pub use topk::{SortDirection, TOPK_LIMIT_THRESHOLD, TopKHeap};
