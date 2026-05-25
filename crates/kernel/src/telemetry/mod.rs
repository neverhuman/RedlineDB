//! Phase 11 structural telemetry counters.
//!
//! Wave 0 Worker F lays down the *plumbing* for the Phase 11 counter
//! surface without wiring any emission sites. The counters are
//! addressable from the engine, exposed via a relaxed-atomic snapshot,
//! and bubbled through `Database::benchmark_stats` so the bench
//! manifest schema is forward-compatible. Subsequent waves will add
//! the `.fetch_add` sites at the leaf-visit / prefetch / heap-recheck
//! / cursor-batch / WAL-batch / lock-wait points.
//!
//! Style mirrors `crate::wal::manager::WalSyncCounters` — relaxed
//! atomics, no global state, snapshot type carries plain `u64`s plus
//! 16-bucket power-of-two histograms identical in shape to
//! `group_commit_batch_buckets`.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use serde::{Deserialize, Serialize};

/// Number of power-of-two buckets in each Phase 11 histogram. Matches
/// the wal coordinator's `GROUP_COMMIT_BUCKET_COUNT` (`16`) so paper
/// figs that already understand the existing bucketing can be reused.
pub const PHASE11_HISTOGRAM_BUCKETS: usize = 16;

/// Engine-level aggregator for Phase 11 telemetry counters.
///
/// The counter surface is additive. Some fields are wired by the
/// index, buffer-pool, lock, WAL, and SQL executor hot paths; fields
/// without an emission site stay at zero until their owning lane lands.
///
/// All fields use `AtomicU64` with `Ordering::Relaxed` to match the
/// existing `WalSyncCounters` style — counters are sampled cheaply by
/// the bench harness and do not need synchronisation with any other
/// memory state.
#[derive(Debug)]
pub struct Phase11Counters {
    /// Bumped each time an executor visits a B-tree leaf page.
    pub leaf_visits: AtomicU64,
    /// Bumped on a buffer-pool prefetch that found the target page
    /// already resident.
    pub prefetch_hits: AtomicU64,
    /// Bumped on a prefetch that issued an actual page read.
    pub prefetch_misses: AtomicU64,
    /// Bumped when the background prefetch worker queue is full and a
    /// hint is dropped on the floor. WS-C4: the prefetch path is now
    /// async — see `BufferPool::try_prefetch`.
    pub prefetch_dropped: AtomicU64,
    /// Bumped each time a heap visibility recheck has to rerun
    /// because a tuple version moved underneath the cursor.
    pub heap_rechecks: AtomicU64,
    /// Bumped each time a cursor emits a batch of rows up to its
    /// caller (one bump per batch, not per row).
    pub cursor_batches_emitted: AtomicU64,
    /// Bumped once when an ORDER BY/LIMIT query uses the ordered
    /// index cursor shortcut.
    pub ordered_limit_path_hits: AtomicU64,
    /// Sum of rows returned by ordered LIMIT shortcuts after heap
    /// visibility rechecks and LIMIT/OFFSET early-stop.
    pub ordered_limit_rows_returned: AtomicU64,
    /// Nanoseconds spent waiting to acquire the index structure mutex.
    pub index_structure_lock_wait_ns: AtomicU64,
    /// Nanoseconds spent holding the index structure mutex.
    pub index_structure_lock_hold_ns: AtomicU64,
    /// Number of index structure mutex acquisitions.
    pub index_structure_lock_acquires: AtomicU64,
    /// Number of index structure mutex acquisitions that observed contention.
    pub index_structure_lock_contentions: AtomicU64,
    /// Number of B-tree leaf splits performed by the index layer.
    pub index_leaf_splits: AtomicU64,
    /// Number of high-key/right-link move-right steps during index descent.
    pub index_move_rights: AtomicU64,
    /// Number of non-empty raw point cursor batches emitted.
    pub index_raw_point_batches: AtomicU64,
    /// Number of non-empty raw range cursor batches emitted.
    pub index_raw_range_batches: AtomicU64,
    /// 16 power-of-two buckets indexed by
    /// `floor(log2(record_count))` saturated at 15. Bucket k covers
    /// `[2^k, 2^(k+1))` records (bucket 0 holds singleton WAL
    /// flushes, bucket 15 holds anything `>= 32768`). Same shape as
    /// `WalSyncCounters::group_commit_batch_buckets`.
    pub wal_batch_size_buckets: [AtomicU64; PHASE11_HISTOGRAM_BUCKETS],
    /// 16 power-of-two buckets of lock-wait durations, in
    /// microseconds. Bucket k covers `[2^k, 2^(k+1)) us`. Bucket 0
    /// catches sub-microsecond waits, bucket 15 saturates at
    /// `>= 32768 us` (~32 ms).
    pub lock_wait_us_buckets: [AtomicU64; PHASE11_HISTOGRAM_BUCKETS],
}

