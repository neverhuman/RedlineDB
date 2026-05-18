use super::{PageBackedHeap, RelationWriteTarget};
use crate::Result;
use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{Lsn, RelId, RowId, TxId};
use crate::txn::Snapshot;

impl PageBackedHeap {
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
}
