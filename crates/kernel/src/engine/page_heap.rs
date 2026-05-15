use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{
    Csn, Lsn, PageGeneration, PageId, PageKind, PageState, RelId, RowId, TUPLE_FLAG_DELETED,
    TuplePtr, TupleVersion, TxId, UndoPtr,
};
use crate::storage::{BufferPool, BufferPoolStats, FlushStats};
use crate::txn::{Snapshot, TupleVisibility, TxState, UndoKind, UndoRecord};
use crate::wal::{WalCoordinator, WalPayload, WalRecordKind};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug)]
pub struct PageBackedHeap {
    rel_id: RelId,
    row_dir: Vec<RwLock<HashMap<RowId, TuplePtr>>>,
    relation_row_dir: Vec<RwLock<HashMap<RelId, HashMap<RowId, TuplePtr>>>>,
    append_lanes: Vec<Mutex<AppendLane>>,
    reusable_heap_pages: Mutex<Vec<PageId>>,
    reusable_undo_pages: Mutex<Vec<PageId>>,
    buffer: Arc<BufferPool>,
    wal: Option<Arc<WalCoordinator>>,
    next_row: AtomicU64,
}

#[derive(Debug, Default)]
struct AppendLane {
    heap_page: Option<PageId>,
    undo_page: Option<PageId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumStats {
    pub rows_scanned: usize,
    pub chains_pruned: usize,
    pub undo_links_removed: usize,
    pub dead_rows_removed: usize,
    pub oldest_active_snapshot_csn: Csn,
}

#[derive(Clone, Copy, Debug)]
pub struct RelationWriteTarget {
    pub rel_id: RelId,
    pub row_id: RowId,
}

impl PageBackedHeap {
    pub fn new(rel_id: RelId, lanes: usize, buffer: Arc<BufferPool>) -> Result<Self> {
        Self::new_with_wal(rel_id, lanes, buffer, None)
    }

    pub fn new_with_wal(
        rel_id: RelId,
        lanes: usize,
        buffer: Arc<BufferPool>,
        wal: Option<Arc<WalCoordinator>>,
    ) -> Result<Self> {
        let lanes = lanes.max(1).min(u16::MAX as usize);
        let mut row_dir = Vec::with_capacity(lanes);
        let mut relation_row_dir = Vec::with_capacity(lanes);
        let mut append_lanes = Vec::with_capacity(lanes);
        for _ in 0..lanes {
            row_dir.push(RwLock::new(HashMap::new()));
            relation_row_dir.push(RwLock::new(HashMap::new()));
            append_lanes.push(Mutex::new(AppendLane::default()));
        }
        Ok(Self {
            rel_id,
            row_dir,
            relation_row_dir,
            append_lanes,
            reusable_heap_pages: Mutex::new(Vec::new()),
            reusable_undo_pages: Mutex::new(Vec::new()),
            buffer,
            wal,
            next_row: AtomicU64::new(1),
        })
    }

    pub fn reserve_row_id(&self) -> RowId {
        RowId(self.next_row.fetch_add(1, Ordering::Relaxed))
    }

    pub fn insert_with_row_id(
        &self,
        tx_id: TxId,
        row_id: RowId,
        payload: Vec<u8>,
        lsn: Lsn,
    ) -> Result<()> {
        advance_atomic_past(&self.next_row, row_id.0);
        self.insert_recovered_at(tx_id, self.rel_id, row_id, payload, lsn)
    }

    pub fn insert_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
        lsn: Lsn,
    ) -> Result<()> {
        advance_atomic_past(&self.next_row, row_id.0);
        self.insert_recovered_at(tx_id, rel_id, row_id, payload, lsn)
    }

    pub fn insert_recovered(&self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        // LSN sentinel: legit init. Recovery is replaying an already-logged
        // mutation; no fresh WAL record is appended, so the page-LSN argument
        // routes the heap append through the no-WAL branch of `append_cell`.
        self.insert_recovered_at(tx_id, self.rel_id, row_id, payload, Lsn::ZERO)
    }

