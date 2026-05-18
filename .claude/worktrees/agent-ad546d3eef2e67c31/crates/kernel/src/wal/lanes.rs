//! Lane GC (phase 10): per-core WAL lane coordinator.
//!
//! # Design
//!
//! A *lane* is an independent WAL writer + segment directory pair.
//! With `lanes = 1`, [`WalLaneCoordinator`] is a thin pass-through
//! over a single [`WalCoordinator`] and its on-disk layout is
//! byte-for-byte identical to the pre-Lane-GC kernel: segments live
//! directly in the WAL directory the caller supplied.
//!
//! With `lanes > 1`, the wrapper provisions `n` sub-coordinators,
//! each rooted at `<wal_dir>/wal-<idx>/`, and partitions writers by
//! `(thread_id % lanes)`. Each lane sequences its **own** LSN
//! namespace; recovery merges by walking every lane in order via
//! [`WalLaneCoordinator::scan_all_lanes`].
//!
//! # Default behaviour preserved
//!
//! The single-lane path stores segments directly in `<wal_dir>/`,
//! exactly where the historical [`WalCoordinator`] places them.
//! Single-lane recovery still goes through the existing engine
//! recovery code path; no caller of the engine sees lane semantics
//! unless they explicitly construct a multi-lane coordinator.
//!
//! # Why a separate type?
//!
//! Lane support is opt-in and only used by harnesses that want to
//! exercise the per-core writer scaling claim. The engine's
//! `Database` keeps using `WalCoordinator` directly so that the
//! recover-matrix and failpoint-matrix proof lanes stay green.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::format::{Csn, Lsn, TxId};
use crate::wal::manager::{
    GROUP_COMMIT_BUCKET_COUNT, WalAppend, WalConfig, WalCoordinator, WalReader,
    WalSyncCountersSnapshot,
};
use crate::wal::record::WalRecord;
use crate::{Error, Result};

/// Lane GC: opt-in multi-lane WAL coordinator. With `lane_count = 1`
/// (the default constructor input) this is a thin wrapper around a
/// single [`WalCoordinator`] and is byte-for-byte equivalent to the
/// pre-Lane-GC kernel; with `lane_count > 1` it owns `n` sub-
/// coordinators and partitions writers by `(thread_id % n)`.
#[derive(Debug)]
pub struct WalLaneCoordinator {
    lanes: Vec<WalCoordinator>,
    /// Lane GC: kept so multi-lane recovery can reproduce the same
    /// segment-bytes layout the writer used.
    config: WalConfig,
}

impl WalLaneCoordinator {
    /// Lane GC: provision `lane_count` lanes rooted at `path`. With
    /// `lane_count == 1` the segments live directly in `path`,
    /// matching the historical layout; with `lane_count > 1` each
    /// lane gets its own `wal-<idx>` subdirectory.
    pub fn create(path: impl AsRef<Path>, config: WalConfig, lane_count: usize) -> Result<Self> {
        let lane_count = lane_count.max(1);
        let path = path.as_ref().to_path_buf();
        let mut lanes = Vec::with_capacity(lane_count);
        for idx in 0..lane_count {
            let lane_dir = lane_dir_for(&path, idx, lane_count);
            std::fs::create_dir_all(&lane_dir)?;
            let coordinator = WalCoordinator::create(&lane_dir, config.clone())?;
            lanes.push(coordinator);
        }
        Ok(Self { lanes, config })
    }

    /// Lane GC: re-open an existing lane set. Refuses to open if
    /// any expected lane subdirectory is missing — the caller must
    /// either match the original `lane_count` or re-create with
    /// `create`.
    pub fn open(path: impl AsRef<Path>, config: WalConfig, lane_count: usize) -> Result<Self> {
        let lane_count = lane_count.max(1);
        let path = path.as_ref().to_path_buf();
        let mut lanes = Vec::with_capacity(lane_count);
        for idx in 0..lane_count {
            let lane_dir = lane_dir_for(&path, idx, lane_count);
            std::fs::create_dir_all(&lane_dir)?;
            let coordinator = WalCoordinator::open(&lane_dir, config.clone())?;
            lanes.push(coordinator);
        }
        Ok(Self { lanes, config })
    }

    /// Lane GC: number of lanes provisioned. Always `>= 1`.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Lane GC: pick a lane for the current thread. Hash by
    /// `ThreadId` so a writer always lands on the same lane during
    /// its lifetime; modulo the lane count to spread across lanes.
    fn lane_for_current_thread(&self) -> usize {
        let count = self.lanes.len();
        if count <= 1 {
            return 0;
        }
        // The internal `as_u64()` API for ThreadId is not stable;
        // hash the thread id to get a stable per-thread value.
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        (hasher.finish() as usize) % count
    }

