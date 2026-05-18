use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use redlinedb_kernel::engine::ConcurrentTxStatus;
use redlinedb_kernel::format::TxId;
use redlinedb_kernel::format::{DEFAULT_PAGE_SIZE, Lsn, Page, PageId, PageKind, RelId, TuplePtr};
use redlinedb_kernel::index::{BtreeIndex, IndexDescriptor, IndexId, IndexRowRef, IndexUniqueness};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::wal::{WalConfig, WalManager, WalPayload, WalReader, WalRecordKind};

#[test]
fn page_special_area_round_trips() {
    let mut page = Page::new_with_special(
        DEFAULT_PAGE_SIZE,
        PageKind::BtreeLeaf,
        PageId(9),
        RelId(3),
        128,
    )
    .unwrap();
    {
        let special = page.special_bytes_mut().unwrap();
        special[0..4].copy_from_slice(&1234_u32.to_le_bytes());
    }
    page.set_page_lsn(Lsn::new(1)).unwrap();
    let decoded = Page::from_bytes(page.as_bytes().to_vec()).unwrap();
    let special = decoded.special_bytes().unwrap();
    assert_eq!(u32::from_le_bytes(special[0..4].try_into().unwrap()), 1234);
}

#[test]
fn index_insert_lookup() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(1), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    index
        .insert(
            b"k001",
            IndexRowRef::new(TuplePtr::new_with_generation(
                PageId(42),
                1,
                redlinedb_kernel::format::PageGeneration::ONE,
            )),
        )
        .unwrap();
    index
        .insert(
            b"k001",
            IndexRowRef::new(TuplePtr::new_with_generation(
                PageId(43),
                2,
                redlinedb_kernel::format::PageGeneration::ONE,
            )),
        )
        .unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 2);
}

#[test]
fn index_split_and_parent_update() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(2), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    for i in 0..8_u64 {
        let key = format!("k{i:03}");
        index
            .insert(
                key.as_bytes(),
                IndexRowRef::new(TuplePtr::new_with_generation(
                    PageId(100 + i),
                    i as u16,
                    redlinedb_kernel::format::PageGeneration::ONE,
                )),
            )
            .unwrap();
    }

    for i in 0..8_u64 {
        let key = format!("k{i:03}");
        let hits = index.point_lookup(key.as_bytes()).unwrap();
        assert_eq!(hits.len(), 1);
    }

    let range = index.range_scan(b"k000", b"k999").unwrap();
    assert_eq!(range.len(), 8);
    assert!(index.validate().unwrap().errors.is_empty());
}

#[test]
fn index_recursive_split_propagates_beyond_root() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 4096).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(5), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    for i in 0..64_u64 {
        let key = format!("k{i:03}");
        index
            .insert(
                key.as_bytes(),
                IndexRowRef::new(TuplePtr::new_with_generation(
                    PageId(300 + i),
                    i as u16,
                    redlinedb_kernel::format::PageGeneration::ONE,
                )),
            )
            .unwrap();
    }

    for i in 0..64_u64 {
        let key = format!("k{i:03}");
        assert_eq!(index.point_lookup(key.as_bytes()).unwrap().len(), 1);
    }

    assert_eq!(index.range_scan(b"k000", b"k999").unwrap().len(), 64);
    let report = index.validate().unwrap();
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(report.internal_pages >= 2);
}

