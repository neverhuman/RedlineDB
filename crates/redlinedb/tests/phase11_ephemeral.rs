use redlinedb::{Database, OpenOptions, Step};
use tempfile::tempdir;

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
