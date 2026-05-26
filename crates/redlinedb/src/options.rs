use std::time::Duration;

use redlinedb_kernel::telemetry::Phase11CountersSnapshot;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MemoryOptions {
    pub cache_bytes: usize,
}

impl Default for MemoryOptions {
    fn default() -> Self {
        Self {
            cache_bytes: 16 * 1024 * 1024,
        }
    }
}

// Batch D3: the optimizer / query-memory / analyze option structs are owned by
// the SQL crate (`redlinedb_sql`). The public `redlinedb` API keeps its
// historical type names by re-aliasing the canonical definitions so existing
// call sites (`OptimizerOptions::default()`, struct-literal construction, etc.)
// continue to compile unchanged while there is now exactly one source of truth
// for the layout, defaults, and hashing behaviour.
pub use redlinedb_sql::OptimizerConfig as OptimizerOptions;
pub use redlinedb_sql::QueryMemoryConfig as QueryMemoryOptions;
pub use redlinedb_sql::StatsConfig as AnalyzeOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Durability {
    Strict,
    Normal,
    UnsafeDev,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenOptions {
    pub create: bool,
    pub read_only: bool,
    pub durability: Durability,
    pub memory: MemoryOptions,
    pub optimizer: OptimizerOptions,
    pub query_memory: QueryMemoryOptions,
    pub stats: AnalyzeOptions,
    pub statement_cache_capacity: usize,
    pub busy_timeout: Duration,
    pub process_owner_lock: bool,
    pub temp_dir: Option<std::path::PathBuf>,
    /// Size of the per-`Database` Rayon thread pool used by future
    /// intra-query parallel operators. `None` (default) picks
    /// `min(num_cpus, 8)`. `Some(0)` or `Some(1)` opts out of the pool —
    /// operators fall back to their serial path. `Some(n)` builds an
    /// `n`-thread non-global pool dedicated to this database.
    pub rayon_threads: Option<usize>,
    /// WS-C9: opt-in lean defaults for short-lived ephemeral databases
    /// (`:memory:`, CLI one-shots, smoke tests). When `true`:
    ///   * buffer pool capacity is forced to `LEAN_BUFFER_POOL_PAGES`
    ///     (256 pages = 1 MB at 4 KB) instead of the larger calculation
    ///     from `memory.cache_bytes`.
    ///   * statement cache capacity is forced to
    ///     `LEAN_STATEMENT_CACHE_CAPACITY` (8) instead of the user's
    ///     setting.
    /// Long-lived servers keep `false` so steady-state throughput is
    /// unchanged.
    pub lean_ephemeral: bool,
}

/// Buffer pool capacity, in pages, when `lean_ephemeral = true`.
/// 256 pages × 4 KB = 1 MB.
pub const LEAN_BUFFER_POOL_PAGES: usize = 256;
/// Statement cache capacity when `lean_ephemeral = true`.
pub const LEAN_STATEMENT_CACHE_CAPACITY: usize = 8;

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            create: true,
            read_only: false,
            durability: Durability::Strict,
            memory: MemoryOptions::default(),
            optimizer: OptimizerOptions::default(),
            query_memory: QueryMemoryOptions::default(),
            stats: AnalyzeOptions::default(),
            statement_cache_capacity: 128,
            busy_timeout: Duration::from_secs(5),
            process_owner_lock: true,
            temp_dir: None,
            rayon_threads: None,
            lean_ephemeral: false,
        }
    }
}

impl OpenOptions {
    /// Fluent setter for `busy_timeout`. Callers can write
    /// `OpenOptions::default().with_busy_timeout(Duration::from_millis(100))`
    /// instead of constructing the struct field-by-field.
    #[must_use]
    pub fn with_busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn with_create(mut self, create: bool) -> Self {
        self.create = create;
        self
    }

    #[must_use]
    pub fn with_durability(mut self, durability: Durability) -> Self {
        self.durability = durability;
        self
    }

    #[must_use]
    pub fn with_statement_cache_capacity(mut self, capacity: usize) -> Self {
        self.statement_cache_capacity = capacity;
        self
    }

    #[must_use]
    pub fn with_process_owner_lock(mut self, locked: bool) -> Self {
        self.process_owner_lock = locked;
        self
    }

