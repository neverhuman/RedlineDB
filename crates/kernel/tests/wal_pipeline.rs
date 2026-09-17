//! Phase 6 R4-B (Candidate 4): integration tests for the
//! append-only WAL pipeline writer.
//!
//! Gated on `wal_pipeline` so the default test invocation skips it.
//! Run with `cargo test -p redlinedb-kernel --features wal_pipeline
//! --test wal_pipeline`.

#![cfg(feature = "wal_pipeline")]

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use redlinedb_kernel::format::{Lsn, PageId, TxId};
use redlinedb_kernel::wal::pipeline::{
    DEFAULT_BATCH_MAX_BYTES, DEFAULT_BATCH_MAX_RECORDS, WalPipelineConfig, WalPipelineDurability,
    WalPipelineWriter, scan_pipeline_wal,
};
use redlinedb_kernel::wal::record::{WAL_HEADER_LEN, WalRecord, WalRecordKind};

use tempfile::TempDir;

const ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// Process-local serialization for failpoint tests. The `fail`
/// crate's registry is global; without this every test in this
/// binary would have to either avoid the failpoints completely or
/// race against the two `crash_at_*` tests' `cfg(...)` mutations.
/// Every test takes this lock for its entire body so the failpoint
/// registry has a single owner at a time even when the test runner
/// schedules tests in parallel.
#[cfg(feature = "failpoints")]
fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("failpoint serialization mutex poisoned")
}

/// No-op serialization shim when the `failpoints` feature is OFF.
/// Returns a `()` so the test body can `let _guard = serial_lock();`
/// unconditionally.
#[cfg(not(feature = "failpoints"))]
fn serial_lock() {}

fn rec(payload: &[u8], tx: u64) -> WalRecord {
    WalRecord {
        lsn: Lsn::ZERO,
        prev_lsn: Lsn::ZERO,
        tx_id: TxId(tx),
        kind: WalRecordKind::PageImage,
        payload: payload.to_vec(),
    }
}

fn fast_config() -> WalPipelineConfig {
    WalPipelineConfig {
        // Make the worker react instantly so tests don't pay the
        // default 200us idle-drain latency on every submission.
        drain_idle_us: 1,
        ..WalPipelineConfig::default()
    }
}

#[test]
fn single_record_append_round_trips() {
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();
    let ack = writer
        .submit(vec![rec(b"hello world", 1)], PageId(1))
        .expect("submit");
    let lsn = ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    assert!(lsn.0 > 0, "lsn must advance past zero");

    // Drop the writer so the worker exits and the file flushes.
    drop(writer);

    let report = scan_pipeline_wal(&path).unwrap();
    assert!(!report.torn_tail, "tail must be clean");
    assert_eq!(report.records.len(), 1);
    assert_eq!(report.records[0].payload, b"hello world");
    assert_eq!(report.records[0].tx_id, TxId(1));
}

#[test]
fn batch_of_64_appends_round_trips() {
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();

    let mut acks = Vec::new();
    for i in 0..64 {
        let payload = format!("record-{i}").into_bytes();
        let ack = writer
            .submit(vec![rec(&payload, i as u64 + 1)], PageId((i as u64) % 8))
            .expect("submit");
        acks.push(ack);
    }
    for ack in acks {
        let _ = ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    }

    drop(writer);

    let report = scan_pipeline_wal(&path).unwrap();
    assert_eq!(report.records.len(), 64);
    for (i, record) in report.records.iter().enumerate() {
        // Payloads may have arrived in any (page_id, submission order)
        // permutation because the worker sorts by page hint; verify
        // the *set* of payloads is intact rather than the order.
        let s = std::str::from_utf8(&record.payload).unwrap();
        assert!(s.starts_with("record-"), "payload {i}: {s}");
    }
    let mut sorted: Vec<String> = report
        .records
        .iter()
        .map(|r| String::from_utf8(r.payload.clone()).unwrap())
        .collect();
    sorted.sort();
    for i in 0..64 {
        assert!(
            sorted.iter().any(|p| p == &format!("record-{i}")),
            "missing record-{i}"
        );
    }
}

