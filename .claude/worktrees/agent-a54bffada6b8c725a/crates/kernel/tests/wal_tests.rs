use redlinedb_kernel::Error;
use redlinedb_kernel::format::{Csn, Lsn, TxId};
use redlinedb_kernel::wal::{
    WalConfig, WalCoordinator, WalManager, WalPayload, WalReader, WalRecord, WalRecordKind,
};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

#[test]
fn wal_record_round_trips() {
    let record = WalRecord {
        lsn: Lsn(1024),
        prev_lsn: Lsn(512),
        tx_id: TxId(9),
        kind: WalRecordKind::Commit,
        payload: b"commit payload".to_vec(),
    };

    let encoded = record.encode().unwrap();
    let decoded = WalRecord::decode(&encoded).unwrap();
    assert_eq!(decoded, record);
}

#[test]
fn wal_record_detects_corruption() {
    let record = WalRecord {
        lsn: Lsn(1),
        prev_lsn: Lsn(0),
        tx_id: TxId(1),
        kind: WalRecordKind::PageDelta,
        payload: b"delta".to_vec(),
    };

    let mut encoded = record.encode().unwrap();
    let last = encoded.len() - 1;
    encoded[last] ^= 0x01;

    let err = WalRecord::decode(&encoded).unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn wal_manager_appends_flushes_and_reopens() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 1024,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();

    let first_lsn = manager
        .append(WalRecordKind::Begin, TxId(1), b"begin".to_vec())
        .unwrap();
    assert_eq!(first_lsn.start_lsn, Lsn(0));
    assert!(first_lsn.end_lsn > first_lsn.start_lsn);
    assert_eq!(manager.durable_lsn(), Lsn(0));
    let durable_lsn = manager.flush().unwrap();
    assert_eq!(durable_lsn, manager.written_lsn());
    assert!(manager.durable_lsn().0 > 0);
    drop(manager);

    let mut reader = WalReader::new(temp.path(), config.clone());
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lsn, Lsn(0));
    assert_eq!(records[0].prev_lsn, Lsn(0));
    assert_eq!(records[0].kind, WalRecordKind::Begin);
    assert_eq!(records[0].payload, b"begin");

    let mut reopened = WalManager::open(temp.path(), config).unwrap();
    let second_lsn = reopened
        .append(WalRecordKind::Commit, TxId(1), b"commit".to_vec())
        .unwrap();
    assert!(second_lsn.start_lsn.0 > first_lsn.start_lsn.0);
    reopened.flush().unwrap();

    let mut reader = WalReader::new(
        temp.path(),
        WalConfig {
            segment_bytes: 1024,
            ..WalConfig::default()
        },
    );
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].prev_lsn, records[0].lsn);
    assert_eq!(records[1].kind, WalRecordKind::Commit);
}

#[test]
fn wal_manager_assigns_strictly_increasing_lsns_and_prev_lsn() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 1024,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();

    let a = manager
        .append(WalRecordKind::Begin, TxId(1), b"a".to_vec())
        .unwrap();
    let b = manager
        .append(WalRecordKind::PageDelta, TxId(1), b"b".to_vec())
        .unwrap();
    let c = manager
        .append(WalRecordKind::Commit, TxId(1), b"c".to_vec())
        .unwrap();
    manager.flush().unwrap();

    assert!(a.start_lsn < b.start_lsn);
    assert!(b.start_lsn < c.start_lsn);
    assert_eq!(a.end_lsn, b.start_lsn);
    assert_eq!(b.end_lsn, c.start_lsn);

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records[0].prev_lsn, Lsn(0));
    assert_eq!(records[1].prev_lsn, records[0].lsn);
    assert_eq!(records[2].prev_lsn, records[1].lsn);
}

#[test]
fn wal_reader_scans_across_segment_rollover() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 128,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();

    manager
        .append(WalRecordKind::Begin, TxId(1), vec![b'a'; 32])
        .unwrap();
    manager
        .append(WalRecordKind::PageDelta, TxId(1), vec![b'b'; 32])
        .unwrap();
    manager
        .append(WalRecordKind::Commit, TxId(1), vec![b'c'; 32])
        .unwrap();
    manager.flush().unwrap();

    let segment_count = std::fs::read_dir(temp.path())
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".wal")
        })
        .count();
    assert!(segment_count >= 2);

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].payload, vec![b'a'; 32]);
    assert_eq!(records[1].payload, vec![b'b'; 32]);
    assert_eq!(records[2].payload, vec![b'c'; 32]);
}

