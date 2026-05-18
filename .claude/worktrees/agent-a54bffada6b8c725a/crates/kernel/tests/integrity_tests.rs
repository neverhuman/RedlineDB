use std::sync::Arc;
use std::time::Duration;

use redlinedb_kernel::catalog::{
    ColumnSpec, CreateIndexSpec, CreateTableSpec, DbName, IndexColumnSpec, IndexOrigin,
    QualifiedName, SortDir, ValueRef, encode_record,
};
use redlinedb_kernel::engine::{Engine, EngineConfig};
use redlinedb_kernel::format::{DEFAULT_PAGE_SIZE, PageId, RelId, RowId, TuplePtr};
use redlinedb_kernel::index::{
    BtreeIndex, IndexDescriptor, IndexId as PhysicalIndexId, IndexRowRef, IndexUniqueness,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::txn::Isolation;
use redlinedb_kernel::wal::WalConfig;
use tempfile::TempDir;

fn test_engine() -> (TempDir, Arc<Engine>) {
    let temp = TempDir::new().expect("temp dir");
    let engine = Engine::create(
        temp.path(),
        EngineConfig {
            rel_id: RelId(1),
            wal: WalConfig {
                segment_bytes: 65_536,
                ..WalConfig::default()
            },
            commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
            lock_shards: 32,
            busy_timeout: Duration::from_millis(250),
            heap_lanes: 4,
            page_size: DEFAULT_PAGE_SIZE,
            buffer_pool_pages: 256,
            data_file_name: "data.redline".to_owned(),
        },
    )
    .expect("engine create");
    (temp, engine)
}

fn create_kv_table_only(engine: &Arc<Engine>) {
    let mut tx = engine.begin(Isolation::Snapshot).expect("begin");
    engine
        .create_table(
            &mut tx,
            CreateTableSpec {
                schema: None,
                name: DbName::new("kv"),
                if_not_exists: false,
                columns: vec![
                    ColumnSpec {
                        name: DbName::new("k"),
                        declared_type: Some("INTEGER".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                    },
                    ColumnSpec {
                        name: DbName::new("v"),
                        declared_type: Some("TEXT".to_owned()),
                        constraints: vec![],
                        collation: None,
                        default_value: None,
                    },
                ],
                constraints: vec![],
                strict: false,
                without_rowid: false,
                normalized_sql: Some("CREATE TABLE kv(k INTEGER, v TEXT)".to_owned()),
            },
        )
        .expect("create table");
    engine.commit(tx).expect("commit");
}

fn create_kv_index(engine: &Arc<Engine>) {
    let mut tx = engine.begin(Isolation::Snapshot).expect("begin");
    engine
        .create_index(
            &mut tx,
            CreateIndexSpec {
                schema: None,
                name: DbName::new("ix_kv_k"),
                table: QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new("kv"),
                },
                unique: false,
                columns: vec![IndexColumnSpec {
                    name: DbName::new("k"),
                    sort_dir: SortDir::Asc,
                    collation: None,
                }],
                origin: IndexOrigin::User,
                normalized_sql: Some("CREATE INDEX ix_kv_k ON kv(k)".to_owned()),
            },
        )
        .expect("create index");
    engine.commit(tx).expect("commit");
}

fn insert_kv_row(engine: &Arc<Engine>, key: i64, value: &str) -> RowId {
    let snapshot = engine.schema_snapshot();
    let table = snapshot
        .tables
        .iter()
        .find(|t| t.name.as_ref() == "kv")
        .cloned()
        .expect("kv table");
    let mut tx = engine.begin(Isolation::Snapshot).expect("begin");
    let row_id = engine.reserve_row_id();
    let mut payload = Vec::new();
    encode_record(
        &[ValueRef::Integer(key), ValueRef::Text(value)],
        &mut payload,
    )
    .expect("encode record");
    engine
        .insert_for_relation(&mut tx, table.relation_id, row_id, payload)
        .expect("insert");
    engine.commit(tx).expect("commit");
    row_id
}

fn lookup_kv_index(engine: &Arc<Engine>) -> Arc<BtreeIndex> {
    let snapshot = engine.schema_snapshot();
    let index = snapshot
        .indexes
        .iter()
        .find(|i| i.name.as_ref() == "ix_kv_k")
        .expect("kv index in snapshot");
    engine.index_handle(index.index_id).expect("index handle")
}

fn encode_logical_key_for_int(value: i64) -> Vec<u8> {
    use redlinedb_kernel::catalog::{SortDir, ValueRef, encode_index_key};
    let parts = vec![ValueRef::Integer(value)];
    let dirs = vec![SortDir::Asc];
    let mut buf = Vec::new();
    let encoded = encode_index_key(&parts, &dirs, &mut buf);
    encoded.bytes
}

#[test]
fn integrity_clean_database_reports_zero_anomalies() {
    let (_temp, engine) = test_engine();
    create_kv_table_only(&engine);
    insert_kv_row(&engine, 1, "alpha");
    insert_kv_row(&engine, 2, "beta");
    insert_kv_row(&engine, 3, "gamma");
    create_kv_index(&engine);

    let report = engine.integrity_check_full().expect("integrity ok");
    assert!(
        report.is_clean(),
        "expected clean report, got {:#?}",
        report
    );
    let kv = report
        .relations
        .iter()
        .find(|r| r.relation_name == "kv")
        .expect("kv relation");
    assert_eq!(kv.heap_row_count, 3);
    assert_eq!(kv.indexes.len(), 1);
    assert_eq!(kv.indexes[0].entry_count, 3);
    assert_eq!(kv.indexes[0].heap_minus_index, 0);
    assert_eq!(kv.indexes[0].index_minus_heap, 0);
}

#[test]
fn integrity_detects_index_minus_heap_orphan() {
    let (_temp, engine) = test_engine();
    create_kv_table_only(&engine);
    insert_kv_row(&engine, 1, "alpha");
    insert_kv_row(&engine, 2, "beta");
    create_kv_index(&engine);

    // Inject a bogus index entry pointing at a row_id the heap never saw.
    let index = lookup_kv_index(&engine);
    let phantom_row = RowId(99_999_999);
    let phantom_key = encode_logical_key_for_int(7);
    index
        .insert(
            &phantom_key,
            IndexRowRef::with_row_id(
                phantom_row,
                TuplePtr::new_with_generation(
                    PageId(1),
                    0,
                    redlinedb_kernel::format::PageGeneration::ONE,
                ),
            ),
        )
        .expect("inject phantom index entry");

    let report = engine.integrity_check_full().expect("integrity ok");
    let kv = report
        .relations
        .iter()
        .find(|r| r.relation_name == "kv")
        .expect("kv relation");
    let ix = kv
        .indexes
        .iter()
        .find(|i| i.index_name == "ix_kv_k")
        .expect("kv index");
    assert_eq!(ix.entry_count, 3);
    assert_eq!(ix.index_minus_heap, 1);
    assert_eq!(ix.heap_minus_index, 0);
    assert!(!report.is_clean(), "report should not be clean");
}

#[test]
fn integrity_detects_heap_minus_index_missing() {
    let (_temp, engine) = test_engine();
    create_kv_table_only(&engine);
    let row = insert_kv_row(&engine, 42, "answer");
    insert_kv_row(&engine, 100, "century");
    create_kv_index(&engine);

    let index = lookup_kv_index(&engine);
    let key = encode_logical_key_for_int(42);
    let entries = index
        .iter_all_entries()
        .expect("dump index")
        .into_iter()
        .filter(|entry| entry.row.row_id == row)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "expected exactly one entry for k=42");
    let target = entries.into_iter().next().unwrap();
    index
        .delete_mark(&key, target.row)
        .expect("drop index entry");

    let report = engine.integrity_check_full().expect("integrity ok");
    let kv = report
        .relations
        .iter()
        .find(|r| r.relation_name == "kv")
        .expect("kv relation");
    let ix = kv
        .indexes
        .iter()
        .find(|i| i.index_name == "ix_kv_k")
        .expect("kv index");
    assert_eq!(ix.heap_minus_index, 1);
    assert_eq!(ix.index_minus_heap, 0);
    assert!(!report.is_clean());
}

