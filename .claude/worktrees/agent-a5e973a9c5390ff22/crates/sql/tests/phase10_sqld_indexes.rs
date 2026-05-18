//! Phase-11 SQL-D A6: partial and expression indexes are now executed
//! end-to-end. The original phase-10 "parser-only" assertions have been
//! flipped: both forms must now build successfully. Functional coverage
//! lives in `parity_partial_index.rs` and `parity_expr_index.rs`.
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
fn partial_index_predicate_builds() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX i_pos ON t(a) WHERE a > 0")
        .expect("partial index should build");
}

#[test]
fn expression_index_builds() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("CREATE INDEX i_expr ON t(abs(a))")
        .expect("expression index should build");
}

#[test]
fn plain_index_still_works() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX i_basic ON t(a, b)")
        .expect("plain index should still build");
}
