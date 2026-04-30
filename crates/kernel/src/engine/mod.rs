pub mod concurrent_heap;
pub mod lock;
pub mod page_heap;
pub mod tx;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::catalog::{
    CatalogManager, CatalogStore, IndexId as CatalogIndexId, SchemaEpoch, SqliteSchemaRow,
    apply_alter_table, apply_create_index, apply_create_table, apply_drop_index, apply_drop_table,
    apply_set_index_meta_page_id, bootstrap_schema, lookup_table,
};
use crate::engine::lock::{RowKey, RowLockManager};
use crate::engine::page_heap::{PageBackedHeap, VacuumStats};
use crate::engine::tx::PendingIndexHandle;
use crate::format::{Csn, DEFAULT_PAGE_SIZE, Lsn, Page, PageId, RelId, RowId, TxId};
use crate::index::{
    BtreeIndex, INDEX_VERSION, IndexDescriptor, IndexId as PhysicalIndexId, IndexUniqueness,
};
use crate::storage::{
    BufferPool, BufferPoolStats, ControlFile, ControlStore, DEFAULT_CHECKPOINT_BATCH_PAGES,
    PageFile, TxStatusCheckpoint, TxStatusStore,
};
use crate::telemetry::{Phase11Counters, Phase11CountersSnapshot};
use crate::txn::Isolation;
use crate::wal::{
    WalConfig, WalCoordinator, WalPayload, WalReader, WalRecord, WalRecordKind, WalScanReport,
    WalSyncCountersSnapshot,
};
use crate::{Error, Result};

const BEGIN_LOCK_KEY: RowKey = RowKey {
    rel_id: RelId::ZERO,
    row_id: RowId::ZERO,
};

