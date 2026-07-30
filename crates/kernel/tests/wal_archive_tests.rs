use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use redlinedb_kernel::format::{Lsn, TimelineId, TxId};
use redlinedb_kernel::wal::{
    ARCHIVE_LAG_ALERT_AFTER_MS, ARCHIVE_SEAL_AFTER_MS, ARCHIVE_SEAL_BYTES, ArchiveReceiptVerifier,
    ArchiveSealPolicy, SealedWalRange, VerifiedArchiveReceipt, WalConfig, WalManager,
    WalRecordKind, WalRetentionHorizons, advance_archive_watermark, archive_lag, archive_watermark,
    read_durable_prefix,
};
use redlinedb_kernel::{Error, Result};
use tempfile::TempDir;

struct ExactReceipt(&'static [u8]);

impl ArchiveReceiptVerifier for ExactReceipt {
    fn verify(&self, _range: &SealedWalRange, receipt: &[u8]) -> Result<()> {
        if receipt == self.0 {
            Ok(())
        } else {
            Err(Error::CorruptWal("archive receipt rejected"))
        }
    }
}

fn sealed(start_lsn: u64, end_lsn: u64, marker: u8) -> SealedWalRange {
    SealedWalRange {
        timeline: TimelineId(1),
        start_lsn: Lsn(start_lsn),
        end_lsn: Lsn(end_lsn),
        byte_len: end_lsn - start_lsn,
        sha256: [marker; 32],
    }
}

#[test]
fn seal_policy_uses_sixteen_mib_or_five_seconds() {
    let policy = ArchiveSealPolicy::default();
    assert!(!policy.should_seal(0, None, 50_000));
    assert!(!policy.should_seal(ARCHIVE_SEAL_BYTES - 1, Some(10_000), 14_999));
    assert!(policy.should_seal(ARCHIVE_SEAL_BYTES, Some(10_000), 10_000));
    assert!(policy.should_seal(1, Some(10_000), 10_000 + ARCHIVE_SEAL_AFTER_MS));
}

#[test]
fn archive_lag_alerts_only_after_thirty_seconds() {
    assert_eq!(archive_lag(None, 40_000).lag_ms, 0);
    assert!(!archive_lag(Some(10_000), 10_000 + ARCHIVE_LAG_ALERT_AFTER_MS).alert);
    assert!(archive_lag(Some(10_000), 10_001 + ARCHIVE_LAG_ALERT_AFTER_MS).alert);
}

#[test]
fn watermark_requires_verified_contiguous_receipts() {
    let temp = TempDir::new().expect("tempdir");
    let verifier = ExactReceipt(b"durable");

    let first =
        VerifiedArchiveReceipt::verify(sealed(0, 100, 1), b"durable", &verifier).expect("verify");
    let watermark = advance_archive_watermark(temp.path(), &first).expect("advance");
    assert_eq!(watermark.archived_lsn, Lsn(100));
    assert_eq!(watermark.generation, 1);
    assert_eq!(
        archive_watermark(temp.path(), TimelineId(1)).expect("read"),
        watermark
    );
    assert!(archive_watermark(temp.path(), TimelineId(2)).is_err());

    let replay = advance_archive_watermark(temp.path(), &first).expect("idempotent replay");
    assert_eq!(replay, watermark);
    let gap =
        VerifiedArchiveReceipt::verify(sealed(101, 200, 2), b"durable", &verifier).expect("verify");
    assert_eq!(
        advance_archive_watermark(temp.path(), &gap).expect_err("gap"),
        Error::CorruptWal("archive receipt is not contiguous")
    );

    let second =
        VerifiedArchiveReceipt::verify(sealed(100, 200, 3), b"durable", &verifier).expect("verify");
    let watermark = advance_archive_watermark(temp.path(), &second).expect("advance");
    assert_eq!(watermark.archived_lsn, Lsn(200));
    assert_eq!(watermark.generation, 2);
}

#[cfg(unix)]
#[test]
fn symlinked_watermark_never_becomes_native_authority() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("tempdir");
    let external = temp.path().join("external");
    fs::write(
        &external,
        "version=2\ntimeline=1\nprevious_lsn=0\narchived_lsn=100\ngeneration=1\nreceipt_sha256=\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
    )
    .expect("external");
    let state = temp.path().join("state");
    fs::create_dir(&state).expect("state");
    symlink(&external, state.join("archive.watermark")).expect("symlink");
    assert!(archive_watermark(&state, TimelineId(1)).is_err());
}

