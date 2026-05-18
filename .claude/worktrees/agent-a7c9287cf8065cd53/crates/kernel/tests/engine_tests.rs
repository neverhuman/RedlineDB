use redlinedb_kernel::Error;
use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
    IndexColumnSpec, IndexOrigin, QualifiedName, SchemaId, SortDir, ValueRef, encode_record,
};
use redlinedb_kernel::engine::{CommitOutcome, Engine, EngineConfig};
use redlinedb_kernel::format::{Csn, RelId, RowId};
use redlinedb_kernel::index::{BtreeIndex, INDEX_VERSION};
use redlinedb_kernel::storage::PageFile;
use redlinedb_kernel::txn::{Isolation, TxState};
use redlinedb_kernel::wal::{WalConfig, WalPayload, WalReader, WalRecordKind};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

fn test_engine() -> (TempDir, Arc<Engine>) {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(
        temp.path(),
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
        },
    )
    .unwrap();
    (temp, engine)
}

#[test]
fn engine_uses_bounded_page_backed_heap_residency() {
    let temp = TempDir::new().unwrap();
    let engine = Engine::create(
        temp.path(),
        EngineConfig {
            rel_id: RelId(1),
            wal: WalConfig {
                segment_bytes: 65536,
                ..WalConfig::default()
            },
            commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
            lock_shards: 32,
            busy_timeout: Duration::from_millis(250),
            heap_lanes: 4,
            page_size: redlinedb_kernel::format::DEFAULT_PAGE_SIZE,
            buffer_pool_pages: 16,
            data_file_name: "data.redline".to_owned(),
        },
    )
    .unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    for idx in 0..80 {
        engine.insert(&mut tx, vec![idx as u8; 1024]).unwrap();
    }
    engine.commit(tx).unwrap();

    assert!(engine.resident_heap_pages() <= 16);
}

#[test]
fn storage_stats_snapshot_reports_core_counters() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.insert(&mut tx, b"alpha".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let stats = engine.storage_stats().unwrap();
    assert!(stats.wal_written_lsn.0 > 0);
    assert!(stats.wal_durable_lsn.0 > 0);
    assert!(stats.buffer.resident_pages >= 1);
    assert!(stats.tx.committed_states >= 1);
}

#[test]
fn engine_rebuilds_v1_index_meta_on_open() {
    let (temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t_migrate"),
                columns: vec![ColumnSpec {
                    name: DbName::new("v"),
                    declared_type: Some("TEXT".to_owned()),
                    constraints: Vec::new(),
                    collation: None,
                    default_value: None,
                }],
                constraints: Vec::new(),
                if_not_exists: false,
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE t_migrate(v TEXT)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.reserve_row_id();
    let mut payload = Vec::new();
    encode_record(&[ValueRef::Text("alpha")], &mut payload).unwrap();
    engine
        .insert_for_relation(&mut tx, table.relation_id, row, payload)
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let index = engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_t_migrate_v"),
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("t_migrate"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("v"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_t_migrate_v ON t_migrate(v)".to_owned()),
            },
        )
        .unwrap();
    let old_meta = index.meta_page_id.unwrap();
    engine.commit(tx).unwrap();
    engine.checkpoint().unwrap();
    drop(engine);

    let page_file = PageFile::open(
        temp.path().join("data.redline"),
        redlinedb_kernel::format::DEFAULT_PAGE_SIZE,
    )
    .unwrap();
    let mut page = page_file.read_page(old_meta).unwrap();
    redlinedb_kernel::format::bytes::write_u16(page.special_bytes_mut().unwrap(), 4, 1).unwrap();
    let lsn = page.header().unwrap().page_lsn;
    page.set_page_lsn(lsn).unwrap();
    page_file.write_page(&page).unwrap();
    page_file.sync_data().unwrap();
    drop(page_file);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    let migrated = reopened
        .schema_snapshot()
        .indexes
        .iter()
        .find(|idx| idx.name.as_ref() == "ix_t_migrate_v")
        .unwrap()
        .clone();
    let new_meta = migrated.meta_page_id.unwrap();
    assert_ne!(old_meta, new_meta);
    assert_eq!(
        BtreeIndex::format_version(reopened.buffer_pool_for_tests(), new_meta).unwrap(),
        INDEX_VERSION
    );
    assert!(reopened.index_handle(migrated.index_id).is_some());
}

#[test]
fn engine_heap_pages_do_not_flush_before_wal_is_durable() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"alpha".to_vec()).unwrap();
    engine.update(&mut tx, row, b"beta".to_vec()).unwrap();

    let err = engine.flush_heap_pages().unwrap_err();
    assert_eq!(
        err,
        Error::CorruptPage("dirty page lsn exceeds durable wal lsn")
    );

    engine.commit(tx).unwrap();
    engine.flush_heap_pages().unwrap();
}

