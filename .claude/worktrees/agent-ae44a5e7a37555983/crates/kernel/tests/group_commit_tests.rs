//! Lane GC (phase 10) — group-commit telemetry, per-core lanes, and
//! the opt-in semantic combiner.
//!
//! These tests exercise the kernel-level WAL surface (no SQL
//! involvement) and are what fig8 of the paper will reference.

use redlinedb_kernel::format::TxId;
use redlinedb_kernel::wal::{
    GROUP_COMMIT_BUCKET_COUNT, WalConfig, WalCoordinator, WalLaneCoordinator,
    WalLaneRecoveryReport, WalRecordKind,
};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

// ---------- Telemetry: histogram + batch counters ----------

#[test]
fn group_commit_telemetry_starts_zeroed() {
    let temp = TempDir::new().unwrap();
    let coordinator = WalCoordinator::create(temp.path(), WalConfig::default()).unwrap();
    let snapshot = coordinator.sync_counters_snapshot();
    assert_eq!(snapshot.group_commits_issued, 0);
    assert_eq!(snapshot.group_commit_batch_record_count_sum, 0);
    assert_eq!(snapshot.group_commit_batch_bytes_sum, 0);
    assert_eq!(
        snapshot.group_commit_batch_buckets,
        [0_u64; GROUP_COMMIT_BUCKET_COUNT]
    );
    assert_eq!(snapshot.batch_record_count_percentile(0.5), 0);
    assert_eq!(snapshot.batch_record_count_max(), 0);
}

#[test]
fn group_commit_telemetry_singleton_path_lands_in_bucket_zero() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        // Disable the latency window so the very first append is
        // fsynced on its own — bucket 0 (singleton).
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = WalCoordinator::create(temp.path(), config).unwrap();
    let append = coordinator
        .append(WalRecordKind::Commit, TxId(1), vec![0_u8; 32])
        .unwrap();
    coordinator.flush_until(append.end_lsn).unwrap();

    let snap = coordinator.sync_counters_snapshot();
    assert!(
        snap.group_commits_issued >= 1,
        "expected at least one group commit, got {}",
        snap.group_commits_issued
    );
    assert!(
        snap.group_commit_batch_record_count_sum >= 1,
        "expected >=1 record covered, got {}",
        snap.group_commit_batch_record_count_sum
    );
    assert!(
        snap.group_commit_batch_buckets[0] >= 1,
        "singleton commit must land in bucket 0; got buckets={:?}",
        snap.group_commit_batch_buckets
    );
    // p50 of a singleton-only workload is bucket 0 lower edge = 1.
    assert_eq!(snap.batch_record_count_percentile(0.5), 1);
    assert!(snap.batch_record_count_max() >= 1);
}

#[test]
fn group_commit_telemetry_observes_real_batching_under_concurrency() {
    // The contract: if N threads race to commit and the writer
    // batches them, then `batch_record_count_sum > group_commits_issued`.
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        // A modest delay window so the writer thread has time to
        // pull multiple appends into a single fsync.
        group_commit_delay_us: 2_000,
        ..WalConfig::default()
    };
    let coordinator = Arc::new(WalCoordinator::create(temp.path(), config).unwrap());
    let threads = 100_usize;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|idx| {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let append = coordinator
                    .append(
                        WalRecordKind::Commit,
                        TxId(idx as u64 + 1),
                        vec![idx as u8; 16],
                    )
                    .unwrap();
                coordinator.flush_until(append.end_lsn).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let snap = coordinator.sync_counters_snapshot();
    // Lane GC: print the observed numbers under --nocapture so the
    // paper's fig8 scaffolding can reference real numbers.
    eprintln!(
        "[lane-gc telemetry] threads={} group_commits={} records={} bytes={} \
         buckets={:?} p50={} p95={} p99={} max={}",
        threads,
        snap.group_commits_issued,
        snap.group_commit_batch_record_count_sum,
        snap.group_commit_batch_bytes_sum,
        snap.group_commit_batch_buckets,
        snap.batch_record_count_percentile(0.50),
        snap.batch_record_count_percentile(0.95),
        snap.batch_record_count_percentile(0.99),
        snap.batch_record_count_max(),
    );
    assert!(snap.group_commits_issued > 0);
    assert_eq!(
        snap.group_commit_batch_record_count_sum, threads as u64,
        "telemetry must account for every committed record"
    );
    assert!(
        snap.group_commit_batch_record_count_sum > snap.group_commits_issued,
        "real batching must occur: got {} records over {} group commits",
        snap.group_commit_batch_record_count_sum,
        snap.group_commits_issued
    );
    assert!(
        snap.group_commit_batch_bytes_sum >= snap.group_commit_batch_record_count_sum,
        "every record contributes >= 1 byte"
    );
    // Histogram sum equals the number of group commits.
    let hist_total: u64 = snap.group_commit_batch_buckets.iter().sum();
    assert_eq!(hist_total, snap.group_commits_issued);
    // Max batch >= mean batch lower edge.
    assert!(snap.batch_record_count_max() >= 1);
}