#[cfg(feature = "failpoints")]
std::thread_local! {
    /// Lane KH P0 #3: per-thread switch for the
    /// `engine::commit::before_publish` failpoint. The fail-crate
    /// registry is process-wide, so without this guard the closure
    /// would inject the fault on every commit in every parallel test.
    /// Tests call [`arm_commit_failure_for_thread`] before issuing
    /// their commit and clear it afterwards.
    static COMMIT_FAILURE_ARMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arm or disarm the thread-local commit-failure injection used by the
/// `engine::commit::before_publish` failpoint closure. Available only
/// when the kernel is built with `failpoints` so production builds pay
/// nothing for it.
#[cfg(feature = "failpoints")]
pub fn arm_commit_failure_for_thread(armed: bool) {
    COMMIT_FAILURE_ARMED.with(|c| c.set(armed));
}

#[cfg(feature = "failpoints")]
fn commit_failure_armed_for_thread() -> bool {
    COMMIT_FAILURE_ARMED.with(|c| c.get())
}

#[cfg(not(feature = "failpoints"))]
#[allow(dead_code)]
fn commit_failure_armed_for_thread() -> bool {
    false
}

pub use tx::{ConcurrentTxStatus, TxStatusStats, Txn};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointStats {
    pub control: ControlFile,
    pub flushed_pages: usize,
    pub flush_batches: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageStatsSnapshot {
    pub buffer: BufferPoolStats,
    pub tx: TxStatusStats,
    pub checkpoint: Option<ControlFile>,
    pub resident_heap_pages: usize,
    pub wal_written_lsn: Lsn,
    pub wal_durable_lsn: Lsn,
    pub vacuum_horizon_csn: Csn,
    /// Lane BH P1 #7: durability syscall counters bumped by the
    /// WAL writer thread. Surface them through `Database::stats`
    /// so the bench harness can record per-run fsync/pwrite tallies
    /// without reaching into kernel internals.
    pub wal_sync_counters: WalSyncCountersSnapshot,
    /// Phase 11 Wave 0: structural counter surface. The aggregator
    /// is allocated alongside the WAL coordinator's sync counters,
    /// but Wave 0 only defines the addressing — emission sites land
    /// in subsequent waves so every field stays at `0` for now.
    pub phase11_counters: Phase11CountersSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryReport {
    pub scanned_records: usize,
    pub valid_end_lsn: Lsn,
    pub torn_tail: bool,
    pub page_images_redone: usize,
    pub legacy_mutations_redone: usize,
    pub commits_recovered: usize,
    pub replay_from_lsn: Lsn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryTarget {
    Latest,
    Lsn(Lsn),
    Csn(Csn),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RecoveryMetrics {
    page_images_redone: usize,
    legacy_mutations_redone: usize,
    commits_recovered: usize,
}

impl RecoveryReport {
    fn from_scan(scan: WalScanReport, metrics: RecoveryMetrics, replay_from_lsn: Lsn) -> Self {
        Self {
            scanned_records: scan.records.len(),
            valid_end_lsn: scan.valid_end_lsn,
            torn_tail: scan.torn_tail,
            page_images_redone: metrics.page_images_redone,
            legacy_mutations_redone: metrics.legacy_mutations_redone,
            commits_recovered: metrics.commits_recovered,
            replay_from_lsn,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    pub rel_id: RelId,
    pub wal: WalConfig,
    pub commit_durability: CommitDurability,
    pub lock_shards: usize,
    pub busy_timeout: Duration,
    pub heap_lanes: usize,
    pub page_size: usize,
    pub buffer_pool_pages: usize,
    pub data_file_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitDurability {
    Strict,
    Normal,
    UnsafeDev,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed(Csn),
    RolledBack,
    MaybeCommitted,
}

impl Default for EngineConfig {
    fn default() -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(4);
        Self {
            rel_id: RelId(1),
            wal: WalConfig::default(),
            commit_durability: CommitDurability::Strict,
            lock_shards: (parallelism * 4).max(16),
            busy_timeout: Duration::from_millis(250),
            heap_lanes: parallelism.max(4),
            page_size: DEFAULT_PAGE_SIZE,
            buffer_pool_pages: 1024,
            data_file_name: "data.redline".to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct Engine {
    config: EngineConfig,
    data_path: PathBuf,
    wal_dir: PathBuf,
    rel_id: RelId,
    txs: ConcurrentTxStatus,
    buffer: Arc<BufferPool>,
    heap: PageBackedHeap,
    catalog: CatalogManager,
    catalog_store: CatalogStore,
    locks: RowLockManager,
    wal: Arc<WalCoordinator>,
    /// Phase 11 Wave 0: engine-level aggregator for the new
    /// telemetry counters. Lives next to `wal` because it is the
    /// sibling container for non-WAL emission sites (leaf visits,
    /// prefetch, heap rechecks, cursor batches, lock waits) plus
    /// the per-flush WAL batch histogram. Wave 0 only allocates
    /// it; subsequent waves wire the `.fetch_add` sites.
    phase11_counters: Arc<Phase11Counters>,
    control: ControlStore,
    tx_status_store: TxStatusStore,
    checkpoint: Mutex<Option<ControlFile>>,
    /// Live `BtreeIndex` handles keyed by catalog `IndexId`. Populated when
    /// the engine creates an index (via `create_index`) or rehydrates from a
    /// catalog snapshot at open time. Lane A wires this so SQL exec lanes
    /// (B/C) can borrow handles via `Engine::index_handle`.
    index_handles: Mutex<HashMap<CatalogIndexId, Arc<BtreeIndex>>>,
}

impl Engine {
    pub fn create(path: impl AsRef<Path>, config: EngineConfig) -> Result<Arc<Self>> {
        std::fs::create_dir_all(path.as_ref())?;
        let data_path = path.as_ref().join(&config.data_file_name);
        let wal_dir = path.as_ref().join("wal");
        let page_file = Arc::new(PageFile::create(&data_path, config.page_size)?);
        let buffer = Arc::new(BufferPool::new(page_file, config.buffer_pool_pages)?);
        let wal = Arc::new(WalCoordinator::create(&wal_dir, config.wal.clone())?);
        let control = ControlStore::new(path.as_ref())?;
        let tx_status_store = TxStatusStore::new(path.as_ref())?;
        let checkpoint = control.load_latest()?;
        let catalog_store = CatalogStore::new(path.as_ref());
        let loaded_catalog = catalog_store.load().ok().flatten();
        let initial_catalog = loaded_catalog
            .clone()
            .unwrap_or_else(|| bootstrap_schema(RelId(10_000)));
        if loaded_catalog.is_none() {
            catalog_store.save_atomic(&initial_catalog)?;
        }
        let buffer = Arc::clone(&buffer);
        let phase11_counters = Arc::new(Phase11Counters::default());
        let locks = RowLockManager::new(config.lock_shards, config.busy_timeout);
        // Wave 1A-F: pipe Phase11 telemetry into the row-lock manager so
        // contended-acquire waits land in `lock_wait_us_buckets`. Same
        // Arc piped into the WAL coordinator so `wal_batch_size_buckets`
        // gets bumped per fdatasync.
        locks.set_phase11_counters(Arc::clone(&phase11_counters));
        wal.set_phase11_counters(Arc::clone(&phase11_counters));
        Ok(Arc::new(Self {
            config: config.clone(),
            data_path,
            wal_dir,
            rel_id: config.rel_id,
            txs: ConcurrentTxStatus::new(),
            buffer: Arc::clone(&buffer),
            heap: PageBackedHeap::new_with_wal(
                config.rel_id,
                config.heap_lanes,
                buffer,
                Some(Arc::clone(&wal)),
            )?,
            catalog: CatalogManager::new(initial_catalog),
            catalog_store,
            locks,
            wal,
            phase11_counters,
            control,
            tx_status_store,
            checkpoint: Mutex::new(checkpoint),
            index_handles: Mutex::new(HashMap::new()),
        }))
    }

    pub fn open(path: impl AsRef<Path>, config: EngineConfig) -> Result<Arc<Self>> {
        Self::open_with_recovery_report(path, config).map(|(engine, _report)| engine)
    }

    pub fn open_with_recovery_report(
        path: impl AsRef<Path>,
        config: EngineConfig,
    ) -> Result<(Arc<Self>, RecoveryReport)> {
        Self::open_with_recovery_report_and_target(path, config, RecoveryTarget::Latest)
    }

    pub fn open_with_recovery_target(
        path: impl AsRef<Path>,
        config: EngineConfig,
        target: RecoveryTarget,
    ) -> Result<Arc<Self>> {
        Self::open_with_recovery_report_and_target(path, config, target).map(|(engine, _)| engine)
    }

    pub fn open_with_recovery_report_and_target(
        path: impl AsRef<Path>,
        config: EngineConfig,
        target: RecoveryTarget,
    ) -> Result<(Arc<Self>, RecoveryReport)> {
        let wal_dir = path.as_ref().join("wal");
        let mut reader = WalReader::new(&wal_dir, config.wal.clone());
        let scan_report = reader.scan_report()?;
        let txs = ConcurrentTxStatus::new();
        std::fs::create_dir_all(path.as_ref())
            .map_err(|_| Error::CorruptPage("create engine directory failed"))?;
        let control = ControlStore::new(path.as_ref())
            .map_err(|_| Error::CorruptPage("create control store failed"))?;
        let tx_status_store = TxStatusStore::new(path.as_ref())
            .map_err(|_| Error::CorruptPage("create tx status store failed"))?;
        let checkpoint = control
            .load_latest()
            .map_err(|_| Error::CorruptPage("load control file failed"))?;
        let page_path = path.as_ref().join(&config.data_file_name);
        let page_file = if checkpoint.is_some() || page_path.exists() {
            Arc::new(
                PageFile::open(&page_path, config.page_size)
                    .map_err(|_| Error::CorruptPage("open recovered page file failed"))?,
            )
        } else {
            Arc::new(
                PageFile::create(&page_path, config.page_size)
                    .map_err(|_| Error::CorruptPage("create recovered page file failed"))?,
            )
        };
        let buffer = Arc::new(
            BufferPool::new(page_file, config.buffer_pool_pages)
                .map_err(|_| Error::CorruptPage("create buffer pool failed"))?,
        );
        let wal = Arc::new(
            WalCoordinator::open(&wal_dir, config.wal.clone())
                .map_err(|_| Error::CorruptWal("open wal coordinator failed"))?,
        );
        let heap = PageBackedHeap::new_with_wal(
            config.rel_id,
            config.heap_lanes,
            Arc::clone(&buffer),
            Some(Arc::clone(&wal)),
        )
        .map_err(|_| Error::CorruptPage("create heap failed"))?;
        let catalog_store = CatalogStore::new(path.as_ref());
        let initial_catalog = catalog_store
            .load()
            .ok()
            .flatten()
            .unwrap_or_else(|| bootstrap_schema(RelId(10_000)));
        let replay_from_lsn = if let Some(checkpoint) = checkpoint {
            let tx_status = tx_status_store.load(checkpoint.generation)?;
            if tx_status.generation != checkpoint.generation {
                return Err(Error::CorruptPage(
                    "tx status checkpoint generation mismatch",
                ));
            }
            match target {
                RecoveryTarget::Latest => {}
                RecoveryTarget::Lsn(limit) if limit < checkpoint.checkpoint_lsn => {
                    return Err(Error::CorruptWal(
                        "requested recovery target is older than checkpoint base",
                    ));
                }
                RecoveryTarget::Csn(limit) if limit < tx_status.published_csn => {
                    return Err(Error::CorruptWal(
                        "requested recovery target is older than checkpoint base",
                    ));
                }
                RecoveryTarget::Lsn(_) | RecoveryTarget::Csn(_) => {}
            }
            for (tx_id, csn) in tx_status.entries {
                txs.publish_recovered_commit(tx_id, csn);
            }
            txs.restore_frontier(
                tx_status.next_tx,
                tx_status.next_csn,
                tx_status.published_csn,
            );
            checkpoint.checkpoint_lsn
        } else {
            // LSN sentinel: legit init. No checkpoint exists yet, so recovery
            // replays the entire WAL starting from the very beginning.
            Lsn::ZERO
        };
        let metrics = recover_heap(&scan_report.records, replay_from_lsn, target, &txs, &heap)?;
        let recovered_catalog = recover_catalog_snapshot(&scan_report.records, target)?;
        let page_count = heap.page_count()?;
        heap.load_row_directory_from_pages(page_count)?;
        heap.load_reusable_pages_from_pages(page_count)?;
        let catalog = CatalogManager::new(recovered_catalog.unwrap_or(initial_catalog));
        let phase11_counters = Arc::new(Phase11Counters::default());
        let locks = RowLockManager::new(config.lock_shards, config.busy_timeout);
        // Wave 1A-F: same telemetry pipe on the open path.
        locks.set_phase11_counters(Arc::clone(&phase11_counters));
        wal.set_phase11_counters(Arc::clone(&phase11_counters));
        let engine = Arc::new(Self {
            config: config.clone(),
            data_path: page_path,
            wal_dir,
            rel_id: config.rel_id,
            txs,
            buffer: Arc::clone(&buffer),
            heap,
            catalog,
            catalog_store,
            locks,
            wal,
            phase11_counters,
            control,
            tx_status_store,
            checkpoint: Mutex::new(checkpoint),
            index_handles: Mutex::new(HashMap::new()),
        });
        engine.rehydrate_index_handles()?;
        Ok((
            engine,
            RecoveryReport::from_scan(scan_report, metrics, replay_from_lsn),
        ))
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        self.locks.set_timeout(timeout);
    }

    pub fn begin(&self, isolation: Isolation) -> Result<Txn> {
        if isolation == Isolation::Serializable {
            return Err(Error::UnsupportedIsolation);
        }
        Ok(self.txs.begin_txn(isolation))
    }

    pub fn reserve_begin_lock(&self, tx: &mut Txn) -> Result<()> {
        if tx.has_row_lock(BEGIN_LOCK_KEY) {
            return Ok(());
        }
        self.locks
            .lock(BEGIN_LOCK_KEY.rel_id, BEGIN_LOCK_KEY.row_id, tx.id())?;
        tx.push_row_lock(BEGIN_LOCK_KEY);
        Ok(())
    }

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
            crate::engine::page_heap::RelationWriteTarget { rel_id, row_id },
            payload,
            Lsn(1),
        )
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

    pub fn commit(&self, mut tx: Txn) -> Result<CommitOutcome> {
        tx.ensure_open()?;
        let pending_schema = tx.pending_schema_snapshot();
        if let Some(snapshot) = pending_schema.as_deref() {
            let snapshot_bytes = crate::catalog::encode_snapshot(snapshot)?;
            self.wal.append(
                WalRecordKind::Logical,
                tx.id(),
                WalPayload::CatalogSnapshot {
                    tx_id: tx.id(),
                    schema_epoch: snapshot.meta.schema_epoch.0,
                    snapshot: snapshot_bytes,
                }
                .encode()?,
            )?;
        }

        let (csn, append) = match self
            .wal
            .append_commit(tx.id(), || self.txs.reserve_commit_csn())
        {
            Ok(value) => value,
            Err(err) => {
                self.txs.abort(tx.id());
                self.release_locks(&mut tx);
                tx.close();
                return Err(err);
            }
        };

        let commit_barrier = match self.config.commit_durability {
            CommitDurability::Strict => self.wal.flush_until(append.end_lsn),
            CommitDurability::Normal => self.wal.write_until(append.end_lsn),
            CommitDurability::UnsafeDev => Ok(append.end_lsn),
        };
        if let Err(err) = commit_barrier {
            self.txs.cancel_reserved_csn(csn);
            self.txs.abort(tx.id());
            self.release_locks(&mut tx);
            tx.close();
            return Err(err);
        }

        // Lane E failpoint: WAL fsync has acked but the CSN is not yet
        // visible to in-memory observers. The injected path returns
        // `MaybeCommitted` after publishing the commit locally, so higher
        // layers can surface the uncertainty without replaying any SQL-side
        // index repair.
        let _pending_schema_for_closure = pending_schema.clone();
        crate::fail_point!("engine::commit::before_publish", |arg: Option<String>| {
            if !commit_failure_armed_for_thread() {
                let _ = arg;
                return Ok(self.finish_commit(
                    &mut tx,
                    csn,
                    _pending_schema_for_closure.clone(),
                    CommitOutcome::Committed(csn),
                ));
            }
            let _detail =
                arg.unwrap_or_else(|| "engine::commit::before_publish injected fault".to_string());
            Ok(self.finish_commit(
                &mut tx,
                csn,
                _pending_schema_for_closure.clone(),
                CommitOutcome::MaybeCommitted,
            ))
        });
        Ok(self.finish_commit(&mut tx, csn, pending_schema, CommitOutcome::Committed(csn)))
    }

    fn finish_commit(
        &self,
        tx: &mut Txn,
        csn: Csn,
        pending_schema: Option<Arc<crate::catalog::SchemaSnapshot>>,
        outcome: CommitOutcome,
    ) -> CommitOutcome {
        self.txs.publish_commit(tx.id(), csn);
        if let Some(snapshot) = pending_schema {
            let _ = self.catalog_store.save_atomic(&snapshot);
            self.catalog.publish(snapshot);
        }
        if !tx.pending_index_handles().is_empty()
            && let Ok(mut handles) = self.index_handles.lock()
        {
            for action in tx.pending_index_handles() {
                match action {
                    PendingIndexHandle::Install(index_id, handle) => {
                        handles.insert(*index_id, Arc::clone(handle));
                    }
                    PendingIndexHandle::Remove(index_id) => {
                        handles.remove(index_id);
                    }
                }
            }
        }
        self.release_locks(tx);
        tx.close();
        outcome
    }

    pub fn rollback(&self, mut tx: Txn) -> Result<()> {
        tx.ensure_open()?;
        self.txs.abort(tx.id());
        self.release_locks(&mut tx);
        tx.close();
        Ok(())
    }

    pub fn tx_state(&self, tx_id: TxId) -> crate::txn::TxState {
        self.txs.state(tx_id)
    }

    pub fn tx_status_stats(&self) -> TxStatusStats {
        self.txs.stats()
    }

    pub fn tx_status(&self) -> &ConcurrentTxStatus {
        &self.txs
    }

    #[doc(hidden)]
    pub fn buffer_pool_for_tests(&self) -> &BufferPool {
        &self.buffer
    }

    pub fn schema_epoch(&self) -> SchemaEpoch {
        self.catalog.version()
    }

    pub fn schema_snapshot(&self) -> Arc<crate::catalog::SchemaSnapshot> {
        self.catalog.current()
    }

    pub fn validate_schema_epoch(&self, epoch: SchemaEpoch) -> Result<()> {
        if self.schema_epoch() == epoch {
            Ok(())
        } else {
            Err(Error::SchemaChanged)
        }
    }

    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let snapshot = self.catalog.current();
        let mut errors = Vec::new();
        match PageFile::open(&self.data_path, self.config.page_size) {
            Ok(page_file) => match page_file.page_count() {
                Ok(page_count) => {
                    for page_no in 1..=page_count {
                        if let Err(err) = page_file.read_page(PageId(page_no)) {
                            errors.push(format!("page {page_no}: {err}"));
                        }
                    }
                }
                Err(err) => errors.push(format!("page file count: {err}")),
            },
            Err(err) => errors.push(format!("page file open: {err}")),
        }
        for index in &snapshot.indexes {
            if snapshot.table_by_id(index.table_id).is_none() {
                errors.push(format!(
                    "catalog index {} references missing table",
                    index.name
                ));
            }
        }
        let mut wal_reader = WalReader::new(&self.wal_dir, self.config.wal.clone());
        if let Err(err) = wal_reader.scan_report() {
            errors.push(format!("wal prefix scan: {err}"));
        }
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        for index in &snapshot.indexes {
            if index.meta_page_id.is_none() {
                continue;
            }
            let Some(handle) = handles.get(&index.index_id) else {
                errors.push(format!("index {} has no open handle", index.name));
                continue;
            };
            let report = handle.validate()?;
            for error in report.errors {
                errors.push(format!("index {}: {error}", index.name));
            }
        }
        Ok(errors)
    }

    pub fn sqlite_schema(&self) -> Vec<SqliteSchemaRow> {
        self.catalog.current().sqlite_schema_rows()
    }

    pub fn lookup_table(
        &self,
        tx: &Txn,
        name: crate::catalog::QualifiedName,
    ) -> Result<Arc<crate::catalog::TableDef>> {
        let snapshot = self.catalog_snapshot_for_tx(tx);
        lookup_table(&snapshot, &name)
    }

    pub fn create_table(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateTableSpec,
    ) -> Result<Arc<crate::catalog::TableDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = apply_create_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        let next = Arc::new(next);
        let table = next
            .tables
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created table missing from snapshot"))?;
        tx.set_pending_schema_snapshot(Arc::clone(&next));
        Ok(table)
    }

    pub fn drop_table(&self, tx: &mut Txn, spec: crate::catalog::DropTableSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = apply_drop_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    pub fn create_index(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateIndexSpec,
    ) -> Result<Arc<crate::catalog::IndexDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        // Step 1: build the catalog delta so we know the index_id.
        let next = apply_create_index((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        let created_index = next
            .indexes
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Step 2: allocate physical B-tree pages with the WAL coordinator.
        let descriptor = IndexDescriptor::new(
            PhysicalIndexId(created_index.index_id.0),
            created_index.relation_id,
            if created_index.unique {
                IndexUniqueness::Unique
            } else {
                IndexUniqueness::NonUnique
            },
        );
        let btree = BtreeIndex::create_with_wal(
            Arc::clone(&self.buffer),
            descriptor,
            Some(Arc::clone(&self.wal)),
        )?;
        // Log PageImage records for meta + root so recovery can reconstruct
        // the B-tree even if no checkpoint runs before engine close.
        btree.record_initial_page_images(tx.id())?;
        let meta_page_id = btree.meta_page_id();

        // Step 3: persist meta_page_id back into the snapshot.
        let with_meta = apply_set_index_meta_page_id(next, created_index.index_id, meta_page_id)?;
        let with_meta = Arc::new(with_meta);
        let final_index = with_meta
            .index_by_id(created_index.index_id)
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Step 4: DDL backfill — index every visible row of the underlying
        // table at the time of CREATE INDEX. The backfill uses the in-memory
        // snapshot/tx_status; if the table is empty this is a no-op.
        let table = with_meta
            .table_by_id(final_index.table_id)
            .ok_or(Error::ObjectNotFound)?;
        self.backfill_index(tx, &btree, &table, &final_index)?;

        // Step 5: install the handle only if the surrounding DDL transaction
        // commits. Rollback must not expose a handle for a catalog entry that
        // never became visible.
        tx.push_pending_index_handle(PendingIndexHandle::Install(
            final_index.index_id,
            Arc::new(btree),
        ));
        tx.set_pending_schema_snapshot(Arc::clone(&with_meta));
        Ok(final_index)
    }

    pub fn drop_index(&self, tx: &mut Txn, spec: crate::catalog::DropIndexSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        // Find the index id BEFORE applying the drop (the snapshot mutates).
        let snapshot = self.catalog_snapshot_for_tx(tx);
        let removed_id = crate::catalog::lookup_index(&snapshot, &spec.name)
            .ok()
            .map(|idx| idx.index_id);

        let next = apply_drop_index((*snapshot).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));

        // Page reuse: PageBackedHeap currently does not support marking
        // arbitrary index meta/root pages as reusable (it tracks Heap/Undo
        // kinds only). The pages remain allocated until vacuum/checkpoint
        // reclaims them via a future enhancement. TODO: wire btree page
        // reclamation through PageBackedHeap once it supports BtreeMeta and
        // BtreeLeaf reusability.
        if let Some(index_id) = removed_id {
            tx.push_pending_index_handle(PendingIndexHandle::Remove(index_id));
        }
        Ok(())
    }

    /// Returns the live `BtreeIndex` handle for the given catalog `IndexId`,
    /// if one has been allocated. SQL exec lanes (B/C) use this to issue
    /// physical lookups and maintenance operations against the index.
    pub fn index_handle(&self, index_id: CatalogIndexId) -> Option<Arc<BtreeIndex>> {
        self.index_handles
            .lock()
            .ok()
            .and_then(|handles| handles.get(&index_id).cloned())
    }

    pub fn alter_table(&self, tx: &mut Txn, spec: crate::catalog::AlterTableSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = apply_alter_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    pub fn oldest_active_snapshot_csn(&self) -> Csn {
        self.txs.oldest_active_snapshot_csn()
    }

    pub fn vacuum(&self) -> Result<VacuumStats> {
        self.vacuum_with_horizon(self.oldest_active_snapshot_csn())
    }

    pub fn vacuum_with_horizon(&self, horizon: Csn) -> Result<VacuumStats> {
        self.heap.vacuum(horizon, &self.txs)
    }

    pub fn flush_heap_pages(&self) -> Result<()> {
        let durable_lsn = self.wal.durable_lsn()?;
        self.heap.flush_all(durable_lsn)
    }

    pub fn resident_heap_pages(&self) -> usize {
        self.heap.resident_pages()
    }

    pub fn row_directory_entries(&self) -> Result<Vec<(RowId, crate::format::TuplePtr)>> {
        self.heap.row_directory_entries()
    }

    pub fn relation_rowids(&self, rel_id: RelId) -> Result<Vec<RowId>> {
        self.heap.relation_rowids(rel_id)
    }

    pub fn relation_entries(&self, rel_id: RelId) -> Result<Vec<(RowId, crate::format::TuplePtr)>> {
        self.heap.relation_entries(rel_id)
    }

    pub fn buffer_pool_stats(&self) -> BufferPoolStats {
        self.heap.buffer_stats()
    }

    pub fn storage_stats(&self) -> Result<StorageStatsSnapshot> {
        Ok(StorageStatsSnapshot {
            buffer: self.heap.buffer_stats(),
            tx: self.txs.stats(),
            checkpoint: self.checkpoint_info()?,
            resident_heap_pages: self.heap.resident_pages(),
            wal_written_lsn: self.wal.written_lsn()?,
            wal_durable_lsn: self.wal.durable_lsn()?,
            vacuum_horizon_csn: self.oldest_active_snapshot_csn(),
            wal_sync_counters: self.wal.sync_counters_snapshot(),
            phase11_counters: self.phase11_counters_snapshot(),
        })
    }

    /// Phase 11 Wave 0: relaxed-atomic snapshot of the Phase 11
    /// counter aggregator. Mirrors
    /// [`crate::wal::WalCoordinator::sync_counters_snapshot`] for
    /// downstream telemetry callers (the bench harness picks it up
    /// via `Database::benchmark_stats`).
    pub fn phase11_counters_snapshot(&self) -> Phase11CountersSnapshot {
        self.phase11_counters.snapshot()
    }

    /// Phase 11 Wave 0: shared handle to the Phase 11 counter
    /// aggregator. Wave 1+ instrumentation sites can clone this
    /// `Arc` to gain direct access for `.fetch_add` calls without
    /// going through an additional accessor.
    pub fn phase11_counters(&self) -> Arc<Phase11Counters> {
        Arc::clone(&self.phase11_counters)
    }

    pub fn checkpoint(&self) -> Result<ControlFile> {
        self.checkpoint_with_stats().map(|stats| stats.control)
    }

    pub fn checkpoint_with_stats(&self) -> Result<CheckpointStats> {
        let durable_lsn = self.wal.flush_all()?;
        let flush = self
            .heap
            .flush_dirty_batches(durable_lsn, DEFAULT_CHECKPOINT_BATCH_PAGES)?;
        let page_count = self.heap.page_count()?;
        let mut checkpoint = self
            .checkpoint
            .lock()
            .map_err(|_| Error::CorruptPage("checkpoint mutex poisoned"))?;
        let generation = checkpoint
            .map(|control| control.generation + 1)
            .unwrap_or(1);
        self.tx_status_store.write(&TxStatusCheckpoint {
            generation,
            next_tx: self.txs.next_tx(),
            next_csn: self.txs.next_csn(),
            published_csn: self.txs.published_csn(),
            entries: self.txs.committed_states(),
        })?;
        self.catalog_store
            .save_atomic(self.catalog.current().as_ref())?;
        // Lane E failpoint: armed before the new control-file generation lands
        // on disk. A crash here forces recovery to fall back to the previous
        // generation, exercising the dual-control-file protocol.
        crate::fail_point!("engine::checkpoint");
        let next = self
            .control
            .write_next(*checkpoint, durable_lsn, page_count)?;
        self.wal
            .prune_segments_below_checkpoint_lsn(next.checkpoint_lsn)?;
        *checkpoint = Some(next);
        Ok(CheckpointStats {
            control: next,
            flushed_pages: flush.flushed_pages,
            flush_batches: flush.batches,
        })
    }

    pub fn checkpoint_info(&self) -> Result<Option<ControlFile>> {
        self.checkpoint
            .lock()
            .map(|checkpoint| *checkpoint)
            .map_err(|_| Error::CorruptPage("checkpoint mutex poisoned"))
    }

    /// Lane INT: structural validation across every catalog index handle,
    /// returning the per-index `errors` lists for the
    /// `redline_index_check` PRAGMA. Complements the flat
    /// `integrity_check()` which returns errors as strings only.
    pub fn integrity_check_per_index(&self) -> Result<Vec<(String, Vec<String>)>> {
        let snapshot = self.catalog.current();
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        let mut out = Vec::new();
        for index in &snapshot.indexes {
            let Some(btree) = handles.get(&index.index_id) else {
                continue;
            };
            let validation = btree.validate()?;
            let errors = validation
                .errors
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>();
            out.push((index.name.to_string(), errors));
        }
        Ok(out)
    }

    /// Lane INT: full heap/index/page equivalence check. Returns the
    /// structured [`crate::integrity::IntegrityReport`] consumed by the
    /// `redline_full_check` PRAGMA and the bench certification harness.
    pub fn integrity_check_full(&self) -> Result<crate::integrity::IntegrityReport> {
        crate::integrity::run_full(self)
    }

    pub(crate) fn buffer_for_integrity(&self) -> &Arc<BufferPool> {
        &self.buffer
    }

    pub(crate) fn heap_for_integrity(&self) -> &PageBackedHeap {
        &self.heap
    }

    pub(crate) fn txs_for_integrity(&self) -> &ConcurrentTxStatus {
        &self.txs
    }

    pub(crate) fn txs_snapshot_for_integrity(&self) -> crate::txn::Snapshot {
        self.txs.snapshot()
    }

    pub(crate) fn read_raw_page_bytes_for_integrity(
        &self,
        page_id: crate::format::PageId,
    ) -> Result<Vec<u8>> {
        self.buffer.read_page_bytes_unchecked(page_id)
    }

    pub(crate) fn index_handles_for_integrity(
        &self,
    ) -> Result<HashMap<CatalogIndexId, Arc<BtreeIndex>>> {
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        Ok(handles.clone())
    }

    fn refresh_read_committed(&self, tx: &mut Txn) {
        if tx.isolation() == Isolation::ReadCommitted {
            tx.replace_snapshot(self.txs.snapshot());
        }
    }

    fn catalog_snapshot_for_tx(&self, tx: &Txn) -> Arc<crate::catalog::SchemaSnapshot> {
        tx.pending_schema_snapshot()
            .unwrap_or_else(|| self.catalog.current())
    }

    fn lock_row(&self, tx: &mut Txn, row_id: RowId) -> Result<()> {
        let key = RowKey {
            rel_id: self.rel_id,
            row_id,
        };
        if tx.has_row_lock(key) {
            return Ok(());
        }
        self.locks.lock(self.rel_id, row_id, tx.id())?;
        tx.push_row_lock(key);
        Ok(())
    }

    fn lock_row_in_rel(&self, tx: &mut Txn, rel_id: RelId, row_id: RowId) -> Result<()> {
        let key = RowKey { rel_id, row_id };
        if tx.has_row_lock(key) {
            return Ok(());
        }
        self.locks.lock(rel_id, row_id, tx.id())?;
        tx.push_row_lock(key);
        Ok(())
    }

    fn release_locks(&self, tx: &mut Txn) {
        let tx_id = tx.id();
        for key in tx.drain_row_locks() {
            self.locks.unlock(key.rel_id, key.row_id, tx_id);
        }
    }

    /// Reopens every catalog `IndexDef` whose `meta_page_id` is set, stashing
    /// a `BtreeIndex` handle keyed by catalog `IndexId`. Indexes whose
    /// `meta_page_id` is `None` are legacy/backwards-compat (created before
    /// Lane A wired physical pages); they are skipped silently because the
    /// SQL exec layer cannot use them yet.
    fn rehydrate_index_handles(self: &Arc<Self>) -> Result<()> {
        let snapshot = self.catalog.current();
        let mut rebuilt = Vec::new();
        let mut opened = Vec::new();
        for index in &snapshot.indexes {
            let Some(meta_page_id) = index.meta_page_id else {
                // Pre-Lane-A index without physical pages; nothing to reopen.
                continue;
            };
            let descriptor = IndexDescriptor::new(
                PhysicalIndexId(index.index_id.0),
                index.relation_id,
                if index.unique {
                    IndexUniqueness::Unique
                } else {
                    IndexUniqueness::NonUnique
                },
            );
            let version = BtreeIndex::format_version(&self.buffer, meta_page_id)?;
            if version == INDEX_VERSION {
                let btree = BtreeIndex::open_with_wal(
                    Arc::clone(&self.buffer),
                    meta_page_id,
                    descriptor,
                    Some(Arc::clone(&self.wal)),
                )?;
                opened.push((index.index_id, Arc::new(btree)));
            } else if version == 1 {
                let table = snapshot
                    .table_by_id(index.table_id)
                    .ok_or(Error::CatalogCorrupt("index table missing during rebuild"))?;
                rebuilt.push((index.as_ref().clone(), table));
            } else {
                return Err(Error::UnsupportedVersion(version));
            }
        }
        let mut next_snapshot = (*snapshot).clone();
        let mut rebuild_tx = if rebuilt.is_empty() {
            None
        } else {
            Some(self.begin(Isolation::Snapshot)?)
        };
        for (index, table) in rebuilt {
            let descriptor = IndexDescriptor::new(
                PhysicalIndexId(index.index_id.0),
                index.relation_id,
                if index.unique {
                    IndexUniqueness::Unique
                } else {
                    IndexUniqueness::NonUnique
                },
            );
            let btree = BtreeIndex::create_with_wal(
                Arc::clone(&self.buffer),
                descriptor,
                Some(Arc::clone(&self.wal)),
            )?;
            let tx = rebuild_tx
                .as_mut()
                .ok_or(Error::CorruptPage("missing index rebuild transaction"))?;
            btree.record_initial_page_images(tx.id())?;
            self.backfill_index(tx, &btree, &table, &index)?;
            next_snapshot =
                apply_set_index_meta_page_id(next_snapshot, index.index_id, btree.meta_page_id())?;
            opened.push((index.index_id, Arc::new(btree)));
        }
        if let Some(mut tx) = rebuild_tx {
            let next_snapshot = Arc::new(next_snapshot);
            tx.set_pending_schema_snapshot(next_snapshot);
            match self.commit(tx)? {
                CommitOutcome::Committed(_) => {}
                CommitOutcome::MaybeCommitted => {
                    return Err(Error::CorruptWal("index rebuild maybe committed"));
                }
                CommitOutcome::RolledBack => {
                    return Err(Error::CorruptWal("index rebuild rolled back"));
                }
            }
        }
        let mut handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        for (index_id, btree) in opened {
            handles.insert(index_id, btree);
        }
        Ok(())
    }

    /// Walks the heap relation backing this index's table and inserts every
    /// visible row into the freshly-built B-tree. Called from `create_index`
    /// to make the index immediately usable for the rest of the transaction.
    /// On a non-empty table this performs the SQLite-style synchronous
    /// CREATE INDEX backfill. On an empty table it is a no-op.
    fn backfill_index(
        &self,
        tx: &mut Txn,
        btree: &BtreeIndex,
        table: &crate::catalog::TableDef,
        index: &crate::catalog::IndexDef,
    ) -> Result<()> {
        use crate::catalog::{
            EncodedIndexKey, IndexKeySource, RecordRef, RecordScratch, ValueRef, encode_index_key,
        };

        // Snapshot the row directory for this relation BEFORE we begin so the
        // backfill does not race with concurrent inserts in the same tx.
        let entries = self.heap.relation_entries(table.relation_id)?;
        if entries.is_empty() {
            return Ok(());
        }
        let mut scratch = RecordScratch::default();
        let mut key_buf = Vec::new();
        let dirs: Vec<crate::catalog::SortDir> =
            index.keys.iter().map(|key| key.sort_dir).collect();
        for (row_id, _ptr) in entries {
            let payload = self.heap.get_for_relation(
                &self.txs,
                tx.snapshot(),
                Some(tx.id()),
                table.relation_id,
                row_id,
            )?;
            let Some(payload) = payload else {
                continue;
            };
            let record = RecordRef::new(&payload)
                .map_err(|_| Error::CorruptPage("index backfill: malformed heap record"))?;
            record
                .decode_into(&mut scratch)
                .map_err(|_| Error::CorruptPage("index backfill: record decode failed"))?;
            let mut parts: Vec<ValueRef<'_>> = Vec::with_capacity(index.keys.len());
            for key in &index.keys {
                let IndexKeySource::Column { attnum } = key.source;
                let value = record
                    .value_at(&scratch, attnum as usize)
                    .map_err(|_| Error::CorruptPage("index backfill: column out of range"))?;
                parts.push(value);
            }
            let EncodedIndexKey {
                bytes,
                contains_null,
            } = encode_index_key(&parts, &dirs, &mut key_buf);
            // SQLite NULL-uniqueness rule: skip the unique conflict check
            // when any leading key component is NULL — duplicates of NULL
            // are allowed in unique indexes.
            if index.unique && !contains_null {
                let owner = tx.id().0;
                let _guard = btree.lock_unique_key(owner, &bytes)?;
                if !btree
                    .point_lookup_visible(&self.txs, tx.snapshot(), Some(tx.id()), &bytes)?
                    .is_empty()
                {
                    return Err(Error::WriteConflict);
                }
            }
            let row_ref = crate::index::IndexRowRef::with_row_id(
                row_id,
                crate::format::TuplePtr::new_with_generation(
                    crate::format::PageId(0),
                    0,
                    crate::format::PageGeneration::ONE,
                ),
            );
            btree.insert_tx(tx.id(), &bytes, row_ref)?;
        }
        Ok(())
    }
}

fn recover_heap(
    records: &[WalRecord],
    replay_from_lsn: Lsn,
    target: RecoveryTarget,
    txs: &ConcurrentTxStatus,
    heap: &PageBackedHeap,
) -> Result<RecoveryMetrics> {
    let mut committed = std::collections::HashMap::new();
    let mut metrics = RecoveryMetrics::default();
    for record in records {
        if record.kind == WalRecordKind::Commit {
            match WalPayload::decode(&record.payload)? {
                WalPayload::Commit { tx_id, csn } => {
                    if commit_visible(record.lsn, csn, target) {
                        committed.insert(tx_id, csn);
                        txs.publish_recovered_commit(tx_id, csn);
                        metrics.commits_recovered += 1;
                    }
                }
                _ => return Err(Error::CorruptWal("commit record has non-commit payload")),
            }
        }
    }

    for record in records {
        if record.lsn < replay_from_lsn {
            continue;
        }
        if matches!(target, RecoveryTarget::Lsn(limit) if record.lsn >= limit) {
            continue;
        }
        if record.kind == WalRecordKind::Commit {
            continue;
        }
        match WalPayload::decode(&record.payload)? {
            WalPayload::PageImage {
                page_id: _,
                page_lsn: _,
                page_bytes,
            } if committed.contains_key(&record.tx_id) => {
                let page = Page::from_bytes(page_bytes)?;
                heap.redo_page_image(page, record_end_lsn(record))?;
                metrics.page_images_redone += 1;
            }
            WalPayload::PageImage { .. } => {}
            WalPayload::HeapInsert {
                tx_id,
                rel_id,
                row_id,
                payload,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.insert_recovered_for_relation(tx_id, rel_id, row_id, payload)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapUpdate {
                tx_id,
                rel_id,
                row_id,
                payload,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.update_recovered_for_relation(tx_id, rel_id, row_id, payload)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapDelete {
                tx_id,
                rel_id,
                row_id,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.delete_recovered_for_relation(tx_id, rel_id, row_id)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapInsert { .. }
            | WalPayload::HeapUpdate { .. }
            | WalPayload::HeapDelete { .. } => {}
            WalPayload::SegmentSeal { .. }
            | WalPayload::BackupBegin { .. }
            | WalPayload::BackupEnd { .. }
            | WalPayload::TimelineFork { .. }
            | WalPayload::LogicalTxn { .. }
            | WalPayload::CatalogSnapshot { .. } => {}
            WalPayload::Commit { .. } => unreachable!("commit records are skipped above"),
        }
    }

    Ok(metrics)
}

fn recover_catalog_snapshot(
    records: &[WalRecord],
    target: RecoveryTarget,
) -> Result<Option<Arc<crate::catalog::SchemaSnapshot>>> {
    let mut committed = std::collections::HashSet::new();
    for record in records {
        if record.kind == WalRecordKind::Commit
            && let WalPayload::Commit { tx_id, csn } = WalPayload::decode(&record.payload)?
            && commit_visible(record.lsn, csn, target)
        {
            committed.insert(tx_id);
        }
    }

    let mut latest: Option<(Lsn, Arc<crate::catalog::SchemaSnapshot>)> = None;
    for record in records {
        if record.kind != WalRecordKind::Logical || !committed.contains(&record.tx_id) {
            continue;
        }
        let WalPayload::CatalogSnapshot {
            tx_id: _,
            schema_epoch: _,
            snapshot,
        } = WalPayload::decode(&record.payload)?
        else {
            continue;
        };
        let snapshot = Arc::new(crate::catalog::decode_snapshot(&snapshot)?);
        match &latest {
            Some((lsn, _)) if *lsn >= record.lsn => {}
            _ => latest = Some((record.lsn, snapshot)),
        }
    }

    Ok(latest.map(|(_, snapshot)| snapshot))
}

fn commit_visible(record_lsn: Lsn, csn: Csn, target: RecoveryTarget) -> bool {
    match target {
        RecoveryTarget::Latest => true,
        RecoveryTarget::Lsn(limit) => record_lsn < limit,
        RecoveryTarget::Csn(limit) => csn <= limit,
    }
}

fn record_end_lsn(record: &WalRecord) -> Lsn {
    Lsn(record.lsn.0 + record.encoded_len() as u64)
}
