use super::{PageBackedHeap, advance_atomic_past};
use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{
    Csn, PageId, PageKind, PageState, RelId, RowId, TUPLE_FLAG_DELETED, TuplePtr, TupleVersion,
    TxId, UndoPtr,
};
use crate::txn::{Snapshot, TxState};
use crate::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

impl PageBackedHeap {
    pub fn vacuum(
        &self,
        horizon: Csn,
        tx_status: &ConcurrentTxStatus,
    ) -> Result<super::VacuumStats> {
        let mut stats = super::VacuumStats {
            oldest_active_snapshot_csn: horizon,
            ..super::VacuumStats::default()
        };
        let rows = self.all_relation_entries()?;
        stats.rows_scanned = rows.len();

        for (rel_id, row_id, ptr) in rows {
            self.vacuum_row(rel_id, row_id, ptr, horizon, tx_status, &mut stats)?;
        }

        Ok(stats)
    }

    pub fn load_row_directory_from_pages(&self, page_count: u64) -> Result<()> {
        for page_no in 1..=page_count {
            let page_id = PageId(page_no);
            let guard = match self.buffer.pin(page_id) {
                Ok(guard) => guard,
                Err(Error::InvalidMagic { actual: 0, .. }) => continue,
                Err(err) => return Err(err),
            };
            guard.with_page(|page| {
                let header = page.header()?;
                if header.kind != PageKind::Heap || header.rel_id != self.rel_id {
                    return Ok(());
                }
                if header.state == PageState::Reusable {
                    return Ok(());
                }

                for slot in (0..page.slot_count()?).rev() {
                    let tuple = TupleVersion::decode(page.cell(slot)?)?;
                    advance_atomic_past(&self.next_row, tuple.row_id.0);
                    let rel_id = if tuple.rel_id == RelId::ZERO {
                        self.rel_id
                    } else {
                        tuple.rel_id
                    };
                    let ptr = TuplePtr::new_with_generation(page_id, slot, header.generation);
                    if self.head(tuple.row_id)?.is_none() {
                        self.set_head(tuple.row_id, ptr)?;
                    }
                    if self.head_for_relation(rel_id, tuple.row_id)?.is_none() {
                        self.set_relation_head(rel_id, tuple.row_id, ptr)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn invalidate_row_directory_for_pages(&self, pages: &[PageId]) -> Result<()> {
        let pages: HashSet<PageId> = pages.iter().copied().collect();
        for shard in &self.row_dir {
            let mut shard = shard
                .write()
                .map_err(|_| Error::CorruptPage("row directory shard poisoned"))?;
            shard.retain(|_, ptr| !pages.contains(&ptr.page_id));
        }
        for shard in &self.relation_row_dir {
            let mut shard = shard
                .write()
                .map_err(|_| Error::CorruptPage("relation row directory shard poisoned"))?;
            shard.retain(|_, rows| {
                rows.retain(|_, ptr| !pages.contains(&ptr.page_id));
                !rows.is_empty()
            });
        }
        Ok(())
    }

    pub fn load_reusable_pages_from_pages(&self, page_count: u64) -> Result<()> {
        for page_no in 1..=page_count {
            let page_id = PageId(page_no);
            let guard = match self.buffer.pin(page_id) {
                Ok(guard) => guard,
                Err(Error::InvalidMagic { actual: 0, .. }) => continue,
                Err(err) => return Err(err),
            };
            let header = guard.with_page(|page| page.header())?;
            if header.rel_id != self.rel_id || header.state != PageState::Reusable {
                continue;
            }
            self.push_reusable_page(header.kind, page_id)?;
        }
        Ok(())
    }

    pub fn row_directory_entries(&self) -> Result<Vec<(RowId, TuplePtr)>> {
        let mut rows = Vec::new();
        for shard in &self.row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
            rows.extend(shard.iter().map(|(row_id, ptr)| (*row_id, *ptr)));
        }
        Ok(rows)
    }

    pub fn relation_entries(&self, rel_id: RelId) -> Result<Vec<(RowId, TuplePtr)>> {
        let mut rows = Vec::new();
        for shard in &self.relation_row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
            if let Some(entries) = shard.get(&rel_id) {
                rows.extend(entries.iter().map(|(row_id, ptr)| (*row_id, *ptr)));
            }
        }
        Ok(rows)
    }

    /// Walk the relation's row directory and return every row that is
    /// visible to `snapshot`. Lane INT consumes this for the heap/index
    /// equivalence check: every row returned here MUST resolve to an index
    /// entry on every catalog index over the relation. Rows that are
    /// invisible (uncommitted or shadowed by a later version), tombstoned,
    /// or whose head has been concurrently vacuumed are silently skipped —
    /// they are not part of the durable visible state.
    pub fn iter_visible_rows_for_relation(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        rel_id: RelId,
    ) -> Result<Vec<(RowId, Vec<u8>)>> {
        // Lane E failpoint: armed before the integrity checker iterates the
        // heap so a future failpoint case can exercise crash-during-check.
        crate::fail_point!("integrity::heap::visible");
        let entries = self.relation_entries(rel_id)?;
        let mut out = Vec::with_capacity(entries.len());
        for (row_id, _ptr) in entries {
            let payload = self.get_for_relation(tx_status, snapshot, None, rel_id, row_id)?;
            if let Some(payload) = payload {
                out.push((row_id, payload));
            }
        }
        Ok(out)
    }

    pub fn relation_rowids(&self, rel_id: RelId) -> Result<Vec<RowId>> {
        let mut rows = Vec::new();
        for shard in &self.relation_row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
            if let Some(entries) = shard.get(&rel_id) {
                rows.extend(entries.keys().copied());
            }
        }
        Ok(rows)
    }

    fn all_relation_entries(&self) -> Result<Vec<(RelId, RowId, TuplePtr)>> {
        let mut rows = Vec::new();
        for shard in &self.relation_row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
            for (rel_id, entries) in shard.iter() {
                rows.extend(entries.iter().map(|(row_id, ptr)| (*rel_id, *row_id, *ptr)));
            }
        }
        Ok(rows)
    }

    pub(crate) fn head(&self, row_id: RowId) -> Result<Option<TuplePtr>> {
        let shard = self.row_dir_shard(row_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        Ok(shard.get(&row_id).copied())
    }

    pub(crate) fn head_for_relation(
        &self,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<Option<TuplePtr>> {
        let shard = self.relation_row_dir_shard(rel_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        Ok(shard
            .get(&rel_id)
            .and_then(|entries| entries.get(&row_id).copied()))
    }

    pub(crate) fn set_head(&self, row_id: RowId, ptr: TuplePtr) -> Result<()> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        shard.insert(row_id, ptr);
        Ok(())
    }

    pub(crate) fn set_relation_head(
        &self,
        rel_id: RelId,
        row_id: RowId,
        ptr: TuplePtr,
    ) -> Result<()> {
        let shard = self.relation_row_dir_shard(rel_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        shard.entry(rel_id).or_default().insert(row_id, ptr);
        Ok(())
    }

    fn remove_head_if(&self, row_id: RowId, expected: TuplePtr) -> Result<bool> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        if shard.get(&row_id).copied() == Some(expected) {
            shard.remove(&row_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn remove_relation_head_if(
        &self,
        rel_id: RelId,
        row_id: RowId,
        expected: TuplePtr,
    ) -> Result<bool> {
        let shard = self.relation_row_dir_shard(rel_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        if let Some(entries) = shard.get_mut(&rel_id)
            && entries.get(&row_id).copied() == Some(expected)
        {
            entries.remove(&row_id);
            if entries.is_empty() {
                shard.remove(&rel_id);
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn vacuum_row(
        &self,
        rel_id: RelId,
        row_id: RowId,
        ptr: TuplePtr,
        horizon: Csn,
        tx_status: &ConcurrentTxStatus,
        stats: &mut super::VacuumStats,
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

    pub(crate) fn lane_for_row(&self, row_id: RowId) -> usize {
        row_id.0 as usize % self.append_lanes.len()
    }

    fn row_dir_shard(&self, row_id: RowId) -> &RwLock<HashMap<RowId, TuplePtr>> {
        &self.row_dir[row_id.0 as usize % self.row_dir.len()]
    }

    fn relation_row_dir_shard(
        &self,
        rel_id: RelId,
    ) -> &RwLock<HashMap<RelId, HashMap<RowId, TuplePtr>>> {
        &self.relation_row_dir[rel_id.0 as usize % self.relation_row_dir.len()]
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

    fn push_reusable_page(&self, kind: PageKind, page_id: PageId) -> Result<()> {
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
