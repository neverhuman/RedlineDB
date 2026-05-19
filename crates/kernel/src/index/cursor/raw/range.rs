use std::ops::Bound;

use crate::Result;
use crate::format::PageId;
use crate::telemetry::Phase11Counters;

use super::super::super::cells::{LeafCell, LeafCellRef, LeafEntry};
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
        bound_in_range(&self.start, &self.end, logical_key)
    }

    fn lower_bound_allows(&self, logical_key: &[u8]) -> bool {
        bound_lower_allows(&self.start, logical_key)
    }

    fn at_or_past_end(&self, logical_key: &[u8]) -> bool {
        bound_at_or_past_end(&self.end, logical_key)
    }

    fn leaf_chain_past_end(&self) -> bool {
        bound_leaf_chain_past_end(&self.end, self.last_logical_key.as_deref())
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

    fn scan_current_leaf_entries<F>(&mut self, mut on_entry: F) -> Result<bool>
    where
        F: FnMut(LeafCellRef<'_>) -> Result<bool>,
    {
        let Some(leaf_id) = self.current_leaf else {
            self.exhausted = true;
            return Ok(false);
        };
        let leaf_latch = self.index.inner.latches.get(leaf_id);
        let _leaf_read = leaf_latch.read();
        let guard = self.index.inner.buffer.pin(leaf_id)?;
        self.index
            .inner
            .range_scan_leaves_visited
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(c) = self.counters {
            c.leaf_visits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        let mut next_leaf = None;
        let mut next_entry_idx = self.entry_idx;
        let mut stop_at_end_bound = false;
        let batch_result = guard.with_page(|page| {
            let header = BtreeIndex::read_page_header(page)?;
            next_leaf = header.right;
            let slot_count = usize::from(page.slot_count()?);
            let mut slot = self.entry_idx;
            while slot < slot_count {
                let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                if self.at_or_past_end(entry.logical_key) {
                    stop_at_end_bound = true;
                    break;
                }
                if !on_entry(entry)? {
                    slot += 1;
                    break;
                }
                slot += 1;
            }
            next_entry_idx = slot;
            Ok(())
        });
        batch_result?;
        self.entry_idx = next_entry_idx;
        self.next_leaf = next_leaf;
        self.last_logical_key = None;
        Ok(stop_at_end_bound)
    }

    fn prefetch_hint(&mut self, target: PageId) {
        self.prefetch_hints_emitted = self.prefetch_hints_emitted.saturating_add(1);
        if let Some(c) = self.counters {
            self.index.inner.buffer.prefetch(target, c);
        }
    }
}

pub(super) fn bound_lower_allows(start: &Bound<Vec<u8>>, logical_key: &[u8]) -> bool {
    match start {
        Bound::Included(b) => logical_key >= b.as_slice(),
        Bound::Excluded(b) => logical_key > b.as_slice(),
        Bound::Unbounded => true,
    }
}

pub(super) fn bound_at_or_past_end(end: &Bound<Vec<u8>>, logical_key: &[u8]) -> bool {
    match end {
        Bound::Included(b) => logical_key > b.as_slice(),
        Bound::Excluded(b) => logical_key >= b.as_slice(),
        Bound::Unbounded => false,
    }
}

pub(super) fn bound_in_range(
    start: &Bound<Vec<u8>>,
    end: &Bound<Vec<u8>>,
    logical_key: &[u8],
) -> bool {
    bound_lower_allows(start, logical_key) && !bound_at_or_past_end(end, logical_key)
}

pub(super) fn bound_leaf_chain_past_end(
    end: &Bound<Vec<u8>>,
    last_logical_key: Option<&[u8]>,
) -> bool {
    let Some(last) = last_logical_key else {
        return false;
    };
    match end {
        Bound::Excluded(b) => last >= b.as_slice(),
        Bound::Included(b) => last > b.as_slice(),
        Bound::Unbounded => false,
    }
}