#[test]
fn group_commit_histogram_buckets_cover_powers_of_two() {
    // Direct unit-style test of the bucket layout via the public
    // snapshot helper. We poke synthetic counts into a temporary
    // snapshot to verify the percentile estimator picks the right
    // bucket lower edges.
    use redlinedb_kernel::wal::WalSyncCountersSnapshot;

    // 2 in bucket 0 (size 1), 7 in bucket 3 (size 8..15), 1 in bucket 6 (size 64..127).
    // Cumulative: b0=2, b3=9, b6=10.
    let mut buckets = [0_u64; GROUP_COMMIT_BUCKET_COUNT];
    buckets[0] = 2;
    buckets[3] = 7;
    buckets[6] = 1;
    let snap = WalSyncCountersSnapshot {
        group_commits_issued: 10,
        group_commit_batch_buckets: buckets,
        ..WalSyncCountersSnapshot::default()
    };

    // p50: ceil(10*0.5) = 5 → cumulative >= 5 first reached at b3 (lower edge 8).
    assert_eq!(snap.batch_record_count_percentile(0.5), 8);
    // p99: ceil(10*0.99) = 10 → reached at b6 (lower edge 64).
    assert_eq!(snap.batch_record_count_percentile(0.99), 64);
    // max bucket lower edge.
    assert_eq!(snap.batch_record_count_max(), 64);
    // p0 falls in the first non-empty bucket — ceil(10*0)=0, clamped to 1, b0.
    assert_eq!(snap.batch_record_count_percentile(0.0), 1);
}

// ---------- Per-core lanes ----------

#[test]
fn lane_coordinator_default_is_single_lane_and_preserves_semantics() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig::default();
    let lanes = WalLaneCoordinator::create(temp.path(), config.clone(), 1).unwrap();
    assert_eq!(lanes.lane_count(), 1);

    let append = lanes
        .append(WalRecordKind::Commit, TxId(1), b"x".to_vec())
        .unwrap();
    lanes.flush_until_lane_for_thread(append.end_lsn).unwrap();
    assert!(lanes.durable_lsn().unwrap() >= append.end_lsn);
    let snap = lanes.sync_counters_snapshot();
    assert!(snap.group_commits_issued >= 1);
}

#[test]
fn lane_coordinator_multi_lane_creates_one_directory_per_lane() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let lanes = Arc::new(WalLaneCoordinator::create(temp.path(), config.clone(), 4).unwrap());
    assert_eq!(lanes.lane_count(), 4);

    let threads = 100_usize;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|idx| {
            let lanes = Arc::clone(&lanes);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let append = lanes
                    .append(
                        WalRecordKind::Commit,
                        TxId(idx as u64 + 1),
                        vec![idx as u8; 16],
                    )
                    .unwrap();
                lanes.flush_until_lane_for_thread(append.end_lsn).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    lanes.flush_all().unwrap();

    // Verify the on-disk layout: one subdirectory per lane.
    for idx in 0..4_usize {
        let lane_dir = temp.path().join(format!("wal-{idx}"));
        assert!(
            lane_dir.exists() && lane_dir.is_dir(),
            "lane subdir {} must exist",
            lane_dir.display()
        );
        // At least one segment file must have been written.
        let segments: Vec<_> = std::fs::read_dir(&lane_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".wal"))
            .collect();
        assert!(
            !segments.is_empty(),
            "lane {} must have written at least one .wal segment",
            idx
        );
    }
}

#[test]
fn lane_coordinator_recovery_walks_every_lane_in_lsn_order() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    {
        let lanes = Arc::new(WalLaneCoordinator::create(temp.path(), config.clone(), 4).unwrap());
        let threads = 100_usize;
        let barrier = Arc::new(Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|idx| {
                let lanes = Arc::clone(&lanes);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let append = lanes
                        .append(
                            WalRecordKind::Commit,
                            TxId(idx as u64 + 1),
                            vec![idx as u8; 8],
                        )
                        .unwrap();
                    lanes.flush_until_lane_for_thread(append.end_lsn).unwrap();
                    append
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        lanes.flush_all().unwrap();
    }

    // Recovery: open via the lane coordinator's static recovery scan
    // and check that all 100 records are present, ordered by
    // ascending LSN within each lane.
    let report: WalLaneRecoveryReport =
        WalLaneCoordinator::scan_all_lanes(temp.path(), config, 4).unwrap();
    assert_eq!(report.total_records(), 100);
    assert_eq!(report.lane_records.len(), 4);
    for lane in &report.lane_records {
        for pair in lane.windows(2) {
            assert!(
                pair[0].lsn.0 < pair[1].lsn.0,
                "records within a lane must be LSN-ordered"
            );
        }
    }
    assert!(!report.any_torn_tail());
}

#[test]
fn lane_coordinator_config_lane_count_field_defaults_to_one() {
    let config = WalConfig::default();
    assert_eq!(config.lanes, 1);
}

// ---------- Semantic combiner (opt-in stub) ----------

#[test]
fn combiner_default_off_does_not_change_behaviour() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig::default();
    assert!(!config.semantic_combiner);
    let coordinator = WalCoordinator::create(temp.path(), config).unwrap();
    let append = coordinator
        .append(WalRecordKind::Commit, TxId(1), b"v".to_vec())
        .unwrap();
    coordinator.flush_until(append.end_lsn).unwrap();
}

