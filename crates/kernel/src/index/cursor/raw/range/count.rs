use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;

use super::super::super::super::BtreeIndex;
use super::super::super::super::cells::LeafCell;
use super::super::super::CursorYield;
use super::super::shared::{self, BatchKind};
use super::RawIndexCursor;

impl<'idx> RawIndexCursor<'idx> {
    pub fn next_count_batch(&mut self, max_batch: usize) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let mut pushed = 0_usize;
        let mut visibility_cache = Vec::new();
        loop {
            if pushed > 0 && pushed >= max_batch {
                return shared::finish_batch(self.counters, BatchKind::Range, pushed);
            }
            let Some(leaf_id) = self.current_leaf else {
                self.exhausted = true;
                break;
            };
            let leaf_latch = self.index.inner.latches.get(leaf_id);
            let _leaf_read = leaf_latch.read();
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
            let batch_result = guard.with_page(|page| {
                let header = BtreeIndex::read_page_header(page)?;
                next_leaf = header.right;
                let slot_count = usize::from(page.slot_count()?);
                let mut slot = self.entry_idx;
                while slot < slot_count {
                    if pushed > 0 && pushed >= max_batch {
                        break;
                    }
                    let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                    if shared::matches_ref_cached(self.view, &entry, &mut visibility_cache)
                        && self.in_range(entry.logical_key)
                    {
                        pushed += 1;
                    }
                    slot += 1;
                }
                next_entry_idx = slot;
                Ok(())
            });
            batch_result?;
            self.entry_idx = next_entry_idx;
            self.next_leaf = next_leaf;
            if pushed > 0 && pushed >= max_batch {
                return shared::finish_batch(self.counters, BatchKind::Range, pushed);
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
            shared::finish_batch(self.counters, BatchKind::Range, pushed)
        }
    }

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
            let leaf_latch = self.index.inner.latches.get(leaf_id);
            let _leaf_read = leaf_latch.read();
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
                        && shared::matches_ref_cached(self.view, &entry, &mut visibility_cache)
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
        if count > 0 {
            shared::bump_batch_counters(self.counters, BatchKind::Range);
        }
        Ok(count)
    }
}
