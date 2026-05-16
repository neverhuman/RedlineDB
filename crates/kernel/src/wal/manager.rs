use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::format::Lsn;
use crate::io::{FileSystem, StdFileSystem};
use crate::telemetry::Phase11Counters;
use crate::wal::{WAL_HEADER_LEN, WalRecord};
use crate::{Error, Result};

pub const DEFAULT_WAL_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_WAL_BUFFER_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_WAL_WRITE_BATCH_BYTES: usize = 1024 * 1024;
pub const DEFAULT_GROUP_COMMIT_DELAY_US: u64 = 200;
pub const DEFAULT_GROUP_COMMIT_MAX_BATCH_BYTES: u64 = 4 * 1024 * 1024;
const WAL_SEGMENT_EXT: &str = ".wal";

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
    /// `true`, mutations that the WAL coordinator can prove to be
    /// commutative deltas on the same `(rel_id, row_id, column)`
    /// tuple may be merged before fsync. Default `false` — the
    /// combiner is wired as a stub today (see
    /// [`crate::wal::combiner`]) and will be enabled once the safety
    /// proof lands; consumers may set the flag without changing
    /// visible behaviour.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalAppend {
    pub start_lsn: Lsn,
    pub end_lsn: Lsn,
}

/// Lane BH P1 #7: per-coordinator counters for the durability syscalls.
///
/// Bumped by `WalManager` at the precise sites where the kernel
/// actually issues a `pwrite`/`fdatasync`/`fsync`-equivalent. Exposed
/// via [`WalCoordinator::sync_counters_snapshot`] for downstream
/// telemetry (the bench harness reads this through
/// `Database::benchmark_stats` so the certify manifest's
/// `fsync_count` / `fdatasync_count` / `pwrite_count` fields are no
/// longer always `None` on Redline rows).
///
/// Counters are intentionally per-coordinator (not global) so two
/// concurrent benchmark engines in the same process do not pollute
/// one another's metrics.
///
/// Lane GC (phase 10): also tracks **group-commit batching**
/// telemetry — the writer thread bumps `group_commits_issued` once
/// per single fsync that covers N waiters and accumulates batch
/// sizes (in records and bytes) plus a 16-bucket power-of-two
/// histogram so the bench harness and the paper's fig8 can derive
/// p50 / p95 / p99 / max group-commit batch sizes without holding
/// the coordinator mutex.
#[derive(Debug)]
pub struct WalSyncCounters {
    fsyncs_issued: AtomicU64,
    fdatasyncs_issued: AtomicU64,
    pwrites_issued: AtomicU64,
    /// Lane GC: number of distinct fdatasync-bounded groups that the
    /// writer thread issued. One bump per `wal::flush` site; not the
    /// same as `fdatasyncs_issued` (which also counts segment
    /// rotations). `group_commit_batch_record_count_sum /
    /// group_commits_issued` is the mean batch fan-in.
    group_commits_issued: AtomicU64,
    group_commit_batch_bytes_sum: AtomicU64,
    group_commit_batch_record_count_sum: AtomicU64,
    /// Lane GC: 16 power-of-two buckets indexed by
    /// `floor(log2(record_count))` saturated at 15. Bucket k covers
    /// `[2^k, 2^(k+1))` records (bucket 0 holds singleton commits,
    /// bucket 15 holds anything >= 32768). Reservoir-free histogram
    /// — exact counts, atomic-only updates, sufficient for paper-
    /// grade p50/p95/p99 estimates given the bucket count.
    group_commit_batch_buckets: [AtomicU64; GROUP_COMMIT_BUCKET_COUNT],
}

