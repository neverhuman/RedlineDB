use crate::catalog::IndexId as CatalogIndexId;
use crate::catalog::SchemaSnapshot;
use crate::engine::lock::RowKey;
use crate::format::{Csn, TxId};
use crate::index::BtreeIndex;
use crate::txn::{Isolation, Snapshot, TxState};
use crate::{Error, Result};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

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

#[derive(Clone, Debug)]
pub struct ConcurrentTxStatus {
    inner: Arc<TxStatusInner>,
}

#[derive(Debug)]
struct TxStatusInner {
    shards: Vec<RwLock<HashMap<TxId, TxState>>>,
    active_snapshots: Mutex<HashMap<TxId, Csn>>,
    next_tx: AtomicU64,
    next_csn: AtomicU64,
    published_csn: AtomicU64,
    frontier: Mutex<CsnFrontier>,
}

#[derive(Debug, Default)]
struct CsnFrontier {
    pending: BTreeSet<u64>,
    completed: BTreeSet<u64>,
    skipped: BTreeSet<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxStatusStats {
    pub next_tx: TxId,
    pub next_csn: Csn,
    pub published_csn: Csn,
    pub active_transactions: usize,
    pub active_snapshots: usize,
    pub committed_states: usize,
    pub pending_csns: usize,
}

impl ConcurrentTxStatus {
    pub fn new() -> Self {
        Self::with_shards(64)
    }

    pub fn with_shards(shard_count: usize) -> Self {
        let shard_count = shard_count.max(1);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(HashMap::new()));
        }
        Self {
            inner: Arc::new(TxStatusInner {
                shards,
                active_snapshots: Mutex::new(HashMap::new()),
                next_tx: AtomicU64::new(1),
                next_csn: AtomicU64::new(1),
                published_csn: AtomicU64::new(0),
                frontier: Mutex::new(CsnFrontier::default()),
            }),
        }
    }

    pub fn begin(&self) -> TxId {
        let tx = TxId(self.inner.next_tx.fetch_add(1, Ordering::Relaxed));
        self.inner.set_state(tx, TxState::InProgress);
        tx
    }

    pub(crate) fn begin_txn(&self, isolation: Isolation) -> Txn {
        let tx = self.begin();
        let snapshot = self.snapshot();
        let lifecycle = Arc::new(TxnLifecycle {
            tx_id: tx,
            inner: Arc::downgrade(&self.inner),
            closed: AtomicBool::new(false),
        });
        self.inner.set_active_snapshot(tx, snapshot.visible_csn);
        Txn::new(tx, isolation, snapshot, lifecycle)
    }

    pub fn reserve_csn(&self) -> Csn {
        self.reserve_commit_csn()
    }

    pub fn reserve_commit_csn(&self) -> Csn {
        let csn = Csn(self.inner.next_csn.fetch_add(1, Ordering::SeqCst));
        let mut frontier = self
            .inner
            .frontier
            .lock()
            .expect("csn frontier mutex poisoned");
        frontier.pending.insert(csn.0);
        csn
    }

    pub fn publish_commit(&self, tx: TxId, csn: Csn) {
        self.inner.set_state(tx, TxState::Committed(csn));
        self.inner.complete_csn(csn);
        self.inner.unregister_active(tx);
    }

    pub fn cancel_reserved_csn(&self, csn: Csn) {
        self.inner.skip_csn(csn);
    }

    pub fn publish_recovered_commit(&self, tx: TxId, csn: Csn) {
        self.inner.set_state(tx, TxState::Committed(csn));
        advance_atomic_past(&self.inner.next_tx, tx.0);
        advance_atomic_past(&self.inner.next_csn, csn.0);
        self.inner.complete_csn(csn);
    }

    pub fn restore_frontier(&self, next_tx: TxId, next_csn: Csn, published_csn: Csn) {
        advance_atomic_to_at_least(&self.inner.next_tx, next_tx.0.max(1));
        advance_atomic_to_at_least(&self.inner.next_csn, next_csn.0.max(1));
        advance_atomic_to_at_least(&self.inner.published_csn, published_csn.0);
    }

    pub fn committed_states(&self) -> Vec<(TxId, Csn)> {
        let mut entries = Vec::new();
        for shard in &self.inner.shards {
            let shard = shard.read().expect("tx status shard poisoned");
            for (tx, state) in shard.iter() {
                if let TxState::Committed(csn) = state {
                    entries.push((*tx, *csn));
                }
            }
        }
        entries.sort_unstable_by_key(|(tx, _)| tx.0);
        entries
    }

    pub fn abort(&self, tx: TxId) {
        self.inner.abort(tx);
    }

    pub fn snapshot(&self) -> Snapshot {
        let next_tx = self.inner.next_tx.load(Ordering::SeqCst);
        Snapshot {
            visible_csn: Csn(self.inner.published_csn.load(Ordering::Acquire)),
            xmin: TxId(next_tx),
            xmax: TxId(next_tx),
            active: BTreeSet::new(),
        }
    }

    pub fn state(&self, tx: TxId) -> TxState {
        self.inner.state(tx)
    }

    pub fn is_tx_visible(&self, tx: TxId, snapshot: &Snapshot, owner: Option<TxId>) -> bool {
        if Some(tx) == owner {
            return true;
        }
        match self.state(tx) {
            TxState::Committed(csn) => csn <= snapshot.visible_csn,
            TxState::InProgress | TxState::Aborted => false,
        }
    }

    pub fn oldest_active_snapshot_csn(&self) -> Csn {
        let active = self
            .inner
            .active_snapshots
            .lock()
            .expect("active snapshot mutex poisoned");
        active
            .values()
            .copied()
            .min()
            .unwrap_or(Csn(self.inner.published_csn.load(Ordering::Acquire)))
    }

    pub fn next_tx(&self) -> TxId {
        TxId(self.inner.next_tx.load(Ordering::SeqCst))
    }

    pub fn next_csn(&self) -> Csn {
        Csn(self.inner.next_csn.load(Ordering::SeqCst))
    }

    pub fn published_csn(&self) -> Csn {
        Csn(self.inner.published_csn.load(Ordering::Acquire))
    }

    pub fn stats(&self) -> TxStatusStats {
        let mut active_transactions = 0_usize;
        let mut committed_states = 0_usize;
        for shard in &self.inner.shards {
            let shard = shard.read().expect("tx status shard poisoned");
            for state in shard.values() {
                match state {
                    TxState::InProgress => active_transactions += 1,
                    TxState::Committed(_) => committed_states += 1,
                    TxState::Aborted => {}
                }
            }
        }
        let active_snapshots = self
            .inner
            .active_snapshots
            .lock()
            .expect("active snapshot mutex poisoned")
            .len();
        let pending_csns = self
            .inner
            .frontier
            .lock()
            .expect("csn frontier mutex poisoned")
            .pending
            .len();
        TxStatusStats {
            next_tx: self.next_tx(),
            next_csn: self.next_csn(),
            published_csn: self.published_csn(),
            active_transactions,
            active_snapshots,
            committed_states,
            pending_csns,
        }
    }
}

