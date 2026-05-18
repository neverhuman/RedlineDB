//! Lane SQL-D phase 10: partial and expression indexes (parser-only).
//!
//! Both forms are parsed by sqlparser; we accept them in `bind_create_index`
//! and surface a clear `UnsupportedSql` so callers can distinguish "bad
//! syntax" from "syntax recognised but execution not implemented".
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("idx.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

#[test]
fn partial_index_predicate_is_parser_only() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    let res = conn.execute("CREATE INDEX i_pos ON t(a) WHERE a > 0");
    assert!(res.is_err(), "partial index should be parser-only");
    let msg = format!("{:?}", res.unwrap_err()).to_ascii_lowercase();
    assert!(msg.contains("partial") || msg.contains("not yet"));
}

#[test]
fn expression_index_is_parser_only() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    let res = conn.execute("CREATE INDEX i_expr ON t(abs(a))");
    assert!(res.is_err(), "expression index should be parser-only");
    let msg = format!("{:?}", res.unwrap_err()).to_ascii_lowercase();
    assert!(msg.contains("expression") || msg.contains("not yet"));
}

#[test]
fn plain_index_still_works() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX i_basic ON t(a, b)")
        .expect("plain index should still build");
}