#[test]
fn uncommitted_index_pages_do_not_flush_before_wal_is_durable() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t"),
                if_not_exists: false,
                columns: vec![ColumnSpec {
                    name: DbName::new("v"),
                    declared_type: Some("TEXT".to_owned()),
                    constraints: vec![],
                    collation: None,
                    default_value: None,
                }],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE t(v TEXT)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.reserve_row_id();
    let mut payload = Vec::new();
    encode_record(&[ValueRef::Text("alpha")], &mut payload).unwrap();
    engine
        .insert_for_relation(&mut tx, table.relation_id, row, payload)
        .unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_t_v"),
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("t"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("v"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_t_v ON t(v)".to_owned()),
            },
        )
        .unwrap();

    let err = engine.flush_heap_pages().unwrap_err();
    assert_eq!(
        err,
        Error::CorruptPage("dirty page lsn exceeds durable wal lsn")
    );
    engine.rollback(tx).unwrap();
}

#[test]
fn committed_insert_becomes_visible_only_after_commit() {
    let (_temp, engine) = test_engine();
    let mut writer = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut writer, b"alpha".to_vec()).unwrap();

    let mut observer = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(engine.get(&mut observer, row).unwrap(), None);
    assert_eq!(
        engine.get(&mut writer, row).unwrap(),
        Some(b"alpha".to_vec())
    );

    engine.commit(writer).unwrap();
    let mut later = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut later, row).unwrap(),
        Some(b"alpha".to_vec())
    );
}

#[test]
fn rollback_insert_update_and_delete_remain_invisible() {
    let (_temp, engine) = test_engine();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let rolled_back_row = engine.insert(&mut tx, b"ghost".to_vec()).unwrap();
    engine.rollback(tx).unwrap();
    let mut observer = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(engine.get(&mut observer, rolled_back_row).unwrap(), None);

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"live".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .update(&mut tx, row, b"rolled back".to_vec())
        .unwrap();
    engine.rollback(tx).unwrap();
    let mut observer = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut observer, row).unwrap(),
        Some(b"live".to_vec())
    );

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine.delete(&mut tx, row).unwrap();
    engine.rollback(tx).unwrap();
    let mut observer = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut observer, row).unwrap(),
        Some(b"live".to_vec())
    );
}

#[test]
fn same_rowid_in_different_relations_survives_update_delete_rollback_and_vacuum() {
    let (_temp, engine) = test_engine();
    let rel_a = RelId(21);
    let rel_b = RelId(22);
    let row = RowId(100);

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
    engine.rollback(tx).unwrap();

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get_for_relation(&mut tx, rel_a, row).unwrap(),
        Some(b"a0".to_vec())
    );
    assert_eq!(
        engine.get_for_relation(&mut tx, rel_b, row).unwrap(),
        Some(b"b0".to_vec())
    );

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .update_for_relation(&mut tx, rel_a, row, b"a2".to_vec())
        .unwrap();
    engine.delete_for_relation(&mut tx, rel_b, row).unwrap();
    let delete_csn = match engine.commit(tx).unwrap() {
        CommitOutcome::Committed(csn) => csn,
        outcome => panic!("unexpected commit outcome: {outcome:?}"),
    };

    let retained = engine.vacuum_with_horizon(delete_csn).unwrap();
    assert_eq!(retained.dead_rows_removed, 0);
    let removed = engine.vacuum_with_horizon(Csn(delete_csn.0 + 1)).unwrap();
    assert_eq!(removed.dead_rows_removed, 1);

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get_for_relation(&mut tx, rel_a, row).unwrap(),
        Some(b"a2".to_vec())
    );
    assert_eq!(engine.get_for_relation(&mut tx, rel_b, row).unwrap(), None);
}