#[test]
fn wal_reader_stops_at_truncated_final_record() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 1024,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();
    manager
        .append(WalRecordKind::Begin, TxId(1), b"good".to_vec())
        .unwrap();
    manager
        .append(WalRecordKind::Commit, TxId(1), b"truncated".to_vec())
        .unwrap();
    manager.flush().unwrap();
    drop(manager);

    let path = temp.path().join("00000000000000000001.wal");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let len = file.metadata().unwrap().len();
    file.set_len(len - 4).unwrap();

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload, b"good");
}

#[test]
fn wal_reader_stops_at_corrupt_final_record() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 1024,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();
    manager
        .append(WalRecordKind::Begin, TxId(1), b"good".to_vec())
        .unwrap();
    manager
        .append(WalRecordKind::Commit, TxId(1), b"corrupt".to_vec())
        .unwrap();
    manager.flush().unwrap();
    drop(manager);

    flip_last_byte(temp.path().join("00000000000000000001.wal").as_path());

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload, b"good");
}

#[test]
fn wal_reader_errors_on_corrupt_non_final_record() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 1024,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();
    manager
        .append(WalRecordKind::Begin, TxId(1), b"first".to_vec())
        .unwrap();
    manager
        .append(WalRecordKind::PageDelta, TxId(1), b"second".to_vec())
        .unwrap();
    manager
        .append(WalRecordKind::Commit, TxId(1), b"third".to_vec())
        .unwrap();
    manager.flush().unwrap();
    drop(manager);

    let path = temp.path().join("00000000000000000001.wal");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let first_len = WalRecord {
        lsn: Lsn(0),
        prev_lsn: Lsn(0),
        tx_id: TxId(1),
        kind: WalRecordKind::Begin,
        payload: b"first".to_vec(),
    }
    .encode()
    .unwrap()
    .len() as u64;
    file.seek(SeekFrom::Start(first_len + 50)).unwrap();
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 0x01;
    file.seek(SeekFrom::Start(first_len + 50)).unwrap();
    file.write_all(&byte).unwrap();

    let mut reader = WalReader::new(temp.path(), config);
    let err = reader.scan().unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn wal_manager_prunes_old_segments_after_checkpoint_lsn() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 256,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();

    while wal_segment_count(temp.path()) < 3 {
        let _ = manager
            .append(WalRecordKind::PageDelta, TxId(1), b"x".to_vec())
            .unwrap();
        manager.flush().unwrap();
    }

    let checkpoint_lsn = Lsn(config.segment_bytes * 2);
    let removed = manager
        .prune_segments_below_checkpoint_lsn(checkpoint_lsn)
        .unwrap();
    assert_eq!(removed, 2);

    let keep_segment = checkpoint_lsn.0 / config.segment_bytes + 1;
    let segments = wal_segment_numbers(temp.path());
    assert_eq!(segments, vec![keep_segment]);
}

#[test]
fn wal_manager_prune_is_idempotent() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 256,
        ..WalConfig::default()
    };
    let mut manager = WalManager::create(temp.path(), config.clone()).unwrap();
    let _ = manager
        .append(WalRecordKind::PageDelta, TxId(1), b"payload".to_vec())
        .unwrap();
    manager.flush().unwrap();
    manager.prune_segments_below(2).unwrap();
    let removed_again = manager.prune_segments_below(2).unwrap();
    assert_eq!(removed_again, 0);
}

#[test]
fn wal_coordinator_group_flushes_concurrent_targets() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 1_000,
        ..WalConfig::default()
    };
    let coordinator = Arc::new(WalCoordinator::create(temp.path(), config.clone()).unwrap());
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|idx| {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let append = coordinator
                    .append(WalRecordKind::Commit, TxId(idx + 1), vec![idx as u8; 16])
                    .unwrap();
                barrier.wait();
                coordinator.flush_until(append.end_lsn).unwrap();
                append
            })
        })
        .collect();

    let mut appends: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    appends.sort_unstable_by_key(|append| append.start_lsn.0);
    for pair in appends.windows(2) {
        assert_eq!(pair[0].end_lsn, pair[1].start_lsn);
    }
    let last = appends.last().unwrap();
    assert!(coordinator.durable_lsn().unwrap() >= last.end_lsn);

    let mut reader = WalReader::new(temp.path(), config);
    assert_eq!(reader.scan().unwrap().len(), 8);
}

#[test]
fn wal_coordinator_flush_until_returns_when_already_durable() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = WalCoordinator::create(temp.path(), config).unwrap();
    let append = coordinator
        .append(WalRecordKind::Commit, TxId(1), b"commit".to_vec())
        .unwrap();

    let first = coordinator.flush_until(append.end_lsn).unwrap();
    let second = coordinator.flush_until(append.start_lsn).unwrap();
    assert_eq!(first, second);
}

