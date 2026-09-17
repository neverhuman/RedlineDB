use std::ops::Bound;

use crate::Result;
use crate::format::PageId;
use crate::telemetry::Phase11Counters;

use super::super::super::cells::LeafEntry;
use super::super::super::policy::{ActiveIndexCursorPolicy, IndexCursorPolicy};
use super::super::SnapshotView;
use super::super::{BtreeIndex, KeyRange, bound_to_owned};
use super::scratch::IndexScanScratch;

mod count;
mod keys;
mod rowid;

/// Phase 5 WS-A2c: cursor walk direction. `Forward` keeps the historic
/// left-to-right leaf-chain walk; `Reverse` walks right-to-left via the
/// `PageHeader::left` link so `ORDER BY x DESC LIMIT n` can early-stop
/// without falling back to an in-memory sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

pub struct RawIndexCursor<'idx> {
    index: &'idx BtreeIndex,
    start: Bound<Vec<u8>>,
    end: Bound<Vec<u8>>,
    view: SnapshotView<'idx>,
    counters: Option<&'idx Phase11Counters>,
    direction: Direction,
    current_leaf: Option<PageId>,
    entries: Vec<LeafEntry>,
    entry_idx: usize,
    next_leaf: Option<PageId>,
    last_logical_key: Option<Vec<u8>>,
    exhausted: bool,
    prefetch_hints_emitted: u64,
}

impl<'idx> RawIndexCursor<'idx> {
    pub fn open(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
    ) -> Result<Self> {
        Self::open_with_counters(index, range, view, None)
    }

    pub fn open_with_counters(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
        counters: Option<&'idx Phase11Counters>,
    ) -> Result<Self> {
        let start_owned = bound_to_owned(range.start);
        let end_owned = bound_to_owned(range.end);
        let probe: &[u8] = match &start_owned {
            Bound::Included(b) | Bound::Excluded(b) => b.as_slice(),
            Bound::Unbounded => &[],
        };
        let leaf_id = index.find_leaf(index.meta()?.root_page_id, probe)?;
        let mut cursor = Self {
            index,
            start: start_owned,
            end: end_owned,
            view,
            counters,
            direction: Direction::Forward,
            current_leaf: Some(leaf_id),
            entries: Vec::new(),
            entry_idx: 0,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
            prefetch_hints_emitted: 0,
        };
        cursor.load_current_leaf()?;
        if let Some(next_next) = cursor.next_leaf
            && ActiveIndexCursorPolicy::prefetch_right_sibling(cursor.entries.len(), true)
        {
            cursor.prefetch_hint(next_next);
        }
        Ok(cursor)
    }

    /// Phase 5 WS-A2c: open the cursor for reverse iteration. The
    /// returned cursor positions at the right-most leaf covered by
    /// `range.end` (or the tree's right-most leaf if `end` is unbounded)
    /// and `next_rowid_batch` walks right-to-left until the lower bound
    /// is reached. Visibility filtering is identical to the forward
    /// path.
    pub fn open_reverse_with_counters(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
        counters: Option<&'idx Phase11Counters>,
    ) -> Result<Self> {
        let start_owned = bound_to_owned(range.start);
        let end_owned = bound_to_owned(range.end);
        let root = index.meta()?.root_page_id;
        let leaf_id = match &end_owned {
            Bound::Included(b) | Bound::Excluded(b) => index.find_leaf(root, b.as_slice())?,
            Bound::Unbounded => index.find_rightmost_leaf(root)?,
        };
        let cursor = Self {
            index,
            start: start_owned,
            end: end_owned,
            view,
            counters,
            direction: Direction::Reverse,
            current_leaf: Some(leaf_id),
            entries: Vec::new(),
            // Sentinel: the reverse rowid loop sees `entry_idx == usize::MAX`
            // and positions at `slot_count - 1` on the first leaf visit.
            entry_idx: usize::MAX,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
            prefetch_hints_emitted: 0,
        };
        Ok(cursor)
    }

    /// Phase 5 WS-A4: same as [`open_with_counters`] but reuses
    /// `scratch.entries` for the per-leaf `LeafEntry` buffer. The
    /// scratch's `Vec` is swapped into the cursor before the first
    /// `load_current_leaf` so the initial leaf decode appends into the
    /// already-capacity-warm buffer. Pair with
    /// [`close_into_scratch`](Self::close_into_scratch) to swap the
    /// buffer back so the next cursor opened against the same scratch
    /// inherits the capacity. Forward-walk only; reverse-walk callers
    /// can still use [`open_reverse_with_counters`] (which never
    /// touches `self.entries`).
    pub fn open_with_scratch_and_counters(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
        counters: Option<&'idx Phase11Counters>,
        scratch: &mut IndexScanScratch,
    ) -> Result<Self> {
        let start_owned = bound_to_owned(range.start);
        let end_owned = bound_to_owned(range.end);
        let probe: &[u8] = match &start_owned {
            Bound::Included(b) | Bound::Excluded(b) => b.as_slice(),
            Bound::Unbounded => &[],
        };
        let leaf_id = index.find_leaf(index.meta()?.root_page_id, probe)?;
        // Move the scratch's reusable buffer into the cursor. The
        // buffer is empty (the scratch was either freshly built or
        // reset by the previous `close_into_scratch`) so this is just
        // a capacity hand-off.
        let mut entries: Vec<LeafEntry> = Vec::new();
        std::mem::swap(&mut entries, &mut scratch.entries);
        scratch.key_arena.reset();
        let mut cursor = Self {
            index,
            start: start_owned,
            end: end_owned,
            view,
            counters,
            direction: Direction::Forward,
            current_leaf: Some(leaf_id),
            entries,
            entry_idx: 0,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
            prefetch_hints_emitted: 0,
        };
        cursor.load_current_leaf()?;
        if let Some(next_next) = cursor.next_leaf
            && ActiveIndexCursorPolicy::prefetch_right_sibling(cursor.entries.len(), true)
        {
            cursor.prefetch_hint(next_next);
        }
        Ok(cursor)
    }