#[test]
fn long_snapshot_keeps_old_row_after_update_commits() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"old".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut long_reader = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut long_reader, row).unwrap(),
        Some(b"old".to_vec())
    );

    let mut writer = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut writer, row, b"new".to_vec()).unwrap();
    engine.commit(writer).unwrap();

    assert_eq!(
        engine.get(&mut long_reader, row).unwrap(),
        Some(b"old".to_vec())
    );
    let mut later = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(engine.get(&mut later, row).unwrap(), Some(b"new".to_vec()));
}

#[test]
fn read_committed_refreshes_snapshot_on_later_read() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"old".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut reader = engine.begin(Isolation::ReadCommitted).unwrap();
    assert_eq!(engine.get(&mut reader, row).unwrap(), Some(b"old".to_vec()));

    let mut writer = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut writer, row, b"new".to_vec()).unwrap();
    engine.commit(writer).unwrap();

    assert_eq!(engine.get(&mut reader, row).unwrap(), Some(b"new".to_vec()));
}

#[test]
fn concurrent_disjoint_updates_all_commit() {
    let (_temp, engine) = test_engine();
    let mut seed = engine.begin(Isolation::Snapshot).unwrap();
    let rows: Vec<_> = (0..32)
        .map(|idx| {
            engine
                .insert(&mut seed, format!("old-{idx}").into_bytes())
                .unwrap()
        })
        .collect();
    engine.commit(seed).unwrap();

    let barrier = Arc::new(Barrier::new(rows.len()));
    let handles: Vec<_> = rows
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, row)| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                engine
                    .update(&mut tx, row, format!("new-{idx}").into_bytes())
                    .unwrap();
                match engine.commit(tx).unwrap() {
                    CommitOutcome::Committed(csn) => csn,
                    outcome => panic!("unexpected commit outcome: {outcome:?}"),
                }
            })
        })
        .collect();

    let mut csns = Vec::new();
    for handle in handles {
        csns.push(handle.join().unwrap());
    }
    csns.sort();
    csns.dedup();
    assert_eq!(csns.len(), rows.len());

    let mut reader = engine.begin(Isolation::Snapshot).unwrap();
    for (idx, row) in rows.into_iter().enumerate() {
        assert_eq!(
            engine.get(&mut reader, row).unwrap(),
            Some(format!("new-{idx}").into_bytes())
        );
    }
}

#[test]
fn concurrent_same_row_snapshot_updates_have_one_winner() {
    let (_temp, engine) = test_engine();
    let mut seed = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut seed, b"base".to_vec()).unwrap();
    engine.commit(seed).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                barrier.wait();
                let update = engine.update(&mut tx, row, format!("winner-{idx}").into_bytes());
                if update.is_err() {
                    let _ = engine.rollback(tx);
                    return update.map(|_| Csn(0));
                }
                engine.commit(tx).map(|outcome| match outcome {
                    CommitOutcome::Committed(csn) => csn,
                    CommitOutcome::MaybeCommitted | CommitOutcome::RolledBack => Csn(0),
                })
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners = results.iter().filter(|result| result.is_ok()).count();
    let losers = results
        .iter()
        .filter(|result| {
            matches!(
                result,
                Err(Error::SerializationFailure | Error::LockTimeout)
            )
        })
        .count();
    assert_eq!(winners, 1);
    assert_eq!(losers, 1);
}

