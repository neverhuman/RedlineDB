use crate::catalog::IndexId as CatalogIndexId;
use crate::catalog::SchemaSnapshot;
use crate::engine::lock::RowKey;
use crate::format::{Csn, TxId};
use crate::index::BtreeIndex;
use crate::txn::{Isolation, Snapshot};
use crate::{Error, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

#[path = "tx/status.rs"]
mod status;
use status::TxStatusInner;
pub use status::{ConcurrentTxStatus, TxStatusStats};

#[derive(Debug)]
pub struct Txn {
    id: TxId,
    isolation: Isolation,
    snapshot: Snapshot,
    pending_schema_snapshot: Option<Arc<SchemaSnapshot>>,
    pending_index_handles: Vec<PendingIndexHandle>,
    row_locks: Vec<RowKey>,
    open: bool,
    lifecycle: Option<Arc<TxnLifecycle>>,
}

#[derive(Debug, Clone)]
pub(crate) enum PendingIndexHandle {
    Install(CatalogIndexId, Arc<BtreeIndex>),
    Remove(CatalogIndexId),
}

impl Txn {
    pub(crate) fn new(
        id: TxId,
        isolation: Isolation,
        snapshot: Snapshot,
        lifecycle: Arc<TxnLifecycle>,
    ) -> Self {
        Self {
            id,
            isolation,
            snapshot,
            pending_schema_snapshot: None,
            pending_index_handles: Vec::new(),
            row_locks: Vec::new(),
            open: true,
            lifecycle: Some(lifecycle),
        }
    }

    pub fn id(&self) -> TxId {
        self.id
    }

    pub fn isolation(&self) -> Isolation {
        self.isolation
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub(crate) fn replace_snapshot(&mut self, snapshot: Snapshot) {
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.update_snapshot(snapshot.visible_csn);
        }
        self.snapshot = snapshot;
    }

    pub(crate) fn set_pending_schema_snapshot(&mut self, snapshot: Arc<SchemaSnapshot>) {
        self.pending_schema_snapshot = Some(snapshot);
    }

    pub(crate) fn pending_schema_snapshot(&self) -> Option<Arc<SchemaSnapshot>> {
        self.pending_schema_snapshot.as_ref().map(Arc::clone)
    }

    pub(crate) fn push_pending_index_handle(&mut self, action: PendingIndexHandle) {
        self.pending_index_handles.push(action);
    }

    pub(crate) fn pending_index_handles(&self) -> &[PendingIndexHandle] {
        &self.pending_index_handles
    }

    pub(crate) fn ensure_open(&self) -> Result<()> {
        if self.open {
            Ok(())
        } else {
            Err(Error::TransactionClosed)
        }
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        if let Some(lifecycle) = self.lifecycle.take() {
            lifecycle.close();
        }
    }

    pub(crate) fn has_row_lock(&self, key: RowKey) -> bool {
        self.row_locks.contains(&key)
    }

    pub(crate) fn push_row_lock(&mut self, key: RowKey) {
        self.row_locks.push(key);
    }

    pub(crate) fn drain_row_locks(&mut self) -> impl Iterator<Item = RowKey> + '_ {
        self.row_locks.drain(..)
    }
}

#[derive(Debug)]
pub(crate) struct TxnLifecycle {
    tx_id: TxId,
    inner: Weak<TxStatusInner>,
    closed: AtomicBool,
}

impl TxnLifecycle {
    fn update_snapshot(&self, csn: Csn) {
        if let Some(inner) = self.inner.upgrade() {
            inner.set_active_snapshot(self.tx_id, csn);
        }
    }

    fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst)
            && let Some(inner) = self.inner.upgrade()
        {
            inner.unregister_active(self.tx_id);
        }
    }
}

impl Drop for TxnLifecycle {
    fn drop(&mut self) {
        if !self.closed.swap(true, Ordering::SeqCst)
            && let Some(inner) = self.inner.upgrade()
        {
            inner.abort(self.tx_id);
            inner.unregister_active(self.tx_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::lock::RowKey;
    use crate::format::{RelId, RowId};

    #[test]
    fn row_locks_track_relation_identity() {
        let txs = ConcurrentTxStatus::new();
        let mut tx = txs.begin_txn(Isolation::Snapshot);
        let first = RowKey {
            rel_id: RelId(1),
            row_id: RowId(7),
        };
        let second = RowKey {
            rel_id: RelId(2),
            row_id: RowId(7),
        };
        tx.push_row_lock(first);
        assert!(tx.has_row_lock(first));
        assert!(!tx.has_row_lock(second));
        let locked: Vec<_> = tx.drain_row_locks().collect();
        assert_eq!(locked, vec![first]);
    }
}
