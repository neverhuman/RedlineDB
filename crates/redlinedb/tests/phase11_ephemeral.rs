use redlinedb::{Database, OpenOptions, Step};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn expected_default_volatile_root() -> PathBuf {
    let root = PathBuf::from("/dev/shm/redlinedb-ephemeral");
    if writable_root(&root) {
        root
    } else {
        std::env::temp_dir()
    }
}

fn writable_root(root: &Path) -> bool {
    if std::fs::create_dir_all(root).is_err() {
        return false;
    }
    let probe = root.join(format!(".redlinedb-test-probe-{}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&probe)?;
        file.write_all(b"ok")?;
        drop(file);
        std::fs::remove_file(&probe)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&probe);
    }
    result.is_ok()
}

#[test]
fn in_memory_database_uses_default_volatile_root() {
    let expected_root = expected_default_volatile_root();
    let db = Database::create_in_memory(OpenOptions::default()).expect("create in-memory db");
    let db_path = db.path().to_path_buf();

    assert!(
        db_path.starts_with(&expected_root),
        "default volatile root should be under {:?}, got {:?}",
        expected_root,
        db_path
    );
}

#[test]
fn in_memory_database_shares_state_and_cleans_up() {
    let root = tempdir().expect("tempdir");
    let opts = OpenOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..OpenOptions::default()
    };

    let db = Database::create_in_memory(opts).expect("create in-memory db");
    let db_path = db.path().to_path_buf();
    assert!(db_path.starts_with(root.path()));
    assert!(
        !db_path.join("owner.lock").exists(),
        "volatile in-memory databases should not take an owner lock"
    );
    assert!(
        !db_path.join("wal").exists(),
        "private in-memory databases should not create a WAL directory"
    );
    assert!(
        !db_path.join("schema.redline").exists(),
        "private in-memory databases should not persist catalog sidecars"
    );
    assert!(
        !db_path.join("user_version.redline").exists(),
        "private in-memory databases should not persist user_version sidecars"
    );

    let mut conn1 = db.connect().expect("conn1");
    let mut conn2 = db.connect().expect("conn2");

    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)", ())
        .expect("create table");
    conn1
        .execute("INSERT INTO t VALUES (1, 'one')", ())
        .expect("insert row");
    assert_eq!(
        db.benchmark_stats()
            .expect("benchmark stats")
            .wal
            .fdatasyncs_issued,
        0,
        "volatile in-memory DDL/DML should not issue WAL fdatasyncs"
    );
    let checkpoint = db.checkpoint().expect("volatile checkpoint");
    assert_eq!(checkpoint.generation, 0);
    assert_eq!(checkpoint.checkpoint_lsn, 0);
    assert!(!db_path.join("wal").exists());
    assert!(!db_path.join("schema.redline").exists());
    assert!(!db_path.join("user_version.redline").exists());

    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("prepare");
    match stmt.step().expect("step") {
        Step::Row(row) => assert_eq!(row.get::<String>(0).expect("v"), "one"),
        Step::Done => panic!("expected row"),
    }
    assert!(matches!(stmt.step().expect("done"), Step::Done));

    drop(stmt);
    drop(conn1);
    drop(conn2);
    drop(db);

    assert!(
        !db_path.exists(),
        "ephemeral database path should be removed"
    );
}

#[test]
fn named_ephemeral_sessions_are_reused_while_compatible() {
    let root = tempdir().expect("tempdir");
    let opts = OpenOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..OpenOptions::default()
    };

    let db1 =
        Database::create_ephemeral("phase11-session", opts.clone()).expect("create first session");
    let db_path = db1.path().to_path_buf();
    let mut conn1 = db1.connect().expect("conn1");
    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)", ())
        .expect("create table");
    conn1
        .execute("INSERT INTO t VALUES (1, 'shared')", ())
        .expect("insert row");

    let db2 = Database::create_ephemeral("phase11-session", opts).expect("reuse session");
    assert_eq!(db1.path(), db2.path());

    let mut conn2 = db2.connect().expect("conn2");
    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("prepare");
    match stmt.step().expect("step") {
        Step::Row(row) => assert_eq!(row.get::<String>(0).expect("v"), "shared"),
        Step::Done => panic!("expected row"),
    }
    assert!(matches!(stmt.step().expect("done"), Step::Done));

    drop(stmt);
    drop(conn1);
    drop(conn2);
    drop(db1);
    drop(db2);

    assert!(
        !db_path.exists(),
        "named ephemeral session should be removed when last owner drops"
    );
}