#[cfg(unix)]
#[test]
fn symlinked_state_root_never_receives_watermark_authority() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("tempdir");
    let external = temp.path().join("external-state");
    fs::create_dir(&external).expect("external state");
    let linked = temp.path().join("linked-state");
    symlink(&external, &linked).expect("state symlink");
    let receipt =
        VerifiedArchiveReceipt::verify(sealed(0, 100, 1), b"durable", &ExactReceipt(b"durable"))
            .expect("verify");

    assert!(advance_archive_watermark(&linked, &receipt).is_err());
    assert!(!external.join("archive.watermark").exists());
}

#[test]
fn watermark_recovers_fsynced_pre_rename_intent() {
    let temp = TempDir::new().expect("tempdir");
    let receipt =
        VerifiedArchiveReceipt::verify(sealed(0, 100, 1), b"durable", &ExactReceipt(b"durable"))
            .expect("verify");
    let digest = receipt
        .receipt_sha256()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fs::write(
        temp.path().join("archive.watermark.next"),
        format!(
            "version=2\ntimeline=1\nprevious_lsn=0\narchived_lsn=100\ngeneration=1\nreceipt_sha256={digest}\n"
        ),
    )
    .expect("pending intent");

    let recovered = advance_archive_watermark(temp.path(), &receipt).expect("recover");
    assert_eq!(recovered.archived_lsn, Lsn(100));
    assert_eq!(recovered.generation, 1);
    assert!(!temp.path().join("archive.watermark.next").exists());
}

#[test]
fn watermark_discards_partial_pre_fsync_intent_and_retries() {
    let temp = TempDir::new().expect("tempdir");
    fs::write(
        temp.path().join("archive.watermark.next"),
        b"version=2\ntimeline=1\nprevious_lsn=",
    )
    .expect("partial intent");
    let receipt =
        VerifiedArchiveReceipt::verify(sealed(0, 100, 1), b"durable", &ExactReceipt(b"durable"))
            .expect("verify");

    let watermark = advance_archive_watermark(temp.path(), &receipt).expect("retry");
    assert_eq!(watermark.archived_lsn, Lsn(100));
    assert_eq!(watermark.generation, 1);
}

