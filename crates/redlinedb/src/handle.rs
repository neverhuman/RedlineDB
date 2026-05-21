//! Thread-safe `Database` handle, factory constructors, and admin operations
//! (checkpoint, vacuum, backup, replication-slot bridges).

use std::cell::Cell;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::Duration;

use redlinedb_kernel::format::DbId;

use crate::connection::Connection;
use crate::error::Result;
use crate::options::{
    BackupOptions, BackupStats, BenchmarkStats, BufferStats, CheckpointBenchStats, CheckpointStats,
    DatabaseStats, Durability, OpenOptions, TxBenchStats, VacuumStats, WalBenchStats,
};
use crate::phase8::{
    self, ArchiveStats, PhysicalBackupOptions, PhysicalBackupStats, ReplicationSlot,
    ReplicationSlotStats, RestoreOptions, RestoreStats, RetentionHorizon,
};
use crate::registry;
use crate::snapshot;
use crate::statement::Prepared;

/// Thread-safe handle to a database image.
///
/// `Database` is cheap to clone, `Send + Sync`, and intended to be the
/// pooling boundary: open one `Database`, then hand out fresh
/// [`Connection`] values to worker threads as needed. All connections opened
/// from the same `Database` see the same state.
pub struct Database {
    pub(crate) inner: Arc<registry::DatabaseEntry>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, OpenOptions::default())
    }

    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(
            path,
            OpenOptions {
                create: true,
                ..Default::default()
            },
        )
    }

    pub fn open_with_options(path: impl AsRef<Path>, options: OpenOptions) -> Result<Self> {
        let inner = registry::open_database(path, &options, options.create)?;
        Ok(Self { inner })
    }

    /// Create a private ephemeral database rooted under the directory
    /// named by [`OpenOptions::temp_dir`] when provided. Without an
    /// explicit root, Linux uses `/dev/shm/redlinedb-ephemeral` when it
    /// is writable, otherwise the process scratch directory.
    ///
    /// Multiple connections opened from the returned [`Database`] share the
    /// same transient state. The backing directory disappears when the last
    /// `Database` owner drops.
    pub fn create_in_memory(options: OpenOptions) -> Result<Self> {
        let options = volatile_open_options(options);
        let inner = registry::create_in_memory_database(&options)?;
        Ok(Self { inner })
    }

    /// Create or reopen a process-local ephemeral database identified by
    /// `session_name`.
    ///
    /// Subsequent calls with the same `session_name` reuse the same live
    /// session for as long as at least one [`Database`] handle is still
    /// alive. When the final owner drops, the owned ephemeral root is
    /// removed.
    pub fn create_ephemeral(session_name: &str, options: OpenOptions) -> Result<Self> {
        let options = volatile_open_options(options);
        let inner = registry::create_ephemeral_database(session_name, &options)?;
        Ok(Self { inner })
    }

    pub fn connect(&self) -> Result<Connection> {
        let busy_timeout = *self
            .inner
            .busy_timeout
            .lock()
            .expect("busy timeout poisoned");
        Ok(Connection {
            inner: self.inner.db.connect(),
            read_only: self.inner.fingerprint.read_only,
            busy_timeout,
            interrupted: Arc::clone(&self.inner.interrupt),
            _sync_marker: Cell::new(()),
        })
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        *self
            .inner
            .busy_timeout
            .lock()
            .expect("busy timeout poisoned") = timeout;
        self.inner.db.set_busy_timeout(timeout);
    }

    pub fn prepare(&self, sql: &str) -> Result<Prepared> {
        let mut conn = self.connect()?;
        let stmt = conn.prepare(sql)?;
        Ok(Prepared {
            template: stmt.template(),
        })
    }

    pub fn checkpoint(&self) -> Result<CheckpointStats> {
        let checkpoint = self.inner.db.checkpoint()?;
        let _ = phase8::update_retention(self);
        Ok(CheckpointStats {
            generation: checkpoint.control.generation,
            checkpoint_lsn: checkpoint.control.checkpoint_lsn.0,
            page_count: checkpoint.control.page_count,
            flushed_pages: checkpoint.flushed_pages,
            flush_batches: checkpoint.flush_batches,
        })
    }

    pub fn vacuum(&self) -> Result<VacuumStats> {
        let vacuum = self.inner.db.vacuum()?;
        Ok(VacuumStats {
            rows_scanned: vacuum.rows_scanned,
            chains_pruned: vacuum.chains_pruned,
            undo_links_removed: vacuum.undo_links_removed,
            dead_rows_removed: vacuum.dead_rows_removed,
            oldest_active_snapshot_csn: vacuum.oldest_active_snapshot_csn.0,
        })
    }

    pub fn stats(&self) -> Result<DatabaseStats> {
        let stats = self.inner.db.stats()?;
        Ok(DatabaseStats {
            schema_epoch: self.inner.db.schema_epoch().0,
            checkpoint_generation: stats.checkpoint.map(|control| control.generation),
            resident_heap_pages: stats.resident_heap_pages,
            wal_written_lsn: stats.wal_written_lsn.0,
            wal_durable_lsn: stats.wal_durable_lsn.0,
            vacuum_horizon_csn: stats.vacuum_horizon_csn.0,
            table_count: stats.tx.committed_states,
            column_count: stats.tx.active_transactions,
            index_count: stats.tx.active_snapshots,
        })
    }

    pub fn benchmark_stats(&self) -> Result<BenchmarkStats> {
        let stats = self.inner.db.stats()?;
        let retained_bytes = self
            .archive_stats()
            .map(|archive| archive.archived_bytes)
            .unwrap_or(0);
        Ok(BenchmarkStats {
            buffer: BufferStats {
                resident_pages: stats.buffer.resident_pages,
                reads: stats.buffer.reads,
                writes: stats.buffer.writes,
                evictions: stats.buffer.evictions,
                checkpoint_flushes: stats.buffer.checkpoint_flushes,
            },
            tx: TxBenchStats {
                next_tx: stats.tx.next_tx.0,
                next_csn: stats.tx.next_csn.0,
                published_csn: stats.tx.published_csn.0,
                active_transactions: stats.tx.active_transactions,
                active_snapshots: stats.tx.active_snapshots,
                committed_states: stats.tx.committed_states,
                pending_csns: stats.tx.pending_csns,
            },
            wal: WalBenchStats {
                written_lsn: stats.wal_written_lsn.0,
                durable_lsn: stats.wal_durable_lsn.0,
                retained_bytes,
                // Lane BH P1 #7: forward kernel WAL coordinator
                // syscall counters so the bench harness no longer
                // has to leave `process_metrics.fsync_count` etc.
                // as `None` on the macOS / no-strace paths.
                fsyncs_issued: stats.wal_sync_counters.fsyncs_issued,
                fdatasyncs_issued: stats.wal_sync_counters.fdatasyncs_issued,
                pwrites_issued: stats.wal_sync_counters.pwrites_issued,
                group_commits_issued: stats.wal_sync_counters.group_commits_issued,
                group_commit_batch_bytes_sum: stats.wal_sync_counters.group_commit_batch_bytes_sum,
                group_commit_batch_record_count_sum: stats
                    .wal_sync_counters
                    .group_commit_batch_record_count_sum,
                group_commit_batch_p50: stats.wal_sync_counters.batch_record_count_percentile(0.50),
                group_commit_batch_p95: stats.wal_sync_counters.batch_record_count_percentile(0.95),
                group_commit_batch_p99: stats.wal_sync_counters.batch_record_count_percentile(0.99),
                group_commit_batch_max: stats.wal_sync_counters.batch_record_count_max(),
            },
            checkpoint: CheckpointBenchStats {
                generation: stats.checkpoint.map(|control| control.generation),
                vacuum_horizon_csn: stats.vacuum_horizon_csn.0,
            },
            // Phase 11 Wave 0: forward the structural counter
            // snapshot. All fields are zero today; emission lands in
            // subsequent waves. `Some(_)` so manifests carry the
            // typed shape (Wave 1 dashboards can rely on a stable
            // schema) while `#[serde(skip_serializing_if =
            // "Option::is_none")]` keeps the door open for engines
            // that cannot supply the counters at all.
            phase11_counters: Some(stats.phase11_counters),
        })
    }

    pub fn backup_to_path(
        &self,
        dst: impl AsRef<Path>,
        options: BackupOptions,
    ) -> Result<BackupStats> {
        snapshot::backup_to_path(self, dst, options)
    }

    pub fn backup_physical_to_path(
        &self,
        dst: impl AsRef<Path>,
        options: PhysicalBackupOptions,
    ) -> Result<PhysicalBackupStats> {
        phase8::backup_physical_to_path(self, dst, options)
    }

    pub fn restore_from_backup(
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
        options: RestoreOptions,
    ) -> Result<RestoreStats> {
        phase8::restore_from_backup(src, dst, options)
    }

    pub fn create_physical_slot(&self, name: &str) -> Result<ReplicationSlot> {
        phase8::create_physical_slot(self, name, true)
    }

    pub fn create_logical_slot(&self, name: &str) -> Result<ReplicationSlot> {
        phase8::create_logical_slot(self, name, true)
    }

    pub fn drop_replication_slot(&self, name: &str) -> Result<()> {
        phase8::drop_replication_slot(self, name)
    }

    pub fn archive_stats(&self) -> Result<ArchiveStats> {
        phase8::archive_stats(self)
    }

    pub fn replication_slots(&self) -> Result<Vec<ReplicationSlotStats>> {
        phase8::replication_slots(self)
    }

    pub fn retention_horizon(&self) -> Result<RetentionHorizon> {
        phase8::retention_horizon(self)
    }

    pub fn database_id(&self) -> Result<DbId> {
        phase8::current_database_id(self)
    }

    pub fn interrupt_all(&self) {
        self.inner.interrupt.store(true, AtomicOrdering::Relaxed);
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }
}

