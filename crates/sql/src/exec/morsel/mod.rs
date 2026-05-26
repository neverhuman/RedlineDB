//! Phase 6 Morsel/Vector executor — scaffolding only.
//!
//! Defines the columnar batch primitives (`Morsel`, `ColumnBatch`,
//! `Bitmap`, `BytesArena`) and a tuple-to-morsel `MorselBuilder` adapter.
//! NO operator wiring yet; M2-M8 land separately per
//! `docs/phase6-morsel-vector.md`. The `dead_code` allow is intentional
//! for this scaffolding wave — the consumers arrive in M2 (scan) and M3
//! (filter kernels); without the allow, the entire module trips
//! `unused_*` until those land.
#![allow(dead_code)]

use smallvec::SmallVec;

pub mod arena;
pub mod bitmap;
pub mod builder;
pub mod column;
pub mod filter;
pub mod hash_agg;
pub mod scan;

#[allow(unused_imports)]
pub use arena::BytesArena;
#[allow(unused_imports)]
pub use bitmap::Bitmap;
#[allow(unused_imports)]
pub use builder::{ColumnKind, MorselBuilder};
#[allow(unused_imports)]
pub use column::ColumnBatch;
#[allow(unused_imports)]
pub use filter::{
    filter_i64_eq, filter_i64_ge, filter_i64_gt, filter_i64_le, filter_i64_lt, filter_i64_ne,
};
#[allow(unused_imports)]
pub use hash_agg::{AggKind as MorselAggKind, AggSpec, GroupSpec, MorselHashAggregator};
#[allow(unused_imports)]
pub use scan::{MorselScan, RowRef, ScanSource};

/// Hard upper bound on rows per morsel. Matches the inline-storage budget
/// of `Bitmap` (16 × u64 = 1024 bits).
pub const MAX_BATCH_ROWS: usize = 1024;

#[derive(Debug)]
pub struct Morsel<'a> {
    pub columns: SmallVec<[ColumnBatch<'a>; 8]>,
    pub validity: Bitmap,
    pub len: u16,
}

impl<'a> Morsel<'a> {
    pub fn new() -> Self {
        Self {
            columns: SmallVec::new(),
            validity: Bitmap::new(0),
            len: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    pub fn live_rows(&self) -> usize {
        self.validity.count_ones()
    }
}

impl<'a> Default for Morsel<'a> {
    fn default() -> Self {
        Self::new()
    }
}