impl Default for Phase11Counters {
    fn default() -> Self {
        Self {
            leaf_visits: AtomicU64::new(0),
            prefetch_hits: AtomicU64::new(0),
            prefetch_misses: AtomicU64::new(0),
            prefetch_dropped: AtomicU64::new(0),
            heap_rechecks: AtomicU64::new(0),
            cursor_batches_emitted: AtomicU64::new(0),
            ordered_limit_path_hits: AtomicU64::new(0),
            ordered_limit_rows_returned: AtomicU64::new(0),
            index_structure_lock_wait_ns: AtomicU64::new(0),
            index_structure_lock_hold_ns: AtomicU64::new(0),
            index_structure_lock_acquires: AtomicU64::new(0),
            index_structure_lock_contentions: AtomicU64::new(0),
            index_leaf_splits: AtomicU64::new(0),
            index_move_rights: AtomicU64::new(0),
            index_raw_point_batches: AtomicU64::new(0),
            index_raw_range_batches: AtomicU64::new(0),
            // AtomicU64 is not Copy so the array literal must use
            // `from_fn`. 16 elements, fixed at compile time.
            wal_batch_size_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            lock_wait_us_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl Phase11Counters {
    /// Allocate a fresh counter aggregator with every field at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Capture a relaxed snapshot of every counter. Safe to call from
    /// any thread without holding a mutex.
    pub fn snapshot(&self) -> Phase11CountersSnapshot {
        let mut wal_batch = [0_u64; PHASE11_HISTOGRAM_BUCKETS];
        for (idx, slot) in self.wal_batch_size_buckets.iter().enumerate() {
            wal_batch[idx] = slot.load(Relaxed);
        }
        let mut lock_wait = [0_u64; PHASE11_HISTOGRAM_BUCKETS];
        for (idx, slot) in self.lock_wait_us_buckets.iter().enumerate() {
            lock_wait[idx] = slot.load(Relaxed);
        }
        Phase11CountersSnapshot {
            leaf_visits: self.leaf_visits.load(Relaxed),
            prefetch_hits: self.prefetch_hits.load(Relaxed),
            prefetch_misses: self.prefetch_misses.load(Relaxed),
            prefetch_dropped: self.prefetch_dropped.load(Relaxed),
            heap_rechecks: self.heap_rechecks.load(Relaxed),
            cursor_batches_emitted: self.cursor_batches_emitted.load(Relaxed),
            ordered_limit_path_hits: self.ordered_limit_path_hits.load(Relaxed),
            ordered_limit_rows_returned: self.ordered_limit_rows_returned.load(Relaxed),
            index_structure_lock_wait_ns: self.index_structure_lock_wait_ns.load(Relaxed),
            index_structure_lock_hold_ns: self.index_structure_lock_hold_ns.load(Relaxed),
            index_structure_lock_acquires: self.index_structure_lock_acquires.load(Relaxed),
            index_structure_lock_contentions: self.index_structure_lock_contentions.load(Relaxed),
            index_leaf_splits: self.index_leaf_splits.load(Relaxed),
            index_move_rights: self.index_move_rights.load(Relaxed),
            index_raw_point_batches: self.index_raw_point_batches.load(Relaxed),
            index_raw_range_batches: self.index_raw_range_batches.load(Relaxed),
            wal_batch_size_buckets: wal_batch,
            lock_wait_us_buckets: lock_wait,
        }
    }

    /// Zero every counter. Useful for separating warmup vs measured
    /// phases inside a single bench process — the harness can reset
    /// after warmup so the steady-state numbers do not include
    /// startup noise. No-op effect on correctness either way.
    pub fn reset(&self) {
        self.leaf_visits.store(0, Relaxed);
        self.prefetch_hits.store(0, Relaxed);
        self.prefetch_misses.store(0, Relaxed);
        self.prefetch_dropped.store(0, Relaxed);
        self.heap_rechecks.store(0, Relaxed);
        self.cursor_batches_emitted.store(0, Relaxed);
        self.ordered_limit_path_hits.store(0, Relaxed);
        self.ordered_limit_rows_returned.store(0, Relaxed);
        self.index_structure_lock_wait_ns.store(0, Relaxed);
        self.index_structure_lock_hold_ns.store(0, Relaxed);
        self.index_structure_lock_acquires.store(0, Relaxed);
        self.index_structure_lock_contentions.store(0, Relaxed);
        self.index_leaf_splits.store(0, Relaxed);
        self.index_move_rights.store(0, Relaxed);
        self.index_raw_point_batches.store(0, Relaxed);
        self.index_raw_range_batches.store(0, Relaxed);
        for slot in self.wal_batch_size_buckets.iter() {
            slot.store(0, Relaxed);
        }
        for slot in self.lock_wait_us_buckets.iter() {
            slot.store(0, Relaxed);
        }
    }
}

/// Plain-`u64` snapshot of [`Phase11Counters`]. Carries serde derives
/// so the bench manifest can serialise it directly through
/// `serde_json::to_value`. The schema is **additive** — fields use
/// `#[serde(default)]` so manifests written by older binaries keep
/// deserialising once new buckets/fields land in later waves.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Phase11CountersSnapshot {
    #[serde(default)]
    pub leaf_visits: u64,
    #[serde(default)]
    pub prefetch_hits: u64,
    #[serde(default)]
    pub prefetch_misses: u64,
    #[serde(default)]
    pub prefetch_dropped: u64,
    #[serde(default)]
    pub heap_rechecks: u64,
    #[serde(default)]
    pub cursor_batches_emitted: u64,
    #[serde(default)]
    pub ordered_limit_path_hits: u64,
    #[serde(default)]
    pub ordered_limit_rows_returned: u64,
    #[serde(default)]
    pub index_structure_lock_wait_ns: u64,
    #[serde(default)]
    pub index_structure_lock_hold_ns: u64,
    #[serde(default)]
    pub index_structure_lock_acquires: u64,
    #[serde(default)]
    pub index_structure_lock_contentions: u64,
    #[serde(default)]
    pub index_leaf_splits: u64,
    #[serde(default)]
    pub index_move_rights: u64,
    #[serde(default)]
    pub index_raw_point_batches: u64,
    #[serde(default)]
    pub index_raw_range_batches: u64,
    /// 16 power-of-two buckets. See
    /// [`Phase11Counters::wal_batch_size_buckets`] for the shape.
    #[serde(default = "default_phase11_histogram")]
    pub wal_batch_size_buckets: [u64; PHASE11_HISTOGRAM_BUCKETS],
    /// 16 power-of-two buckets in microseconds. See
    /// [`Phase11Counters::lock_wait_us_buckets`] for the shape.
    #[serde(default = "default_phase11_histogram")]
    pub lock_wait_us_buckets: [u64; PHASE11_HISTOGRAM_BUCKETS],
}

/// Helper for `#[serde(default)]` on the histogram arrays — `Default`
/// for `[u64; 16]` already produces zeros, but the closure form keeps
/// the call site explicit and lets us swap the default if we ever
/// change the bucket count.
fn default_phase11_histogram() -> [u64; PHASE11_HISTOGRAM_BUCKETS] {
    [0_u64; PHASE11_HISTOGRAM_BUCKETS]
}

/// Pick the histogram bucket for a positive count. Bucket 0 catches
/// `<= 1`; for `value >= 2` the bucket is `bits(value - 1)` (i.e.
/// `ceil(log2(value))`), saturating at the top bucket. Mirrors the
/// existing `group_commit_bucket_index` helper in
/// `crate::wal::manager` — kept as a free fn here so future emission
/// sites can call it without depending on wal-internal helpers.
#[inline]
pub fn phase11_bucket_index(value: u64) -> usize {
    if value <= 1 {
        return 0;
    }
    // floor(log2(value)) for value >= 2 fits in usize.
    let leading = (value - 1).leading_zeros();
    let bits = u64::BITS - leading;
    let bucket = bits as usize;
    if bucket >= PHASE11_HISTOGRAM_BUCKETS {
        PHASE11_HISTOGRAM_BUCKETS - 1
    } else {
        bucket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_zero() {
        let counters = Phase11Counters::new();
        let snap = counters.snapshot();
        assert_eq!(snap.leaf_visits, 0);
        assert_eq!(snap.prefetch_hits, 0);
        assert_eq!(snap.prefetch_misses, 0);
        assert_eq!(snap.prefetch_dropped, 0);
        assert_eq!(snap.heap_rechecks, 0);
        assert_eq!(snap.cursor_batches_emitted, 0);
        assert_eq!(snap.ordered_limit_path_hits, 0);
        assert_eq!(snap.ordered_limit_rows_returned, 0);
        assert_eq!(snap.index_structure_lock_wait_ns, 0);
        assert_eq!(snap.index_structure_lock_hold_ns, 0);
        assert_eq!(snap.index_structure_lock_acquires, 0);
        assert_eq!(snap.index_structure_lock_contentions, 0);
        assert_eq!(snap.index_leaf_splits, 0);
        assert_eq!(snap.index_move_rights, 0);
        assert_eq!(snap.index_raw_point_batches, 0);
        assert_eq!(snap.index_raw_range_batches, 0);
        assert!(snap.wal_batch_size_buckets.iter().all(|&v| v == 0));
        assert!(snap.lock_wait_us_buckets.iter().all(|&v| v == 0));
    }

    #[test]
    fn snapshot_round_trips_through_serde_json() {
        let counters = Phase11Counters::new();
        let snap = counters.snapshot();
        let json = serde_json::to_value(snap).expect("serialise");
        let back: Phase11CountersSnapshot = serde_json::from_value(json).expect("deserialise");
        assert_eq!(snap, back);
    }

    #[test]
    fn bucket_index_matches_wal_shape() {
        // Singletons (and zero) land in bucket 0.
        assert_eq!(phase11_bucket_index(0), 0);
        assert_eq!(phase11_bucket_index(1), 0);
        // Bucket k for value v >= 2 is `bits(v - 1)` —
        // matches `wal::manager::group_commit_bucket_index`.
        assert_eq!(phase11_bucket_index(2), 1);
        assert_eq!(phase11_bucket_index(3), 2);
        assert_eq!(phase11_bucket_index(4), 2);
        assert_eq!(phase11_bucket_index(5), 3);
        assert_eq!(phase11_bucket_index(8), 3);
        assert_eq!(phase11_bucket_index(9), 4);
        // Top bucket saturates.
        assert_eq!(
            phase11_bucket_index(1_u64 << 40),
            PHASE11_HISTOGRAM_BUCKETS - 1
        );
    }

    #[test]
    fn reset_zeroes_every_counter() {
        let counters = Phase11Counters::new();
        // Touch every field via Relaxed stores so reset is a real
        // round-trip; emission sites are still elsewhere — this is
        // a pure unit-test exercise.
        counters.leaf_visits.store(7, Relaxed);
        counters.prefetch_hits.store(8, Relaxed);
        counters.prefetch_misses.store(9, Relaxed);
        counters.prefetch_dropped.store(99, Relaxed);
        counters.heap_rechecks.store(10, Relaxed);
        counters.cursor_batches_emitted.store(11, Relaxed);
        counters.ordered_limit_path_hits.store(12, Relaxed);
        counters.ordered_limit_rows_returned.store(13, Relaxed);
        counters.index_structure_lock_wait_ns.store(14, Relaxed);
        counters.index_structure_lock_hold_ns.store(15, Relaxed);
        counters.index_structure_lock_acquires.store(16, Relaxed);
        counters.index_structure_lock_contentions.store(17, Relaxed);
        counters.index_leaf_splits.store(18, Relaxed);
        counters.index_move_rights.store(19, Relaxed);
        counters.index_raw_point_batches.store(20, Relaxed);
        counters.index_raw_range_batches.store(21, Relaxed);
        counters.wal_batch_size_buckets[3].store(12, Relaxed);
        counters.lock_wait_us_buckets[5].store(13, Relaxed);
        counters.reset();
        let snap = counters.snapshot();
        assert_eq!(snap, Phase11CountersSnapshot::default());
    }
}