    /// Phase 5 WS-A4: close the cursor and return its `entries` buffer
    /// to `scratch` so the next cursor opened against the same scratch
    /// reuses the warm capacity. Mirrors the
    /// [`close`](super::super::super::raw::range::rowid)-via-drop
    /// semantics; the cursor is consumed.
    pub fn close_into_scratch(mut self, scratch: &mut IndexScanScratch) {
        // Reset the scratch (drops the placeholder empty Vec we left
        // there at open time) then swap the warm cursor buffer back in.
        scratch.reset();
        std::mem::swap(&mut self.entries, &mut scratch.entries);
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    fn in_range(&self, logical_key: &[u8]) -> bool {
        let lower_ok = self.lower_bound_allows(logical_key);
        if !lower_ok {
            return false;
        }
        !self.at_or_past_end(logical_key)
    }

    fn lower_bound_allows(&self, logical_key: &[u8]) -> bool {
        match &self.start {
            Bound::Included(b) => logical_key >= b.as_slice(),
            Bound::Excluded(b) => logical_key > b.as_slice(),
            Bound::Unbounded => true,
        }
    }

    fn at_or_past_end(&self, logical_key: &[u8]) -> bool {
        match &self.end {
            Bound::Included(b) => logical_key > b.as_slice(),
            Bound::Excluded(b) => logical_key >= b.as_slice(),
            Bound::Unbounded => false,
        }
    }

    /// Phase 5 WS-A2c: reverse-walk mirror of `lower_bound_allows`.
    /// Returns true iff `logical_key` is strictly before the cursor's
    /// `end` bound (so it is a valid row to emit).
    pub(super) fn upper_bound_allows(&self, logical_key: &[u8]) -> bool {
        match &self.end {
            Bound::Included(b) => logical_key <= b.as_slice(),
            Bound::Excluded(b) => logical_key < b.as_slice(),
            Bound::Unbounded => true,
        }
    }

    /// Phase 5 WS-A2c: reverse-walk mirror of `at_or_past_end`. Returns
    /// true iff `logical_key` has fallen past the lower bound, so the
    /// reverse walk should stop.
    pub(super) fn at_or_before_start(&self, logical_key: &[u8]) -> bool {
        match &self.start {
            Bound::Included(b) => logical_key < b.as_slice(),
            Bound::Excluded(b) => logical_key <= b.as_slice(),
            Bound::Unbounded => false,
        }
    }

    fn leaf_chain_past_end(&self) -> bool {
        ActiveIndexCursorPolicy::stop_after_leaf(self.last_logical_key.as_deref(), &self.end)
    }

    fn advance_to(&mut self, next_id: PageId) -> Result<()> {
        self.current_leaf = Some(next_id);
        self.load_current_leaf()?;
        if let Some(next_next) = self.next_leaf
            && ActiveIndexCursorPolicy::prefetch_right_sibling(self.entries.len(), true)
        {
            self.prefetch_hint(next_next);
        }
        Ok(())
    }

    fn load_current_leaf(&mut self) -> Result<()> {
        let leaf_id = match self.current_leaf {
            Some(id) => id,
            None => {
                self.entries.clear();
                self.entry_idx = 0;
                self.next_leaf = None;
                self.last_logical_key = None;
                return Ok(());
            }
        };
        let guard = self.index.inner.buffer.pin(leaf_id)?;
        self.index
            .inner
            .range_scan_leaves_visited
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(c) = self.counters {
            c.leaf_visits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        let (right, mut entries, last_key) = guard.with_page(|page| {
            let header = BtreeIndex::read_page_header(page)?;
            let entries = self.index.read_leaf_entries(page)?;
            let last_key: Option<Vec<u8>> = entries.last().map(|e| e.logical_key.clone());
            Ok((header.right, entries, last_key))
        })?;
        // Phase 5 WS-A4: clear-and-append preserves the capacity-warm
        // buffer when the cursor was opened with a scratch. The
        // `read_leaf_entries` call still allocates a transient Vec, but
        // `append` moves its elements into our hot buffer (the source
        // vec is dropped empty) so subsequent leaves on this cursor
        // and the next cursor opened against the same scratch reuse
        // the same backing storage.
        self.entries.clear();
        self.entries.append(&mut entries);
        self.entry_idx = 0;
        self.next_leaf = right;
        self.last_logical_key = last_key;
        Ok(())
    }

    fn prefetch_hint(&mut self, target: PageId) {
        self.prefetch_hints_emitted = self.prefetch_hints_emitted.saturating_add(1);
        if let Some(c) = self.counters {
            self.index.inner.buffer.prefetch(target, c);
        }
    }
}