impl Default for WalSyncCounters {
    fn default() -> Self {
        Self {
            fsyncs_issued: AtomicU64::new(0),
            fdatasyncs_issued: AtomicU64::new(0),
            pwrites_issued: AtomicU64::new(0),
            group_commits_issued: AtomicU64::new(0),
            group_commit_batch_bytes_sum: AtomicU64::new(0),
            group_commit_batch_record_count_sum: AtomicU64::new(0),
            // Lane GC: AtomicU64 is not Copy so the array literal
            // must use `from_fn`. 16 elements is fixed at compile
            // time so this is a constant-cost initialiser.
            group_commit_batch_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

/// Lane GC: number of power-of-two buckets in the group-commit
/// histogram. 16 buckets cover singleton commits up through
/// `>= 32768` waiters, which is more than enough headroom for any
/// realistic concurrent workload.
pub const GROUP_COMMIT_BUCKET_COUNT: usize = 16;

impl WalSyncCounters {
    pub fn snapshot(&self) -> WalSyncCountersSnapshot {
        let mut buckets = [0_u64; GROUP_COMMIT_BUCKET_COUNT];
        for (idx, slot) in self.group_commit_batch_buckets.iter().enumerate() {
            buckets[idx] = slot.load(AtomicOrdering::Relaxed);
        }
        WalSyncCountersSnapshot {
            fsyncs_issued: self.fsyncs_issued.load(AtomicOrdering::Relaxed),
            fdatasyncs_issued: self.fdatasyncs_issued.load(AtomicOrdering::Relaxed),
            pwrites_issued: self.pwrites_issued.load(AtomicOrdering::Relaxed),
            group_commits_issued: self.group_commits_issued.load(AtomicOrdering::Relaxed),
            group_commit_batch_bytes_sum: self
                .group_commit_batch_bytes_sum
                .load(AtomicOrdering::Relaxed),
            group_commit_batch_record_count_sum: self
                .group_commit_batch_record_count_sum
                .load(AtomicOrdering::Relaxed),
            group_commit_batch_buckets: buckets,
        }
    }

    fn bump_fdatasync(&self) {
        self.fdatasyncs_issued.fetch_add(1, AtomicOrdering::Relaxed);
    }

    fn bump_pwrite(&self) {
        self.pwrites_issued.fetch_add(1, AtomicOrdering::Relaxed);
    }

    /// Currently the WAL writer only invokes `sync_data`
    /// (fdatasync), but the public counter is exposed so future
    /// full-fsync sites have a place to land without breaking the
    /// snapshot wire format.
    #[allow(dead_code)]
    fn bump_fsync(&self) {
        self.fsyncs_issued.fetch_add(1, AtomicOrdering::Relaxed);
    }

    /// Lane GC: record one group-commit fsync covering `record_count`
    /// queued WAL records totalling `byte_count` bytes. Called by the
    /// writer thread immediately after the underlying `flush()`
    /// call returns durable. `record_count == 0` is intentionally
    /// treated as a no-op so latency-driven flushes that find an
    /// empty queue do not skew the histogram.
    fn record_group_commit(&self, record_count: u64, byte_count: u64) {
        if record_count == 0 {
            return;
        }
        self.group_commits_issued
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.group_commit_batch_record_count_sum
            .fetch_add(record_count, AtomicOrdering::Relaxed);
        self.group_commit_batch_bytes_sum
            .fetch_add(byte_count, AtomicOrdering::Relaxed);
        let bucket = group_commit_bucket_index(record_count);
        self.group_commit_batch_buckets[bucket].fetch_add(1, AtomicOrdering::Relaxed);
    }
}

/// Lane GC: pick the histogram bucket for a group-commit fan-in.
/// Bucket 0 holds singleton commits, bucket k holds
/// `[2^k, 2^(k+1))`, bucket 15 saturates at `>= 32768`.
fn group_commit_bucket_index(record_count: u64) -> usize {
    if record_count <= 1 {
        return 0;
    }
    // floor(log2(record_count)) for record_count >= 2 fits in usize.
    let leading = (record_count - 1).leading_zeros();
    let bits = u64::BITS - leading;
    let bucket = bits as usize;
    if bucket >= GROUP_COMMIT_BUCKET_COUNT {
        GROUP_COMMIT_BUCKET_COUNT - 1
    } else {
        bucket
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalSyncCountersSnapshot {
    pub fsyncs_issued: u64,
    pub fdatasyncs_issued: u64,
    pub pwrites_issued: u64,
    /// Lane GC: number of distinct group-commit fsyncs the writer
    /// has issued since coordinator open.
    pub group_commits_issued: u64,
    /// Lane GC: sum of bytes covered by all group-commit fsyncs.
    pub group_commit_batch_bytes_sum: u64,
    /// Lane GC: sum of WAL records covered by all group-commit
    /// fsyncs. `group_commit_batch_record_count_sum /
    /// group_commits_issued` gives the mean batch fan-in (a load-
    /// bearing number for the paper's fig8).
    pub group_commit_batch_record_count_sum: u64,
    /// Lane GC: 16 power-of-two buckets indexed by
    /// `floor(log2(record_count))` saturated at 15. See
    /// [`WalSyncCountersSnapshot::batch_record_count_percentile`]
    /// for an estimator that walks them.
    pub group_commit_batch_buckets: [u64; GROUP_COMMIT_BUCKET_COUNT],
}

impl WalSyncCountersSnapshot {
    /// Lane GC: lower-bound estimate of the `q`th percentile (q in
    /// [0.0, 1.0]) of group-commit batch sizes (in records). Returns
    /// the **lower edge** of the bucket containing the `q`-quantile,
    /// i.e. `2^k` for bucket `k`. Returns `0` if no group commits
    /// have been recorded.
    pub fn batch_record_count_percentile(&self, q: f64) -> u64 {
        if self.group_commits_issued == 0 {
            return 0;
        }
        let q = q.clamp(0.0, 1.0);
        let target = ((self.group_commits_issued as f64) * q).ceil() as u64;
        let target = target.max(1);
        let mut cumulative = 0_u64;
        for (idx, &count) in self.group_commit_batch_buckets.iter().enumerate() {
            cumulative = cumulative.saturating_add(count);
            if cumulative >= target {
                return 1_u64 << idx;
            }
        }
        // Fallback: top bucket lower edge.
        1_u64 << (GROUP_COMMIT_BUCKET_COUNT - 1)
    }

    /// Lane GC: lower-bound estimate of the **maximum** group-commit
    /// batch size (in records) — the lower edge of the highest
    /// non-empty bucket. Returns `0` if no group commits recorded.
    pub fn batch_record_count_max(&self) -> u64 {
        for (idx, &count) in self.group_commit_batch_buckets.iter().enumerate().rev() {
            if count > 0 {
                return 1_u64 << idx;
            }
        }
        0
    }
}

#[derive(Debug)]
pub struct WalManager<Fs: FileSystem = StdFileSystem> {
    dir: PathBuf,
    fs: Fs,
    config: WalConfig,
    active_segment: u64,
    active_offset: u64,
    active_file: Fs::File,
    written_lsn: Lsn,
    durable_lsn: Lsn,
    prev_lsn: Lsn,
    /// Lane BH P1 #7: optional pointer to the coordinator's shared
    /// sync counters; left `None` for raw `WalManager` use (e.g.
    /// recovery scans) and populated when `WalCoordinator::new`
    /// hands ownership of the manager to the writer thread.
    sync_counters: Option<Arc<WalSyncCounters>>,
}

#[derive(Debug)]
pub struct WalCoordinator {
    shared: Arc<WalCoordinatorShared>,
    writer: Mutex<Option<JoinHandle<()>>>,
    config: WalConfig,
    dir: PathBuf,
    /// Lane BH P1 #7: shared counters bumped by the writer thread.
    sync_counters: Arc<WalSyncCounters>,
}

#[derive(Debug)]
struct WalCoordinatorShared {
    state: Mutex<WalCoordinatorState>,
    cvar: Condvar,
    /// Wave 1A-F: optional Phase 11 telemetry sink, installed
    /// post-construction by [`WalCoordinator::set_phase11_counters`].
    /// The writer thread reads this directly (no state-mutex hop) so
    /// it can bump `wal_batch_size_buckets` per fdatasync.
    phase11: std::sync::RwLock<Option<Arc<Phase11Counters>>>,
}

#[derive(Debug)]
struct WalCoordinatorState {
    reserved_lsn: Lsn,
    written_lsn: Lsn,
    prev_lsn: Lsn,
    durable_lsn: Lsn,
    pending: VecDeque<QueuedWalRecord>,
    pending_bytes: usize,
    flush_requested_lsn: Lsn,
    shutdown: bool,
    failure: Option<&'static str>,
}

#[derive(Debug)]
struct QueuedWalRecord {
    append: WalAppend,
    encoded: Vec<u8>,
}

#[derive(Debug)]
pub struct WalReader<Fs: FileSystem = StdFileSystem> {
    dir: PathBuf,
    fs: Fs,
    config: WalConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalScanReport {
    pub records: Vec<WalRecord>,
    pub valid_end_lsn: Lsn,
    pub torn_tail: bool,
}

fn validate_config(config: &WalConfig) -> Result<()> {
    if config.segment_bytes < WAL_HEADER_LEN as u64 {
        return Err(Error::CorruptWal("wal segment size too small"));
    }
    if config.wal_buffer_bytes < WAL_HEADER_LEN {
        return Err(Error::CorruptWal("wal buffer too small"));
    }
    if config.wal_write_batch_bytes == 0 {
        return Err(Error::CorruptWal("wal write batch must be nonzero"));
    }
    Ok(())
}

fn validate_record_position(
    record: &WalRecord,
    segment: u64,
    offset: u64,
    segment_bytes: u64,
) -> Result<()> {
    let expected = (segment - 1)
        .checked_mul(segment_bytes)
        .and_then(|base| base.checked_add(offset))
        .ok_or(Error::CorruptWal("lsn overflow"))?;
    if record.lsn.0 != expected {
        return Err(Error::CorruptWal(
            "record lsn does not match segment position",
        ));
    }
    Ok(())
}

fn segment_for_lsn(lsn: Lsn, segment_bytes: u64) -> u64 {
    lsn.0 / segment_bytes + 1
}

fn offset_for_lsn(lsn: Lsn, segment_bytes: u64) -> u64 {
    lsn.0 % segment_bytes
}

fn segment_path(dir: &Path, segment: u64) -> PathBuf {
    dir.join(format!("{segment:020}{WAL_SEGMENT_EXT}"))
}

fn segment_numbers_on_disk(dir: &Path) -> Result<Vec<u64>> {
    let mut segments = Vec::new();
    if !dir.exists() {
        return Ok(segments);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str()
            && let Some(segment) = parse_segment_name(name)
        {
            segments.push(segment);
        }
    }
    segments.sort_unstable();
    Ok(segments)
}

fn parse_segment_name(name: &str) -> Option<u64> {
    let number = name.strip_suffix(WAL_SEGMENT_EXT)?;
    if number.len() != 20 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

mod coordinator;
mod storage;
