//! Lane SQL-D phase 10: COLLATE clauses (BINARY/NOCASE/RTRIM).
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("collation.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn first_i64(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_i64(0).expect("i64")
}

#[test]
fn binary_default_distinguishes_case() {
    let (_dir, conn) = open();
    assert_eq!(first_i64(&conn, "SELECT 'abc' = 'ABC'"), 0);
}

#[test]
fn nocase_collation_in_comparison() {
    let (_dir, conn) = open();
    assert_eq!(first_i64(&conn, "SELECT 'abc' COLLATE NOCASE = 'ABC'"), 1);
}

#[test]
fn rtrim_collation_strips_trailing_spaces() {
    let (_dir, conn) = open();
    assert_eq!(first_i64(&conn, "SELECT 'abc   ' COLLATE RTRIM = 'abc'"), 1);
}

#[test]
fn nocase_collation_in_order_by() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(v TEXT)").expect("create");
    conn.execute("INSERT INTO t VALUES ('Zebra'), ('apple'), ('Banana')")
        .expect("insert");
    let mut stmt = conn
        .prepare("SELECT v FROM t ORDER BY v COLLATE NOCASE ASC")
        .expect("prepare");
    let mut got = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        got.push(stmt.column_text(0).expect("text").to_owned());
    }
    assert_eq!(got, vec!["apple", "Banana", "Zebra"]);
}
