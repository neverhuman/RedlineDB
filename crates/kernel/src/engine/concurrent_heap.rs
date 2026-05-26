use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{RelId, RowId, TuplePtr, TupleVersion, TxId, UndoPtr};
use crate::txn::{Snapshot, TupleVisibility, TxState, UndoKind, UndoRecord};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::thread;

#[derive(Debug)]
pub struct ConcurrentHeap {
    row_dir: Vec<RwLock<HashMap<RowId, TuplePtr>>>,
    lanes: Vec<Mutex<Vec<TupleVersion>>>,
    undo: Mutex<Vec<UndoRecord>>,
    next_row: AtomicU64,
}

impl ConcurrentHeap {
    pub fn new(_rel_id: RelId, lanes: usize) -> Result<Self> {
        let lanes = lanes.max(1).min(u16::MAX as usize);
        let mut row_dir = Vec::with_capacity(lanes);
        let mut lane_vecs = Vec::with_capacity(lanes);
        for _ in 0..lanes {
            row_dir.push(RwLock::new(HashMap::new()));
            lane_vecs.push(Mutex::new(Vec::new()));
        }
        Ok(Self {
            row_dir,
            lanes: lane_vecs,
            undo: Mutex::new(Vec::new()),
            next_row: AtomicU64::new(1),
        })
    }

    pub fn insert(&self, tx_id: TxId, payload: Vec<u8>) -> Result<RowId> {
        let row_id = RowId(self.next_row.fetch_add(1, Ordering::Relaxed));
        self.insert_recovered(tx_id, row_id, payload)?;
        Ok(row_id)
    }

    pub fn reserve_row_id(&self) -> RowId {
        RowId(self.next_row.fetch_add(1, Ordering::Relaxed))
    }

    pub fn insert_with_row_id(&self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        advance_atomic_past(&self.next_row, row_id.0);
        self.insert_recovered(tx_id, row_id, payload)
    }

    pub fn update(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
        let current = self.visible_tuple_for_write(tx_id, snapshot, tx_status, row_id)?;
        self.append_update_version(tx_id, row_id, payload, current)
    }

    pub fn update_recovered(&self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        let current = self.current_tuple(row_id)?;
        self.append_update_version(tx_id, row_id, payload, current)
    }

    fn append_update_version(
        &self,
        tx_id: TxId,
        row_id: RowId,
        payload: Vec<u8>,
        current: TupleVersion,
    ) -> Result<()> {
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(UndoRecord {
            kind: UndoKind::UpdateBeforeImage,
            tx_id,
            row_id,
            prev_undo: current.undo_head,
            before_image: before.encode()?,
        })?;

        let mut next = TupleVersion::new(row_id, RelId::ZERO, tx_id, payload);
        next.undo_head = undo_ptr;
        let ptr = self.append_tuple(row_id, next)?;
        self.set_head(row_id, ptr)
    }

    pub fn delete(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
    ) -> Result<()> {
        let current = self.visible_tuple_for_write(tx_id, snapshot, tx_status, row_id)?;
        self.append_delete_version(tx_id, row_id, current)
    }

    pub fn delete_recovered(&self, tx_id: TxId, row_id: RowId) -> Result<()> {
        let current = self.current_tuple(row_id)?;
        self.append_delete_version(tx_id, row_id, current)
    }

    fn append_delete_version(
        &self,
        tx_id: TxId,
        row_id: RowId,
        current: TupleVersion,
    ) -> Result<()> {
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(UndoRecord {
            kind: UndoKind::DeleteBeforeImage,
            tx_id,
            row_id,
            prev_undo: current.undo_head,
            before_image: before.encode()?,
        })?;

        let mut tombstone = TupleVersion::deleted(row_id, RelId::ZERO, tx_id);
        tombstone.undo_head = undo_ptr;
        let ptr = self.append_tuple(row_id, tombstone)?;
        self.set_head(row_id, ptr)
    }

