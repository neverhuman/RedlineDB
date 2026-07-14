use super::PageBackedHeap;
use crate::engine::page_heap::encode_undo_ptr;
use crate::format::{
    Lsn, PAGE_HEADER_LEN, PageGeneration, PageId, PageKind, RelId, RowId, SLOT_LEN, TuplePtr,
    TupleVersion, TxId, UndoPtr,
};
use crate::txn::{UndoKind, UndoRecord};
use crate::wal::{WalPayload, WalRecordKind};
use crate::{Error, Result};

impl PageBackedHeap {
    pub(crate) fn append_update_version(
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
        // Validate the replacement before appending its before-image. Otherwise a rejected
        // oversized UPDATE leaves an unreachable undo cell behind even though no heap tuple or
        // WAL record was written.
        let mut next = TupleVersion::new(row_id, rel_id, tx_id, payload);
        self.ensure_cell_size(next.encoded_size()?)?;
        let wal_payload = if lsn != Lsn::ZERO {
            Some(WalPayload::HeapUpdate {
                tx_id,
                rel_id,
                row_id,
                payload: next.payload.clone(),
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

        next.undo_head = undo_ptr;
        let ptr = self.append_tuple(tx_id, row_id, next, lsn, wal_payload)?;
        self.set_head(row_id, ptr)?;
        self.set_relation_head(rel_id, row_id, ptr)
    }

    pub(crate) fn append_delete_version(
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

    pub(crate) fn append_tuple(
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

    pub(crate) fn append_undo(
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

    pub(crate) fn append_cell(
        &self,
        tx_id: TxId,
        current_page: &mut Option<PageId>,
        kind: PageKind,
        encoded: &[u8],
        lsn: Lsn,
        wal_payload: Option<WalPayload>,
    ) -> Result<(PageId, u16, PageGeneration)> {
        // `PageFull` on an existing page means "try a fresh page". If the encoded cell cannot
        // fit even an empty page, however, retrying can never succeed. The old loop allocated and
        // dirtied one new page per iteration forever; reject the record before allocating any
        // page instead.
        self.ensure_cell_size(encoded.len())?;
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

    fn ensure_cell_size(&self, needed: usize) -> Result<()> {
        let maximum = self
            .buffer
            .page_size()
            .saturating_sub(PAGE_HEADER_LEN + SLOT_LEN);
        if needed > maximum {
            return Err(Error::RecordTooLarge { needed, maximum });
        }
        Ok(())
    }
}