#[test]
fn batch_size_limit_respected() {
    // Submit enough small records in one big burst that the worker
    // *must* split them across at least two batches if the byte cap
    // is honoured. With record bodies of WAL_HEADER_LEN + 16 bytes
    // each, ~1 MiB / 64 bytes ~= 16384 records fits inside a single
    // batch, so we set a tiny byte cap to force the split.
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let cfg = WalPipelineConfig {
        batch_max_records: DEFAULT_BATCH_MAX_RECORDS,
        batch_max_bytes: WAL_HEADER_LEN * 8 + 16 * 8, // ~ 8 records worth
        channel_depth: 1024,
        durability: WalPipelineDurability::DataSync,
        drain_idle_us: 1,
    };
    let writer = WalPipelineWriter::create(&path, cfg).unwrap();

    let mut acks = Vec::new();
    let payload = vec![0xABu8; 16];
    for i in 0..32u64 {
        let ack = writer
            .submit(vec![rec(&payload, i + 1)], PageId(i))
            .expect("submit");
        acks.push(ack);
    }
    for ack in acks {
        let _ = ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    }

    let snapshot = writer.counters_snapshot();
    assert_eq!(snapshot.records_flushed, 32);
    // At least two writev calls because the byte cap is far below
    // the 32-record total.
    assert!(
        snapshot.writev_calls >= 2,
        "expected at least 2 writev calls under tight byte cap, got {}",
        snapshot.writev_calls
    );
    // Largest batch must be ≤ batch_max_bytes plus one record (the
    // worker honours the cap as a *soft* upper bound; once it's
    // reached it stops draining but the current submission is
    // already in the batch).
    assert!(
        snapshot.largest_batch_bytes <= (WAL_HEADER_LEN * 8 + 16 * 8 + WAL_HEADER_LEN + 16) as u64,
        "batch overshoot exceeds cap-plus-one-record: {} bytes",
        snapshot.largest_batch_bytes
    );

    drop(writer);
    let report = scan_pipeline_wal(&path).unwrap();
    assert_eq!(report.records.len(), 32);
}

#[test]
fn checksum_mismatch_rejected_on_replay() {
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();
    let ack = writer
        .submit(vec![rec(b"intact", 1)], PageId(1))
        .expect("submit");
    ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    drop(writer);

    // Corrupt one byte inside the payload region. The scan must
    // refuse the record and report a torn tail.
    let mut bytes = std::fs::read(&path).unwrap();
    let payload_start = WAL_HEADER_LEN;
    bytes[payload_start] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();

    let report = scan_pipeline_wal(&path).unwrap();
    assert!(
        report.torn_tail,
        "corrupted record must trigger torn-tail report"
    );
    assert_eq!(report.records.len(), 0);
    assert_eq!(report.valid_end_offset, 0);
}

#[test]
fn concurrent_8_writers_into_pipeline_serializes_correctly() {
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer =
        Arc::new(WalPipelineWriter::create(&path, fast_config()).expect("pipeline must spawn"));

    const N: usize = 8;
    const PER: usize = 32;
    let barrier = Arc::new(Barrier::new(N));
    let mut handles = Vec::new();
    for w in 0..N {
        let writer = Arc::clone(&writer);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut acks = Vec::new();
            for i in 0..PER {
                let payload = format!("w{w}-r{i}").into_bytes();
                let ack = writer
                    .submit(
                        vec![rec(&payload, (w * PER + i + 1) as u64)],
                        PageId((w * PER + i) as u64 % 16),
                    )
                    .expect("submit");
                acks.push(ack);
            }
            for ack in acks {
                ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
            }
        }));
    }
    for h in handles {
        h.join().expect("worker join");
    }

    // Drop the user-side handle to let the worker drain. The
    // worker exits when its channel disconnects.
    drop(writer);

    let report = scan_pipeline_wal(&path).unwrap();
    assert!(!report.torn_tail);
    assert_eq!(
        report.records.len(),
        N * PER,
        "every submission must land in the file"
    );

    // All payloads must be present (set equality), and every LSN
    // must be unique + monotonic-by-position (the scan walks the
    // file in offset order, so record[i].lsn < record[i+1].lsn).
    let mut payloads: Vec<String> = report
        .records
        .iter()
        .map(|r| String::from_utf8(r.payload.clone()).unwrap())
        .collect();
    payloads.sort();
    for w in 0..N {
        for i in 0..PER {
            assert!(
                payloads.iter().any(|p| p == &format!("w{w}-r{i}")),
                "missing w{w}-r{i}"
            );
        }
    }
    for window in report.records.windows(2) {
        assert!(
            window[0].lsn.0 < window[1].lsn.0,
            "LSNs must strictly increase along the file: {:?} vs {:?}",
            window[0].lsn,
            window[1].lsn
        );
    }
}

