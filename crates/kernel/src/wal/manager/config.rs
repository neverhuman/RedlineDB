/// Default WAL segment size in bytes.
pub const DEFAULT_WAL_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Default in-memory WAL buffer size in bytes.
pub const DEFAULT_WAL_BUFFER_BYTES: usize = 16 * 1024 * 1024;
/// Default write batch size in bytes.
pub const DEFAULT_WAL_WRITE_BATCH_BYTES: usize = 1024 * 1024;
/// Default group-commit delay in microseconds.
pub const DEFAULT_GROUP_COMMIT_DELAY_US: u64 = 200;
/// Default maximum group-commit batch size in bytes.
pub const DEFAULT_GROUP_COMMIT_MAX_BATCH_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalConfig {
    pub segment_bytes: u64,
    pub wal_buffer_bytes: usize,
    pub wal_write_batch_bytes: usize,
    pub group_commit_delay_us: u64,
    pub group_commit_max_batch_bytes: u64,
    /// Lane GC (phase 10): number of per-core WAL lanes. `1` (the
    /// default) preserves the historical single-writer single-
    /// segment-set behaviour byte-for-byte. `> 1` partitions writers
    /// by `(thread_id % lanes)` across independent sub-coordinators
    /// each owning their own segment subdirectory; recovery walks
    /// every lane in LSN order. Multi-lane mode is opt-in via
    /// [`crate::wal::WalLaneCoordinator`]; the engine itself stays
    /// single-lane today.
    pub lanes: usize,
    /// Lane GC (phase 10): opt-in semantic commit combiner. When
    /// `true`, adjacent `WalPayload::CombinedSemanticDelta` audit
    /// records on the same `(tx_id, rel_id, row_id)` may be folded
    /// in-buffer before fsync. Default `false` preserves the
    /// historical byte-for-byte behaviour.
    pub semantic_combiner: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            segment_bytes: DEFAULT_WAL_SEGMENT_BYTES,
            wal_buffer_bytes: DEFAULT_WAL_BUFFER_BYTES,
            wal_write_batch_bytes: DEFAULT_WAL_WRITE_BATCH_BYTES,
            group_commit_delay_us: DEFAULT_GROUP_COMMIT_DELAY_US,
            group_commit_max_batch_bytes: DEFAULT_GROUP_COMMIT_MAX_BATCH_BYTES,
            lanes: 1,
            semantic_combiner: false,
        }
    }
}
