use super::*;

impl Engine {
    pub fn get(&self, tx: &mut Txn, row_id: RowId) -> Result<Option<Vec<u8>>> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        let snapshot = tx.snapshot().clone();
        self.heap.get(&self.txs, &snapshot, Some(tx.id()), row_id)
    }

    pub fn get_for_relation(
        &self,
        tx: &mut Txn,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<Option<Vec<u8>>> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        let snapshot = tx.snapshot().clone();
        self.heap
            .get_for_relation(&self.txs, &snapshot, Some(tx.id()), rel_id, row_id)
    }

    pub fn insert(&self, tx: &mut Txn, payload: Vec<u8>) -> Result<RowId> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        let row_id = self.heap.reserve_row_id();
        // LSN sentinel: mutation. The heap append_cell logs a PageImage with
        // the real WAL end-LSN; this argument only flags the page as dirty.
        self.heap
            .insert_with_row_id(tx.id(), row_id, payload, Lsn(1))?;
        Ok(row_id)
    }

    pub fn insert_for_relation(
        &self,
        tx: &mut Txn,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.heap
            .insert_for_relation(tx.id(), rel_id, row_id, payload, Lsn(1))
    }

    pub fn reserve_row_id(&self) -> RowId {
        self.heap.reserve_row_id()
    }

    pub fn insert_with_row_id(&self, tx: &mut Txn, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.heap
            .insert_with_row_id(tx.id(), row_id, payload, Lsn(1))
    }

    pub fn update(&self, tx: &mut Txn, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.lock_row(tx, row_id)?;
        self.refresh_read_committed(tx);
        self.heap
            .update(tx.id(), tx.snapshot(), &self.txs, row_id, payload, Lsn(1))
    }

    pub fn update_for_relation(
        &self,
        tx: &mut Txn,
        rel_id: RelId,
        row_id: RowId,
        payload: Vec<u8>,
    ) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.lock_row_in_rel(tx, rel_id, row_id)?;
        self.refresh_read_committed(tx);
        self.heap.update_for_relation(
            tx.id(),
            tx.snapshot(),
            &self.txs,
            RelationWriteTarget { rel_id, row_id },
            payload,
            Lsn(1),
        )
    }

    pub fn lock_row_for_relation(&self, tx: &mut Txn, rel_id: RelId, row_id: RowId) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.lock_row_in_rel(tx, rel_id, row_id)?;
        self.refresh_read_committed(tx);
        Ok(())
    }

    pub fn delete(&self, tx: &mut Txn, row_id: RowId) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.lock_row(tx, row_id)?;
        self.refresh_read_committed(tx);
        self.heap
            .delete(tx.id(), tx.snapshot(), &self.txs, row_id, Lsn(1))
    }

    pub fn delete_for_relation(&self, tx: &mut Txn, rel_id: RelId, row_id: RowId) -> Result<()> {
        tx.ensure_open()?;
        self.refresh_read_committed(tx);
        self.lock_row_in_rel(tx, rel_id, row_id)?;
        self.refresh_read_committed(tx);
        self.heap
            .delete_for_relation(tx.id(), tx.snapshot(), &self.txs, rel_id, row_id, Lsn(1))
    }
}