fn volatile_open_options(mut options: OpenOptions) -> OpenOptions {
    options.create = true;
    options.durability = Durability::UnsafeDev;
    options.process_owner_lock = false;
    if options.temp_dir.is_none() {
        options.temp_dir = Some(registry::standard_volatile_root());
    }
    options
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub(crate) fn sql_options(options: &OpenOptions) -> redlinedb_sql::DbOptions {
    let mut db = redlinedb_sql::DbOptions::default();
    let page_size = db.engine.page_size.max(1);
    let buffer_pages = (options.memory.cache_bytes / page_size).max(16);
    db.engine.buffer_pool_pages = buffer_pages;
    db.engine.busy_timeout = options.busy_timeout;
    db.engine.commit_durability = match options.durability {
        Durability::Strict => redlinedb_kernel::engine::CommitDurability::Strict,
        Durability::Normal => redlinedb_kernel::engine::CommitDurability::Normal,
        Durability::UnsafeDev => redlinedb_kernel::engine::CommitDurability::UnsafeDev,
    };
    db.optimizer.enabled = options.optimizer.enabled;
    db.optimizer.max_exact_join_tables = options.optimizer.max_exact_join_tables;
    db.optimizer.max_join_alternatives = options.optimizer.max_join_alternatives;
    db.optimizer.enable_multi_index_or = options.optimizer.enable_multi_index_or;
    db.optimizer.enable_multi_index_and = options.optimizer.enable_multi_index_and;
    db.optimizer.enable_covering_index = options.optimizer.enable_covering_index;
    db.query_memory.work_mem_bytes = options.query_memory.work_mem_bytes;
    db.query_memory.max_spill_bytes = options.query_memory.max_spill_bytes;
    db.query_memory.batch_rows = options.query_memory.batch_rows;
    db.statement_cache_capacity = options.statement_cache_capacity;
    db.temp_dir = options.temp_dir.clone();
    db.stats.exact_analyze_row_threshold = options.stats.exact_analyze_row_threshold;
    db.stats.sample_rows = options.stats.sample_rows;
    db.stats.mcv_capacity = options.stats.mcv_capacity;
    db.stats.histogram_buckets = options.stats.histogram_buckets;
    db
}

pub(crate) fn private_in_memory_sql_options(options: &OpenOptions) -> redlinedb_sql::DbOptions {
    let mut db = sql_options(options);
    db.engine.lock_shards = db.engine.lock_shards.clamp(1, 4);
    db.engine.heap_lanes = 1;
    db.engine.buffer_pool_pages = db.engine.buffer_pool_pages.clamp(16, 1024);
    db.unique_lock_shards = db.unique_lock_shards.clamp(1, 16);
    db.statement_cache_capacity = db.statement_cache_capacity.min(16);
    db
}