#[test]
fn reader_does_not_block_while_writers_commit() {
    let (_temp, engine) = test_engine();
    let mut seed = engine.begin(Isolation::Snapshot).unwrap();
    let rows: Vec<_> = (0..16)
        .map(|idx| {
            engine
                .insert(&mut seed, format!("old-{idx}").into_bytes())
                .unwrap()
        })
        .collect();
    engine.commit(seed).unwrap();

    let reader_engine = Arc::clone(&engine);
    let reader_rows = rows.clone();
    let reader = thread::spawn(move || {
        for _ in 0..100 {
            let mut tx = reader_engine.begin(Isolation::Snapshot).unwrap();
            for row in &reader_rows {
                let _ = reader_engine.get(&mut tx, *row).unwrap();
            }
            reader_engine.rollback(tx).unwrap();
        }
    });

    let writers: Vec<_> = rows
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, row)| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || {
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                engine
                    .update(&mut tx, row, format!("new-{idx}").into_bytes())
                    .unwrap();
                engine.commit(tx).unwrap();
            })
        })
        .collect();

    for writer in writers {
        writer.join().unwrap();
    }
    reader.join().unwrap();
}

#[test]
fn commit_records_are_written_in_csn_order() {
    let (temp, engine) = test_engine();
    for idx in 0..4 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .insert(&mut tx, format!("row-{idx}").into_bytes())
            .unwrap();
        engine.commit(tx).unwrap();
    }

    let mut reader = WalReader::new(
        temp.path().join("wal"),
        WalConfig {
            segment_bytes: 65536,
            ..WalConfig::default()
        },
    );
    let records: Vec<_> = reader
        .scan()
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == WalRecordKind::Commit)
        .collect();
    assert_eq!(records.len(), 4);
    let mut last_lsn = 0;
    let mut last_csn = 0;
    for record in records {
        assert_eq!(record.kind, WalRecordKind::Commit);
        assert!(record.lsn.0 >= last_lsn);
        let WalPayload::Commit { tx_id, csn } = WalPayload::decode(&record.payload).unwrap() else {
            panic!("expected commit payload");
        };
        assert!(tx_id.0 > 0);
        assert!(csn.0 > last_csn);
        assert_eq!(engine.tx_state(tx_id), TxState::Committed(csn));
        last_lsn = record.lsn.0;
        last_csn = csn.0;
    }
}

#[test]
fn concurrent_autocommit_inserts_recover_after_group_commit() {
    let (temp, engine) = test_engine();
    let barrier = Arc::new(Barrier::new(32));
    let handles: Vec<_> = (0..32)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                let row = engine
                    .insert(&mut tx, format!("group-{idx}").into_bytes())
                    .unwrap();
                barrier.wait();
                let csn = match engine.commit(tx).unwrap() {
                    CommitOutcome::Committed(csn) => csn,
                    outcome => panic!("unexpected commit outcome: {outcome:?}"),
                };
                (idx, row, csn)
            })
        })
        .collect();
    let mut rows: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    rows.sort_by_key(|(idx, _, _)| *idx);
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    let mut reader = reopened.begin(Isolation::Snapshot).unwrap();
    for (idx, row, csn) in rows {
        assert!(csn.0 > 0);
        assert_eq!(
            reopened.get(&mut reader, row).unwrap(),
            Some(format!("group-{idx}").into_bytes())
        );
    }
}

#[test]
fn concurrent_commit_records_are_written_in_csn_order() {
    let (temp, engine) = test_engine();
    let barrier = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut tx = engine.begin(Isolation::Snapshot).unwrap();
                engine
                    .insert(&mut tx, format!("row-{idx}").into_bytes())
                    .unwrap();
                barrier.wait();
                engine.commit(tx).unwrap()
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let mut reader = WalReader::new(
        temp.path().join("wal"),
        WalConfig {
            segment_bytes: 65536,
            ..WalConfig::default()
        },
    );
    let records: Vec<_> = reader
        .scan()
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == WalRecordKind::Commit)
        .collect();
    assert_eq!(records.len(), 16);

    let mut last_csn = 0;
    for record in records {
        let WalPayload::Commit { csn, .. } = WalPayload::decode(&record.payload).unwrap() else {
            panic!("expected commit payload");
        };
        assert!(csn.0 > last_csn);
        last_csn = csn.0;
    }
}

