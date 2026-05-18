use std::ops::Bound;

use crate::Result;
use crate::format::PageId;
use crate::telemetry::Phase11Counters;

use super::super::super::cells::LeafEntry;
use super::super::SnapshotView;
use super::super::{BtreeIndex, KeyRange, bound_to_owned};

mod count;
mod keys;
mod rowid;

pub struct RawIndexCursor<'idx> {
    index: &'idx BtreeIndex,
    start: Bound<Vec<u8>>,
    end: Bound<Vec<u8>>,
    view: SnapshotView<'idx>,
    counters: Option<&'idx Phase11Counters>,
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
            current_leaf: Some(leaf_id),
            entries: Vec::new(),
            entry_idx: 0,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
            prefetch_hints_emitted: 0,
        };
        cursor.load_current_leaf()?;
        if let Some(next_next) = cursor.next_leaf {
            cursor.prefetch_hint(next_next);
        }
        Ok(cursor)
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

    fn leaf_chain_past_end(&self) -> bool {
        let Some(last) = self.last_logical_key.as_deref() else {
            return false;
        };
        match &self.end {
            Bound::Excluded(b) => last >= b.as_slice(),
            Bound::Included(b) => last > b.as_slice(),
            Bound::Unbounded => false,
        }
    }

    fn advance_to(&mut self, next_id: PageId) -> Result<()> {
        self.current_leaf = Some(next_id);
        self.load_current_leaf()?;
        if let Some(next_next) = self.next_leaf {
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
        let (right, entries, last_key) = guard.with_page(|page| {
            let header = BtreeIndex::read_page_header(page)?;
            let entries = self.index.read_leaf_entries(page)?;
            let mut last_key: Option<Vec<u8>> = None;
            for entry in &entries {
                last_key = Some(entry.logical_key.clone());
            }
            Ok((header.right, entries, last_key))
        })?;
        self.entries = entries;
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
