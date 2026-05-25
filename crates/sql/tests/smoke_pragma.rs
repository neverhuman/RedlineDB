//! PRAGMA-surface smoke tests: schema introspection (`table_info`,
//! `index_list`, `index_info`, `table_list`, `index_xinfo`,
//! `foreign_key_list`), connection-scoped settings (`foreign_keys`,
//! `user_version`, `schema_version`, `database_list`), `integrity_check`,
//! and persistence of connection state across reopen.
//!
//! Split off from the original `tests/sql_smoke.rs` (Phase 11 Wave 0).
//! Each `#[test] fn` here is verbatim from the source file.

mod common;

use common::{open_database, step_done};
use redlinedb_sql::{Database, DbOptions, Step};
use tempfile::tempdir;

#[test]
fn pragma_introspection_and_state_round_trips_work() {
    let (dir, conn) = open_database();
    let db_path = dir.path().join("redlinedb-sql-smoke.db");

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT NOT NULL DEFAULT 'bee')")
        .expect("create table");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");

    let mut table_info = conn.prepare("PRAGMA table_info(t)").expect("table info");
    let mut table_rows = Vec::new();
    while let Step::Row = table_info.step().expect("step table_info") {
        table_rows.push((
            table_info.column_i64(0).expect("cid"),
            table_info.column_text(1).expect("name").to_owned(),
            table_info.column_text(2).expect("type").to_owned(),
            table_info.column_i64(3).expect("notnull"),
            table_info.column_value(4).expect("default").clone(),
            table_info.column_i64(5).expect("pk"),
        ));
    }
    assert_eq!(table_rows.len(), 2);
    assert_eq!(table_rows[0].1, "a");
    assert_eq!(table_rows[0].5, 1);
    assert_eq!(table_rows[1].1, "b");
    assert_eq!(table_rows[1].3, 1);

    let mut index_list = conn.prepare("PRAGMA index_list(t)").expect("index list");
    let mut index_rows = Vec::new();
    while let Step::Row = index_list.step().expect("step index_list") {
        index_rows.push((
            index_list.column_i64(0).expect("seq"),
            index_list.column_text(1).expect("name").to_owned(),
            index_list.column_i64(2).expect("unique"),
            index_list.column_text(3).expect("origin").to_owned(),
            index_list.column_i64(4).expect("partial"),
        ));
    }
    assert!(
        index_rows
            .iter()
            .any(|row| row.1 == "t_b_idx" && row.2 == 0 && row.3 == "c")
    );
    // SQLite-parity: PRAGMA index_list hides the implicit `sqlite_autoindex_*`
    // entry generated for an INTEGER PRIMARY KEY rowid alias. The PK
    // is encoded by the rowid itself, so the user-visible index list
    // contains only user-defined indexes (the single `c`-origin entry
    // above).
    assert!(!index_rows.iter().any(|row| row.3 == "pk"));

    let mut index_info = conn
        .prepare("PRAGMA index_info(t_b_idx)")
        .expect("index info");
    assert_eq!(index_info.step().expect("step"), Step::Row);
    assert_eq!(index_info.column_i64(0).expect("seqno"), 0);
    assert_eq!(index_info.column_i64(1).expect("cid"), 1);
    assert_eq!(index_info.column_text(2).expect("name"), "b");
    assert_eq!(index_info.step().expect("done"), Step::Done);

    let mut table_xinfo = conn.prepare("PRAGMA table_xinfo(t)").expect("table xinfo");
    let mut xinfo_rows = Vec::new();
    while let Step::Row = table_xinfo.step().expect("step table_xinfo") {
        xinfo_rows.push((
            table_xinfo.column_i64(0).expect("cid"),
            table_xinfo.column_text(1).expect("name").to_owned(),
            table_xinfo.column_i64(6).expect("hidden"),
        ));
    }
    assert_eq!(xinfo_rows.len(), 2);
    assert!(xinfo_rows.iter().all(|row| row.2 == 0));

    let mut table_list = conn.prepare("PRAGMA table_list").expect("table list");
    let mut table_list_rows = Vec::new();
    while let Step::Row = table_list.step().expect("step table_list") {
        table_list_rows.push((
            table_list.column_text(0).expect("schema").to_owned(),
            table_list.column_text(1).expect("name").to_owned(),
            table_list.column_text(2).expect("type").to_owned(),
            table_list.column_i64(3).expect("ncol"),
            table_list.column_i64(4).expect("wr"),
            table_list.column_i64(5).expect("strict"),
        ));
    }
    assert!(
        table_list_rows
            .iter()
            .any(|row| row.1 == "t" && row.2 == "table")
    );

    let mut index_xinfo = conn
        .prepare("PRAGMA index_xinfo(t_b_idx)")
        .expect("index xinfo");
    assert_eq!(index_xinfo.step().expect("step"), Step::Row);
    assert_eq!(index_xinfo.column_i64(0).expect("seqno"), 0);
    assert_eq!(index_xinfo.column_i64(1).expect("cid"), 1);
    assert_eq!(index_xinfo.column_text(2).expect("name"), "b");
    assert_eq!(index_xinfo.column_i64(3).expect("desc"), 0);
    assert_eq!(index_xinfo.column_text(4).expect("coll"), "BINARY");
    assert_eq!(index_xinfo.column_i64(5).expect("key"), 1);
    assert_eq!(index_xinfo.step().expect("done"), Step::Done);

    let mut fk_list = conn
        .prepare("PRAGMA foreign_key_list(t)")
        .expect("foreign key list");
    assert_eq!(fk_list.step().expect("done"), Step::Done);

    let mut foreign_keys = conn.prepare("PRAGMA foreign_keys").expect("foreign_keys");
    assert_eq!(foreign_keys.step().expect("step"), Step::Row);
    assert_eq!(foreign_keys.column_i64(0).expect("foreign_keys"), 0);
    assert_eq!(foreign_keys.step().expect("done"), Step::Done);

    let mut set_foreign_keys = conn
        .prepare("PRAGMA foreign_keys = ON")
        .expect("set foreign_keys");
    step_done(&mut set_foreign_keys);

    let mut foreign_keys = conn.prepare("PRAGMA foreign_keys").expect("foreign_keys");
    assert_eq!(foreign_keys.step().expect("step"), Step::Row);
    assert_eq!(foreign_keys.column_i64(0).expect("foreign_keys"), 1);
    assert_eq!(foreign_keys.step().expect("done"), Step::Done);

    let mut user_version = conn.prepare("PRAGMA user_version").expect("user version");
    assert_eq!(user_version.step().expect("step"), Step::Row);
    assert_eq!(user_version.column_i64(0).expect("user_version"), 0);
    assert_eq!(user_version.step().expect("done"), Step::Done);

    let mut set_user_version = conn
        .prepare("PRAGMA user_version = 7")
        .expect("set user version");
    step_done(&mut set_user_version);

    let mut user_version = conn.prepare("PRAGMA user_version").expect("user version");
    assert_eq!(user_version.step().expect("step"), Step::Row);
    assert_eq!(user_version.column_i64(0).expect("user_version"), 7);
    assert_eq!(user_version.step().expect("done"), Step::Done);

    let mut schema_version = conn
        .prepare("PRAGMA schema_version")
        .expect("schema version");
    assert_eq!(schema_version.step().expect("step"), Step::Row);
    assert!(schema_version.column_i64(0).expect("schema_version") > 0);
    assert_eq!(schema_version.step().expect("done"), Step::Done);

    let mut db_list = conn.prepare("PRAGMA database_list").expect("database_list");
    assert_eq!(db_list.step().expect("step"), Step::Row);
    assert_eq!(db_list.column_i64(0).expect("seq"), 0);
    assert_eq!(db_list.column_text(1).expect("name"), "main");
    assert_eq!(
        db_list.column_text(2).expect("file"),
        db_path.to_string_lossy().as_ref()
    );
    assert_eq!(db_list.step().expect("done"), Step::Done);

    let mut integrity_check = conn
        .prepare("PRAGMA integrity_check")
        .expect("integrity_check");
    assert_eq!(integrity_check.step().expect("step"), Step::Row);
    assert_eq!(integrity_check.column_text(0).expect("integrity"), "ok");
    assert_eq!(integrity_check.step().expect("done"), Step::Done);
}

#[test]
fn pragma_table_list_reports_without_rowid_and_strict_bits_separately() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE wr(a TEXT, b INT, PRIMARY KEY(a,b)) WITHOUT ROWID")
        .expect("create without rowid table");
    conn.execute("CREATE TABLE strict_t(a INTEGER) STRICT")
        .expect("create strict table");

    let mut stmt = conn.prepare("PRAGMA table_list").expect("table_list");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step table_list") {
        rows.push((
            stmt.column_text(1).expect("name").to_owned(),
            stmt.column_i64(4).expect("wr"),
            stmt.column_i64(5).expect("strict"),
        ));
    }

    assert!(rows.iter().any(|row| row == &("wr".to_owned(), 1, 0)));
    assert!(rows.iter().any(|row| row == &("strict_t".to_owned(), 0, 1)));
}

#[test]
fn pragma_user_version_survives_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("user-version.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create database");
        let conn = db.connect();
        conn.execute("PRAGMA user_version = 42")
            .expect("set user_version");
    }
    let db = Database::open(&path, DbOptions::default()).expect("open database");
    let conn = db.connect();
    let mut stmt = conn.prepare("PRAGMA user_version").expect("user_version");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("value"), 42);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}