#[test]
fn dropped_open_transaction_aborts_and_unregisters_snapshot() {
    let (_temp, engine) = test_engine();
    let tx = engine.begin(Isolation::Snapshot).unwrap();
    let tx_id = tx.id();
    assert_eq!(engine.tx_status_stats().active_transactions, 1);
    assert_eq!(engine.tx_status_stats().active_snapshots, 1);
    drop(tx);

    assert_eq!(engine.tx_state(tx_id), TxState::Aborted);
    assert_eq!(engine.tx_status_stats().active_transactions, 0);
    assert_eq!(engine.tx_status_stats().active_snapshots, 0);
}

#[test]
fn checkpoint_reopen_restores_tx_frontier_metadata() {
    let (temp, engine) = test_engine();
    for idx in 0..8 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .insert(&mut tx, format!("frontier-{idx}").into_bytes())
            .unwrap();
        engine.commit(tx).unwrap();
    }
    let before = engine.tx_status_stats();
    assert_eq!(before.published_csn, Csn(8));
    engine.checkpoint().unwrap();
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    let after = reopened.tx_status_stats();
    assert_eq!(after.published_csn, before.published_csn);
    assert!(after.next_tx >= before.next_tx);
    assert!(after.next_csn >= before.next_csn);

    let mut tx = reopened.begin(Isolation::Snapshot).unwrap();
    let row = reopened
        .insert(&mut tx, b"after-frontier".to_vec())
        .unwrap();
    let csn = match reopened.commit(tx).unwrap() {
        CommitOutcome::Committed(csn) => csn,
        outcome => panic!("unexpected commit outcome: {outcome:?}"),
    };
    assert!(csn > before.published_csn);
    let mut reader = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut reader, row).unwrap(),
        Some(b"after-frontier".to_vec())
    );
}

#[test]
fn engine_vacuum_respects_active_snapshot_horizon() {
    let (_temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"old".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    let mut long_reader = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut long_reader, row).unwrap(),
        Some(b"old".to_vec())
    );

    let mut writer = engine.begin(Isolation::Snapshot).unwrap();
    engine.update(&mut writer, row, b"new".to_vec()).unwrap();
    engine.commit(writer).unwrap();

    let retained = engine.vacuum().unwrap();
    assert_eq!(retained.oldest_active_snapshot_csn, Csn(1));
    assert_eq!(retained.chains_pruned, 0);
    assert_eq!(
        engine.get(&mut long_reader, row).unwrap(),
        Some(b"old".to_vec())
    );

    engine.rollback(long_reader).unwrap();
    let mut advance = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .insert(&mut advance, b"advance-horizon".to_vec())
        .unwrap();
    engine.commit(advance).unwrap();

    let pruned = engine.vacuum().unwrap();
    assert_eq!(pruned.oldest_active_snapshot_csn, Csn(3));
    assert_eq!(pruned.chains_pruned, 1);
    assert_eq!(pruned.undo_links_removed, 1);

    let mut later = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(engine.get(&mut later, row).unwrap(), Some(b"new".to_vec()));
}

#[test]
fn vacuumed_latest_row_survives_checkpoint_reopen() {
    let (temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let row = engine.insert(&mut tx, b"v0".to_vec()).unwrap();
    engine.commit(tx).unwrap();

    for idx in 1..4 {
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .update(&mut tx, row, format!("v{idx}").into_bytes())
            .unwrap();
        engine.commit(tx).unwrap();
    }

    let stats = engine.vacuum_with_horizon(Csn(100)).unwrap();
    assert_eq!(stats.chains_pruned, 1);
    assert_eq!(stats.undo_links_removed, 3);
    engine.checkpoint().unwrap();
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    let mut reader = reopened.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        reopened.get(&mut reader, row).unwrap(),
        Some(b"v3".to_vec())
    );
}

