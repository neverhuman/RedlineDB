use super::{PageBackedHeap, RelationWriteTarget};
use crate::engine::page_heap::{
    decode_undo_ptr, resolve_visible_payload_for_read, resolve_visible_tuple_for_write,
};
use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{Lsn, PageId, PageKind, RelId, RowId, TuplePtr, TupleVersion, TxId, UndoPtr};
use crate::txn::{Snapshot, UndoRecord};
use crate::{Error, Result};

impl PageBackedHeap {
    pub fn get(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        row_id: RowId,
    ) -> Result<Option<Vec<u8>>> {
        self.get_for_relation(tx_status, snapshot, owner, self.rel_id, row_id)
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
        self.visible_payload_from_current(current, tx_status, snapshot, owner)
    }

    pub(super) fn visible_tuple_for_write(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        row_id: RowId,
    ) -> Result<TupleVersion> {
        self.visible_tuple_for_write_in_relation(
            tx_id,
            snapshot,
            tx_status,
            RelationWriteTarget {
                rel_id: self.rel_id,
                row_id,
            },
        )
    }

    pub(super) fn visible_tuple_for_write_in_relation(
        &self,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
        target: RelationWriteTarget,
    ) -> Result<TupleVersion> {
        let current = self.current_tuple_for_relation(target.rel_id, target.row_id)?;
        self.visible_tuple_for_write_from_current(current, tx_id, snapshot, tx_status)
    }

    #[allow(dead_code)]
    pub(super) fn current_tuple(&self, row_id: RowId) -> Result<TupleVersion> {
        self.current_tuple_for_relation(self.rel_id, row_id)
    }

    pub(crate) fn current_tuple_for_relation(
        &self,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<TupleVersion> {
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

    pub(crate) fn current_tuple_recovered(
        &self,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<TupleVersion> {
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

    fn visible_payload_from_current(
        &self,
        current: TupleVersion,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> Result<Option<Vec<u8>>> {
        resolve_visible_payload_for_read(current, tx_status, snapshot, owner, |undo_ptr| {
            self.read_undo(undo_ptr)
        })
    }

    fn visible_tuple_for_write_from_current(
        &self,
        current: TupleVersion,
        tx_id: TxId,
        snapshot: &Snapshot,
        tx_status: &ConcurrentTxStatus,
    ) -> Result<TupleVersion> {
        resolve_visible_tuple_for_write(current, tx_id, snapshot, tx_status, |undo_ptr| {
            self.read_undo(undo_ptr)
        })
    }

    pub(crate) fn read_tuple(&self, ptr: TuplePtr) -> Result<TupleVersion> {
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

    pub(crate) fn overwrite_tuple(&self, ptr: TuplePtr, tuple: &TupleVersion) -> Result<()> {
        if ptr.is_null() {
            return Err(Error::CorruptPage("null tuple pointer"));
        }
        let encoded = tuple.encode()?;
        let guard = self.buffer.pin(ptr.page_id)?;
        guard.with_page_mut(|page| page.overwrite_cell(ptr.slot, &encoded))?;
        guard.mark_dirty(Lsn::ZERO)
    }

    pub(crate) fn read_undo(&self, ptr: UndoPtr) -> Result<UndoRecord> {
        if ptr == UndoPtr::ZERO {
            return Err(Error::CorruptPage("null undo pointer"));
        }
        let (page_id, slot) = decode_undo_ptr(ptr);
        let guard = self.buffer.pin(page_id)?;
        guard.with_page(|page| UndoRecord::decode(page.cell(slot)?))
    }
}