#[test]
fn index_recovery_replays_page_images_with_torn_tail() {
    let source = TempDir::new().unwrap();
    let source_page_file =
        Arc::new(PageFile::create(source.path().join("source.redline"), 512).unwrap());
    let source_buffer = Arc::new(BufferPool::new(Arc::clone(&source_page_file), 64).unwrap());
    let source_index = BtreeIndex::create(
        Arc::clone(&source_buffer),
        IndexDescriptor::new(IndexId(6), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    for i in 0..18_u64 {
        let key = format!("r{i:03}");
        source_index
            .insert(
                key.as_bytes(),
                IndexRowRef::new(TuplePtr::new_with_generation(
                    PageId(500 + i),
                    i as u16,
                    redlinedb_kernel::format::PageGeneration::ONE,
                )),
            )
            .unwrap();
    }
    // Pages mark dirty at Lsn(1) for any mutation; flush durable horizon must
    // cover those dirty page-LSNs in this synthetic test (no real WAL flow).
    source_buffer.flush_all(Lsn::new(1)).unwrap();

    let wal_dir = source.path().join("wal");
    let mut wal = WalManager::create(&wal_dir, WalConfig::default()).unwrap();
    for page in snapshot_index_pages(&source_buffer, RelId(1)).unwrap() {
        let payload = WalPayload::PageImage {
            page_id: page.header().unwrap().page_id,
            page_lsn: page.header().unwrap().page_lsn,
            page_bytes: page.as_bytes().to_vec(),
        }
        .encode()
        .unwrap();
        wal.append(WalRecordKind::PageImage, TxId::ZERO, payload)
            .unwrap();
    }
    wal.flush().unwrap();

    let torn = WalPayload::PageImage {
        page_id: PageId(9_999),
        page_lsn: Lsn::new(1),
        page_bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
    }
    .encode()
    .unwrap();
    let torn_path = wal_dir.join(format!("{:020}.wal", 1_u64));
    let mut torn_file = OpenOptions::new().append(true).open(&torn_path).unwrap();
    torn_file
        .write_all(&torn[..torn.len().saturating_sub(7)])
        .unwrap();

    let mut reader = WalReader::new(&wal_dir, WalConfig::default());
    let scan = reader.scan_report().unwrap();
    assert!(scan.torn_tail);

    let recovered = TempDir::new().unwrap();
    let recovered_page_file =
        Arc::new(PageFile::create(recovered.path().join("recovered.redline"), 512).unwrap());
    let recovered_buffer = Arc::new(BufferPool::new(Arc::clone(&recovered_page_file), 64).unwrap());
    let recovered_index = BtreeIndex::create(
        Arc::clone(&recovered_buffer),
        IndexDescriptor::new(IndexId(6), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    for record in scan.records {
        if let WalPayload::PageImage {
            page_id: _,
            page_lsn: _,
            page_bytes,
        } = WalPayload::decode(&record.payload).unwrap()
        {
            recovered_index
                .redo_page_image(Page::from_bytes(page_bytes).unwrap())
                .unwrap();
        }
    }

    let reopened = BtreeIndex::open(
        Arc::clone(&recovered_buffer),
        PageId(1),
        IndexDescriptor::new(IndexId(6), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();
    assert_eq!(reopened.point_lookup(b"r003").unwrap().len(), 1);
    assert_eq!(reopened.range_scan(b"r000", b"r999").unwrap().len(), 18);
    assert!(reopened.validate().unwrap().errors.is_empty());
}

#[test]
fn index_concurrent_writers_and_readers_stress() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 4096).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(7), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    let mut writer_handles = Vec::new();
    for writer in 0..4_u64 {
        let index = index.clone();
        writer_handles.push(thread::spawn(move || {
            for i in 0..16_u64 {
                let key = format!("w{writer}_{i:03}");
                index
                    .insert(
                        key.as_bytes(),
                        IndexRowRef::new(TuplePtr::new_with_generation(
                            PageId(800 + writer * 100 + i),
                            i as u16,
                            redlinedb_kernel::format::PageGeneration::ONE,
                        )),
                    )
                    .unwrap();
            }
        }));
    }

    for handle in writer_handles {
        handle.join().unwrap();
    }

    for writer in 0..4_u64 {
        for i in 0..16_u64 {
            let key = format!("w{writer}_{i:03}");
            assert_eq!(index.point_lookup(key.as_bytes()).unwrap().len(), 1);
        }
    }
    assert_eq!(index.range_scan(b"w0_000", b"w9_999").unwrap().len(), 64);
    assert!(index.validate().unwrap().errors.is_empty());
}

#[test]
fn index_redo_page_image_restores_corruption() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(3), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    index
        .insert(
            b"k001",
            IndexRowRef::new(TuplePtr::new_with_generation(
                PageId(77),
                1,
                redlinedb_kernel::format::PageGeneration::ONE,
            )),
        )
        .unwrap();

    let snapshot = buffer
        .pin(PageId(2))
        .unwrap()
        .with_page(|page| Ok(page.clone()))
        .unwrap();

    buffer
        .pin(PageId(2))
        .unwrap()
        .with_page_mut(|page| {
            page.as_mut_bytes_for_io_test()[128] ^= 0xFF;
            Ok(())
        })
        .unwrap();

    index.redo_page_image(snapshot).unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 1);
}

#[test]
fn unique_lock_waits_and_conflict_checks() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(4), RelId(1), IndexUniqueness::Unique),
    )
    .unwrap();

    let lock_guard = index.lock_unique_key(1, b"k001").unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let index_clone = index.clone();
    let handle = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = index_clone.insert_unique(
            2,
            b"k001",
            IndexRowRef::new(TuplePtr::new_with_generation(
                PageId(88),
                1,
                redlinedb_kernel::format::PageGeneration::ONE,
            )),
        );
        done_tx.send(result).unwrap();
    });

    started_rx.recv().unwrap();
    thread::sleep(Duration::from_millis(50));
    drop(lock_guard);

    let result = done_rx.recv().unwrap();
    assert!(result.is_ok());
    handle.join().unwrap();

    index
        .insert_unique(
            1,
            b"k002",
            IndexRowRef::new(TuplePtr::new_with_generation(
                PageId(89),
                2,
                redlinedb_kernel::format::PageGeneration::ONE,
            )),
        )
        .unwrap();
    let duplicate = index.insert_unique(
        3,
        b"k002",
        IndexRowRef::new(TuplePtr::new_with_generation(
            PageId(90),
            3,
            redlinedb_kernel::format::PageGeneration::ONE,
        )),
    );
    assert_eq!(
        duplicate.unwrap_err(),
        redlinedb_kernel::Error::WriteConflict
    );
}