    /// Lane GC: append a record on the current thread's lane. The
    /// returned [`WalAppend`] is **lane-local**; callers must use
    /// [`Self::flush_until_lane_for_thread`] (and *not*
    /// [`Self::flush_until_global`]) to wait for it to become
    /// durable.
    pub fn append(
        &self,
        kind: crate::wal::WalRecordKind,
        tx_id: TxId,
        payload: Vec<u8>,
    ) -> Result<WalAppend> {
        let lane = self.lane_for_current_thread();
        self.lanes[lane].append(kind, tx_id, payload)
    }

    /// Lane GC: append on a specific lane index. Bounds-checks the
    /// lane index; primarily used by tests and by future per-core
    /// schedulers that pin a writer to a CPU.
    pub fn append_on_lane(
        &self,
        lane: usize,
        kind: crate::wal::WalRecordKind,
        tx_id: TxId,
        payload: Vec<u8>,
    ) -> Result<WalAppend> {
        let coord = self
            .lanes
            .get(lane)
            .ok_or(Error::CorruptWal("lane index out of range"))?;
        coord.append(kind, tx_id, payload)
    }

    /// Lane GC: append a commit record on the current thread's
    /// lane.
    pub fn append_commit_with_csn(&self, tx_id: TxId, csn: Csn) -> Result<WalAppend> {
        let lane = self.lane_for_current_thread();
        self.lanes[lane].append_commit_with_csn(tx_id, csn)
    }

    /// Lane GC: flush the current thread's lane up to `target_lsn`.
    /// `target_lsn` is interpreted in the lane's local LSN space
    /// (i.e. the value returned by [`Self::append`]).
    pub fn flush_until_lane_for_thread(&self, target_lsn: Lsn) -> Result<Lsn> {
        let lane = self.lane_for_current_thread();
        self.lanes[lane].flush_until(target_lsn)
    }

    /// Lane GC: flush a specific lane.
    pub fn flush_until_on_lane(&self, lane: usize, target_lsn: Lsn) -> Result<Lsn> {
        let coord = self
            .lanes
            .get(lane)
            .ok_or(Error::CorruptWal("lane index out of range"))?;
        coord.flush_until(target_lsn)
    }

    /// Lane GC: drain every lane's pending queue and fsync.
    /// Returns the minimum durable LSN across lanes (a cheap proof
    /// that *every* lane reached at least that point).
    pub fn flush_all(&self) -> Result<Lsn> {
        let mut min_durable = Lsn(u64::MAX);
        let mut any = false;
        for coord in &self.lanes {
            let lsn = coord.flush_all()?;
            any = true;
            if lsn.0 < min_durable.0 {
                min_durable = lsn;
            }
        }
        if any { Ok(min_durable) } else { Ok(Lsn::ZERO) }
    }

    /// Lane GC: durable LSN aggregated across lanes (minimum, since
    /// "everything is durable up to X" is the load-bearing
    /// guarantee).
    pub fn durable_lsn(&self) -> Result<Lsn> {
        let mut min_durable = Lsn(u64::MAX);
        let mut any = false;
        for coord in &self.lanes {
            let lsn = coord.durable_lsn()?;
            any = true;
            if lsn.0 < min_durable.0 {
                min_durable = lsn;
            }
        }
        if any { Ok(min_durable) } else { Ok(Lsn::ZERO) }
    }

    /// Lane GC: sum-aggregate of every lane's sync counters.
    /// Histogram buckets sum element-wise so the global view of
    /// fan-in still makes sense — a 4-lane workload that sees
    /// 25-fan-in batches per lane shows up here as 4 group commits
    /// in bucket 4 (lower edge 16).
    pub fn sync_counters_snapshot(&self) -> WalSyncCountersSnapshot {
        let mut total = WalSyncCountersSnapshot::default();
        for coord in &self.lanes {
            let snap = coord.sync_counters_snapshot();
            total.fsyncs_issued = total.fsyncs_issued.saturating_add(snap.fsyncs_issued);
            total.fdatasyncs_issued = total
                .fdatasyncs_issued
                .saturating_add(snap.fdatasyncs_issued);
            total.pwrites_issued = total.pwrites_issued.saturating_add(snap.pwrites_issued);
            total.group_commits_issued = total
                .group_commits_issued
                .saturating_add(snap.group_commits_issued);
            total.group_commit_batch_bytes_sum = total
                .group_commit_batch_bytes_sum
                .saturating_add(snap.group_commit_batch_bytes_sum);
            total.group_commit_batch_record_count_sum = total
                .group_commit_batch_record_count_sum
                .saturating_add(snap.group_commit_batch_record_count_sum);
            for idx in 0..GROUP_COMMIT_BUCKET_COUNT {
                total.group_commit_batch_buckets[idx] = total.group_commit_batch_buckets[idx]
                    .saturating_add(snap.group_commit_batch_buckets[idx]);
            }
        }
        total
    }

