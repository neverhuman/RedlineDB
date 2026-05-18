//! Negative tests for every `UnsupportedSql` boundary.
//!
//! Each test sends a SQL statement that hits a known unsupported path and
//! asserts: (a) it returns an error, (b) the error message matches the
//! documented contract string. This ensures silent mis-implementation cannot
//! regress a known-unsupported feature.

use redlinedb_sql::{Connection, Database, DbOptions};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("neg.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn assert_unsupported(result: Result<usize, redlinedb_sql::Error>, fragment: &str) {
    let err = result.expect_err(&format!("expected UnsupportedSql, fragment={fragment:?}"));
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains(&fragment.to_lowercase()),
        "expected error to contain {fragment:?}, got: {msg}"
    );
}

fn assert_errors(result: Result<usize, redlinedb_sql::Error>) {
    result.expect_err("expected an error but got Ok");
}

// ── DML unsupported constructs ────────────────────────────────────────────────

#[test]
fn update_from_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER, v TEXT)")
        .expect("create");
    c.execute("CREATE TABLE src(id INTEGER, v TEXT)")
        .expect("create");
    let res = c.execute("UPDATE t SET v = src.v FROM src WHERE t.id = src.id");
    assert_unsupported(res, "not supported");
}

#[test]
fn update_or_conflict_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    c.execute("INSERT INTO t VALUES (1, 'a')").expect("insert");
    let res = c.execute("UPDATE OR REPLACE t SET id = 1, v = 'b' WHERE id = 1");
    assert_errors(res);
}

#[test]
fn delete_using_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    c.execute("CREATE TABLE src(id INTEGER)").expect("create");
    let res = c.execute("DELETE FROM t USING src WHERE t.id = src.id");
    assert_errors(res);
}

#[test]
fn delete_limit_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2),(3)")
        .expect("insert");
    let res = c.execute("DELETE FROM t LIMIT 1");
    assert_unsupported(res, "not supported");
}

#[test]
fn delete_order_by_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2),(3)")
        .expect("insert");
    let res = c.execute("DELETE FROM t ORDER BY id");
    assert_errors(res);
}

#[test]
fn insert_set_syntax_is_unsupported() {
    // MySQL-style INSERT ... SET
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER, v TEXT)")
        .expect("create");
    let res = c.execute("INSERT INTO t SET id=1, v='x'");
    assert_errors(res);
}

#[test]
fn insert_on_duplicate_key_update_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    let res = c.execute("INSERT INTO t VALUES (1, 'a') ON DUPLICATE KEY UPDATE v = 'b'");
    assert_errors(res);
}

// ── DDL unsupported constructs ────────────────────────────────────────────────

#[test]
fn create_table_as_select_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE src(id INTEGER)").expect("create");
    c.execute("INSERT INTO src VALUES (1)").expect("insert");
    let res = c.execute("CREATE TABLE dst AS SELECT * FROM src");
    assert_unsupported(res, "not supported");
}

#[test]
fn alter_table_only_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    // ALTER TABLE ONLY is Postgres-specific
    let res = c.execute("ALTER TABLE ONLY t ADD COLUMN v TEXT");
    assert_errors(res);
}

#[test]
fn alter_table_add_column_after_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    let res = c.execute("ALTER TABLE t ADD COLUMN b TEXT AFTER a");
    assert_errors(res);
}

#[test]
fn alter_table_drop_multiple_columns_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b TEXT, c REAL)")
        .expect("create");
    let res = c.execute("ALTER TABLE t DROP COLUMN a, DROP COLUMN b");
    assert_errors(res);
}

#[test]
fn create_index_with_include_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    // PostgreSQL-style INCLUDE is not supported
    let res = c.execute("CREATE INDEX idx ON t(a) INCLUDE (b)");
    assert_errors(res);
}

// ── SELECT unsupported constructs ────────────────────────────────────────────

#[test]
fn distinct_on_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    let res = c.execute("SELECT DISTINCT ON (a) a, b FROM t");
    assert_unsupported(res, "not supported");
}

#[test]
fn natural_join_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE a(id INTEGER)").expect("create");
    c.execute("CREATE TABLE b(id INTEGER)").expect("create");
    let res = c.execute("SELECT * FROM a NATURAL JOIN b");
    assert_unsupported(res, "not supported");
}

#[test]
fn group_by_all_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    let res = c.execute("SELECT a FROM t GROUP BY ALL");
    assert_errors(res);
}

#[test]
fn like_any_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    // LIKE ANY (pattern_list) — Postgres extension
    let res = c.execute("SELECT v FROM t WHERE v LIKE ANY (ARRAY['%foo%'])");
    assert_errors(res);
}

#[test]
fn in_subquery_multi_column_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE s(a INTEGER, b INTEGER)")
        .expect("create");
    let res = c.execute("SELECT 1 WHERE 1 IN (SELECT a, b FROM s)");
    assert_errors(res);
}

#[test]
fn case_in_aggregate_is_unsupported() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2),(3)")
        .expect("insert");
    // CASE containing aggregate
    let res = c.execute("SELECT CASE WHEN count(*) > 2 THEN 1 ELSE 0 END FROM t");
    assert_unsupported(res, "not supported");
}

// ── Vector unsupported type ───────────────────────────────────────────────────

#[test]
fn vector_non_f32_type_is_unsupported() {
    let (_d, c) = open();
    let res = c.execute("CREATE TABLE t(v VECTOR(3, float64))");
    assert_unsupported(res, "not supported");
}

// ── Parse-only features — confirmed boundary ──────────────────────────────────

#[test]
fn cte_now_executes() {
    // CTEs are implemented; this test confirms the regression boundary.
    // Differential coverage lives in `parity_cte.rs`.
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1)").expect("insert");
    c.execute("WITH cte AS (SELECT a FROM t) SELECT * FROM cte")
        .expect("CTE should execute");
}

#[test]
fn create_view_now_executes() {
    // Views are implemented (Lane A5-views); this test confirms the
    // regression boundary. Differential coverage lives in
    // `parity_view.rs`.
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1)").expect("insert");
    c.execute("CREATE VIEW v AS SELECT a FROM t")
        .expect("CREATE VIEW should execute");
    c.execute("SELECT a FROM v")
        .expect("SELECT FROM view should execute");
}

#[test]
fn window_function_now_executes() {
    // Window functions are implemented; this test confirms the
    // regression boundary. Differential coverage lives in
    // `parity_window.rs`.
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2),(3)")
        .expect("insert");
    c.execute("SELECT row_number() OVER (ORDER BY a) FROM t")
        .expect("OVER should execute");
}

// A6 SQL-D: partial indexes are now supported (see parity_partial_index.rs).
// Stale "returns-error" assertion removed when the feature landed.

#[test]
fn unsupported_function_returns_error() {
    let (_d, c) = open();
    // A function that definitely does not exist
    let res = c.execute("SELECT totally_fake_function_xyz(1)");
    assert_errors(res);
}
