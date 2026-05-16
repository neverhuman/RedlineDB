use super::{PageBackedHeap, RelationWriteTarget};
use crate::engine::page_heap::{advance_atomic_past, encode_undo_ptr};
use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{
    Lsn, PageGeneration, PageId, PageKind, RelId, RowId, TuplePtr, TupleVersion, TxId, UndoPtr,
};
use crate::txn::{Snapshot, UndoKind, UndoRecord};
use crate::wal::{WalPayload, WalRecordKind};
use crate::{Error, Result};

impl PageBackedHeap {
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
        self.insert_recovered_at(tx_id, self.rel_id, row_id, payload, Lsn::ZERO)
    }

    pub fn insert_recovered_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
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
        self.append_delete_version(tx_id, self.rel_id, row_id, current, Lsn::ZERO)
    }

    pub fn delete_recovered_for_relation(
        &self,
        tx_id: TxId,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<()> {
        let current = self.current_tuple_recovered(rel_id, row_id)?;
        self.append_delete_version(tx_id, rel_id, row_id, current, Lsn::ZERO)
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
                            if lsn != Lsn::ZERO {
                                if let Some(payload) = wal_payload.as_ref() {
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
}
