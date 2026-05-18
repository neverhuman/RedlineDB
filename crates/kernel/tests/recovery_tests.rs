use redlinedb_kernel::Error;
use redlinedb_kernel::engine::{CommitOutcome, Engine, EngineConfig};
use redlinedb_kernel::format::{Csn, RelId, RowId};
use redlinedb_kernel::txn::Isolation;
use redlinedb_kernel::wal::{WalConfig, WalPayload};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

fn config() -> EngineConfig {
    EngineConfig {
        rel_id: RelId(1),
        wal: WalConfig {
            segment_bytes: 65536,
            ..WalConfig::default()
        },
        commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
        lock_shards: 32,
        busy_timeout: Duration::from_millis(250),
        heap_lanes: 16,
        page_size: redlinedb_kernel::format::DEFAULT_PAGE_SIZE,
        buffer_pool_pages: 256,
        data_file_name: "data.redline".to_owned(),
    }
}

#[test]
fn committed_insert_survives_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"alpha".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"alpha".to_vec()));
}

#[test]
fn open_with_recovery_report_records_page_image_redo() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"alpha".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut tx, row, b"beta".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let (reopened, report) = Engine::open_with_recovery_report(temp.path(), config()).unwrap();
    assert!(report.scanned_records > 0);
    assert!(report.valid_end_lsn.0 > 0);
    assert!(report.legacy_mutations_redone > 0);
    assert!(report.commits_recovered > 0);
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"beta".to_vec()));
}

#[test]
fn uncommitted_insert_is_absent_after_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"ghost".to_vec()).unwrap();
    drop(tx);
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), None);
}

#[test]
fn committed_update_and_delete_survive_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let updated = engine.insert(&mut tx, b"old".to_vec()).unwrap();
    let deleted = engine.insert(&mut tx, b"delete me".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut tx, updated, b"new".to_vec()).unwrap();
    engine.delete(&mut tx, deleted).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut tx, updated).unwrap(),
        Some(b"new".to_vec())
    );
    assert_eq!(reopened.get(&mut tx, deleted).unwrap(), None);
}

#[test]
fn same_rowid_relation_update_and_delete_survive_recovery() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let rel_a = RelId(31);
    let rel_b = RelId(32);
    let row = RowId(9);

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .insert_for_relation(&mut tx, rel_a, row, b"a0".to_vec())
        .unwrap();
    engine
        .insert_for_relation(&mut tx, rel_b, row, b"b0".to_vec())
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .update_for_relation(&mut tx, rel_a, row, b"a1".to_vec())
        .unwrap();
    engine.delete_for_relation(&mut tx, rel_b, row).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get_for_relation(&mut tx, rel_a, row).unwrap(),
        Some(b"a1".to_vec())
    );
    assert_eq!(
        reopened.get_for_relation(&mut tx, rel_b, row).unwrap(),
        None
    );
}

#[test]
fn rolled_back_update_and_delete_do_not_survive_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"live".to_vec()).unwrap();
    let keep = engine.insert(&mut tx, b"keep".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .update(&mut tx, row, b"rolled back".to_vec())
        .unwrap();
    engine.delete(&mut tx, keep).unwrap();
    engine.rollback(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"live".to_vec()));
    assert_eq!(reopened.get(&mut tx, keep).unwrap(), Some(b"keep".to_vec()));
}

#[test]
fn multiple_committed_updates_recover_latest_visible_row() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"v0".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    for idx in 1..8 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .update(&mut tx, row, format!("v{idx}").into_bytes())
            .unwrap();
        engine.commit(tx).unwrap();
    }
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"v7".to_vec()));
}

#[test]
fn concurrent_commits_recover_all_durable_rows() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                let row = engine
                    .insert(&mut tx, format!("row-{idx}").into_bytes())
                    .unwrap();
                barrier.wait();
                engine.commit(tx).unwrap();
                row
            })
        })
        .collect();
    let rows: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    for (idx, row) in rows.into_iter().enumerate() {
        assert_eq!(
            reopened.get(&mut tx, row).unwrap(),
            Some(format!("row-{idx}").into_bytes())
        );
    }
}