#[test]
fn concurrent_watermark_cas_admits_exactly_one_conflicting_receipt() {
    let temp = Arc::new(TempDir::new().expect("tempdir"));
    let barrier = Arc::new(Barrier::new(2));
    let mut threads = Vec::new();
    for receipt_bytes in [b"left".as_slice(), b"right".as_slice()] {
        let temp = Arc::clone(&temp);
        let barrier = Arc::clone(&barrier);
        threads.push(thread::spawn(move || {
            let receipt = VerifiedArchiveReceipt::verify(
                sealed(0, 100, 1),
                receipt_bytes,
                &ExactReceipt(receipt_bytes),
            )
            .expect("verify");
            barrier.wait();
            advance_archive_watermark(temp.path(), &receipt)
        }));
    }
    let results = threads
        .into_iter()
        .map(|thread| thread.join().expect("join"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(
        archive_watermark(temp.path(), TimelineId(1))
            .expect("watermark")
            .generation,
        1
    );
}

#[test]
fn concurrent_exact_replay_is_idempotent_across_instances() {
    let temp = Arc::new(TempDir::new().expect("tempdir"));
    let barrier = Arc::new(Barrier::new(2));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let temp = Arc::clone(&temp);
        let barrier = Arc::clone(&barrier);
        threads.push(thread::spawn(move || {
            let receipt = VerifiedArchiveReceipt::verify(
                sealed(0, 100, 1),
                b"durable",
                &ExactReceipt(b"durable"),
            )
            .expect("verify");
            barrier.wait();
            advance_archive_watermark(temp.path(), &receipt).expect("idempotent advance")
        }));
    }
    let watermarks = threads
        .into_iter()
        .map(|thread| thread.join().expect("join"))
        .collect::<Vec<_>>();
    assert_eq!(watermarks[0], watermarks[1]);
    assert_eq!(watermarks[0].generation, 1);
}

#[test]
fn verifier_rejection_never_reaches_the_watermark() {
    let temp = TempDir::new().expect("tempdir");
    let error = VerifiedArchiveReceipt::verify(
        sealed(0, 100, 1),
        b"not-durable",
        &ExactReceipt(b"durable"),
    )
    .expect_err("reject");
    assert_eq!(error, Error::CorruptWal("archive receipt rejected"));
    assert_eq!(
        archive_watermark(temp.path(), TimelineId(1))
            .expect("watermark")
            .generation,
        0
    );
}

#[test]
fn sealed_range_requires_exact_address_span() {
    let mut range = sealed(10, 20, 1);
    range.byte_len = 9;
    assert_eq!(
        range.validate().expect_err("short range must fail"),
        Error::CorruptWal("invalid sealed wal range")
    );
}

#[test]
fn durable_prefix_rejects_missing_native_segments() {
    let temp = TempDir::new().expect("tempdir");
    let error = read_durable_prefix(temp.path(), 128, TimelineId(1), Lsn::ZERO, Lsn(1), 1)
        .expect_err("missing segment");
    assert!(matches!(error, Error::Io(_)));
}

#[test]
fn durable_prefix_rejects_short_nonfinal_segment() {
    let temp = TempDir::new().expect("tempdir");
    fs::write(temp.path().join("00000000000000000001.wal"), [7_u8; 64]).expect("short segment");
    fs::write(temp.path().join("00000000000000000002.wal"), [8_u8; 1]).expect("next segment");

    let error = read_durable_prefix(temp.path(), 128, TimelineId(1), Lsn::ZERO, Lsn(129), 129)
        .expect_err("address gap must fail");
    assert_eq!(
        error,
        Error::CorruptWal("wal segment contains an address gap")
    );
}

#[cfg(unix)]
#[test]
fn durable_prefix_rejects_symlinked_or_sparse_segment() {
    use std::os::unix::fs::symlink;

    let linked = TempDir::new().expect("tempdir");
    let external = linked.path().join("external");
    fs::write(&external, [1_u8; 8]).expect("external");
    symlink(&external, linked.path().join("00000000000000000001.wal")).expect("symlink");
    assert!(read_durable_prefix(linked.path(), 128, TimelineId(1), Lsn::ZERO, Lsn(8), 8).is_err());

    let sparse = TempDir::new().expect("tempdir");
    let file =
        fs::File::create(sparse.path().join("00000000000000000001.wal")).expect("sparse segment");
    file.set_len(128).expect("sparse length");
    let error = read_durable_prefix(sparse.path(), 128, TimelineId(1), Lsn::ZERO, Lsn(128), 128)
        .expect_err("sparse extent must fail");
    assert_eq!(
        error,
        Error::CorruptWal("wal segment has invalid physical extent")
    );
}

#[test]
fn durable_prefix_streams_bounded_bytes_across_segments() {
    let temp = TempDir::new().expect("tempdir");
    let config = WalConfig {
        segment_bytes: 128,
        ..WalConfig::default()
    };
    let mut wal = WalManager::create(temp.path(), config.clone()).expect("create");
    for value in 0_u8..8 {
        wal.append(WalRecordKind::PageDelta, TxId(1), vec![value; 24])
            .expect("append");
    }
    let durable_lsn = wal.flush().expect("flush");
    drop(wal);

    let mut start = Lsn::ZERO;
    let mut streamed = 0_u64;
    while let Some(prefix) = read_durable_prefix(
        temp.path(),
        config.segment_bytes,
        TimelineId(1),
        start,
        durable_lsn,
        47,
    )
    .expect("prefix")
    {
        assert_eq!(prefix.range.start_lsn, start);
        assert_eq!(prefix.range.byte_len, prefix.bytes.len() as u64);
        assert!(prefix.bytes.len() <= 47);
        streamed += prefix.range.byte_len;
        start = prefix.range.end_lsn;
    }
    let bytes_on_disk = fs::read_dir(temp.path())
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".wal"))
        .map(|entry| entry.metadata().expect("metadata").len())
        .sum::<u64>();
    assert_eq!(streamed, bytes_on_disk);
    assert_eq!(start, durable_lsn);
}

#[test]
fn pruning_uses_the_minimum_required_horizon() {
    let temp = TempDir::new().expect("tempdir");
    let config = WalConfig {
        segment_bytes: 128,
        ..WalConfig::default()
    };
    let mut wal = WalManager::create(temp.path(), config.clone()).expect("create");
    while wal_segments(temp.path()).len() < 4 {
        wal.append(WalRecordKind::PageDelta, TxId(1), vec![7; 24])
            .expect("append");
        wal.flush().expect("flush");
    }

    let removed = wal
        .prune_segments_below_horizons(WalRetentionHorizons {
            checkpoint_lsn: Lsn(config.segment_bytes * 3),
            replication_slot_lsn: Lsn(config.segment_bytes * 2),
            required_archive_lsn: Lsn(config.segment_bytes),
        })
        .expect("prune");
    assert_eq!(removed, 1);
    assert_eq!(wal_segments(temp.path()).first().copied(), Some(2));
}

fn wal_segments(path: &std::path::Path) -> Vec<u64> {
    let mut segments = fs::read_dir(path)
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".wal"))
                .and_then(|value| value.parse::<u64>().ok())
        })
        .collect::<Vec<_>>();
    segments.sort_unstable();
    segments
}