    pub fn insert_recovered(&self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        advance_atomic_past(&self.next_row, row_id.0);
        let tuple = TupleVersion::new(row_id, RelId::ZERO, tx_id, payload);
        let ptr = self.append_tuple(row_id, tuple)?;
        self.set_head(row_id, ptr)
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
        let current = self.current_tuple(row_id)?;
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

    fn current_tuple(&self, row_id: RowId) -> Result<TupleVersion> {
        let ptr = self
            .head(row_id)?
            .ok_or(Error::CorruptPage("row id missing from row directory"))?;
        self.read_tuple(ptr)
    }

    fn append_tuple(&self, row_id: RowId, tuple: TupleVersion) -> Result<TuplePtr> {
        let lane_idx = self.lane_for_row(row_id);
        let mut lane = self.lanes[lane_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("heap lane poisoned"))?;
        if lane.len() > u16::MAX as usize {
            return Err(Error::CorruptPage("heap lane tuple slot overflow"));
        }
        let slot = lane.len() as u16;
        lane.push(tuple);
        Ok(TuplePtr::new(
            crate::format::PageId((lane_idx + 1) as u64),
            slot,
        ))
    }

    fn read_tuple(&self, ptr: TuplePtr) -> Result<TupleVersion> {
        if ptr.is_null() || ptr.page_id.0 == 0 {
            return Err(Error::CorruptPage("null tuple pointer"));
        }
        let lane_idx = ptr.page_id.0 as usize - 1;
        let lane = self
            .lanes
            .get(lane_idx)
            .ok_or(Error::CorruptPage("heap lane missing"))?
            .lock()
            .map_err(|_| Error::CorruptPage("heap lane poisoned"))?;
        lane.get(ptr.slot as usize)
            .cloned()
            .ok_or(Error::CorruptPage("tuple slot missing"))
    }

    fn append_undo(&self, record: UndoRecord) -> Result<UndoPtr> {
        let mut undo = self
            .undo
            .lock()
            .map_err(|_| Error::CorruptPage("undo store poisoned"))?;
        undo.push(record);
        Ok(UndoPtr(undo.len() as u64))
    }

    fn read_undo(&self, ptr: UndoPtr) -> Result<UndoRecord> {
        if ptr == UndoPtr::ZERO {
            return Err(Error::CorruptPage("null undo pointer"));
        }
        let undo = self
            .undo
            .lock()
            .map_err(|_| Error::CorruptPage("undo store poisoned"))?;
        undo.get(ptr.0 as usize - 1)
            .cloned()
            .ok_or(Error::CorruptPage("undo pointer out of bounds"))
    }

    fn head(&self, row_id: RowId) -> Result<Option<TuplePtr>> {
        let shard = self.row_dir_shard(row_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        Ok(shard.get(&row_id).copied())
    }

    fn set_head(&self, row_id: RowId, ptr: TuplePtr) -> Result<()> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        shard.insert(row_id, ptr);
        Ok(())
    }

    fn lane_for_row(&self, row_id: RowId) -> usize {
        row_id.0 as usize % self.lanes.len()
    }

    fn row_dir_shard(&self, row_id: RowId) -> &RwLock<HashMap<RowId, TuplePtr>> {
        &self.row_dir[row_id.0 as usize % self.row_dir.len()]
    }

    /// WS-C3 (kernel half): partition the heap's lane vector across
    /// `worker_count` threads, gather every row visible to `snapshot`
    /// concurrently, and return the materialised payloads. The lane
    /// vectors are themselves protected by per-lane `Mutex`es so each
    /// worker holds a disjoint set of locks and contention stays
    /// confined to lane boundaries.
    ///
    /// Result ordering is **not** stable across runs — callers that
    /// depend on `(row_id, payload)` order must sort the result.
    /// Callers that feed an unordered hash aggregator or spill sort
    /// can consume the iterator directly.
    ///
    /// The SQL-side gate that decides whether to dispatch through
    /// this API is deferred (see WS-C3 round-2): the production
    /// table path goes through `PageBackedHeap`, not `ConcurrentHeap`,
    /// so this kernel API is wired but not yet consumed by the
    /// executor. Adding the API + tests now keeps the kernel surface
    /// ready for the round-2 SQL change.
    pub fn parallel_scan(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        worker_count: usize,
    ) -> Result<Vec<(RowId, Vec<u8>)>> {
        let workers = worker_count.max(1).min(self.lanes.len().max(1));
        if workers <= 1 {
            return self.serial_scan(tx_status, snapshot, owner);
        }

        let lane_count = self.lanes.len();
        let mut buckets: Vec<Vec<(RowId, Vec<u8>)>> = Vec::with_capacity(workers);
        thread::scope(|scope| -> Result<()> {
            let mut handles = Vec::with_capacity(workers);
            for worker_idx in 0..workers {
                let heap = &*self;
                let handle = scope.spawn(move || -> Result<Vec<(RowId, Vec<u8>)>> {
                    let mut out = Vec::new();
                    let mut lane = worker_idx;
                    while lane < lane_count {
                        heap.collect_lane_visible(tx_status, snapshot, owner, lane, &mut out)?;
                        lane += workers;
                    }
                    Ok(out)
                });
                handles.push(handle);
            }
            for handle in handles {
                let part = handle
                    .join()
                    .map_err(|_| Error::CorruptPage("parallel_scan worker panicked"))??;
                buckets.push(part);
            }
            Ok(())
        })?;
        let total: usize = buckets.iter().map(|b| b.len()).sum();
        let mut merged = Vec::with_capacity(total);
        for bucket in buckets {
            merged.extend(bucket);
        }
        Ok(merged)
    }

    /// Serial reference scan used by `parallel_scan` when only one
    /// worker is requested. Mirrors `parallel_scan`'s visibility logic
    /// so tests can assert set-equality between the two paths.
    pub fn serial_scan(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> Result<Vec<(RowId, Vec<u8>)>> {
        let mut out = Vec::new();
        for lane in 0..self.lanes.len() {
            self.collect_lane_visible(tx_status, snapshot, owner, lane, &mut out)?;
        }
        Ok(out)
    }

    fn collect_lane_visible(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        lane_idx: usize,
        out: &mut Vec<(RowId, Vec<u8>)>,
    ) -> Result<()> {
        // Snapshot the live (row_id, head_ptr) pairs from the matching
        // row-dir shards under the read lock, then drop the lock before
        // walking undo chains. This keeps the per-row work outside the
        // shard write path so writers are not blocked by scanners.
        let mut heads: Vec<(RowId, TuplePtr)> = Vec::new();
        for (shard_idx, shard) in self.row_dir.iter().enumerate() {
            if shard_idx % self.lanes.len() != lane_idx {
                continue;
            }
            let guard = shard
                .read()
                .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
            heads.extend(guard.iter().map(|(r, p)| (*r, *p)));
        }
        for (row_id, _ptr) in heads {
            if let Some(payload) = self.visible_payload(tx_status, snapshot, owner, row_id)? {
                out.push((row_id, payload));
            }
        }
        Ok(())
    }

    fn visible_payload(
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
}

trait ConcurrentVisibility {
    fn visibility_concurrent(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> TupleVisibility;
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