#[test]
fn recovered_engine_allocates_above_recovered_ids() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let old_row = engine.insert(&mut tx, b"old".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    let new_row = reopened.insert(&mut tx, b"new".to_vec()).unwrap();
    let csn = match reopened.commit(tx).unwrap() {
        CommitOutcome::Committed(csn) => csn,
        outcome => panic!("unexpected commit outcome: {outcome:?}"),
    };
    assert!(new_row > old_row);
    assert!(csn.0 > 1);
}

#[test]
fn checkpoint_writes_control_file_and_flushes_heap_pages() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"checkpointed".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let checkpoint = engine.checkpoint().unwrap();
    assert_eq!(checkpoint.generation, 1);
    assert!(checkpoint.checkpoint_lsn.0 > 0);
    assert!(checkpoint.page_count > 0);
    assert_eq!(engine.checkpoint_info().unwrap(), Some(checkpoint));
    assert!(
        std::fs::metadata(temp.path().join("data.redline"))
            .unwrap()
            .len()
            > 0
    );
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    assert_eq!(reopened.checkpoint_info().unwrap(), Some(checkpoint));
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut tx, row).unwrap(),
        Some(b"checkpointed".to_vec())
    );
}

#[test]
fn checkpoint_with_stats_reports_incremental_flush_work() {
    let temp = TempDir::new().unwrap();
    let mut config = config();
    config.page_size = 512;
    config.buffer_pool_pages = 256;
    let engine = Engine::create(temp.path(), config).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    for idx in 0..80 {
        engine.insert(&mut tx, vec![idx as u8; 128]).unwrap();
    }
    engine.commit(tx).unwrap();

    let stats = engine.checkpoint_with_stats().unwrap();
    assert_eq!(stats.control.generation, 1);
    assert!(stats.control.checkpoint_lsn.0 > 0);
    assert!(stats.flushed_pages > 0);
    assert!(stats.flush_batches > 0);
    assert_eq!(engine.checkpoint_info().unwrap(), Some(stats.control));

    let pool_stats = engine.buffer_pool_stats();
    assert!(pool_stats.writes >= stats.flushed_pages as u64);
    assert!(pool_stats.checkpoint_flushes >= stats.flushed_pages as u64);
}

#[test]
fn checkpoint_can_run_while_concurrent_inserts_commit() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut seed = engine.begin(Isolation::Snapshot).unwrap();
    for idx in 0..32 {
        engine
            .insert(&mut seed, format!("seed-{idx}").into_bytes())
            .unwrap();
    }
    engine.commit(seed).unwrap();

    let checkpoint_engine = Arc::clone(&engine);
    let checkpoint = thread::spawn(move || checkpoint_engine.checkpoint_with_stats().unwrap());

    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                let row = engine
                    .insert(&mut tx, format!("concurrent-{idx}").into_bytes())
                    .unwrap();
                engine.commit(tx).unwrap();
                (idx, row)
            })
        })
        .collect();

    let checkpoint = checkpoint.join().unwrap();
    assert!(checkpoint.control.checkpoint_lsn.0 > 0);
    let rows: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    for (idx, row) in rows {
        assert_eq!(
            reopened.get(&mut tx, row).unwrap(),
            Some(format!("concurrent-{idx}").into_bytes())
        );
    }
}

#[test]
fn checkpoint_generation_advances_and_open_falls_back_to_valid_control_copy() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.insert(&mut tx, b"first".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let first = engine.checkpoint().unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"second".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let second = engine.checkpoint().unwrap();
    assert_eq!(second.generation, first.generation + 1);
    drop(engine);

    let mut newer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("CONTROL_B"))
        .unwrap();
    newer.seek(SeekFrom::Start(24)).unwrap();
    newer.write_all(&[0xdd]).unwrap();
    newer.sync_data().unwrap();

    let reopened = Engine::open(temp.path(), config()).unwrap();
    assert_eq!(reopened.checkpoint_info().unwrap(), Some(first));
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut tx, row).unwrap(),
        Some(b"second".to_vec())
    );
}

#[test]
fn open_uses_checkpointed_page_file_when_control_exists() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.insert(&mut tx, b"checkpointed".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    engine.checkpoint().unwrap();
    drop(engine);

    let mut data = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("data.redline"))
        .unwrap();
    data.seek(SeekFrom::Start(128)).unwrap();
    data.write_all(&[0xaa]).unwrap();
    data.sync_data().unwrap();

    let err = Engine::open(temp.path(), config()).unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn post_checkpoint_update_replays_on_top_of_checkpointed_pages() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"before".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    engine.checkpoint().unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut tx, row, b"after".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"after".to_vec()));
}

