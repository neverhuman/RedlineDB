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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OptimizerOptions {
    pub enabled: bool,
    pub max_exact_join_tables: usize,
    pub max_join_alternatives: usize,
    pub enable_multi_index_or: bool,
    pub enable_multi_index_and: bool,
    pub enable_covering_index: bool,
}

impl Default for OptimizerOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_exact_join_tables: 8,
            max_join_alternatives: 4,
            enable_multi_index_or: true,
            enable_multi_index_and: true,
            enable_covering_index: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryMemoryOptions {
    pub work_mem_bytes: usize,
    pub max_spill_bytes: usize,
    pub batch_rows: usize,
}

impl Default for QueryMemoryOptions {
    fn default() -> Self {
        Self {
            work_mem_bytes: 8 * 1024 * 1024,
            max_spill_bytes: 1024 * 1024 * 1024,
            batch_rows: 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnalyzeOptions {
    pub exact_analyze_row_threshold: usize,
    pub sample_rows: usize,
    pub mcv_capacity: usize,
    pub histogram_buckets: usize,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            exact_analyze_row_threshold: 100_000,
            sample_rows: 32_768,
            mcv_capacity: 100,
            histogram_buckets: 100,
        }
    }
}

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
}

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
        }
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