#[test]
fn index_delete_mark_and_compact_leaf() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(8), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    let row1 = IndexRowRef::new(TuplePtr::new_with_generation(
        PageId(200),
        1,
        redlinedb_kernel::format::PageGeneration::ONE,
    ));
    let row2 = IndexRowRef::new(TuplePtr::new_with_generation(
        PageId(201),
        2,
        redlinedb_kernel::format::PageGeneration::ONE,
    ));
    index.insert(b"k001", row1).unwrap();
    index.insert(b"k001", row2).unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 2);

    index.delete_mark(b"k001", row1).unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 1);
    index.compact_leaf_page(PageId(2)).unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 1);

    index.delete_mark(b"k001", row2).unwrap();
    assert_eq!(index.point_lookup(b"k001").unwrap().len(), 0);
    index.compact_leaf_page(PageId(2)).unwrap();
    assert!(index.validate().unwrap().errors.is_empty());
}

#[test]
fn index_mvcc_visibility_tracks_create_and_delete_transactions() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(80), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();
    let txs = ConcurrentTxStatus::new();
    let row = IndexRowRef::with_row_id(
        redlinedb_kernel::format::RowId(1),
        TuplePtr::new_with_generation(
            PageId(200),
            1,
            redlinedb_kernel::format::PageGeneration::ONE,
        ),
    );

    let insert_tx = txs.begin();
    index.insert_tx(insert_tx, b"k001", row).unwrap();
    let other_snapshot = txs.snapshot();
    assert!(
        index
            .point_lookup_visible(&txs, &other_snapshot, None, b"k001")
            .unwrap()
            .is_empty(),
        "uncommitted insert must be invisible to other transactions"
    );
    assert_eq!(
        index
            .point_lookup_visible(&txs, &other_snapshot, Some(insert_tx), b"k001")
            .unwrap(),
        vec![row],
        "own insert must be visible"
    );

    let csn = txs.reserve_csn();
    txs.publish_commit(insert_tx, csn);
    let committed_snapshot = txs.snapshot();
    assert_eq!(
        index
            .point_lookup_visible(&txs, &committed_snapshot, None, b"k001")
            .unwrap(),
        vec![row]
    );

    let delete_tx = txs.begin();
    index.delete_mark_tx(delete_tx, b"k001", row).unwrap();
    let delete_csn = txs.reserve_csn();
    txs.publish_commit(delete_tx, delete_csn);
    let after_delete = txs.snapshot();
    assert!(
        index
            .point_lookup_visible(&txs, &after_delete, None, b"k001")
            .unwrap()
            .is_empty(),
        "committed delete must hide entry"
    );
    assert_eq!(
        index
            .point_lookup_visible(&txs, &committed_snapshot, None, b"k001")
            .unwrap(),
        vec![row],
        "older snapshot must still see the pre-delete entry"
    );
}

