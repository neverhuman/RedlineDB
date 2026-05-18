use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;

use super::super::super::super::cells::LeafCell;
use super::super::super::super::{BtreeIndex, IndexRowRef};
use super::super::super::CursorYield;
use super::super::shared::{self, BatchKind};
use super::RawIndexCursor;

impl<'idx> RawIndexCursor<'idx> {
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
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Range,
                    out.len().saturating_sub(start_len),
                );
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
                        && shared::matches_ref_cached(self.view, &entry, &mut visibility_cache)
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
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Range,
                    out.len().saturating_sub(start_len),
                );
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
        let pushed = out.len().saturating_sub(start_len);
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            shared::finish_batch(self.counters, BatchKind::Range, pushed)
        }
    }

    pub fn close(self) {
        shared::close_cursor(self);
    }
}