#[test]
fn post_checkpoint_uncommitted_insert_is_ignored_after_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.insert(&mut tx, b"base".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    engine.checkpoint().unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"ghost".to_vec()).unwrap();
    drop(tx);
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, row).unwrap(), None);
}

#[test]
fn checkpoint_prunes_stale_wal_segments() {
    let mut config = config();
    config.wal.segment_bytes = 65_536;
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config.clone()).unwrap();

    let mut rows = Vec::new();
    while wal_segment_count(temp.path().join("wal").as_path()).len() < 3 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        rows.push(engine.insert(&mut tx, b"warm".to_vec()).unwrap());
        engine.commit(tx).unwrap();
    }

    let baseline_count = rows.len();
    let checkpoint = engine.checkpoint().unwrap();
    assert!(checkpoint.checkpoint_lsn.0 > 0);

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let fresh_row = engine
        .insert(&mut tx, b"after-checkpoint".to_vec())
        .unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let segments_after = wal_segment_count(temp.path().join("wal").as_path());
    let keep_segment = checkpoint.checkpoint_lsn.0 / config.wal.segment_bytes + 1;
    assert!(!segments_after.is_empty());
    assert!(
        segments_after
            .iter()
            .all(|segment| *segment >= keep_segment)
    );

    let reopened = Engine::open(temp.path(), config).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    for row in rows.into_iter().take(baseline_count) {
        assert_eq!(reopened.get(&mut tx, row).unwrap(), Some(b"warm".to_vec()));
    }
    assert_eq!(
        reopened.get(&mut tx, fresh_row).unwrap(),
        Some(b"after-checkpoint".to_vec())
    );
}

#[test]
fn checkpointed_rows_recover_without_pre_checkpoint_wal() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"wal-pruned".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let checkpoint = engine.checkpoint().unwrap();
    drop(engine);

    std::fs::remove_dir_all(temp.path().join("wal")).unwrap();

    let reopened = Engine::open(temp.path(), config()).unwrap();
    assert_eq!(reopened.checkpoint_info().unwrap(), Some(checkpoint));
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut tx, row).unwrap(),
        Some(b"wal-pruned".to_vec())
    );
}

#[test]
fn reusable_pages_survive_checkpoint_and_reopen() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"live".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.delete(&mut tx, row).unwrap();
    engine.commit(tx).unwrap();

    engine.vacuum_with_horizon(Csn(100)).unwrap();
    engine.checkpoint().unwrap();
    let before = std::fs::metadata(temp.path().join("data.redline"))
        .unwrap()
        .len();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    let new_row = reopened.insert(&mut tx, b"again".to_vec()).unwrap();
    reopened.commit(tx).unwrap();
    let after = std::fs::metadata(temp.path().join("data.redline"))
        .unwrap()
        .len();

    assert_eq!(before, after);
    let mut verify = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut verify, new_row).unwrap(),
        Some(b"again".to_vec())
    );
}

#[test]
fn missing_tx_status_checkpoint_fails_open_with_checkpoint_control() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.insert(&mut tx, b"needs status".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let checkpoint = engine.checkpoint().unwrap();
    drop(engine);

    std::fs::remove_file(
        temp.path()
            .join(format!("TX_STATUS_{:020}", checkpoint.generation)),
    )
    .unwrap();

    let err = Engine::open(temp.path(), config()).unwrap_err();
    assert!(matches!(err, Error::Io(_)));
}

#[test]
fn truncated_final_wal_record_is_ignored_by_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"stable".to_vec()).unwrap();
    engine.commit(tx).unwrap();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let _ghost = engine.insert(&mut tx, b"tail".to_vec()).unwrap();
    drop(tx);
    drop(engine);

    truncate_wal_tail(temp.path(), 3);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut tx, row).unwrap(),
        Some(b"stable".to_vec())
    );
}