#[test]
fn index_mvcc_aborted_insert_and_delete_are_invisible() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(81), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();
    let txs = ConcurrentTxStatus::new();
    let row = IndexRowRef::with_row_id(
        redlinedb_kernel::format::RowId(2),
        TuplePtr::new_with_generation(
            PageId(201),
            2,
            redlinedb_kernel::format::PageGeneration::ONE,
        ),
    );
    let aborted_insert = txs.begin();
    index.insert_tx(aborted_insert, b"k002", row).unwrap();
    txs.abort(aborted_insert);
    let snapshot = txs.snapshot();
    assert!(
        index
            .point_lookup_visible(&txs, &snapshot, None, b"k002")
            .unwrap()
            .is_empty(),
        "aborted insert must not be visible"
    );

    let insert_tx = txs.begin();
    index.insert_tx(insert_tx, b"k003", row).unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(insert_tx, csn);
    let delete_tx = txs.begin();
    index.delete_mark_tx(delete_tx, b"k003", row).unwrap();
    txs.abort(delete_tx);
    let snapshot = txs.snapshot();
    assert_eq!(
        index
            .range_scan_visible(&txs, &snapshot, None, b"k003", b"k004")
            .unwrap(),
        vec![row],
        "aborted delete must leave committed entry visible"
    );

    let final_delete = txs.begin();
    index
        .delete_mark_tx_visible(
            &txs,
            &snapshot,
            Some(final_delete),
            final_delete,
            b"k003",
            row,
        )
        .unwrap();
    let delete_csn = txs.reserve_csn();
    txs.publish_commit(final_delete, delete_csn);
    let snapshot = txs.snapshot();
    assert!(
        index
            .point_lookup_visible(&txs, &snapshot, None, b"k003")
            .unwrap()
            .is_empty(),
        "later committed delete must supersede an aborted delete marker"
    );
}