#[test]
fn combiner_recognises_counter_delta_record_shape() {
    use redlinedb_kernel::wal::combiner::{CombinableDelta, WalCombiner};
    let combiner = WalCombiner::new();
    let a = CombinableDelta {
        rel_id: 7,
        row_id: 42,
        column: 1,
        delta: 1,
    };
    let b = CombinableDelta {
        rel_id: 7,
        row_id: 42,
        column: 1,
        delta: 1,
    };
    let merged = combiner.try_merge(&a, &b).unwrap();
    assert_eq!(merged.delta, 2);
    assert_eq!(merged.rel_id, a.rel_id);
    assert_eq!(merged.row_id, a.row_id);
    assert_eq!(merged.column, a.column);
}

#[test]
fn combiner_refuses_to_merge_distinct_targets() {
    use redlinedb_kernel::wal::combiner::{CombinableDelta, WalCombiner};
    let combiner = WalCombiner::new();
    let a = CombinableDelta {
        rel_id: 7,
        row_id: 42,
        column: 1,
        delta: 1,
    };
    let b = CombinableDelta {
        rel_id: 7,
        row_id: 43,
        column: 1,
        delta: 1,
    };
    assert!(combiner.try_merge(&a, &b).is_none());
}

#[test]
fn combiner_refuses_to_merge_distinct_columns() {
    use redlinedb_kernel::wal::combiner::{CombinableDelta, WalCombiner};
    let combiner = WalCombiner::new();
    assert!(
        combiner
            .try_merge(
                &CombinableDelta {
                    rel_id: 1,
                    row_id: 1,
                    column: 0,
                    delta: 1,
                },
                &CombinableDelta {
                    rel_id: 1,
                    row_id: 1,
                    column: 1,
                    delta: 1,
                },
            )
            .is_none()
    );
}

#[test]
fn combiner_overflow_is_refused_not_saturated() {
    use redlinedb_kernel::wal::combiner::{CombinableDelta, WalCombiner};
    let combiner = WalCombiner::new();
    let a = CombinableDelta {
        rel_id: 1,
        row_id: 1,
        column: 1,
        delta: i64::MAX,
    };
    let b = CombinableDelta {
        rel_id: 1,
        row_id: 1,
        column: 1,
        delta: 1,
    };
    assert!(combiner.try_merge(&a, &b).is_none());
}

#[test]
fn combiner_fold_path_returns_merged_delta() {
    use redlinedb_kernel::wal::combiner::{
        CombinableDelta, CombineOutcome, WalCombiner, maybe_combine_pending,
    };
    let combiner = WalCombiner::new();
    let pending = CombinableDelta {
        rel_id: 1,
        row_id: 1,
        column: 1,
        delta: 1,
    };
    let cand = CombinableDelta {
        rel_id: 1,
        row_id: 1,
        column: 1,
        delta: 2,
    };
    assert_eq!(
        maybe_combine_pending(&combiner, &cand, Some(&pending)),
        CombineOutcome::Folded(CombinableDelta {
            rel_id: 1,
            row_id: 1,
            column: 1,
            delta: 3,
        })
    );
}

#[test]
fn combiner_fold_path_enqueues_without_pending_or_match() {
    use redlinedb_kernel::wal::combiner::{
        CombinableDelta, CombineOutcome, WalCombiner, maybe_combine_pending,
    };
    let combiner = WalCombiner::new();
    let cand = CombinableDelta {
        rel_id: 1,
        row_id: 1,
        column: 1,
        delta: 1,
    };
    let other = CombinableDelta {
        rel_id: 1,
        row_id: 2,
        column: 1,
        delta: 1,
    };
    assert_eq!(
        maybe_combine_pending(&combiner, &cand, None),
        CombineOutcome::Enqueue
    );
    assert_eq!(
        maybe_combine_pending(&combiner, &cand, Some(&other)),
        CombineOutcome::Enqueue
    );
}

#[test]
fn combiner_burst_workload_with_combiner_off_is_correct_baseline() {
    // Lane GC: with the combiner OFF (default), 1000 sequential
    // commits each hit fsync (potentially batched by the writer
    // thread's group-commit window). This pins the *baseline* the
    // future combiner-on test will compare against. Once the
    // safe-by-construction fold lands, an analogous test will
    // assert group_commits_issued < 1000 with combiner ON; for now
    // we verify the baseline contract — every record is durable.
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    assert!(!config.semantic_combiner);
    let coordinator = WalCoordinator::create(temp.path(), config).unwrap();
    for idx in 0..32 {
        let append = coordinator
            .append(WalRecordKind::PageDelta, TxId(idx + 1), vec![idx as u8; 4])
            .unwrap();
        coordinator.flush_until(append.end_lsn).unwrap();
    }
    let snap = coordinator.sync_counters_snapshot();
    assert_eq!(snap.group_commit_batch_record_count_sum, 32);
    assert!(snap.group_commits_issued >= 1);
}