#[test]
fn corrupt_non_final_wal_record_fails_open() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();
    for idx in 0..3 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .insert(&mut tx, format!("row-{idx}").into_bytes())
            .unwrap();
        engine.commit(tx).unwrap();
    }
    drop(engine);

    corrupt_second_wal_record_payload(temp.path());

    let err = Engine::open(temp.path(), config()).unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn old_commit_payload_shape_is_not_accepted_for_commit_records() {
    let err = WalPayload::decode(&[1, 2, 3]).unwrap_err();
    assert!(matches!(err, Error::BufferTooSmall { .. }));
}

#[test]
fn create_index_atomicity_under_simulated_crash_mid_backfill() {
    use redlinedb_kernel::catalog::{
        ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
        IndexColumnSpec, IndexOrigin, QualifiedName, SchemaId, SortDir, ValueRef, encode_record,
    };

    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();

    // CREATE TABLE t(id INTEGER PRIMARY KEY, v BLOB) and seed three rows.
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t"),
                if_not_exists: false,
                columns: vec![
                    ColumnSpec {
                        name: DbName::new("id"),
                        declared_type: Some("INTEGER".to_owned()),
                        constraints: vec![ColumnConstraintSpec::PrimaryKey {
                            sort_dir: SortDir::Asc,
                            conflict: ConflictAction::Abort,
                        }],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                    ColumnSpec {
                        name: DbName::new("v"),
                        declared_type: Some("BLOB".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                ],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE t (id INTEGER PRIMARY KEY, v BLOB)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    for i in 1..=3_i64 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        let mut buf = Vec::new();
        encode_record(
            &[ValueRef::Integer(i), ValueRef::Blob(&[0xc0, i as u8])],
            &mut buf,
        )
        .unwrap();
        let row_id = engine.reserve_row_id();
        engine
            .insert_for_relation(&mut tx, table.relation_id, row_id, buf)
            .unwrap();
        engine.commit(tx).unwrap();
    }

    // BEGIN CREATE INDEX -> backfill happens -> simulate crash by dropping
    // the engine BEFORE commit. The pending tx never commits, so its WAL
    // payloads (PageImage records logged inside the b-tree backfill) MUST be
    // discarded by recovery and the catalog must NOT advertise the index.
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let _ = engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_v"),
                if_not_exists: false,
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("t"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("v"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                    expr_sql: None,
                    expr_referenced_cols: Vec::new(),
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_v ON t(v)".to_owned()),
                predicate_sql: None,
            },
        )
        .unwrap();
    drop(tx); // simulated crash mid-backfill: tx never commits.
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let snapshot = reopened.schema_snapshot();
    // The CREATE INDEX never committed, so it must NOT appear after recovery.
    assert!(
        snapshot.lookup_index(SchemaId(1), "ix_v").is_none(),
        "uncommitted CREATE INDEX must not survive recovery"
    );
    // The base table must still be present.
    assert!(snapshot.lookup_table(SchemaId(1), "t").is_some());
}

#[test]
fn create_index_with_backfill_recovers_meta_page_id_after_commit() {
    use redlinedb_kernel::catalog::{
        ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
        IndexColumnSpec, IndexOrigin, QualifiedName, SchemaId, SortDir, ValueRef, encode_record,
    };

    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t"),
                if_not_exists: false,
                columns: vec![
                    ColumnSpec {
                        name: DbName::new("id"),
                        declared_type: Some("INTEGER".to_owned()),
                        constraints: vec![ColumnConstraintSpec::PrimaryKey {
                            sort_dir: SortDir::Asc,
                            conflict: ConflictAction::Abort,
                        }],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                    ColumnSpec {
                        name: DbName::new("v"),
                        declared_type: Some("TEXT".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                ],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    for i in 1..=4_i64 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        let value = format!("v{i}");
        let mut buf = Vec::new();
        encode_record(&[ValueRef::Integer(i), ValueRef::Text(&value)], &mut buf).unwrap();
        let row_id = engine.reserve_row_id();
        engine
            .insert_for_relation(&mut tx, table.relation_id, row_id, buf)
            .unwrap();
        engine.commit(tx).unwrap();
    }

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_v"),
                if_not_exists: false,
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("t"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("v"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                    expr_sql: None,
                    expr_referenced_cols: Vec::new(),
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_v ON t(v)".to_owned()),
                predicate_sql: None,
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let snapshot = reopened.schema_snapshot();
    let index = snapshot
        .lookup_index(SchemaId(1), "ix_v")
        .expect("committed CREATE INDEX must survive recovery");
    assert!(
        index.meta_page_id.is_some(),
        "meta_page_id must survive recovery for committed CREATE INDEX"
    );
    assert!(
        reopened.index_handle(index.index_id).is_some(),
        "index handle must rehydrate from catalog snapshot"
    );
}

#[test]
fn committed_heap_and_index_deltas_replay_after_reopen() {
    use redlinedb_kernel::catalog::{
        ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
        IndexColumnSpec, IndexOrigin, QualifiedName, SortDir, ValueRef, encode_record,
    };
    use redlinedb_kernel::format::{PageGeneration, PageId, TuplePtr};
    use redlinedb_kernel::index::IndexRowRef;

    let temp = TempDir::new().unwrap();
    let engine = Engine::create(temp.path(), config()).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t"),
                if_not_exists: false,
                columns: vec![
                    ColumnSpec {
                        name: DbName::new("id"),
                        declared_type: Some("INTEGER".to_owned()),
                        constraints: vec![ColumnConstraintSpec::PrimaryKey {
                            sort_dir: SortDir::Asc,
                            conflict: ConflictAction::Abort,
                        }],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                    ColumnSpec {
                        name: DbName::new("v"),
                        declared_type: Some("TEXT".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                        generated: None,
                    },
                ],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let index = engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_v"),
                if_not_exists: false,
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("t"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("v"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                    expr_sql: None,
                    expr_referenced_cols: Vec::new(),
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_v ON t(v)".to_owned()),
                predicate_sql: None,
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    let row_ref = IndexRowRef::with_row_id(
        RowId(1),
        TuplePtr::new_with_generation(PageId(9), 0, PageGeneration::ONE),
    );
    let mut buf = Vec::new();
    encode_record(&[ValueRef::Integer(1), ValueRef::Text("alpha")], &mut buf).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .insert_for_relation(&mut tx, table.relation_id, RowId(1), buf.clone())
        .unwrap();
    engine
        .index_handle(index.index_id)
        .expect("index handle available")
        .insert_tx(tx.id(), b"alpha", row_ref)
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .delete_for_relation(&mut tx, table.relation_id, RowId(1))
        .unwrap();
    engine
        .index_handle(index.index_id)
        .expect("index handle available")
        .delete_mark_tx(tx.id(), b"alpha", row_ref)
        .unwrap();
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(temp.path(), config()).unwrap();
    let reopened_index = reopened
        .index_handle(index.index_id)
        .expect("reopened index handle");
    assert_eq!(
        reopened_index.point_lookup(b"alpha").unwrap(),
        Vec::<IndexRowRef>::new()
    );
    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(reopened.get(&mut tx, RowId(1)).unwrap(), None);
}

fn wal_segment_count(path: &std::path::Path) -> Vec<u64> {
    let mut segments = Vec::new();
    if !path.exists() {
        return segments;
    }
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(number) = name.strip_suffix(".wal")
            && number.len() == 20
            && let Ok(segment) = number.parse()
        {
            segments.push(segment);
        }
    }
    segments.sort_unstable();
    segments
}

fn truncate_wal_tail(path: &std::path::Path, bytes: u64) {
    let wal_path = path.join("wal").join("00000000000000000001.wal");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(wal_path)
        .unwrap();
    let len = file.metadata().unwrap().len();
    file.set_len(len - bytes).unwrap();
}

fn corrupt_second_wal_record_payload(path: &std::path::Path) {
    let wal_path = path.join("wal").join("00000000000000000001.wal");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(wal_path)
        .unwrap();

    let first_len = next_record_len(&mut file, 0);
    let second_payload_byte = first_len + 48;
    file.seek(SeekFrom::Start(second_payload_byte)).unwrap();
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 0x01;
    file.seek(SeekFrom::Start(second_payload_byte)).unwrap();
    file.write_all(&byte).unwrap();
}

fn next_record_len(file: &mut std::fs::File, offset: u64) -> u64 {
    file.seek(SeekFrom::Start(offset + 12)).unwrap();
    let mut len = [0_u8; 4];
    file.read_exact(&mut len).unwrap();
    48 + u32::from_le_bytes(len) as u64
}