#[test]
fn engine_create_index_allocates_meta_page_and_recovers() {
    use redlinedb_kernel::catalog::{
        ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
        IndexColumnSpec, IndexOrigin, QualifiedName, SchemaId, SortDir, ValueRef, encode_index_key,
        encode_record,
    };
    use redlinedb_kernel::engine::{Engine, EngineConfig};
    use redlinedb_kernel::format::DEFAULT_PAGE_SIZE;
    use redlinedb_kernel::txn::Isolation;
    use redlinedb_kernel::wal::WalConfig;

    let temp = TempDir::new().unwrap();
    let config = EngineConfig {
        rel_id: RelId(1),
        wal: WalConfig {
            segment_bytes: 65_536,
            ..WalConfig::default()
        },
        commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
        lock_shards: 32,
        busy_timeout: Duration::from_millis(250),
        heap_lanes: 16,
        page_size: DEFAULT_PAGE_SIZE,
        buffer_pool_pages: 256,
        data_file_name: "data.redline".to_owned(),
    };

    // CREATE TABLE t(id INTEGER PRIMARY KEY, v BLOB).
    let engine = Engine::create(temp.path(), config.clone()).unwrap();
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
                    },
                    ColumnSpec {
                        name: DbName::new("v"),
                        declared_type: Some("BLOB".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
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

    // Insert three rows BEFORE CREATE INDEX so the backfill walks them.
    let row_ids: Vec<_> = (1..=3_i64)
        .map(|i| {
            let mut tx = engine.begin(Isolation::Snapshot).unwrap();
            let mut buf = Vec::new();
            encode_record(
                &[ValueRef::Integer(i), ValueRef::Blob(&[0xab, i as u8])],
                &mut buf,
            )
            .unwrap();
            let row_id = engine.reserve_row_id();
            engine
                .insert_for_relation(&mut tx, table.relation_id, row_id, buf)
                .unwrap();
            engine.commit(tx).unwrap();
            row_id
        })
        .collect();

    // CREATE INDEX ix_v ON t(v).
    let mut tx = engine.begin(Isolation::Snapshot).unwrap();
    let index = engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_v"),
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
                normalized_sql: Some("CREATE INDEX ix_v ON t(v)".to_owned()),
            },
        )
        .unwrap();
    let physical_index_id = index.index_id;
    assert!(
        index.meta_page_id.is_some(),
        "create_index must populate meta_page_id"
    );
    engine.commit(tx).unwrap();

    // The handle should be live in the engine.
    assert!(engine.index_handle(physical_index_id).is_some());

    drop(engine);

    // Reopen and assert the snapshot still holds meta_page_id and the index
    // returns the rows we backfilled.
    let reopened = Engine::open(temp.path(), config).unwrap();
    let snapshot = reopened.schema_snapshot();
    let recovered = snapshot
        .lookup_index(SchemaId(1), "ix_v")
        .expect("index missing after recovery");
    assert!(
        recovered.meta_page_id.is_some(),
        "meta_page_id must survive recovery"
    );
    let handle = reopened
        .index_handle(recovered.index_id)
        .expect("index handle missing after rehydrate");
    let mut key_buf = Vec::new();
    for (i, expected_row) in (1..=3_i64).zip(row_ids.iter()) {
        let key = encode_index_key(
            &[ValueRef::Blob(&[0xab, i as u8])],
            &[SortDir::Asc],
            &mut key_buf,
        );
        let hits = handle.point_lookup(&key.bytes).unwrap();
        assert_eq!(hits.len(), 1, "expected exactly one hit for {i:?}");
        assert_eq!(hits[0].row_id, *expected_row);
    }
}