    #[must_use]
    pub fn with_temp_dir(mut self, temp_dir: impl Into<std::path::PathBuf>) -> Self {
        self.temp_dir = Some(temp_dir.into());
        self
    }

    #[must_use]
    pub fn with_rayon_threads(mut self, threads: Option<usize>) -> Self {
        self.rayon_threads = threads;
        self
    }

    /// WS-C9: opt into lean ephemeral defaults. See
    /// [`OpenOptions::lean_ephemeral`].
    #[must_use]
    pub fn with_lean_ephemeral(mut self, lean: bool) -> Self {
        self.lean_ephemeral = lean;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupOptions {
    pub batch_rows: usize,
    pub include_stats: bool,
    pub recreate_indexes_after_data: bool,
    pub abort_on_schema_change: bool,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            batch_rows: 1024,
            include_stats: true,
            recreate_indexes_after_data: true,
            abort_on_schema_change: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackupStats {
    pub tables_copied: u64,
    pub rows_copied: u64,
    pub indexes_created: u64,
    pub elapsed_ms: u128,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckpointStats {
    pub generation: u64,
    pub checkpoint_lsn: u64,
    pub page_count: u64,
    pub flushed_pages: usize,
    pub flush_batches: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumStats {
    pub rows_scanned: usize,
    pub chains_pruned: usize,
    pub undo_links_removed: usize,
    pub dead_rows_removed: usize,
    pub oldest_active_snapshot_csn: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DatabaseStats {
    pub schema_epoch: u64,
    pub checkpoint_generation: Option<u64>,
    pub resident_heap_pages: usize,
    pub wal_written_lsn: u64,
    pub wal_durable_lsn: u64,
    pub vacuum_horizon_csn: u64,
    pub table_count: usize,
    pub column_count: usize,
    pub index_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BufferStats {
    pub resident_pages: usize,
    pub reads: u64,
    pub writes: u64,
    pub evictions: u64,
    pub checkpoint_flushes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TxBenchStats {
    pub next_tx: u64,
    pub next_csn: u64,
    pub published_csn: u64,
    pub active_transactions: usize,
    pub active_snapshots: usize,
    pub committed_states: usize,
    pub pending_csns: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct WalBenchStats {
    pub written_lsn: u64,
    pub durable_lsn: u64,
    pub retained_bytes: u64,
    /// Lane BH P1 #7: durability syscall counters surfaced from
    /// the kernel's WAL coordinator. Bench harnesses copy these
    /// onto `ProcessMetrics` so `summary.csv` and the run record
    /// no longer report `None` for the Linux-only counters when a
    /// Redline engine is in play.
    #[serde(default)]
    pub fsyncs_issued: u64,
    #[serde(default)]
    pub fdatasyncs_issued: u64,
    #[serde(default)]
    pub pwrites_issued: u64,
    #[serde(default)]
    pub group_commits_issued: u64,
    #[serde(default)]
    pub group_commit_batch_bytes_sum: u64,
    #[serde(default)]
    pub group_commit_batch_record_count_sum: u64,
    #[serde(default)]
    pub group_commit_batch_p50: u64,
    #[serde(default)]
    pub group_commit_batch_p95: u64,
    #[serde(default)]
    pub group_commit_batch_p99: u64,
    #[serde(default)]
    pub group_commit_batch_max: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CheckpointBenchStats {
    pub generation: Option<u64>,
    pub vacuum_horizon_csn: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BenchmarkStats {
    pub buffer: BufferStats,
    pub tx: TxBenchStats,
    pub wal: WalBenchStats,
    pub checkpoint: CheckpointBenchStats,
    /// Phase 11 Wave 0: structural counter surface forwarded from
    /// the kernel. Wave 0 only allocates/exposes the counters; every
    /// field is `0` until subsequent waves add emission sites. The
    /// field is `Option<_>` so manifests written by older binaries
    /// keep deserialising once the schema evolves further.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase11_counters: Option<Phase11CountersSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionStats {
    pub changes: u64,
    pub last_insert_rowid: Option<i64>,
    pub busy_timeout_ms: u64,
    pub interrupted: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecuteSummary {
    pub rows_affected: u64,
    pub rows_returned: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitStats {
    pub changes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionArity {
    Exact(usize),
    Any,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FunctionFlags {
    pub deterministic: bool,
    pub innocuous: bool,
}
