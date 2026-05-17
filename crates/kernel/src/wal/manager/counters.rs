use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Lane GC: number of power-of-two buckets in the group-commit
/// histogram. 16 buckets cover singleton commits up through
/// `>= 32768` waiters, which is more than enough headroom for any
/// realistic concurrent workload.
pub const GROUP_COMMIT_BUCKET_COUNT: usize = 16;

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
        // Quantile beyond the populated tail: report the top bucket's lower edge.
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

    pub(super) fn bump_fdatasync(&self) {
        self.fdatasyncs_issued.fetch_add(1, AtomicOrdering::Relaxed);
    }

    pub(super) fn bump_pwrite(&self) {
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
    pub fn record_group_commit(&self, record_count: u64, byte_count: u64) {
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