#[test]
fn ddl_index_handles_publish_and_remove_only_on_commit() {
    use redlinedb_kernel::catalog::{
        ColumnSpec, CreateIndexSpec, CreateTableSpec, DbName, DropIndexSpec, IndexColumnSpec,
        IndexOrigin, QualifiedName, SchemaId, SortDir,
    };
    use redlinedb_kernel::engine::{Engine, EngineConfig};

    let temp = TempDir::new().unwrap();
    let config = EngineConfig {
        rel_id: RelId(1),
        wal: WalConfig {
            segment_bytes: 65536,
            ..WalConfig::default()
        },
        commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
        lock_shards: 32,
        busy_timeout: Duration::from_millis(250),
        heap_lanes: 16,
        page_size: DEFAULT_PAGE_SIZE,
        buffer_pool_pages: 256,
        data_file_name: "data.redline".to_owned(),
    };
    let engine = Engine::create(temp.path(), config).unwrap();
    let mut tx = engine
        .begin(redlinedb_kernel::txn::Isolation::Snapshot)
        .unwrap();
    engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("t"),
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
                normalized_sql: Some("CREATE TABLE t(v TEXT)".to_owned()),
            },
        )
        .unwrap();
    engine.commit(tx).unwrap();

    let create_spec = || CreateIndexSpec {
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
    };

    let mut tx = engine
        .begin(redlinedb_kernel::txn::Isolation::Snapshot)
        .unwrap();
    let rolled_back_index = engine.create_index(&mut tx, create_spec()).unwrap();
    assert!(engine.index_handle(rolled_back_index.index_id).is_none());
    engine.rollback(tx).unwrap();
    assert!(engine.index_handle(rolled_back_index.index_id).is_none());
    assert!(
        engine
            .schema_snapshot()
            .lookup_index(SchemaId(1), "ix_t_v")
            .is_none()
    );

    let mut tx = engine
        .begin(redlinedb_kernel::txn::Isolation::Snapshot)
        .unwrap();
    let committed_index = engine.create_index(&mut tx, create_spec()).unwrap();
    assert!(engine.index_handle(committed_index.index_id).is_none());
    engine.commit(tx).unwrap();
    assert!(engine.index_handle(committed_index.index_id).is_some());

    let mut tx = engine
        .begin(redlinedb_kernel::txn::Isolation::Snapshot)
        .unwrap();
    engine
        .drop_index(
            &mut tx,
            DropIndexSpec {
                name: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("ix_t_v"),
                },
                if_exists: false,
            },
        )
        .unwrap();
    assert!(engine.index_handle(committed_index.index_id).is_some());
    engine.rollback(tx).unwrap();
    assert!(engine.index_handle(committed_index.index_id).is_some());
}

/// Regression test for the B-tree leaf-split heuristic when many entries
/// share one logical key. Before the fix, splitting a leaf full of duplicates
/// would either lose entries (point_lookup walked only one of the resulting
/// leaves) or surface `Error::PageFull` ("no free slot space on page") because
/// the size estimator under-counted leaf cells by 8 bytes each. The fix:
/// sort entries by `(logical_key, row_ref)` (already stable) and use a
/// physical-key separator when a duplicate run spans leaves; navigate
/// inserts/lookups by physical bytes; walk right at the leaf level to gather
/// duplicates that ended up on later siblings.
#[test]
fn leaf_split_handles_duplicate_keys() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 512).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(99), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    // 200 entries all sharing the same logical key, each with a distinct
    // row_id. With page_size=512 bytes, this run cannot fit on a single
    // leaf — the fix keeps every entry findable across the leaf chain.
    const DUP_COUNT: u64 = 200;
    for i in 0..DUP_COUNT {
        index
            .insert(
                b"dup",
                IndexRowRef::with_row_id(
                    redlinedb_kernel::format::RowId(i),
                    TuplePtr::new_with_generation(
                        PageId(1_000 + i),
                        i as u16,
                        redlinedb_kernel::format::PageGeneration::ONE,
                    ),
                ),
            )
            .expect("insert duplicate key entry");
    }

    // Point lookup must return every entry, regardless of which leaf the
    // duplicate ended up on.
    let hits = index.point_lookup(b"dup").expect("point_lookup");
    assert_eq!(
        hits.len() as u64,
        DUP_COUNT,
        "point_lookup must surface every duplicate entry, got {}",
        hits.len()
    );
    let mut row_ids: Vec<_> = hits.iter().map(|h| h.row_id.0).collect();
    row_ids.sort_unstable();
    let expected: Vec<u64> = (0..DUP_COUNT).collect();
    assert_eq!(row_ids, expected, "all original row_ids must be present");

    // Range scan over the broader key space must return the same set
    // (range_scan walks every leaf from the start key).
    let range = index.range_scan(b"a", b"z").expect("range_scan");
    assert_eq!(
        range.len() as u64,
        DUP_COUNT,
        "range_scan must surface every duplicate entry, got {}",
        range.len()
    );

    // The tree must validate cleanly: every leaf still has children pointers
    // wired up, levels match, and physical keys remain sorted within pages.
    let report = index.validate().expect("validate");
    assert!(
        report.errors.is_empty(),
        "validate errors: {:?}",
        report.errors
    );
    assert!(
        report.leaf_pages >= 2,
        "duplicate run should have spilled into multiple leaves; saw {} leaves",
        report.leaf_pages
    );
}