    pub fn insert_recovered_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
        // LSN sentinel: legit init. Recovery replay path; see `insert_recovered`.
        self.insert_recovered_at(tx_id, rel_id, row_id, payload, Lsn::ZERO)
    }

    pub(crate) fn insert_recovered_at(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
        lsn: Lsn,
    ) -> Result<()> {
        advance_atomic_past(&self.next_row, row_id.0);
        let rel_id = if rel_id == RelId::ZERO {
            self.rel_id
        } else {
            rel_id
        };
        let wal_payload = if lsn != Lsn::ZERO {
            Some(WalPayload::HeapInsert {
                tx_id,
                rel_id,
                row_id,
                payload: payload.clone(),
            })
        } else {
            None
        };
        let tuple = TupleVersion::new(row_id, rel_id, tx_id, payload);
        let ptr = self.append_tuple(tx_id, row_id, tuple, lsn, wal_payload)?;
        self.set_head(row_id, ptr)?;
        self.set_relation_head(rel_id, row_id, ptr)
    }

    pub fn update(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
        payload: Vec<u8>,
        lsn: Lsn,
    ) -> Result<()> {
        let current = self.visible_tuple_for_write(tx_id, snapshot, tx_status, row_id)?;
        self.append_update_version(tx_id, self.rel_id, row_id, payload, current, lsn)
    }

    pub fn update_for_relation(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        target: RelationWriteTarget,
        payload: Vec<u8>,
        lsn: Lsn,
    ) -> Result<()> {
        let current =
            self.visible_tuple_for_write_in_relation(tx_id, snapshot, tx_status, target)?;
        self.append_update_version(tx_id, target.rel_id, target.row_id, payload, current, lsn)
    }

    pub fn update_recovered(&self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        let current = self.current_tuple_recovered(self.rel_id, row_id)?;
        // LSN sentinel: legit init. Recovery replay; no fresh WAL record.
        self.append_update_version(tx_id, self.rel_id, row_id, payload, current, Lsn::ZERO)
    }

    pub fn update_recovered_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
        let current = self.current_tuple_recovered(rel_id, row_id)?;
        // LSN sentinel: legit init. Recovery replay; no fresh WAL record.
        self.append_update_version(tx_id, rel_id, row_id, payload, current, Lsn::ZERO)
    }

    pub fn delete(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
        lsn: Lsn,
    ) -> Result<()> {
        let current = self.visible_tuple_for_write(tx_id, snapshot, tx_status, row_id)?;
        self.append_delete_version(tx_id, self.rel_id, row_id, current, lsn)
    }

    pub fn delete_for_relation(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        rel_id: RelId,
        row_id: RowId,
        lsn: Lsn,
    ) -> Result<()> {
        let current = self.visible_tuple_for_write_in_relation(
            tx_id,
            snapshot,
            tx_status,
            RelationWriteTarget { rel_id, row_id },
        )?;
        self.append_delete_version(tx_id, rel_id, row_id, current, lsn)
    }

    pub fn delete_recovered(&self, tx_id: TxId, row_id: RowId) -> Result<()> {
        let current = self.current_tuple_recovered(self.rel_id, row_id)?;
        // LSN sentinel: legit init. Recovery replay; no fresh WAL record.
        self.append_delete_version(tx_id, self.rel_id, row_id, current, Lsn::ZERO)
    }

    pub fn delete_recovered_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<()> {
        let current = self.current_tuple_recovered(rel_id, row_id)?;
        // LSN sentinel: legit init. Recovery replay; no fresh WAL record.
        self.append_delete_version(tx_id, rel_id, row_id, current, Lsn::ZERO)
    }

    pub fn get(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        row_id: RowId,
    ) -> Result<Option<Vec<u8>>> {
        let Some(ptr) = self.head(row_id)? else {
            return Ok(None);
        };
        let current = self.read_tuple(ptr)?;
        match current.visibility_concurrent(tx_status, snapshot, owner) {
            TupleVisibility::Visible => Ok(Some(current.payload)),
            TupleVisibility::Deleted => Ok(None),
            TupleVisibility::Invisible => {
                self.get_from_undo(tx_status, snapshot, owner, current.undo_head)
            }
        }
    }

    pub fn get_for_relation(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<Option<Vec<u8>>> {
        let Some(ptr) = self.head_for_relation(rel_id, row_id)? else {
            return Ok(None);
        };
        let current = self.read_tuple(ptr)?;
        if current.rel_id != rel_id {
            return Ok(None);
        }
        match current.visibility_concurrent(tx_status, snapshot, owner) {
            TupleVisibility::Visible => Ok(Some(current.payload)),
            TupleVisibility::Deleted => Ok(None),
            TupleVisibility::Invisible => {
                self.get_from_undo(tx_status, snapshot, owner, current.undo_head)
            }
        }
    }

    pub fn flush_all(&self, durable_lsn: Lsn) -> Result<()> {
        self.buffer.flush_all(durable_lsn)
    }

    pub fn flush_dirty_batches(&self, durable_lsn: Lsn, batch_pages: usize) -> Result<FlushStats> {
        self.buffer.flush_dirty_batches(durable_lsn, batch_pages)
    }

    pub fn resident_pages(&self) -> usize {
        self.buffer.resident_pages()
    }

    pub fn buffer_stats(&self) -> BufferPoolStats {
        self.buffer.stats()
    }

    pub fn redo_page_image(&self, mut page: crate::format::Page, lsn: Lsn) -> Result<()> {
        page.set_page_lsn(lsn)?;
        self.buffer.write_page_direct(&page)
    }

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

    pub fn page_count(&self) -> Result<u64> {
        self.buffer.page_count()
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
        let pages: std::collections::HashSet<PageId> = pages.iter().copied().collect();
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

    fn append_update_version(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
        current: TupleVersion,
        lsn: Lsn,
    ) -> Result<()> {
        let rel_id = if rel_id == RelId::ZERO {
            self.rel_id
        } else {
            rel_id
        };
        let wal_payload = if lsn != Lsn::ZERO {
            Some(WalPayload::HeapUpdate {
                tx_id,
                rel_id,
                row_id,
                payload: payload.clone(),
            })
        } else {
            None
        };
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(
            tx_id,
            row_id,
            UndoRecord {
                kind: UndoKind::UpdateBeforeImage,
                tx_id,
                row_id,
                prev_undo: current.undo_head,
                before_image: before.encode()?,
            },
            lsn,
        )?;

        let mut next = TupleVersion::new(row_id, rel_id, tx_id, payload);
        next.undo_head = undo_ptr;
        let ptr = self.append_tuple(tx_id, row_id, next, lsn, wal_payload)?;
        self.set_head(row_id, ptr)?;
        self.set_relation_head(rel_id, row_id, ptr)
    }

    fn append_delete_version(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        current: TupleVersion,
        lsn: Lsn,
    ) -> Result<()> {
        let rel_id = if rel_id == RelId::ZERO {
            self.rel_id
        } else {
            rel_id
        };
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(
            tx_id,
            row_id,
            UndoRecord {
                kind: UndoKind::DeleteBeforeImage,
                tx_id,
                row_id,
                prev_undo: current.undo_head,
                before_image: before.encode()?,
            },
            lsn,
        )?;

        let mut tombstone = TupleVersion::deleted(row_id, rel_id, tx_id);
        tombstone.undo_head = undo_ptr;
        let ptr = self.append_tuple(
            tx_id,
            row_id,
            tombstone,
            lsn,
            Some(WalPayload::HeapDelete {
                tx_id,
                rel_id,
                row_id,
            }),
        )?;
        self.set_head(row_id, ptr)?;
        self.set_relation_head(rel_id, row_id, ptr)
    }

    fn append_tuple(
        &self,
        tx_id: TxId,
        row_id: RowId,
        tuple: TupleVersion,
        lsn: Lsn,
        wal_payload: Option<WalPayload>,
    ) -> Result<TuplePtr> {
        let encoded = tuple.encode()?;
        let lane_idx = self.lane_for_row(row_id);
        let mut lane = self.append_lanes[lane_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("heap append lane poisoned"))?;
        let (page_id, slot, generation) = self.append_cell(
            tx_id,
            &mut lane.heap_page,
            PageKind::Heap,
            &encoded,
            lsn,
            wal_payload,
        )?;
        Ok(TuplePtr::new_with_generation(page_id, slot, generation))
    }

    fn append_undo(
        &self,
        tx_id: TxId,
        row_id: RowId,
        undo: UndoRecord,
        lsn: Lsn,
    ) -> Result<UndoPtr> {
        let encoded = undo.encode()?;
        let lane_idx = self.lane_for_row(row_id);
        let mut lane = self.append_lanes[lane_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("heap append lane poisoned"))?;
        let (page_id, slot, _generation) = self.append_cell(
            tx_id,
            &mut lane.undo_page,
            PageKind::Undo,
            &encoded,
            lsn,
            None,
        )?;
        Ok(encode_undo_ptr(page_id, slot))
    }

    fn append_cell(
        &self,
        tx_id: TxId,
        current_page: &mut Option<PageId>,
        kind: PageKind,
        encoded: &[u8],
        lsn: Lsn,
        wal_payload: Option<WalPayload>,
    ) -> Result<(PageId, u16, PageGeneration)> {
        let mut needs_reinit = false;
        loop {
            let guard = match current_page {
                Some(page_id) => self.buffer.pin(*page_id)?,
                None => {
                    if let Some(page_id) = self.take_reusable_page(kind)? {
                        *current_page = Some(page_id);
                        needs_reinit = true;
                        self.buffer.pin(page_id)?
                    } else {
                        let guard = self.buffer.allocate(kind, self.rel_id)?;
                        *current_page = Some(guard.page_id());
                        needs_reinit = true;
                        guard
                    }
                }
            };

            let mut frame = guard.mutable_frame()?;
            let outcome = {
                let page = frame
                    .page
                    .as_mut()
                    .ok_or(Error::CorruptPage("resident frame missing page"))?;
                if needs_reinit {
                    let next_generation = page.header()?.generation.next();
                    page.reinitialize(kind, guard.page_id(), self.rel_id, next_generation)?;
                }
                let page_generation = page.header()?.generation;
                let mut staged_page = page.clone();
                match staged_page.insert_cell(encoded) {
                    Ok(slot) => {
                        if let Some(wal) = &self.wal {
                            // LSN sentinel: legit logic. Caller passes Lsn(1)
                            // when this is a real, durable mutation;
                            // Lsn::ZERO signals recovery replay where we
                            // must NOT append a new WAL record.
                            if lsn != Lsn::ZERO {
                                if let Some(payload) = wal_payload.as_ref() {
                                    // Lane E failpoint: armed at the moment a
                                    // heap mutation would emit its logical WAL
                                    // delta. Crashing here yields a page that
                                    // took the mutation in memory but never
                                    // reached the WAL.
                                    crate::fail_point!("heap::mutation");
                                    let append = wal.append(
                                        WalRecordKind::PageDelta,
                                        tx_id,
                                        payload.encode()?,
                                    )?;
                                    staged_page.set_page_lsn(append.end_lsn)?;
                                } else {
                                    staged_page.set_page_lsn(lsn)?;
                                }
                                *page = staged_page;
                                Ok(Some((slot, page_generation)))
                            } else {
                                staged_page.set_page_lsn(lsn)?;
                                *page = staged_page;
                                Ok(Some((slot, page_generation)))
                            }
                        } else {
                            staged_page.set_page_lsn(lsn)?;
                            *page = staged_page;
                            Ok(Some((slot, page_generation)))
                        }
                    }
                    Err(Error::PageFull) => Ok(None),
                    Err(err) => Err(err),
                }
            }?;

            match outcome {
                Some((slot, generation)) => {
                    frame.dirty = true;
                    return Ok((guard.page_id(), slot, generation));
                }
                None => {
                    drop(frame);
                    if let Some(page_id) = self.take_reusable_page(kind)? {
                        *current_page = Some(page_id);
                        needs_reinit = true;
                    } else {
                        let guard = self.buffer.allocate(kind, self.rel_id)?;
                        *current_page = Some(guard.page_id());
                        needs_reinit = true;
                    }
                }
            }
        }
    }

    fn get_from_undo(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        undo_ptr: UndoPtr,
    ) -> Result<Option<Vec<u8>>> {
        let mut cursor = undo_ptr;
        while cursor != UndoPtr::ZERO {
            let undo = self.read_undo(cursor)?;
            let tuple = TupleVersion::decode(&undo.before_image)?;
            match tuple.visibility_concurrent(tx_status, snapshot, owner) {
                TupleVisibility::Visible => return Ok(Some(tuple.payload)),
                TupleVisibility::Deleted => return Ok(None),
                TupleVisibility::Invisible => cursor = undo.prev_undo,
            }
        }
        Ok(None)
    }

    fn visible_tuple_for_write(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
    ) -> Result<TupleVersion> {
        let current = self.current_tuple_for_relation(self.rel_id, row_id)?;
        self.visible_current_tuple_for_write(tx_id, snapshot, tx_status, current)
    }

    fn visible_tuple_for_write_in_relation(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        target: RelationWriteTarget,
    ) -> Result<TupleVersion> {
        let current = self.current_tuple_for_relation(target.rel_id, target.row_id)?;
        self.visible_current_tuple_for_write(tx_id, snapshot, tx_status, current)
    }

    fn visible_current_tuple_for_write(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        current: TupleVersion,
    ) -> Result<TupleVersion> {
        match current.visibility_concurrent(tx_status, snapshot, Some(tx_id)) {
            TupleVisibility::Visible => return Ok(current),
            TupleVisibility::Deleted => return Err(Error::NotVisible),
            TupleVisibility::Invisible => {}
        }

        match tx_status.state(current.begin_tx) {
            TxState::Aborted => {
                let mut cursor = current.undo_head;
                while cursor != UndoPtr::ZERO {
                    let undo = self.read_undo(cursor)?;
                    let tuple = TupleVersion::decode(&undo.before_image)?;
                    match tuple.visibility_concurrent(tx_status, snapshot, Some(tx_id)) {
                        TupleVisibility::Visible => return Ok(tuple),
                        TupleVisibility::Deleted => return Err(Error::NotVisible),
                        TupleVisibility::Invisible => cursor = undo.prev_undo,
                    }
                }
                Err(Error::NotVisible)
            }
            TxState::Committed(_) | TxState::InProgress => Err(Error::SerializationFailure),
        }
    }

    #[allow(dead_code)]
    fn current_tuple(&self, row_id: RowId) -> Result<TupleVersion> {
        self.current_tuple_for_relation(self.rel_id, row_id)
    }

    fn current_tuple_for_relation(&self, rel_id: RelId, row_id: RowId) -> Result<TupleVersion> {
        let rel_id = if rel_id == RelId::ZERO {
            self.rel_id
        } else {
            rel_id
        };
        let ptr = self
            .head_for_relation(rel_id, row_id)?
            .ok_or(Error::CorruptPage(
                "row id missing from relation row directory",
            ))?;
        self.read_tuple(ptr)
    }

    fn current_tuple_recovered(&self, rel_id: RelId, row_id: RowId) -> Result<TupleVersion> {
        if let Some(ptr) = self.head_for_relation(rel_id, row_id)? {
            return self.read_tuple(ptr);
        }
        let rel_id = if rel_id == RelId::ZERO {
            self.rel_id
        } else {
            rel_id
        };
        let page_count = self.page_count()?;
        for page_no in 1..=page_count {
            let page_id = PageId(page_no);
            let guard = match self.buffer.pin(page_id) {
                Ok(guard) => guard,
                Err(Error::InvalidMagic { actual: 0, .. }) => continue,
                Err(err) => return Err(err),
            };
            let current = guard.with_page(|page| {
                let header = page.header()?;
                if header.kind != PageKind::Heap || header.rel_id != self.rel_id {
                    return Ok(None);
                }
                for slot in 0..page.slot_count()? {
                    let tuple = TupleVersion::decode(page.cell(slot)?)?;
                    let tuple_rel_id = if tuple.rel_id == RelId::ZERO {
                        self.rel_id
                    } else {
                        tuple.rel_id
                    };
                    if tuple.row_id == row_id && tuple_rel_id == rel_id {
                        return Ok(Some(tuple));
                    }
                }
                Ok(None)
            })?;
            if let Some(current) = current {
                return Ok(current);
            }
        }
        Err(Error::CorruptPage(
            "row id missing from relation row directory",
        ))
    }

    fn read_tuple(&self, ptr: TuplePtr) -> Result<TupleVersion> {
        if ptr.is_null() {
            return Err(Error::CorruptPage("null tuple pointer"));
        }
        let guard = self.buffer.pin(ptr.page_id)?;
        guard.with_page(|page| {
            let header = page.header()?;
            if header.generation != ptr.generation {
                return Err(Error::CorruptPage("tuple pointer generation mismatch"));
            }
            TupleVersion::decode(page.cell(ptr.slot)?)
        })
    }

    fn overwrite_tuple(&self, ptr: TuplePtr, tuple: &TupleVersion) -> Result<()> {
        if ptr.is_null() {
            return Err(Error::CorruptPage("null tuple pointer"));
        }
        let encoded = tuple.encode()?;
        let guard = self.buffer.pin(ptr.page_id)?;
        guard.with_page_mut(|page| page.overwrite_cell(ptr.slot, &encoded))?;
        // LSN sentinel: legit init. Vacuum/horizon prunes already-committed
        // dead undo links in place; no fresh WAL record is appended for this
        // checkpoint-side maintenance, and this preserves the legacy
        // "no-WAL-dependency" marker for the page-LSN field.
        guard.mark_dirty(Lsn::ZERO)
    }

    fn read_undo(&self, ptr: UndoPtr) -> Result<UndoRecord> {
        if ptr == UndoPtr::ZERO {
            return Err(Error::CorruptPage("null undo pointer"));
        }
        let (page_id, slot) = decode_undo_ptr(ptr);
        let guard = self.buffer.pin(page_id)?;
        guard.with_page(|page| UndoRecord::decode(page.cell(slot)?))
    }

    fn head(&self, row_id: RowId) -> Result<Option<TuplePtr>> {
        let shard = self.row_dir_shard(row_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        Ok(shard.get(&row_id).copied())
    }

    fn head_for_relation(&self, rel_id: RelId, row_id: RowId) -> Result<Option<TuplePtr>> {
        let shard = self.relation_row_dir_shard(rel_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        Ok(shard
            .get(&rel_id)
            .and_then(|entries| entries.get(&row_id).copied()))
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
        snapshot: &crate::txn::Snapshot,
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

    fn set_head(&self, row_id: RowId, ptr: TuplePtr) -> Result<()> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        shard.insert(row_id, ptr);
        Ok(())
    }

    fn set_relation_head(&self, rel_id: RelId, row_id: RowId, ptr: TuplePtr) -> Result<()> {
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

    fn lane_for_row(&self, row_id: RowId) -> usize {
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

    fn take_reusable_page(&self, kind: PageKind) -> Result<Option<PageId>> {
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

fn encode_undo_ptr(page_id: PageId, slot: u16) -> UndoPtr {
    UndoPtr((page_id.0 << 16) | slot as u64)
}

fn decode_undo_ptr(ptr: UndoPtr) -> (PageId, u16) {
    (PageId(ptr.0 >> 16), (ptr.0 & 0xffff) as u16)
}

fn advance_atomic_past(value: &AtomicU64, seen: u64) {
    let target = seen.saturating_add(1);
    let mut current = value.load(Ordering::SeqCst);
    while current < target {
        match value.compare_exchange(current, target, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

trait ConcurrentVisibility {
    fn visibility_concurrent(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> TupleVisibility;
}

impl ConcurrentVisibility for TupleVersion {
    fn visibility_concurrent(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> TupleVisibility {
        if !tx_status.is_tx_visible(self.begin_tx, snapshot, owner) {
            return TupleVisibility::Invisible;
        }
        if self.end_tx != TxId::ZERO && tx_status.is_tx_visible(self.end_tx, snapshot, owner) {
            return TupleVisibility::Invisible;
        }
        if self.flags & crate::format::TUPLE_FLAG_DELETED != 0 {
            TupleVisibility::Deleted
        } else {
            TupleVisibility::Visible
        }
    }
}
