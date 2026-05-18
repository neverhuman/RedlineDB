use redlinedb::{
    ArchiveMode, Database, OpenOptions, PhysicalBackupOptions, RestoreOptions, SlotKind, Step,
    ValueRef,
};
use tempfile::tempdir;

#[test]
fn physical_backup_restore_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("src.db");
    let backup = dir.path().join("backup.db");
    let dst = dir.path().join("restore.db");

    let db = Database::create(&src).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT)", ())
        .expect("create table");
    conn.execute("INSERT INTO items VALUES (1, 'one')", ())
        .expect("insert 1");
    conn.execute("INSERT INTO items VALUES (2, 'two')", ())
        .expect("insert 2");

    let backup_stats = db
        .backup_physical_to_path(
            &backup,
            PhysicalBackupOptions {
                include_wal: true,
                archive_mode: ArchiveMode::Off,
            },
        )
        .expect("backup");
    assert!(backup_stats.files_copied > 0);
    assert!(backup_stats.bytes_copied > 0);

    let restore_stats =
        Database::restore_from_backup(&backup, &dst, RestoreOptions::default()).expect("restore");
    assert!(restore_stats.files_copied > 0);
    assert!(restore_stats.bytes_copied > 0);

    let restored = Database::open_with_options(
        &dst,
        OpenOptions {
            create: false,
            ..Default::default()
        },
    )
    .expect("open restored");
    let mut conn = restored.connect().expect("connect restored");
    let mut rows = conn
        .query("SELECT name FROM items ORDER BY id", ())
        .expect("query");
    let mut names = Vec::new();
    while let Step::Row(row) = rows.step().expect("step") {
        match row.get_ref(0).expect("ref") {
            ValueRef::Text(value) => names.push(value.to_owned()),
            other => panic!("unexpected value: {other:?}"),
        }
    }
    assert_eq!(names, vec!["one".to_owned(), "two".to_owned()]);
}

#[test]
fn slots_are_persisted_and_listed() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("slots.db");
    let db = Database::create(&path).expect("create db");

    let physical = db
        .create_physical_slot("physical-a")
        .expect("physical slot");
    let logical = db.create_logical_slot("logical-a").expect("logical slot");
    assert_eq!(physical.kind, SlotKind::Physical);
    assert_eq!(logical.kind, SlotKind::Logical);

    let slots = db.replication_slots().expect("replication slots");
    assert_eq!(slots.len(), 2);
    assert!(slots.iter().any(|slot| slot.name == "physical-a"));
    assert!(slots.iter().any(|slot| slot.name == "logical-a"));

    db.drop_replication_slot("physical-a")
        .expect("drop physical slot");
    let slots = db.replication_slots().expect("replication slots");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].name, "logical-a");
}

#[test]
fn archive_and_retention_stats_are_available() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("stats.db");
    let db = Database::create(&path).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)", ())
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'x')", ())
        .expect("insert");

    let archive = db.archive_stats().expect("archive stats");
    assert_eq!(archive.archive_mode, ArchiveMode::Off);

    let retention = db.retention_horizon().expect("retention");
    assert!(retention.catalog_csn >= retention.vacuum_csn);
}
