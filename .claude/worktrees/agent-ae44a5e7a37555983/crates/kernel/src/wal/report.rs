use super::*;

use crate::wal::WalReader;
use crate::wal::record::WalRecord;

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

impl WalLaneCoordinator {
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
            let lane_dir = super::lane_dir_for(&path, idx, lane_count);
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

/// Lane GC: a tiny round-robin lane selector for the (rare) case
/// where a caller wants explicit lane affinity instead of the
/// `(thread_id % lanes)` default. Wrapping under a mutex keeps the
/// counter monotonic without atomics for what is a slow-path API.
#[derive(Debug, Default)]
pub struct LaneRoundRobin {
    next: std::sync::Mutex<usize>,
}

impl LaneRoundRobin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pick(&self, lane_count: usize) -> usize {
        if lane_count <= 1 {
            return 0;
        }
        let mut guard = match self.next.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
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
        assert_eq!(super::lane_dir_for(&path, 0, 1), path);
    }

    #[test]
    fn lane_dir_multi_lane_uses_subdir() {
        let path = PathBuf::from("/tmp/wal");
        assert_eq!(super::lane_dir_for(&path, 0, 4), path.join("wal-0"));
        assert_eq!(super::lane_dir_for(&path, 3, 4), path.join("wal-3"));
    }

    #[test]
    fn round_robin_wraps() {
        let rr = LaneRoundRobin::new();
        let picks: Vec<usize> = (0..8).map(|_| rr.pick(4)).collect();
        assert_eq!(picks, vec![0, 1, 2, 3, 0, 1, 2, 3]);
    }
}