#[test]
fn integrity_after_recovery_is_clean() {
    let (temp, engine) = test_engine();
    create_kv_table_only(&engine);
    for k in 0..16_i64 {
        insert_kv_row(&engine, k, &format!("v{k}"));
    }
    create_kv_index(&engine);
    engine.checkpoint().expect("checkpoint");
    drop(engine);

    let reopened = Engine::open(
        temp.path(),
        EngineConfig {
            rel_id: RelId(1),
            wal: WalConfig {
                segment_bytes: 65_536,
                ..WalConfig::default()
            },
            commit_durability: redlinedb_kernel::engine::CommitDurability::Strict,
            lock_shards: 32,
            busy_timeout: Duration::from_millis(250),
            heap_lanes: 4,
            page_size: DEFAULT_PAGE_SIZE,
            buffer_pool_pages: 256,
            data_file_name: "data.redline".to_owned(),
        },
    )
    .expect("reopen engine");

    let report = reopened.integrity_check_full().expect("integrity ok");
    assert!(
        report.is_clean(),
        "post-recovery report should be clean, got {report:#?}"
    );
    let kv = report
        .relations
        .iter()
        .find(|r| r.relation_name == "kv")
        .expect("kv relation");
    assert_eq!(kv.heap_row_count, 16);
    assert_eq!(kv.indexes[0].entry_count, 16);
}

#[test]
fn integrity_check_returns_per_index_errors() {
    let (_temp, engine) = test_engine();
    create_kv_table_only(&engine);
    insert_kv_row(&engine, 1, "alpha");
    create_kv_index(&engine);

    let result = engine
        .integrity_check_per_index()
        .expect("integrity_check_per_index");
    let names: Vec<_> = result.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"ix_kv_k"), "kv index in result: {names:?}");
    for (name, errors) in &result {
        assert!(
            errors.is_empty(),
            "expected no validation errors on {name}, got {errors:?}"
        );
    }
}

#[test]
fn iter_all_entries_walks_ten_thousand_keys() {
    let temp = TempDir::new().expect("temp dir");
    let page_file = Arc::new(
        PageFile::create(temp.path().join("data.redline"), DEFAULT_PAGE_SIZE).expect("page file"),
    );
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 4096).expect("buffer"));
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(PhysicalIndexId(7), RelId(2), IndexUniqueness::NonUnique),
    )
    .expect("create index");

    for i in 0..10_000_u64 {
        let key = format!("k{i:05}").into_bytes();
        index
            .insert(
                &key,
                IndexRowRef::with_row_id(
                    RowId(i),
                    TuplePtr::new_with_generation(
                        PageId(1 + i / 64),
                        (i % 64) as u16,
                        redlinedb_kernel::format::PageGeneration::ONE,
                    ),
                ),
            )
            .expect("insert");
    }

    let entries = index.iter_all_entries().expect("dump");
    assert_eq!(entries.len(), 10_000);
    for (i, entry) in entries.iter().enumerate() {
        let expected = format!("k{i:05}").into_bytes();
        assert_eq!(
            entry.logical_key, expected,
            "entries should walk in physical-key order"
        );
        assert_eq!(entry.row.row_id, RowId(i as u64));
    }
}