fn snapshot_index_pages(
    buffer: &Arc<BufferPool>,
    rel_id: RelId,
) -> redlinedb_kernel::Result<Vec<Page>> {
    let mut pages = Vec::new();
    let page_count = buffer.page_count()?;
    for page_no in 1..=page_count {
        let page_id = PageId(page_no);
        if let Ok(guard) = buffer.pin(page_id) {
            let page = guard.with_page(|page| {
                let header = page.header()?;
                if header.rel_id == rel_id
                    && matches!(
                        header.kind,
                        PageKind::BtreeMeta | PageKind::BtreeLeaf | PageKind::BtreeInternal
                    )
                {
                    Ok(Some(page.clone()))
                } else {
                    Ok(None)
                }
            })?;
            if let Some(page) = page {
                pages.push(page);
            }
        }
    }
    Ok(pages)
}

/// Lane KH (Wave 7) P1 #6: a range scan over a small contiguous slice
/// of a large index must terminate as soon as the leaf chain crosses
/// the upper bound — not walk every right sibling to the end of the
/// index. Without the fix, a `WHERE k BETWEEN 5 AND 10` over 5000
/// entries pinned every leaf along the way (O(N)).
#[test]
fn range_scan_terminates_early() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 4096).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 4096).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(99), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    // Insert 5000 entries with zero-padded ASCII keys so byte order
    // tracks numeric order. (`format!("k{:05}", n)` keeps key length
    // constant; without padding, "k10" sorts before "k2" and the
    // range bounds would be wrong.)
    const ENTRIES: u64 = 5_000;
    for i in 0..ENTRIES {
        let key = format!("k{i:05}");
        index
            .insert(
                key.as_bytes(),
                IndexRowRef::new(TuplePtr::new_with_generation(
                    PageId(1_000 + i),
                    (i % u16::MAX as u64) as u16,
                    redlinedb_kernel::format::PageGeneration::ONE,
                )),
            )
            .unwrap();
    }

    // Sanity: tree must have multiple leaves so the early-exit path
    // is reachable. With ~5000 keys + 4 KiB pages there should be
    // dozens of leaves.
    let report = index.validate().expect("validate");
    assert!(
        report.errors.is_empty(),
        "validate errors: {:?}",
        report.errors
    );
    assert!(
        report.leaf_pages > 4,
        "expected many leaves, got {}",
        report.leaf_pages
    );
    let total_leaves = report.leaf_pages as u64;

    // Reset the visit counter to a known baseline; the validate()
    // pass above does not touch range_scan, but assertions here
    // expect a clean number.
    let baseline = index.stats().range_scan_leaves_visited;

    // Probe `5..=10` (inclusive on both ends, encoded half-open).
    // The result must be exactly the six matching entries.
    let start = b"k00005".to_vec();
    // Half-open upper bound: anything strictly less than "k00011"
    // matches keys "k00005".."k00010" inclusive.
    let end = b"k00011".to_vec();
    let hits = index.range_scan(&start, &end).expect("range_scan");
    assert_eq!(
        hits.len(),
        6,
        "range scan must match 6 entries, got {}",
        hits.len()
    );

    let visits = index.stats().range_scan_leaves_visited - baseline;
    assert!(
        visits <= 4,
        "range_scan must terminate within a few leaves; visited {visits} of {total_leaves}",
    );
    assert!(
        visits < total_leaves,
        "range_scan must not walk every leaf; visited {visits} of {total_leaves}",
    );
}
