//! Reusable per-statement scratch for `RawIndexCursor` leaf reads.
//! Phase 5 WS-A4: a `RawIndexCursor` opened with a scratch swaps its
//! freshly-allocated empty `entries: Vec<LeafEntry>` with the scratch's
//! reusable buffer at open time and swaps it back at `close` time. The
//! `bumpalo::Bump` arena is reserved for future per-leaf borrowed-key
//! allocations; today it just sits next to the entry buffer so callers
//! can declare one scratch per statement and amortise its capacity
//! across every cursor the statement opens.

use bumpalo::Bump;

use super::super::super::cells::LeafEntry;

#[derive(Default)]
pub struct IndexScanScratch {
    pub(in crate::index) entries: Vec<LeafEntry>,
    pub(in crate::index) key_arena: Bump,
}

impl IndexScanScratch {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(128),
            key_arena: Bump::with_capacity(64 * 1024),
        }
    }

    /// Drop the contents (preserving allocated capacity) so the next
    /// cursor can reuse the buffers. Called by `RawIndexCursor::close`
    /// when the cursor was opened with a scratch.
    pub(crate) fn reset(&mut self) {
        self.entries.clear();
        self.key_arena.reset();
    }
}
