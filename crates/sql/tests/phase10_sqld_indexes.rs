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

// A6 SQL-D: partial and expression indexes now execute end-to-end via the
// v7 catalog. The old "parser-only" assertions were removed when the
// feature landed; see parity_partial_index.rs + parity_expr_index.rs for
// the differential-rusqlite coverage.

#[test]
fn partial_index_now_executes() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX i_pos ON t(a) WHERE a > 0")
        .expect("partial index should execute");
}

#[test]
fn expression_index_now_executes() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("CREATE INDEX i_expr ON t(abs(a))")
        .expect("expression index should execute");
}

#[test]
fn plain_index_still_works() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX i_basic ON t(a, b)")
        .expect("plain index should still build");
}

#[test]
fn create_index_nulls_first_last_is_rejected() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    for (sql, expected) in [
        (
            "CREATE INDEX i_asc_first ON t(a ASC NULLS FIRST)",
            "unsupported use of NULLS FIRST",
        ),
        (
            "CREATE INDEX i_desc_last ON t(a DESC NULLS LAST)",
            "unsupported use of NULLS LAST",
        ),
        (
            "CREATE INDEX i_asc_last ON t(a ASC NULLS LAST)",
            "unsupported use of NULLS LAST",
        ),
        (
            "CREATE INDEX i_desc_first ON t(a DESC NULLS FIRST)",
            "unsupported use of NULLS FIRST",
        ),
    ] {
        let err = conn.execute(sql).expect_err("NULLS order must be rejected");
        assert!(
            err.to_string().contains(expected),
            "expected {expected:?} in {err:?}"
        );
    }
}