#[test]
fn serializable_is_rejected_until_ssi_exists() {
    let (_temp, engine) = test_engine();
    let err = engine.begin(Isolation::Serializable).unwrap_err();
    assert_eq!(err, Error::UnsupportedIsolation);
}

#[test]
fn ddl_create_table_and_index_survive_reopen() {
    let (temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let table = engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("widgets"),
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
                    },
                    ColumnSpec {
                        name: DbName::new("name"),
                        declared_type: Some("TEXT".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                    },
                ],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some(
                    "CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT)".to_owned(),
                ),
            },
        )
        .unwrap();
    assert_eq!(table.name.as_ref(), "widgets");
    let epoch_after_table = engine.schema_epoch();
    engine.commit(tx).unwrap();
    assert!(engine.schema_epoch() >= epoch_after_table);

    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let index = engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("widgets_name_idx"),
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("widgets"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("name"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX widgets_name_idx ON widgets(name)".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(index.name.as_ref(), "widgets_name_idx");
    engine.commit(tx).unwrap();
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    let snapshot = reopened.schema_snapshot();
    assert!(snapshot.lookup_table(SchemaId(1), "widgets").is_some());
    assert!(
        snapshot
            .lookup_index(SchemaId(1), "widgets_name_idx")
            .is_some()
    );
    let schema_rows = reopened.sqlite_schema();
    assert!(schema_rows.iter().any(|row| row.name.as_ref() == "widgets"));
    assert!(
        schema_rows
            .iter()
            .any(|row| row.name.as_ref() == "widgets_name_idx")
    );
}

#[test]
fn ddl_reopens_when_catalog_sidecar_is_deleted() {
    let (temp, engine) = test_engine();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("recoverable"),
                if_not_exists: false,
                columns: vec![ColumnSpec {
                    name: DbName::new("id"),
                    declared_type: Some("INTEGER".to_owned()),
                    constraints: vec![ColumnConstraintSpec::PrimaryKey {
                        sort_dir: SortDir::Asc,
                        conflict: ConflictAction::Abort,
                    }],
                    collation: None,
                    default_value: None,
                }],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some(
                    "CREATE TABLE recoverable (id INTEGER PRIMARY KEY)".to_owned(),
                ),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();
    std::fs::remove_file(temp.path().join("schema.redline")).unwrap();
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
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
        },
    )
    .unwrap();
    assert!(
        reopened
            .schema_snapshot()
            .lookup_table(SchemaId(1), "recoverable")
            .is_some()
    );
}

#[test]
fn ddl_rollback_discards_catalog_changes_and_epoch_validation_detects_stale_snapshots() {
    let (_temp, engine) = test_engine();
    let epoch_before = engine.schema_epoch();
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("ghosts"),
                if_not_exists: false,
                columns: vec![ColumnSpec {
                    name: DbName::new("id"),
                    declared_type: Some("INTEGER".to_owned()),
                    constraints: vec![ColumnConstraintSpec::PrimaryKey {
                        sort_dir: SortDir::Asc,
                        conflict: ConflictAction::Abort,
                    }],
                    collation: None,
                    default_value: None,
                }],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE ghosts (id INTEGER PRIMARY KEY)".to_owned()),
            },
        )
        .unwrap();
    engine.rollback(tx).unwrap();
    assert_eq!(engine.schema_epoch(), epoch_before);
    assert!(
        engine
            .schema_snapshot()
            .lookup_table(SchemaId(1), "ghosts")
            .is_none()
    );
    assert_eq!(engine.validate_schema_epoch(epoch_before), Ok(()));
    assert_eq!(
        engine.validate_schema_epoch(redlinedb_kernel::catalog::SchemaEpoch(9999)),
        Err(Error::SchemaChanged)
    );
}