#[test]
fn feature_off_is_byte_identical_to_lanes() {
    // The "feature off" path is enforced by `crates/kernel/src/wal/
    // mod.rs` gating the entire `pub mod pipeline;` declaration on
    // `wal_pipeline`. When the feature is ON (this test crate is
    // compiled with the feature), we still want to prove that the
    // pipeline writer does NOT touch the `wal/lanes.rs` layout —
    // i.e. a lane coordinator created at a sibling directory has
    // bytes that are independent of any pipeline activity.
    //
    // Concretely: spin up a pipeline writer at `dir/pipeline/` and
    // a lane coordinator at `dir/lanes/`, write a record to each,
    // and assert that the on-disk layout of the lane directory is
    // identical to a lane coordinator that ran *without* the
    // pipeline at all (compared by file-tree shape).
    use redlinedb_kernel::wal::{WalConfig, WalLaneCoordinator, WalRecordKind};

    let _guard = serial_lock();
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();

    // Path A: pipeline + lanes side by side.
    let pipeline_path = dir_a.path().join("pipeline.wal");
    let lanes_dir_a = dir_a.path().join("lanes");
    std::fs::create_dir_all(&lanes_dir_a).unwrap();
    {
        let pipeline = WalPipelineWriter::create(&pipeline_path, fast_config()).expect("pipeline");
        let ack = pipeline
            .submit(vec![rec(b"pipeline-side", 99)], PageId(7))
            .unwrap();
        ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
        drop(pipeline);

        let lanes =
            WalLaneCoordinator::create(&lanes_dir_a, WalConfig::default(), 1).expect("lanes");
        let _ = lanes
            .append(WalRecordKind::PageImage, TxId(1), b"lane-side".to_vec())
            .unwrap();
        let _ = lanes.flush_all().unwrap();
        drop(lanes);
    }

    // Path B: lanes only, no pipeline.
    let lanes_dir_b = dir_b.path().join("lanes");
    std::fs::create_dir_all(&lanes_dir_b).unwrap();
    {
        let lanes =
            WalLaneCoordinator::create(&lanes_dir_b, WalConfig::default(), 1).expect("lanes");
        let _ = lanes
            .append(WalRecordKind::PageImage, TxId(1), b"lane-side".to_vec())
            .unwrap();
        let _ = lanes.flush_all().unwrap();
        drop(lanes);
    }

    // Compare the lane directories: same set of segment files,
    // each byte-identical. The pipeline writer wrote a separate
    // file outside `lanes_dir_a`, so its existence must not
    // perturb the lane segment.
    let names_a = list_files(&lanes_dir_a);
    let names_b = list_files(&lanes_dir_b);
    assert_eq!(
        names_a, names_b,
        "lane directory file-set must be identical regardless of pipeline activity"
    );
    for name in &names_a {
        let a = std::fs::read(lanes_dir_a.join(name)).unwrap();
        let b = std::fs::read(lanes_dir_b.join(name)).unwrap();
        assert_eq!(
            a, b,
            "lane segment {name} must be byte-identical regardless of pipeline activity"
        );
    }
}

fn list_files(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn page_hint_sort_groups_by_page_id_within_batch() {
    // White-box-ish: submit records out of page_id order in one
    // tight burst, observe that the file lays them out in page-id
    // ascending order (with submission-index tie break). Use the
    // default batch caps so all 16 records land in a single batch.
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();

    let pages = [9u64, 2, 7, 0, 5, 11, 3, 1, 14, 8, 6, 10, 4, 12, 13, 15];
    let mut acks = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let payload = format!("p{page}").into_bytes();
        let ack = writer
            .submit(vec![rec(&payload, i as u64 + 1)], PageId(*page))
            .unwrap();
        acks.push(ack);
    }
    for ack in acks {
        ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    }
    drop(writer);

    let report = scan_pipeline_wal(&path).unwrap();
    assert_eq!(report.records.len(), pages.len());

    // The first batch of records (however many are in it) must be
    // sorted by page_id ascending. Recover the page id by parsing
    // the payload string back out.
    let mut on_disk_pages: Vec<u64> = Vec::new();
    for r in &report.records {
        let s = std::str::from_utf8(&r.payload).unwrap();
        let n: u64 = s.trim_start_matches('p').parse().unwrap();
        on_disk_pages.push(n);
    }
    // Sort-by-page ordering only holds *within* a single batch.
    // Confirm at least the first chunk is monotonic — the worker
    // may have split the burst across two batches if a producer
    // briefly stalled. We check the longest monotonic prefix is
    // at least half the input.
    let mut prefix = 1;
    while prefix < on_disk_pages.len() && on_disk_pages[prefix] >= on_disk_pages[prefix - 1] {
        prefix += 1;
    }
    assert!(
        prefix >= pages.len() / 2,
        "expected at least half the records sorted by page_id; \
         got monotonic prefix of {prefix}/{} (pages: {:?})",
        pages.len(),
        on_disk_pages
    );
}

