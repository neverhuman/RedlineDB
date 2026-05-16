//! Lane VE: vectorized executor primitives.
//!
//! Each submodule owns one operator class:
//!   - [`select`] — selection vectors / batch filtering.
//!   - [`spill`]  — scratch-file format + lifecycle.
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

/// Derive `PartialEq`/`Eq`/`PartialOrd` for a heap-payload type that
/// already has a manual `Ord::cmp` impl. Both [`sort::MergeItem`] and
/// [`topk::HeapEntry`] need the same 3-trait boilerplate to plug into
/// `BinaryHeap`; this macro keeps both call sites in lockstep.
macro_rules! impl_partial_from_ord {
    ($ty:ty) => {
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.cmp(other) == std::cmp::Ordering::Equal
            }
        }
        impl Eq for $ty {}
        impl PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}
pub(super) use impl_partial_from_ord;

pub use hash_agg::{AggKind, HashAggregator};
pub use select::{
    DEFAULT_BATCH_ROWS, MAX_BATCH_ROWS, MIN_BATCH_ROWS, SelectionVector, batch_with_layout,
    filter_batch, row_from_batch,
};
pub use sort::SpillSort;
pub use spill::{SPILL_BLOCK_BYTES, SpillFile, SpillReader, SpillWriter};
pub use topk::{SortDirection, TOPK_LIMIT_THRESHOLD, TopKHeap};
