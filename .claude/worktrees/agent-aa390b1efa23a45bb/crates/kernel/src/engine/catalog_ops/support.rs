use super::*;

impl Engine {
    pub(super) fn catalog_snapshot_for_tx(&self, tx: &Txn) -> Arc<crate::catalog::SchemaSnapshot> {
        match tx.pending_schema_snapshot() {
            Some(snap) => snap,
            None => self.catalog.current(),
        }
    }
}