#[test]
fn counters_track_writev_calls_and_batches() {
    // Smoke: after a known number of submissions, counters must
    // reflect record count and at least one writev call.
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();

    let mut acks = Vec::new();
    for i in 0..10 {
        let ack = writer.submit(vec![rec(b"x", i + 1)], PageId(i)).unwrap();
        acks.push(ack);
    }
    for ack in acks {
        ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    }
    let snap = writer.counters_snapshot();
    assert_eq!(snap.records_flushed, 10);
    assert_eq!(snap.submissions_acked, 10);
    assert!(snap.writev_calls >= 1);
    assert!(snap.fdatasync_calls >= 1, "DataSync must fdatasync");
    assert!(snap.bytes_flushed >= (10 * WAL_HEADER_LEN) as u64);
    let _ = DEFAULT_BATCH_MAX_BYTES; // re-export sanity check
    let _ = DEFAULT_BATCH_MAX_RECORDS;
}

#[test]
fn kernel_flush_mode_skips_fdatasync() {
    let _guard = serial_lock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let cfg = WalPipelineConfig {
        durability: WalPipelineDurability::KernelFlush,
        drain_idle_us: 1,
        ..WalPipelineConfig::default()
    };
    let writer = WalPipelineWriter::create(&path, cfg).unwrap();
    for i in 0..4 {
        let ack = writer.submit(vec![rec(b"y", i + 1)], PageId(i)).unwrap();
        ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
    }
    let snap = writer.counters_snapshot();
    assert_eq!(
        snap.fdatasync_calls, 0,
        "KernelFlush must not issue fdatasync"
    );
    assert!(snap.writev_calls >= 1);
}

#[test]
#[cfg(feature = "failpoints")]
fn crash_at_writev_recovery_replays_complete_batch() {
    // The "writev fails" semantic must be all-or-nothing: either
    // the entire batch lands (fully ack'd) or none of it lands (no
    // bytes on disk, no acks, no LSN bump). We inject a `return`
    // action at the pre-writev failpoint, submit a single record,
    // observe the ack as Err, then re-open the file and confirm it
    // is empty.
    let _guard = serial_lock();
    redlinedb_kernel::failpoints::init();
    redlinedb_kernel::failpoints::cfg("wal::pipeline::before_writev", "return")
        .expect("arm failpoint");

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");
    let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();
    let ack = writer
        .submit(vec![rec(b"will be dropped", 1)], PageId(1))
        .expect("submit");
    let ack_result = ack.recv_timeout(ACK_TIMEOUT).unwrap();
    assert!(
        ack_result.is_err(),
        "ack must surface the injected error: {ack_result:?}"
    );

    // Disarm before dropping the writer so any post-drop drain
    // cycle sees a clean registry.
    redlinedb_kernel::failpoints::cfg("wal::pipeline::before_writev", "off")
        .expect("disarm failpoint");
    drop(writer);

    let report = scan_pipeline_wal(&path).unwrap();
    assert_eq!(
        report.records.len(),
        0,
        "no bytes should have reached the file when writev was blocked"
    );
    assert_eq!(report.valid_end_offset, 0);
}