impl TxStatusInner {
    fn abort(&self, tx: TxId) {
        self.set_state(tx, TxState::Aborted);
        self.unregister_active(tx);
    }

    fn state(&self, tx: TxId) -> TxState {
        let shard = self.shard(tx).read().expect("tx status shard poisoned");
        shard.get(&tx).copied().unwrap_or(TxState::Aborted)
    }

    fn set_state(&self, tx: TxId, state: TxState) {
        let mut shard = self.shard(tx).write().expect("tx status shard poisoned");
        shard.insert(tx, state);
    }

    fn shard(&self, tx: TxId) -> &RwLock<HashMap<TxId, TxState>> {
        &self.shards[tx.0 as usize % self.shards.len()]
    }

    fn set_active_snapshot(&self, tx: TxId, csn: Csn) {
        let mut active = self
            .active_snapshots
            .lock()
            .expect("active snapshot mutex poisoned");
        active.insert(tx, csn);
    }

    fn unregister_active(&self, tx: TxId) {
        let mut active = self
            .active_snapshots
            .lock()
            .expect("active snapshot mutex poisoned");
        active.remove(&tx);
    }

    fn complete_csn(&self, csn: Csn) {
        let mut frontier = self.frontier.lock().expect("csn frontier mutex poisoned");
        frontier.pending.remove(&csn.0);
        frontier.completed.insert(csn.0);
        self.advance_published_csn(&mut frontier);
    }

    fn skip_csn(&self, csn: Csn) {
        let mut frontier = self.frontier.lock().expect("csn frontier mutex poisoned");
        frontier.pending.remove(&csn.0);
        frontier.skipped.insert(csn.0);
        self.advance_published_csn(&mut frontier);
    }

    fn advance_published_csn(&self, frontier: &mut CsnFrontier) {
        let mut published = self.published_csn.load(Ordering::Acquire);
        loop {
            let next = published.saturating_add(1);
            if frontier.completed.remove(&next) || frontier.skipped.remove(&next) {
                published = next;
                self.published_csn.store(published, Ordering::Release);
            } else {
                break;
            }
        }
    }
}

fn advance_atomic_past(value: &AtomicU64, seen: u64) {
    advance_atomic_to_at_least(value, seen.saturating_add(1));
}

fn advance_atomic_to_at_least(value: &AtomicU64, target: u64) {
    let mut current = value.load(Ordering::SeqCst);
    while current < target {
        match value.compare_exchange(current, target, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

impl Default for ConcurrentTxStatus {
    fn default() -> Self {
        Self::new()
    }
}