#[test]
fn wal_coordinator_flush_all_drains_queued_records() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = WalCoordinator::create(temp.path(), config.clone()).unwrap();
    let mut last = Lsn::ZERO;
    for idx in 0..12 {
        last = coordinator
            .append(WalRecordKind::PageDelta, TxId(1), vec![idx as u8; 8])
            .unwrap()
            .end_lsn;
    }

    assert!(coordinator.flush_all().unwrap() >= last);
    let mut reader = WalReader::new(temp.path(), config);
    assert_eq!(reader.scan().unwrap().len(), 12);
}

#[test]
fn wal_coordinator_bounded_buffer_preserves_all_records() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        wal_buffer_bytes: 80,
        wal_write_batch_bytes: 64,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = Arc::new(WalCoordinator::create(temp.path(), config.clone()).unwrap());
    let barrier = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|idx| {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                coordinator
                    .append(WalRecordKind::PageDelta, TxId(idx + 1), vec![idx as u8; 8])
                    .unwrap()
            })
        })
        .collect();
    let mut appends: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    appends.sort_unstable_by_key(|append| append.start_lsn.0);
    for pair in appends.windows(2) {
        assert_eq!(pair[0].end_lsn, pair[1].start_lsn);
    }

    coordinator.flush_all().unwrap();
    drop(coordinator);

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 16);
    for pair in records.windows(2) {
        assert_eq!(pair[1].prev_lsn, pair[0].lsn);
    }
}

#[test]
fn wal_coordinator_commit_csn_follows_wal_order_under_backpressure() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        wal_buffer_bytes: 80,
        wal_write_batch_bytes: 64,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = Arc::new(WalCoordinator::create(temp.path(), config.clone()).unwrap());
    let barrier = Arc::new(Barrier::new(16));
    let next_csn = Arc::new(AtomicU64::new(1));
    let handles: Vec<_> = (0..16)
        .map(|idx| {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            let next_csn = Arc::clone(&next_csn);
            thread::spawn(move || {
                barrier.wait();
                let (csn, append) = coordinator
                    .append_commit(TxId(idx + 1), || {
                        Csn(next_csn.fetch_add(1, Ordering::SeqCst))
                    })
                    .unwrap();
                coordinator.flush_until(append.end_lsn).unwrap();
                (csn, append)
            })
        })
        .collect();
    let mut commits: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    commits.sort_unstable_by_key(|(_, append)| append.start_lsn.0);
    for (idx, (csn, _)) in commits.iter().enumerate() {
        assert_eq!(csn.0, idx as u64 + 1);
    }

    drop(coordinator);

    let mut reader = WalReader::new(temp.path(), config);
    let records = reader.scan().unwrap();
    assert_eq!(records.len(), 16);
    for (idx, record) in records.iter().enumerate() {
        let payload = WalPayload::decode(&record.payload).unwrap();
        match payload {
            WalPayload::Commit { csn, .. } => assert_eq!(csn.0, idx as u64 + 1),
            _ => panic!("expected commit payload"),
        }
    }
}

#[test]
fn wal_coordinator_prune_keeps_active_segment() {
    let temp = TempDir::new().unwrap();
    let config = WalConfig {
        segment_bytes: 4096,
        group_commit_delay_us: 0,
        ..WalConfig::default()
    };
    let coordinator = WalCoordinator::create(temp.path(), config.clone()).unwrap();
    coordinator
        .append(WalRecordKind::PageDelta, TxId(1), b"payload".to_vec())
        .unwrap();
    coordinator.flush_all().unwrap();

    let removed = coordinator
        .prune_segments_below_checkpoint_lsn(Lsn(config.segment_bytes * 100))
        .unwrap();
    assert_eq!(removed, 0);
    assert_eq!(wal_segment_numbers(temp.path()), vec![1]);
}

fn flip_last_byte(path: &std::path::Path) {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    let len = file.metadata().unwrap().len();
    file.seek(SeekFrom::Start(len - 1)).unwrap();
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 0x01;
    file.seek(SeekFrom::Start(len - 1)).unwrap();
    file.write_all(&byte).unwrap();
}

fn wal_segment_count(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let mut count = 0;
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(number) = name.strip_suffix(".wal")
            && number.len() == 20
            && number.bytes().all(|byte| byte.is_ascii_digit())
        {
            count += 1;
        }
    }
    count
}

fn wal_segment_numbers(path: &std::path::Path) -> Vec<u64> {
    if !path.exists() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(number) = name.strip_suffix(".wal")
            && number.len() == 20
            && number.bytes().all(|byte| byte.is_ascii_digit())
            && let Ok(segment) = number.parse::<u64>()
        {
            segments.push(segment);
        }
    }
    segments.sort_unstable();
    segments
}
