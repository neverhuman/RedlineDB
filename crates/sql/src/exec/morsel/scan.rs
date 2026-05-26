//! Morsel scan source. Wraps any row-at-a-time iterator (heap scan,
//! `RawIndexCursor` walk, synthetic generator) and rebatches its output
//! into columnar `Morsel<'arena>`s of up to `batch_target_rows` rows.
//!
//! The trait is intentionally narrow: a single `next_row` -> `Option<&[ValueRef]>`
//! that the scan loop drains into a `MorselBuilder`. Heap-scan and
//! `RawIndexCursor` adapters land alongside the executor wiring in M7;
//! this module exposes the shape so kernels (M3+) and tests (this round)
//! can be written against it.

use bumpalo::Bump;
use redlinedb_kernel::catalog::ValueRef;
use smallvec::SmallVec;

use crate::Result;

use super::builder::{ColumnKind, MorselBuilder};
use super::{MAX_BATCH_ROWS, Morsel};

/// Borrowed row view handed to `MorselScan` by a [`ScanSource`]. The
/// underlying slice may be reused across calls (the scan copies its
/// `ValueRef`s into the morsel before requesting the next row), so
/// implementors may return a borrow of a scratch buffer.
pub type RowRef<'a> = &'a [ValueRef<'a>];

/// Row-at-a-time producer feeding [`MorselScan`].
///
/// Implementors must NOT hold transactional state across calls — the
/// SQL executor's `CURRENT_TX` thread-local is not visible here. A heap
/// or index adapter constructed at M7 must capture an explicit
/// `SnapshotView<'idx>` (which is `Copy` and lock-free) at open time and
/// use it for all visibility checks; this keeps the scan source
/// reusable from worker threads.
pub trait ScanSource {
    /// Yield the next row, or `Ok(None)` when exhausted. Each returned
    /// slice's `ValueRef` lifetime is tied to `&mut self`; the scan
    /// caller copies them into the morsel before re-borrowing.
    fn next_row(&mut self) -> Result<Option<RowRef<'_>>>;
}

/// Rebatches a [`ScanSource`] into `Morsel<'arena>`s.
///
/// `batch_target_rows` is clamped to `[1, MAX_BATCH_ROWS]`. The arena
/// is borrowed for the morsel lifetime so all variable-length payloads
/// (Text/Blob) share a single bump allocation per morsel.
pub struct MorselScan<'a, S: ScanSource> {
    source: S,
    arena: &'a Bump,
    column_kinds: SmallVec<[ColumnKind; 8]>,
    batch_target_rows: u16,
    exhausted: bool,
}

impl<'a, S: ScanSource> MorselScan<'a, S> {
    /// Open a scan against `source`, allocating into `arena`. `kinds`
    /// must match the row width returned by `source.next_row()`.
    pub fn new(
        source: S,
        arena: &'a Bump,
        kinds: &[ColumnKind],
        batch_target_rows: u16,
    ) -> Self {
        let target = batch_target_rows
            .max(1)
            .min(MAX_BATCH_ROWS as u16);
        let mut column_kinds: SmallVec<[ColumnKind; 8]> = SmallVec::with_capacity(kinds.len());
        column_kinds.extend_from_slice(kinds);
        Self {
            source,
            arena,
            column_kinds,
            batch_target_rows: target,
            exhausted: false,
        }
    }

    /// Drain at most `batch_target_rows` rows from the source into a
    /// fresh morsel. Returns `Ok(None)` once the source is exhausted
    /// and no rows remain to flush.
    pub fn next_morsel(&mut self) -> Result<Option<Morsel<'a>>> {
        if self.exhausted {
            return Ok(None);
        }
        let cap = self.batch_target_rows as usize;
        let mut builder = MorselBuilder::with_capacity(self.arena, &self.column_kinds, cap);
        let mut pushed = 0_usize;
        while pushed < cap {
            match self.source.next_row()? {
                Some(row) => {
                    // push_row only errors on shape mismatch / overflow;
                    // both are programmer bugs in this layer, so surface
                    // via a plain panic in debug builds. Release builds
                    // simply skip the row to keep the scan moving.
                    if let Err(_msg) = builder.push_row(row) {
                        debug_assert!(false, "MorselScan push_row failed: {_msg}");
                        continue;
                    }
                    pushed += 1;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if pushed == 0 {
            return Ok(None);
        }
        Ok(Some(builder.finish()))
    }

    /// Target rows-per-morsel (post-clamp).
    pub fn batch_target_rows(&self) -> u16 {
        self.batch_target_rows
    }

    /// `true` once the underlying source returned `None` at least once
    /// and any final partial morsel has been emitted.
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }
}
