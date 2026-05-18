//! Lane SQL-D phase 10: REGEXP operator.
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("regexp.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn first_i64(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_i64(0).expect("i64")
}

#[test]
fn regexp_match_returns_one() {
    let (_dir, conn) = open();
    assert_eq!(first_i64(&conn, "SELECT 'foobar' REGEXP '^foo'"), 1);
}

#[test]
fn regexp_no_match_returns_zero() {
    let (_dir, conn) = open();
    assert_eq!(first_i64(&conn, "SELECT 'baz' REGEXP '^foo'"), 0);
}

#[test]
fn regexp_null_propagates() {
    let (_dir, conn) = open();
    let mut stmt = conn
        .prepare("SELECT NULL REGEXP 'x', 'a' REGEXP NULL")
        .expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let v0 = stmt.column_value(0).expect("v0");
    let v1 = stmt.column_value(1).expect("v1");
    assert!(matches!(v0, redlinedb_sql::SqlValue::Null));
    assert!(matches!(v1, redlinedb_sql::SqlValue::Null));
}

#[test]
fn regexp_compile_error() {
    let (_dir, conn) = open();
    let res = conn.execute("SELECT 'a' REGEXP '['");
    assert!(res.is_err(), "invalid regex should error: {res:?}");
}
