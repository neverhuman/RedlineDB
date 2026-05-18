use std::ops::Bound;
use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;
use crate::format::{PageId, TxId};
use crate::telemetry::Phase11Counters;

use super::super::NON_TRANSACTIONAL_DELETE_TX;
use super::super::cells::{LeafCell, LeafCellRef};
use super::{
    BtreeIndex, CursorYield, IndexRowRef, KeyRange, SnapshotView, SnapshotViewKind, bound_to_owned,
    cached_tx_visible,
};

/// Raw leaf cursor for count / covering scans. It walks the same leaf
/// chain as [`super::IndexCursor`] but decodes cells in place, avoiding
/// the per-leaf `Vec<super::cells::LeafEntry>` materialisation that the
/// general cursor still performs for pre-cursor `Vec`-output parity.
pub struct RawIndexCursor<'idx> {
    index: &'idx BtreeIndex,
    start: Bound<Vec<u8>>,
    end: Bound<Vec<u8>>,
    view: SnapshotView<'idx>,
    counters: Option<&'idx Phase11Counters>,
    current_leaf: Option<PageId>,
    entry_idx: usize,
    next_leaf: Option<PageId>,
    last_logical_key: Option<Vec<u8>>,
    exhausted: bool,
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
        let cursor = Self {
            index,
            start: start_owned,
            end: end_owned,
            view,
            counters,
            current_leaf: Some(leaf_id),
            entry_idx: 0,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
        };
        Ok(cursor)
    }

    pub fn next_count_batch(&mut self, max_batch: usize) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let mut pushed = 0_usize;
        let mut visibility_cache = Vec::new();
        loop {
            if pushed >= max_batch {
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            let Some(leaf_id) = self.current_leaf else {
                self.exhausted = true;
                break;
            };
            let guard = self.index.inner.buffer.pin(leaf_id)?;
            self.index
                .inner
                .range_scan_leaves_visited
                .fetch_add(1, AtomicOrdering::Relaxed);
            if let Some(c) = self.counters {
                c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
            }
            let mut last_key: Option<Vec<u8>> = None;
            let mut next_leaf = None;
            let mut next_entry_idx = self.entry_idx;
            let batch_result = guard.with_page(|page| {
                let header = BtreeIndex::read_page_header(page)?;
                let slot_count = usize::from(page.slot_count()?);
                let mut slot = self.entry_idx;
                while slot < slot_count {
                    if pushed >= max_batch {
                        break;
                    }
                    let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                    last_key = Some(entry.logical_key.to_vec());
                    if self.matches_ref_cached(&entry, &mut visibility_cache)
                        && self.in_range(entry.logical_key)
                    {
                        pushed += 1;
                    }
                    slot += 1;
                }
                next_entry_idx = slot;
                next_leaf = header.right;
                Ok(())
            });
            batch_result?;
            self.entry_idx = next_entry_idx;
            self.next_leaf = next_leaf;
            self.last_logical_key = last_key;
            if pushed >= max_batch {
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    /// Count all remaining visible rows in the scan window in one leaf
    /// walk. This is the SQL COUNT(*) hot path: it avoids re-pinning
    /// a leaf for every batch and stops inside the leaf as soon as the
    /// ordered logical key reaches the upper bound.
    pub fn count_remaining(&mut self) -> Result<usize> {
        if self.exhausted {
            return Ok(0);
        }
        let mut count = 0_usize;
        let mut visibility_cache = Vec::new();
        loop {
            let Some(leaf_id) = self.current_leaf else {
                self.exhausted = true;
                break;
            };
            let guard = self.index.inner.buffer.pin(leaf_id)?;
            self.index
                .inner
                .range_scan_leaves_visited
                .fetch_add(1, AtomicOrdering::Relaxed);
            if let Some(c) = self.counters {
                c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
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
                    if self.lower_bound_allows(entry.logical_key)
                        && self.matches_ref_cached(&entry, &mut visibility_cache)
                    {
                        count = count.saturating_add(1);
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
            if stop_at_end_bound {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if count > 0
            && let Some(c) = self.counters
        {
            c.cursor_batches_emitted
                .fetch_add(1, AtomicOrdering::Relaxed);
        }
        Ok(count)
    }

    pub fn next_batch_with_keys(
        &mut self,
        out: &mut Vec<(Vec<u8>, IndexRowRef)>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        let mut visibility_cache = Vec::new();
        loop {
            if out.len() >= target {
                let pushed = out.len() - start_len;
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            let Some(leaf_id) = self.current_leaf else {
                self.exhausted = true;
                break;
            };
            let guard = self.index.inner.buffer.pin(leaf_id)?;
            self.index
                .inner
                .range_scan_leaves_visited
                .fetch_add(1, AtomicOrdering::Relaxed);
            if let Some(c) = self.counters {
                c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
            }
            let mut last_key: Option<Vec<u8>> = None;
            let mut next_leaf = None;
            let mut next_entry_idx = self.entry_idx;
            let batch_result = guard.with_page(|page| {
                let header = BtreeIndex::read_page_header(page)?;
                let slot_count = usize::from(page.slot_count()?);
                let mut slot = self.entry_idx;
                while slot < slot_count {
                    if out.len() >= target {
                        break;
                    }
                    let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                    last_key = Some(entry.logical_key.to_vec());
                    if self.matches_ref_cached(&entry, &mut visibility_cache)
                        && self.in_range(entry.logical_key)
                    {
                        out.push((entry.logical_key.to_vec(), entry.row));
                    }
                    slot += 1;
                }
                next_entry_idx = slot;
                next_leaf = header.right;
                Ok(())
            });
            batch_result?;
            self.entry_idx = next_entry_idx;
            self.next_leaf = next_leaf;
            self.last_logical_key = last_key;
            if out.len() >= target {
                let pushed = out.len() - start_len;
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        let pushed = out.len() - start_len;
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    /// Yield rowids in raw leaf order without cloning logical keys.
    /// This is used by SQL ordered LIMIT scans that still need one
    /// heap load for projection but do not need key bytes in the
    /// executor.
    pub fn next_rowid_batch(
        &mut self,
        out: &mut Vec<IndexRowRef>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        let mut visibility_cache = Vec::new();
        loop {
            if out.len() >= target {
                let pushed = out.len() - start_len;
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            let Some(leaf_id) = self.current_leaf else {
                self.exhausted = true;
                break;
            };
            let guard = self.index.inner.buffer.pin(leaf_id)?;
            self.index
                .inner
                .range_scan_leaves_visited
                .fetch_add(1, AtomicOrdering::Relaxed);
            if let Some(c) = self.counters {
                c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
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
                    if out.len() >= target {
                        break;
                    }
                    let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                    if self.at_or_past_end(entry.logical_key) {
                        stop_at_end_bound = true;
                        break;
                    }
                    if self.lower_bound_allows(entry.logical_key)
                        && self.matches_ref_cached(&entry, &mut visibility_cache)
                    {
                        out.push(entry.row);
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
            if out.len() >= target {
                let pushed = out.len() - start_len;
                if let Some(c) = self.counters {
                    c.cursor_batches_emitted
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                return Ok(CursorYield::Batch(pushed));
            }
            if stop_at_end_bound {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        let pushed = out.len() - start_len;
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    pub fn close(self) {
        // Drop self.
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

    fn matches_ref_cached(&self, entry: &LeafCellRef<'_>, cache: &mut Vec<(TxId, bool)>) -> bool {
        match self.view.inner {
            SnapshotViewKind::All => entry.delete_tx == TxId::ZERO,
            SnapshotViewKind::Visible {
                tx_status,
                snapshot,
                owner,
            } => {
                if entry.create_tx != TxId::ZERO
                    && !cached_tx_visible(tx_status, snapshot, owner, entry.create_tx, cache)
                {
                    return false;
                }
                if entry.delete_tx == NON_TRANSACTIONAL_DELETE_TX {
                    return false;
                }
                if entry.delete_tx != TxId::ZERO
                    && cached_tx_visible(tx_status, snapshot, owner, entry.delete_tx, cache)
                {
                    return false;
                }
                true
            }
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
}
