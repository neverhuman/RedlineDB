pub mod concurrent_heap;
pub mod lock;
pub mod page_heap;
pub mod tx;

mod catalog_ops;
mod maintenance;
mod recovery;
mod runtime;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::catalog::{CatalogManager, CatalogStore, IndexId as CatalogIndexId};
use crate::engine::lock::{RowKey, RowLockManager};
use crate::engine::page_heap::PageBackedHeap;
use crate::format::{Csn, DEFAULT_PAGE_SIZE, Lsn, RelId, RowId};
use crate::index::BtreeIndex;
use crate::storage::{BufferPool, BufferPoolStats, ControlFile, ControlStore, TxStatusStore};
use crate::telemetry::{Phase11Counters, Phase11CountersSnapshot};
use crate::wal::{WalConfig, WalCoordinator, WalScanReport, WalSyncCountersSnapshot};

const BEGIN_LOCK_KEY: RowKey = RowKey {
    rel_id: RelId::ZERO,
    row_id: RowId::ZERO,
};

#[cfg(feature = "failpoints")]
pub use runtime::arm_commit_failure_for_thread;
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

impl CommitDurability {
    const STRICT_U8: u8 = 0;
    const NORMAL_U8: u8 = 1;
    const UNSAFE_DEV_U8: u8 = 2;

    #[inline]
    fn to_u8(self) -> u8 {
        match self {
            Self::Strict => Self::STRICT_U8,
            Self::Normal => Self::NORMAL_U8,
            Self::UnsafeDev => Self::UNSAFE_DEV_U8,
        }
    }

    #[inline]
    fn from_u8(value: u8) -> Self {
        match value {
            Self::NORMAL_U8 => Self::Normal,
            Self::UNSAFE_DEV_U8 => Self::UnsafeDev,
            _ => Self::Strict, // safest default for unknown
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed(Csn),
    RolledBack,
    MaybeCommitted,
}

impl Default for EngineConfig {
    fn default() -> Self {
        // W7-perf: avoid the cgroup walk in `cached_available_parallelism()`
        // when constructing the default config for volatile (in-memory) databases.
        // Persistent databases call `with_detected_parallelism()` in
        // `Engine::create_inner`, which scales up to match CPU count at that
        // point (still using the process-wide OnceLock cache).  Using a fixed
        // baseline here means constructing DbOptions::default() or
        // EngineConfig::default() for an in-memory database no longer triggers
        // the 4–6 syscall cgroup walk on every fresh process.
        Self {
            rel_id: RelId(1),
            wal: WalConfig::default(),
            commit_durability: CommitDurability::Strict,
            lock_shards: 16,
            busy_timeout: Duration::from_millis(250),
            heap_lanes: 4,
            page_size: DEFAULT_PAGE_SIZE,
            buffer_pool_pages: 1024,
            data_file_name: "data.redline".to_owned(),
        }
    }
}

impl EngineConfig {
    /// Scale `lock_shards` and `heap_lanes` to match `available_parallelism()`.
    ///
    /// Called in `Engine::create_inner` for persistent databases only.  Volatile
    /// (in-memory) databases do not need CPU-scaled shards and skip this call to
    /// avoid the cgroup walk on every fresh process.
    pub(crate) fn with_detected_parallelism(mut self) -> Self {
        let parallelism = crate::cached_available_parallelism();
        self.lock_shards = (parallelism * 4).max(self.lock_shards);
        self.heap_lanes = parallelism.max(self.heap_lanes);
        self
    }
}

#[derive(Debug)]
pub struct Engine {
    config: EngineConfig,
    /// Runtime-mutable commit durability. Init from `config.commit_durability`
    /// at open; updatable via `set_commit_durability` so `PRAGMA synchronous`
    /// propagates from SQL into the commit hot path. Read at every commit via
    /// `Engine::commit_durability()` (atomic load, ~free).
    commit_durability_live: AtomicU8,
    volatile: bool,
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
    fn page_wal(&self) -> Option<Arc<WalCoordinator>> {
        if self.volatile {
            None
        } else {
            Some(Arc::clone(&self.wal))
        }
    }

    /// Current commit durability. Read on the commit hot path; reflects any
    /// runtime change made via `set_commit_durability`.
    #[inline]
    pub fn commit_durability(&self) -> CommitDurability {
        CommitDurability::from_u8(self.commit_durability_live.load(Ordering::Relaxed))
    }

    /// Update commit durability live. In-flight commits observe the new value
    /// at their next durability decision. Used by SQL `PRAGMA synchronous`
    /// propagation; never changes the open-time `EngineConfig.commit_durability`
    /// snapshot (that stays the open-time intent).
    #[inline]
    pub fn set_commit_durability(&self, durability: CommitDurability) {
        self.commit_durability_live
            .store(durability.to_u8(), Ordering::Relaxed);
    }
}
