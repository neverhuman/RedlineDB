use std::fs;
use std::io::Write as _;

use redlinedb::{
    ArchiveMode, Csn, Database, Lsn, OpenOptions, PHYSICAL_BACKUP_MANIFEST_FILE,
    PhysicalBackupOptions, RecoveryTarget, RestoreOptions, SlotKind, Step, ValueRef,
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
    let manifest = Database::physical_backup_manifest(&backup).expect("manifest");
    let verified = Database::verify_physical_backup(&backup).expect("verify backup");
    assert_eq!(manifest.files.len(), verified.len());
    assert!(verified.iter().all(|file| file.sha256.len() == 64));

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
fn physical_restore_preserves_exact_lsn_and_csn_targets() {
    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("source");
    let backup = dir.path().join("backup");
    let lsn_restore = dir.path().join("lsn-restore");
    let csn_restore = dir.path().join("csn-restore");
    let db = Database::create(&src).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", ())
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1)", ())
        .expect("insert");
    let backup_stats = db
        .backup_physical_to_path(&backup, PhysicalBackupOptions::default())
        .expect("backup");
    let manifest = Database::physical_backup_manifest(&backup).expect("manifest");

    let lsn_stats = Database::restore_from_backup(
        &backup,
        &lsn_restore,
        RestoreOptions {
            target: RecoveryTarget::Lsn(Lsn(backup_stats.stop_lsn)),
            preserve_timeline: false,
        },
    )
    .expect("lsn restore");
    assert_eq!(lsn_stats.target_lsn, backup_stats.stop_lsn);
    assert_eq!(lsn_stats.new_timeline, manifest.timeline + 1);

    let csn_stats = Database::restore_from_backup(
        &backup,
        &csn_restore,
        RestoreOptions {
            target: RecoveryTarget::Csn(Csn(backup_stats.stop_csn)),
            preserve_timeline: false,
        },
    )
    .expect("csn restore");
    assert_eq!(csn_stats.target_lsn, backup_stats.stop_lsn);
    assert_eq!(csn_stats.target_csn, backup_stats.stop_csn);
    assert_eq!(csn_stats.new_timeline, manifest.timeline + 1);

    for restored_path in [&lsn_restore, &csn_restore] {
        let restored = Database::open_with_options(
            restored_path,
            OpenOptions {
                create: false,
                ..Default::default()
            },
        )
        .expect("open restored");
        let mut conn = restored.connect().expect("connect");
        let mut rows = conn.query("SELECT id FROM t", ()).expect("query");
        assert!(matches!(rows.step().expect("row"), Step::Row(_)));
    }
}

#[test]
fn backup_and_restore_refuse_existing_native_custody() {
    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("source");
    let backup = dir.path().join("backup");
    let valid_backup = dir.path().join("valid-backup");
    let restore = dir.path().join("restore");
    let db = Database::create(&src).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", ())
        .expect("create table");

    fs::create_dir(&backup).expect("backup dir");
    fs::write(backup.join("native-custody"), b"keep").expect("sentinel");
    let error = db
        .backup_physical_to_path(&backup, PhysicalBackupOptions::default())
        .expect_err("nonempty backup destination");
    assert_eq!(error.code(), redlinedb::ErrorCode::Busy);
    assert_eq!(
        fs::read(backup.join("native-custody")).expect("sentinel"),
        b"keep"
    );

    db.backup_physical_to_path(&valid_backup, PhysicalBackupOptions::default())
        .expect("backup");
    fs::create_dir(&restore).expect("restore dir");
    fs::write(restore.join("native-custody"), b"keep").expect("sentinel");
    let error = Database::restore_from_backup(&valid_backup, &restore, RestoreOptions::default())
        .expect_err("nonempty restore destination");
    assert_eq!(error.code(), redlinedb::ErrorCode::Busy);
    assert_eq!(
        fs::read(restore.join("native-custody")).expect("sentinel"),
        b"keep"
    );
}

#[test]
fn verification_rejects_corrupt_and_hostile_manifest_files() {
    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("source");
    let backup = dir.path().join("backup");
    let db = Database::create(&src).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", ())
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1)", ())
        .expect("insert");
    db.backup_physical_to_path(&backup, PhysicalBackupOptions::default())
        .expect("backup");

    let verified = Database::verify_physical_backup(&backup).expect("verify");
    let first = backup.join(&verified[0].relative_path);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(first)
        .expect("open copied file");
    file.write_all(b"corruption").expect("corrupt");
    assert!(Database::verify_physical_backup(&backup).is_err());

    let missing_backup = dir.path().join("missing");
    db.backup_physical_to_path(&missing_backup, PhysicalBackupOptions::default())
        .expect("missing-file backup");
    let missing_files = Database::verify_physical_backup(&missing_backup).expect("verify");
    fs::remove_file(missing_backup.join(&missing_files[0].relative_path))
        .expect("remove copied file");
    assert!(Database::verify_physical_backup(&missing_backup).is_err());

    let second_backup = dir.path().join("hostile");
    db.backup_physical_to_path(&second_backup, PhysicalBackupOptions::default())
        .expect("second backup");
    let manifest_path = second_backup
        .join("phase8")
        .join(PHYSICAL_BACKUP_MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["files"][0] = serde_json::Value::String("../escape".to_owned());
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
    )
    .expect("write hostile manifest");
    assert!(Database::verify_physical_backup(&second_backup).is_err());
}

#[cfg(unix)]
#[test]
fn verification_rejects_symlinked_backup_entries() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("source");
    let backup = dir.path().join("backup");
    let db = Database::create(&src).expect("create db");
    db.backup_physical_to_path(&backup, PhysicalBackupOptions::default())
        .expect("backup");
    let verified = Database::verify_physical_backup(&backup).expect("verify");
    let first = backup.join(&verified[0].relative_path);
    let external = dir.path().join("external");
    fs::write(&external, b"foreign").expect("external");
    fs::remove_file(&first).expect("remove copied file");
    symlink(&external, &first).expect("symlink");
    assert!(Database::verify_physical_backup(&backup).is_err());
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

#[test]
fn required_archive_mode_fails_closed_on_unavailable_watermark() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("required-archive.db");
    let db = Database::create(&path).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", ())
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1)", ())
        .expect("insert");
    db.set_archive_mode(ArchiveMode::RequiredLocal)
        .expect("required mode");

    db.checkpoint()
        .expect("missing watermark preserves all WAL");
    assert_eq!(
        db.retention_horizon().expect("retention").wal_recycle_lsn,
        0
    );

    let state = path.join("phase8/archive/state");
    fs::create_dir_all(&state).expect("state");
    fs::write(state.join("archive.watermark"), b"corrupt").expect("corrupt watermark");
    let error = db.checkpoint().expect_err("corrupt required watermark");
    assert_eq!(error.code(), redlinedb::ErrorCode::Error);
}