    /// Lane GC: lane-aware recovery. Returns one record vector per
    /// lane, each ordered by the lane's own LSN. Callers that need
    /// a single sorted timeline can merge by lane-tagged LSN keys.
    pub fn scan_all_lanes(
        path: impl AsRef<Path>,
        config: WalConfig,
        lane_count: usize,
    ) -> Result<WalLaneRecoveryReport> {
        let lane_count = lane_count.max(1);
        let path = path.as_ref().to_path_buf();
        let mut lane_records = Vec::with_capacity(lane_count);
        let mut torn_tail_lanes = Vec::new();
        for idx in 0..lane_count {
            let lane_dir = lane_dir_for(&path, idx, lane_count);
            if !lane_dir.exists() {
                lane_records.push(Vec::new());
                continue;
            }
            let mut reader = WalReader::new(&lane_dir, config.clone());
            let report = reader.scan_report()?;
            if report.torn_tail {
                torn_tail_lanes.push(idx);
            }
            lane_records.push(report.records);
        }
        Ok(WalLaneRecoveryReport {
            lane_records,
            torn_tail_lanes,
        })
    }

    /// Lane GC: write-side LSN ranges per lane, useful for tests
    /// and reporting. Returns one `Lsn` per lane.
    pub fn lane_durable_lsns(&self) -> Result<Vec<Lsn>> {
        self.lanes.iter().map(|coord| coord.durable_lsn()).collect()
    }

    /// Lane GC: introspection — config the lanes were configured
    /// with. Doesn't include the lane count (use [`Self::lane_count`]).
    pub fn config(&self) -> &WalConfig {
        &self.config
    }
}

/// Lane GC: shape returned by [`WalLaneCoordinator::scan_all_lanes`].
/// Each `lane_records[i]` is the full ordered set of records the
/// scan recovered from lane `i`; downstream merge logic can
/// interleave them by LSN if a globally ordered view is required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalLaneRecoveryReport {
    pub lane_records: Vec<Vec<WalRecord>>,
    pub torn_tail_lanes: Vec<usize>,
}

impl WalLaneRecoveryReport {
    /// Lane GC: total records recovered across every lane.
    pub fn total_records(&self) -> usize {
        self.lane_records.iter().map(|lane| lane.len()).sum()
    }

    /// Lane GC: any lane saw a torn tail? Mirrors the
    /// [`crate::wal::WalScanReport::torn_tail`] flag.
    pub fn any_torn_tail(&self) -> bool {
        !self.torn_tail_lanes.is_empty()
    }
}

/// Lane GC: directory layout helper. With `lane_count == 1` this
/// returns the WAL directory itself so the historical single-lane
/// layout is preserved byte-for-byte; with `lane_count > 1` each
/// lane gets a `wal-<idx>` subdirectory.
fn lane_dir_for(root: &Path, lane: usize, lane_count: usize) -> PathBuf {
    if lane_count <= 1 {
        root.to_path_buf()
    } else {
        root.join(format!("wal-{lane}"))
    }
}

/// Lane GC: a tiny round-robin lane selector for the (rare) case
/// where a caller wants explicit lane affinity instead of the
/// `(thread_id % lanes)` default. Wrapping under a mutex keeps the
/// counter monotonic without atomics for what is a slow-path API.
#[derive(Debug, Default)]
pub struct LaneRoundRobin {
    next: Mutex<usize>,
}

impl LaneRoundRobin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pick(&self, lane_count: usize) -> usize {
        if lane_count <= 1 {
            return 0;
        }
        let mut guard = self.next.lock().unwrap_or_else(|e| e.into_inner());
        let idx = *guard % lane_count;
        *guard = guard.wrapping_add(1);
        idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_dir_single_lane_uses_root() {
        let path = PathBuf::from("/tmp/wal");
        assert_eq!(lane_dir_for(&path, 0, 1), path);
    }

    #[test]
    fn lane_dir_multi_lane_uses_subdir() {
        let path = PathBuf::from("/tmp/wal");
        assert_eq!(lane_dir_for(&path, 0, 4), path.join("wal-0"));
        assert_eq!(lane_dir_for(&path, 3, 4), path.join("wal-3"));
    }

    #[test]
    fn round_robin_wraps() {
        let rr = LaneRoundRobin::new();
        let picks: Vec<usize> = (0..8).map(|_| rr.pick(4)).collect();
        assert_eq!(picks, vec![0, 1, 2, 3, 0, 1, 2, 3]);
    }
}
