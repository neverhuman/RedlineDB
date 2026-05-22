use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{
    Csn, PageId, PageKind, PageState, RelId, RowId, TUPLE_FLAG_DELETED, TuplePtr, TupleVersion,
    TxId, UndoPtr,
};
use crate::txn::TxState;
use crate::{Error, Result};

use super::super::policy::{ActiveHeapPlacementPolicy, HeapPlacementPolicy, ReuseDecision};
use super::super::{PageBackedHeap, VacuumStats};

impl PageBackedHeap {
    pub fn vacuum(&self, horizon: Csn, tx_status: &ConcurrentTxStatus) -> Result<VacuumStats> {
        let mut stats = VacuumStats {
            oldest_active_snapshot_csn: horizon,
            ..VacuumStats::default()
        };
        let rows = self.all_relation_entries()?;
        stats.rows_scanned = rows.len();

        for (rel_id, row_id, ptr) in rows {
            self.vacuum_row(rel_id, row_id, ptr, horizon, tx_status, &mut stats)?;
        }

        Ok(stats)
    }

    fn vacuum_row(
        &self,
        rel_id: RelId,
        row_id: RowId,
        ptr: TuplePtr,
        horizon: Csn,
        tx_status: &ConcurrentTxStatus,
        stats: &mut VacuumStats,
    ) -> Result<()> {
        if self.head_for_relation(rel_id, row_id)? != Some(ptr) {
            return Ok(());
        }

        let mut current = self.read_tuple(ptr)?;
        let TxState::Committed(current_csn) = tx_status.state(current.begin_tx) else {
            return Ok(());
        };

        if current.flags & TUPLE_FLAG_DELETED != 0 {
            let rel_id = if current.rel_id == RelId::ZERO {
                self.rel_id
            } else {
                current.rel_id
            };
            if current_csn < horizon && self.remove_relation_head_if(rel_id, row_id, ptr)? {
                let _ = self.remove_head_if(row_id, ptr)?;
                stats.dead_rows_removed += 1;
                if self.page_has_no_heads(ptr.page_id)? {
                    self.mark_page_reusable(ptr.page_id, PageKind::Heap)?;
                }
            }
            return Ok(());
        }

        if current.undo_head == UndoPtr::ZERO {
            return Ok(());
        }

        let removable = self.removable_undo_chain_len(current.undo_head, horizon, tx_status)?;
        if removable == 0 {
            return Ok(());
        }
        if self.head_for_relation(rel_id, row_id)? != Some(ptr) {
            return Ok(());
        }

        current.undo_head = UndoPtr::ZERO;
        self.overwrite_tuple(ptr, &current)?;
        stats.chains_pruned += 1;
        stats.undo_links_removed += removable;
        if self.page_has_no_heads(ptr.page_id)? {
            self.mark_page_reusable(ptr.page_id, PageKind::Heap)?;
        }
        Ok(())
    }

    fn removable_undo_chain_len(
        &self,
        undo_head: UndoPtr,
        horizon: Csn,
        tx_status: &ConcurrentTxStatus,
    ) -> Result<usize> {
        let mut cursor = undo_head;
        let mut count = 0;
        while cursor != UndoPtr::ZERO {
            let undo = self.read_undo(cursor)?;
            let tuple = TupleVersion::decode(&undo.before_image)?;
            if tuple.end_tx == TxId::ZERO {
                return Ok(0);
            }
            match tx_status.state(tuple.end_tx) {
                TxState::Committed(end_csn) if end_csn < horizon => {
                    count += 1;
                    cursor = undo.prev_undo;
                }
                TxState::Committed(_) | TxState::InProgress | TxState::Aborted => return Ok(0),
            }
        }
        Ok(count)
    }

    fn page_has_no_heads(&self, page_id: PageId) -> Result<bool> {
        for shard in &self.relation_row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
            if shard
                .values()
                .any(|entries| entries.values().any(|ptr| ptr.page_id == page_id))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn push_reusable_page(&self, kind: PageKind, page_id: PageId) -> Result<()> {
        let reusable = match kind {
            PageKind::Heap => &self.reusable_heap_pages,
            PageKind::Undo => &self.reusable_undo_pages,
            _ => return Ok(()),
        };
        let mut reusable = reusable
            .lock()
            .map_err(|_| Error::CorruptPage("reusable page queue poisoned"))?;
        if !reusable.contains(&page_id) {
            reusable.push(page_id);
        }
        Ok(())
    }

    pub(crate) fn take_reusable_page(&self, kind: PageKind) -> Result<Option<PageId>> {
        let reusable = match kind {
            PageKind::Heap => &self.reusable_heap_pages,
            PageKind::Undo => &self.reusable_undo_pages,
            _ => return Ok(None),
        };
        let mut reusable = reusable
            .lock()
            .map_err(|_| Error::CorruptPage("reusable page queue poisoned"))?;
        if matches!(
            ActiveHeapPlacementPolicy::reusable_page(kind, 0, reusable.len()),
            ReuseDecision::AllocateFresh
        ) {
            return Ok(None);
        }
        Ok(reusable.pop())
    }

    fn mark_page_reusable(&self, page_id: PageId, kind: PageKind) -> Result<()> {
        self.clear_current_page_reference(page_id)?;
        let guard = self.buffer.pin(page_id)?;
        guard.with_page_mut(|page| {
            page.set_state(PageState::Reusable)?;
            page.set_free_class_hint(0)?;
            page.set_dead_bytes_hint(0)?;
            page.set_horizon_csn_hint(0)?;
            page.set_page_lsn(page.header()?.page_lsn)
        })?;
        self.push_reusable_page(kind, page_id)
    }

    fn clear_current_page_reference(&self, page_id: PageId) -> Result<()> {
        for lane in &self.append_lanes {
            let mut lane = lane
                .lock()
                .map_err(|_| Error::CorruptPage("heap append lane poisoned"))?;
            if lane.heap_page == Some(page_id) {
                lane.heap_page = None;
            }
            if lane.undo_page == Some(page_id) {
                lane.undo_page = None;
            }
        }
        Ok(())
    }
}
