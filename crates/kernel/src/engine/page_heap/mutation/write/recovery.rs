use super::PageBackedHeap;
use crate::engine::page_heap::advance_atomic_past;
use crate::format::{Lsn, RelId, RowId, TxId};
use crate::wal::WalPayload;
use crate::{Result, format::TupleVersion};

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
}