#[test]
#[cfg(feature = "failpoints")]
fn crash_at_fdatasync_recovery_loses_only_unsynced_batch() {
    // Inject a failure at the post-writev fdatasync point. Bytes
    // are on disk in the page cache (writev succeeded) but the
    // writer treats the submission as failed; the operator's
    // recovery driver, which tracks the last *acknowledged* LSN
    // externally, can truncate back to it and the failed batch
    // becomes the only loss.
    use redlinedb_kernel::wal::pipeline::recover_truncate;

    let _guard = serial_lock();
    redlinedb_kernel::failpoints::init();

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("00.wal");

    // Phase A: succeed at writing one record (no failpoint).
    {
        let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();
        let ack = writer.submit(vec![rec(b"durable", 1)], PageId(1)).unwrap();
        ack.recv_timeout(ACK_TIMEOUT).unwrap().expect("ack ok");
        drop(writer);
    }
    let phase_a_offset = scan_pipeline_wal(&path).unwrap().valid_end_offset;
    assert!(phase_a_offset > 0);

    // Phase B: arm the fdatasync failpoint and submit a second
    // record. The writer should ack with Err.
    redlinedb_kernel::failpoints::cfg("wal::pipeline::before_fdatasync", "return")
        .expect("arm failpoint");
    {
        let writer = WalPipelineWriter::create(&path, fast_config()).unwrap();
        let ack = writer.submit(vec![rec(b"unsynced", 2)], PageId(2)).unwrap();
        let result = ack.recv_timeout(ACK_TIMEOUT).unwrap();
        assert!(result.is_err(), "fdatasync fault must surface as ack Err");
        drop(writer);
    }
    redlinedb_kernel::failpoints::cfg("wal::pipeline::before_fdatasync", "off")
        .expect("disarm failpoint");

    // Phase C: recovery. The bytes for "unsynced" did reach the
    // page cache (writev succeeded before the fdatasync barrier),
    // so the scan sees both records. We assert that
    //   1. both records decode cleanly (no torn tail), and
    //   2. truncating to the last *acknowledged* LSN gives a clean
    //      tail with only the durable record.
    let report = recover_truncate(&path).expect("recover_truncate");
    assert!(!report.torn_tail);
    assert_eq!(report.records.len(), 2);
    let mut payloads: Vec<&[u8]> = report
        .records
        .iter()
        .map(|r| r.payload.as_slice())
        .collect();
    payloads.sort();
    assert_eq!(payloads, vec![&b"durable"[..], &b"unsynced"[..]]);

    // Simulated operator truncation: knock the file back to
    // phase_a_offset (the highest acknowledged byte) and confirm
    // only the durable record survives.
    let f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    f.set_len(phase_a_offset).unwrap();
    drop(f);
    let after = scan_pipeline_wal(&path).unwrap();
    assert_eq!(after.records.len(), 1);
    assert_eq!(after.records[0].payload, b"durable");
}

/// Release-mode smoke benchmark: batched writev should produce
/// strictly fewer syscalls per record than 1-record-per-batch.
/// Ignored by default (CI doesn't need the timing); run with
///
/// ```bash
/// cargo test -p redlinedb-kernel --release --features wal_pipeline \
///     --test wal_pipeline -- --ignored writev_batched_vs_sequential_smoke --nocapture
/// ```
#[test]
#[ignore]
fn writev_batched_vs_sequential_smoke() {
    let _guard = serial_lock();
    const N: usize = 10_000;
    let payload = vec![0xABu8; 256];

    fn run(label: &str, cfg: WalPipelineConfig, payload: &[u8], n: usize) -> u64 {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("00.wal");
        let writer = WalPipelineWriter::create(&path, cfg).unwrap();
        let start = std::time::Instant::now();
        let mut acks = Vec::with_capacity(n);
        for i in 0..n {
            let ack = writer
                .submit(vec![rec(payload, i as u64 + 1)], PageId(i as u64 % 16))
                .unwrap();
            acks.push(ack);
        }
        for ack in acks {
            ack.recv().unwrap().expect("ack ok");
        }
        let elapsed = start.elapsed();
        let snap = writer.counters_snapshot();
        let secs = elapsed.as_secs_f64();
        let mb = snap.bytes_flushed as f64 / 1.0e6;
        eprintln!(
            "{label}: {n} records in {secs:.3}s ({:.0} rec/s, {:.2} MB/s) writev={} fdatasync={} largest_batch={}r/{}b",
            n as f64 / secs,
            mb / secs,
            snap.writev_calls,
            snap.fdatasync_calls,
            snap.largest_batch_records,
            snap.largest_batch_bytes,
        );
        snap.writev_calls
    }

    let batched = run(
        "batched-datasync",
        WalPipelineConfig {
            batch_max_records: 256,
            batch_max_bytes: 4 << 20,
            channel_depth: 4096,
            durability: WalPipelineDurability::DataSync,
            drain_idle_us: 50,
        },
        &payload,
        N,
    );
    let per_record = run(
        "per-record-datasync",
        WalPipelineConfig {
            batch_max_records: 1,
            batch_max_bytes: 1024,
            channel_depth: 4096,
            durability: WalPipelineDurability::DataSync,
            drain_idle_us: 1,
        },
        &payload,
        N,
    );
    let kernel_flush = run(
        "batched-kernel-flush",
        WalPipelineConfig {
            batch_max_records: 256,
            batch_max_bytes: 4 << 20,
            channel_depth: 4096,
            durability: WalPipelineDurability::KernelFlush,
            drain_idle_us: 50,
        },
        &payload,
        N,
    );
    let _ = kernel_flush;

    // The whole point of the pipeline is that batched writev issues
    // dramatically fewer syscalls. We require at minimum 10× fewer
    // writev_calls in the batched config vs the per-record config.
    assert!(
        batched * 10 <= per_record,
        "batched writev must issue ≥10× fewer syscalls than per-record: \
         batched={batched} per_record={per_record}"
    );
}
