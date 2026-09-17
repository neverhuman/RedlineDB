//! WS-C6: integration tests for the cross-lane WAL flush coalescer.
//!
//! Gated on `wal_cross_lane_coalescer` so the default kernel test
//! invocation skips it. Run with `cargo test -p redlinedb-kernel
//! --features wal_cross_lane_coalescer`.

#![cfg(feature = "wal_cross_lane_coalescer")]

use std::sync::{Arc, Barrier};
use std::thread;

use redlinedb_kernel::format::TxId;
use redlinedb_kernel::wal::{WalConfig, WalLaneCoordinator, WalRecordKind};

use tempfile::TempDir;

const LANES: usize = 4;
const THREADS_PER_LANE: usize = 4;
const TOTAL_THREADS: usize = LANES * THREADS_PER_LANE;
const PER_THREAD_RECORDS: usize = 100;

fn config() -> WalConfig {
    WalConfig {
        segment_bytes: 1 << 20,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    }
}

#[test]
fn coalescer_reduces_fdatasync_count() {
    let temp = TempDir::new().unwrap();
    let lanes = Arc::new(WalLaneCoordinator::create(temp.path(), config(), LANES).unwrap());
    assert!(
        lanes.coalescer_active(),
        "coalescer must be active with feature flag + multi-lane",
    );

    // Pin THREADS_PER_LANE threads to each lane via `append_on_lane`
    // so multiple concurrent commit-waiters share a lane — exactly
    // the workload the coalescer is meant to batch.
    let barrier = Arc::new(Barrier::new(TOTAL_THREADS));
    let mut handles = Vec::with_capacity(TOTAL_THREADS);
    for tidx in 0..TOTAL_THREADS {
        let lane = tidx % LANES;
        let lanes = Arc::clone(&lanes);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for r in 0..PER_THREAD_RECORDS {
                let tx = TxId((tidx * PER_THREAD_RECORDS + r) as u64 + 1);
                let append = lanes
                    .append_on_lane(lane, WalRecordKind::Commit, tx, vec![tidx as u8; 24])
                    .expect("append");
                lanes
                    .flush_until_on_lane(lane, append.end_lsn)
                    .expect("flush");
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    lanes.flush_all().expect("final flush_all");

    let total = TOTAL_THREADS * PER_THREAD_RECORDS;
    let snap = lanes.sync_counters_snapshot();
    // Without coalescing, the upper bound is `total` fdatasyncs
    // (one per flush_until call). The coalescer collapses bursts
    // so multiple waiters on the same lane share a single fsync.
    assert!(
        snap.fdatasyncs_issued < total as u64,
        "coalescer must reduce fdatasyncs: got {} (workload size {})",
        snap.fdatasyncs_issued,
        total,
    );
    let durable = lanes.lane_durable_lsns().expect("durable_lsns");
    for (idx, lsn) in durable.iter().enumerate() {
        assert!(lsn.0 > 0, "lane {idx} must have durable progress");
    }
}

#[test]
fn coalescer_shutdown_falls_back_to_direct_flush() {
    let temp = TempDir::new().unwrap();
    let lanes = Arc::new(WalLaneCoordinator::create(temp.path(), config(), 4).unwrap());

    // Prime every lane so all four have on-disk data.
    for lane in 0..4_usize {
        let append = lanes
            .append_on_lane(
                lane,
                WalRecordKind::Commit,
                TxId(lane as u64 + 1),
                vec![lane as u8; 8],
            )
            .unwrap();
        lanes.flush_until_on_lane(lane, append.end_lsn).unwrap();
    }

    // Shut down the coalescer. Subsequent flushes must succeed via
    // the per-lane fallback path.
    lanes.shutdown_coalescer_for_test();

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for lane in 0..4_usize {
        let lanes = Arc::clone(&lanes);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for r in 0..20_usize {
                let tx = TxId((lane * 1000 + r) as u64 + 100);
                let append = lanes
                    .append_on_lane(lane, WalRecordKind::Commit, tx, vec![lane as u8; 8])
                    .expect("post-shutdown append");
                lanes
                    .flush_until_on_lane(lane, append.end_lsn)
                    .expect("post-shutdown flush must succeed via fallback");
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    lanes.flush_all().expect("flush_all after fallback");
    let durable = lanes.lane_durable_lsns().expect("durable_lsns");
    for (idx, lsn) in durable.iter().enumerate() {
        assert!(
            lsn.0 > 0,
            "lane {idx} must remain durable after coalescer shutdown"
        );
    }
}
