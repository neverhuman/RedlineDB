use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;
use crate::format::PageId;
use crate::telemetry::Phase11Counters;

use super::super::super::cells::LeafCell;
use super::super::super::{BtreeIndex, IndexRowRef};
use super::super::{CursorYield, SnapshotView};
use super::shared::{self, BatchKind};

pub struct RawPointCursor<'idx> {
    index: &'idx BtreeIndex,
    logical_key: Vec<u8>,
    view: SnapshotView<'idx>,
    counters: Option<&'idx Phase11Counters>,
    current_leaf: Option<PageId>,
    entry_idx: usize,
    exhausted: bool,
}

impl<'idx> RawPointCursor<'idx> {
    pub fn open(
        index: &'idx BtreeIndex,
        logical_key: &[u8],
        view: SnapshotView<'idx>,
    ) -> Result<Self> {
        Self::open_with_counters(index, logical_key, view, None)
    }

    pub fn open_with_counters(
        index: &'idx BtreeIndex,
        logical_key: &[u8],
        view: SnapshotView<'idx>,
        counters: Option<&'idx Phase11Counters>,
    ) -> Result<Self> {
        let leaf_id = index.find_leaf(index.meta()?.root_page_id, logical_key)?;
        Ok(Self {
            index,
            logical_key: logical_key.to_vec(),
            view,
            counters,
            current_leaf: Some(leaf_id),
            entry_idx: 0,
            exhausted: false,
        })
    }

    pub fn next_rowid_batch(
        &mut self,
        out: &mut Vec<IndexRowRef>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch.max(1));
        let mut visibility_cache = Vec::new();
        loop {
            if out.len() >= target {
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Point,
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
            if let Some(c) = self.counters {
                c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
            }
            let mut first_entry_key: Option<Vec<u8>> = None;
            let mut next_leaf = None;
            let mut next_entry_idx = self.entry_idx;
            let mut leaf_first_key_past_target = false;
            let batch_result = guard.with_page(|page| {
                let header = BtreeIndex::read_page_header(page)?;
                next_leaf = header.right;
                let slot_count = usize::from(page.slot_count()?);
                if slot_count > 0 {
                    let first = LeafCell::decode_ref(page.cell(0)?)?;
                    leaf_first_key_past_target = first.logical_key > self.logical_key.as_slice();
                    first_entry_key = Some(first.logical_key.to_vec());
                }
                let mut slot = self.entry_idx;
                while slot < slot_count {
                    if out.len() >= target {
                        break;
                    }
                    let entry = LeafCell::decode_ref(page.cell(slot as u16)?)?;
                    if entry.logical_key == self.logical_key.as_slice()
                        && shared::matches_ref_cached(self.view, &entry, &mut visibility_cache)
                    {
                        out.push(entry.row);
                    } else if entry.logical_key > self.logical_key.as_slice()
                        && first_entry_key
                            .as_deref()
                            .is_some_and(|key| key <= self.logical_key.as_slice())
                    {
                        break;
                    }
                    slot += 1;
                }
                next_entry_idx = slot;
                Ok(())
            });
            batch_result?;
            self.entry_idx = next_entry_idx;
            if out.len() >= target {
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Point,
                    out.len().saturating_sub(start_len),
                );
            }
            if leaf_first_key_past_target {
                self.exhausted = true;
                break;
            }
            match next_leaf {
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
        shared::finish_batch(
            self.counters,
            BatchKind::Point,
            out.len().saturating_sub(start_len),
        )
    }

    pub fn close(self) {
        shared::close_cursor(self);
    }
}
