mod common;

use common::step_done;
use redlinedb_kernel::engine::CommitDurability;
use redlinedb_sql::{Database, DbOptions, Error, Step};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn private_in_memory_connections_share_state_and_cleanup_root() {
    let root = tempdir().expect("temp root");
    let opts = DbOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..DbOptions::default()
    };
    let db = Database::create_in_memory(opts).expect("create in-memory db");
    assert_eq!(
        db.engine_config().commit_durability,
        CommitDurability::UnsafeDev
    );
    let db_path = db.path().to_path_buf();
    assert!(db_path.starts_with(root.path()));

    let conn1 = db.connect();
    let conn2 = db.connect();
    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create table");
    conn1
        .execute("INSERT INTO t(id, v) VALUES (1, 'shared')")
        .expect("insert row");

    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("select");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("v"), "shared");
    step_done(&mut stmt);

    drop(stmt);
    drop(conn1);
    drop(conn2);
    drop(db);
    assert!(
        !db_path.exists(),
        "private ephemeral root should be removed on last drop"
    );
}

#[test]
fn named_ephemeral_sessions_reuse_live_database() {
    let root = tempdir().expect("temp root");
    let opts = DbOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..DbOptions::default()
    };

    let db1 = Database::create_ephemeral("phase11-sql-session", opts.clone())
        .expect("create named session");
    assert_eq!(
        db1.engine_config().commit_durability,
        CommitDurability::UnsafeDev
    );
    let conn1 = db1.connect();
    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create table");
    conn1
        .execute("INSERT INTO t(id, v) VALUES (1, 'named')")
        .expect("insert row");

    let db2 = Database::create_ephemeral("phase11-sql-session", opts).expect("reuse session");
    assert_eq!(db1.path(), db2.path());

    let conn2 = db2.connect();
    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("select");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("v"), "named");
    step_done(&mut stmt);
}

#[test]
fn named_ephemeral_session_rejects_incompatible_options() {
    let root = tempdir().expect("temp root");
    let opts = DbOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..DbOptions::default()
    };
    let _db = Database::create_ephemeral("phase11-sql-incompat", opts.clone())
        .expect("create named session");

    let mut other = opts;
    other.busy_timeout = Duration::from_millis(17);
    let err = Database::create_ephemeral("phase11-sql-incompat", other)
        .expect_err("incompatible options must fail");
    assert!(matches!(err, Error::Config(_)), "unexpected error: {err:?}");
}

#[test]
fn named_ephemeral_session_recreates_after_last_handle_drops() {
    let root = tempdir().expect("temp root");
    let opts = DbOptions {
        temp_dir: Some(root.path().to_path_buf()),
        ..DbOptions::default()
    };

    let first_path = {
        let db = Database::create_ephemeral("phase11-sql-cleanup", opts.clone())
            .expect("create named session");
        let path = db.path().to_path_buf();
        assert!(path.exists());
        path
    };
    assert!(
        !first_path.exists(),
        "named ephemeral root should be removed after final handle drops"
    );

    let db = Database::create_ephemeral("phase11-sql-cleanup", opts).expect("recreate session");
    assert!(db.path().exists());
}

#[test]
fn unsafe_dev_metadata_survives_clean_reopen() {
    let root = tempdir().expect("temp root");
    let path = root.path().join("unsafe-metadata.redline");
    let mut opts = DbOptions::default();
    opts.engine.commit_durability = CommitDurability::UnsafeDev;

    {
        let db = Database::create(&path, opts.clone()).expect("create unsafe db");
        let conn = db.connect();
        conn.execute("PRAGMA user_version = 42")
            .expect("set user_version");
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT DEFAULT 'x')")
            .expect("create table");
    }

    let db = Database::open(&path, opts).expect("reopen unsafe db");
    let conn = db.connect();

    let mut version = conn.prepare("PRAGMA user_version").expect("user_version");
    assert_eq!(version.step().expect("version row"), Step::Row);
    assert_eq!(version.column_i64(0).expect("version"), 42);
    step_done(&mut version);

    let mut table = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name = 't'")
        .expect("select catalog table");
    assert_eq!(table.step().expect("table row"), Step::Row);
    assert_eq!(table.column_text(0).expect("table name"), "t");
    step_done(&mut table);
}
