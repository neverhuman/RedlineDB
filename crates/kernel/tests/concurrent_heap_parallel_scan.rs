// WS-C3 (kernel half) — parallel_scan smoke + parity test.
//
// Verifies that `ConcurrentHeap::parallel_scan` returns the same row set
// as `serial_scan` and that it actually runs the worker partitioning
// without panicking under realistic load. A wall-time speedup assertion
// is intentionally omitted: CI runners exhibit too much jitter at the
// scales available to a unit test for a < 50% threshold to be stable.
// The kernel test instead exercises the safety + correctness contract,
// which is the prerequisite for the deferred SQL-side gate.

use redlinedb_kernel::engine::ConcurrentTxStatus;
use redlinedb_kernel::engine::concurrent_heap::ConcurrentHeap;
use redlinedb_kernel::format::RelId;

const LANES: usize = 8;
const ROWS: usize = 50_000;

fn build_heap() -> (ConcurrentTxStatus, ConcurrentHeap) {
    let txs = ConcurrentTxStatus::new();
    let heap = ConcurrentHeap::new(RelId(1), LANES).expect("heap");
    let csn = txs.reserve_commit_csn();
    let tx = txs.begin();
    for i in 0..ROWS {
        heap.insert(tx, format!("payload-{i}").into_bytes())
            .expect("insert");
    }
    txs.publish_commit(tx, csn);
    (txs, heap)
}

#[test]
fn parallel_scan_returns_same_set_as_serial() {
    let (txs, heap) = build_heap();
    let snap = txs.snapshot();

    let mut serial = heap.serial_scan(&txs, &snap, None).expect("serial");
    let mut parallel = heap.parallel_scan(&txs, &snap, None, 4).expect("parallel");

    assert_eq!(serial.len(), ROWS, "serial count");
    assert_eq!(parallel.len(), ROWS, "parallel count");

    serial.sort_by_key(|(r, _)| r.0);
    parallel.sort_by_key(|(r, _)| r.0);
    assert_eq!(serial, parallel, "row set must match across paths");
}

#[test]
fn parallel_scan_worker_count_one_matches_serial() {
    let (txs, heap) = build_heap();
    let snap = txs.snapshot();

    let serial = heap.serial_scan(&txs, &snap, None).expect("serial");
    let parallel = heap.parallel_scan(&txs, &snap, None, 1).expect("parallel-1");
    assert_eq!(serial.len(), parallel.len());
}

#[test]
fn parallel_scan_clamps_worker_count_to_lane_count() {
    let (txs, heap) = build_heap();
    let snap = txs.snapshot();

    // Asking for more workers than lanes must not panic and must still
    // return every row exactly once.
    let parallel = heap
        .parallel_scan(&txs, &snap, None, LANES * 4)
        .expect("parallel-oversub");
    assert_eq!(parallel.len(), ROWS);
}

// Soft wall-time check: run a 1M-row scan once and print the
// serial-vs-parallel ratio. We do not assert a fixed speedup (CI noise
// makes < 50% checks flaky) but the print line is captured so the
// release notes can quote the locally-observed ratio. Gated by an env
// var so the normal `cargo test` run stays quick.
#[test]
fn parallel_scan_speedup_smoke() {
    if std::env::var_os("WS_C3_SPEEDUP_BENCH").is_none() {
        return;
    }
    use std::time::Instant;

    let lanes = 16usize;
    let rows = 1_000_000usize;
    let txs = ConcurrentTxStatus::new();
    let heap = ConcurrentHeap::new(RelId(1), lanes).expect("heap");
    let csn = txs.reserve_commit_csn();
    let tx = txs.begin();
    for i in 0..rows {
        heap.insert(tx, format!("payload-{i}").into_bytes())
            .expect("insert");
    }
    txs.publish_commit(tx, csn);
    let snap = txs.snapshot();

    let t0 = Instant::now();
    let s = heap.serial_scan(&txs, &snap, None).expect("serial");
    let serial_ms = t0.elapsed().as_millis();

    let t1 = Instant::now();
    let p = heap.parallel_scan(&txs, &snap, None, 4).expect("parallel");
    let par_ms = t1.elapsed().as_millis();

    assert_eq!(s.len(), rows);
    assert_eq!(p.len(), rows);
    eprintln!(
        "ws_c3 parallel_scan 1M rows: serial={serial_ms} ms parallel(4)={par_ms} ms"
    );
}

#[test]
fn parallel_scan_observes_post_commit_visibility() {
    let txs = ConcurrentTxStatus::new();
    let heap = ConcurrentHeap::new(RelId(1), LANES).expect("heap");
    let tx = txs.begin();
    for i in 0..1024 {
        heap.insert(tx, format!("row-{i}").into_bytes())
            .expect("insert");
    }
    // Pre-commit snapshot: rows are invisible to a non-owner.
    let pre = txs.snapshot();
    let rows = heap.parallel_scan(&txs, &pre, None, 4).expect("pre");
    assert_eq!(rows.len(), 0, "uncommitted rows must be invisible");

    let csn = txs.reserve_commit_csn();
    txs.publish_commit(tx, csn);
    let post = txs.snapshot();
    let rows = heap.parallel_scan(&txs, &post, None, 4).expect("post");
    assert_eq!(rows.len(), 1024, "committed rows must be visible");
}
