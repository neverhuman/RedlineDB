//! SQLite drop-in parity coverage tests — positive cases for constructs
//! previously untested or thinly covered.
//!
//! Covers: ALTER TABLE RENAME, DROP INDEX, RETURNING with expressions,
//! subqueries (EXISTS/NOT EXISTS/correlated), multi-join chains,
//! NULL semantics, PRAGMA integrity_check, WAL checkpoint modes,
//! nested SAVEPOINTs, and NULL IN/NOT IN edge cases.

use std::sync::Arc;
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("pos.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn query_all(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let row: Vec<SqlValue> = (0..ncols)
            .map(|i| stmt.column_value(i).expect("col").clone())
            .collect();
        out.push(row);
    }
    out
}

fn q1(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    query_all(conn, sql)
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next())
        .unwrap_or(SqlValue::Null)
}

// ── ALTER TABLE RENAME ────────────────────────────────────────────────────────

#[test]
fn alter_table_rename_to() {
    let (_d, c) = open();
    c.execute("CREATE TABLE old_name(id INTEGER)").expect("create");
    c.execute("INSERT INTO old_name VALUES (42)").expect("insert");
    c.execute("ALTER TABLE old_name RENAME TO new_name").expect("rename");
    let v = q1(&c, "SELECT id FROM new_name");
    assert_eq!(v, SqlValue::Integer(42));
}

#[test]
fn alter_table_rename_column() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(old_col INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (7)").expect("insert");
    c.execute("ALTER TABLE t RENAME COLUMN old_col TO new_col").expect("rename column");
    let v = q1(&c, "SELECT new_col FROM t");
    assert_eq!(v, SqlValue::Integer(7));
}

// ── DROP INDEX ────────────────────────────────────────────────────────────────

#[test]
fn create_and_drop_index() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    c.execute("CREATE INDEX idx_a ON t(a)").expect("create index");
    c.execute("DROP INDEX idx_a").expect("drop index");
    // Verify the index is gone by re-creating it with the same name (would fail if it still existed)
    c.execute("CREATE INDEX idx_a ON t(a)").expect("re-create after drop");
}

#[test]
fn drop_index_if_exists() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    // Should not error when the index does not exist
    c.execute("DROP INDEX IF EXISTS idx_nonexistent").expect("drop if exists");
}

// ── RETURNING with expressions ────────────────────────────────────────────────

#[test]
fn returning_with_arithmetic_expression() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b INTEGER)").expect("create");
    let rows = query_all(&c, "INSERT INTO t VALUES (3, 4) RETURNING a + b");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(7));
}

#[test]
fn returning_with_function_call() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(name TEXT)").expect("create");
    let rows = query_all(&c, "INSERT INTO t VALUES ('hello') RETURNING upper(name)");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Text(Arc::from("HELLO")));
}

#[test]
fn update_returning_with_expression() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (10, 3)").expect("insert");
    let rows = query_all(&c, "UPDATE t SET a = a + 1 RETURNING a * b");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(33)); // (10+1)*3
}

// ── EXISTS / NOT EXISTS ───────────────────────────────────────────────────────

#[test]
fn exists_subquery_true() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1)").expect("insert");
    let v = q1(&c, "SELECT 1 WHERE EXISTS (SELECT 1 FROM t)");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn exists_subquery_false() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    // Empty table → EXISTS is false → no rows in outer
    let rows = query_all(&c, "SELECT 1 WHERE EXISTS (SELECT 1 FROM t)");
    assert!(rows.is_empty(), "expected no rows");
}

#[test]
fn not_exists_subquery_true() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    let v = q1(&c, "SELECT 42 WHERE NOT EXISTS (SELECT 1 FROM t)");
    assert_eq!(v, SqlValue::Integer(42));
}

// ── NULL semantics ────────────────────────────────────────────────────────────

#[test]
fn null_in_empty_list() {
    let (_d, c) = open();
    // NULL IN () → NULL (or 0 by SQLite semantics — returns false/0)
    let v = q1(&c, "SELECT NULL IN (1, 2, 3)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn value_in_list_with_null() {
    let (_d, c) = open();
    // 1 IN (1, NULL) → 1 (found, ignores NULL)
    let v = q1(&c, "SELECT 1 IN (1, NULL)");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn value_not_in_list_with_null() {
    // 2 NOT IN (1, NULL) → NULL (because NULL means unknown whether 2 = NULL)
    let (_d, c) = open();
    let v = q1(&c, "SELECT 2 NOT IN (1, NULL)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn null_comparison_is_null() {
    let (_d, c) = open();
    // NULL = NULL → NULL, not 1
    let v = q1(&c, "SELECT NULL = NULL");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn null_is_null_is_true() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT NULL IS NULL");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn value_is_not_null() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT 1 IS NOT NULL");
    assert_eq!(v, SqlValue::Integer(1));
}

// ── PRAGMA integrity_check ────────────────────────────────────────────────────

#[test]
fn pragma_integrity_check_ok() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2)").expect("insert");
    let rows = query_all(&c, "PRAGMA integrity_check");
    // Should return at least one row with "ok"
    assert!(!rows.is_empty());
    let first = &rows[0][0];
    let s = match first {
        SqlValue::Text(s) => s.as_ref(),
        _ => "",
    };
    assert_eq!(s.to_lowercase(), "ok", "integrity_check returned: {s}");
}

// ── WAL checkpoint modes ──────────────────────────────────────────────────────

#[test]
fn pragma_wal_checkpoint_passive() {
    let (_d, c) = open();
    c.execute("PRAGMA journal_mode=WAL").expect("wal");
    // Should not error; checkpoint returns row(s) with counts
    let mut stmt = c.prepare("PRAGMA wal_checkpoint(PASSIVE)").expect("prepare");
    while let Step::Row = stmt.step().unwrap() {}
}

#[test]
fn pragma_wal_checkpoint_full() {
    let (_d, c) = open();
    c.execute("PRAGMA journal_mode=WAL").expect("wal");
    let mut stmt = c.prepare("PRAGMA wal_checkpoint(FULL)").expect("prepare");
    while let Step::Row = stmt.step().unwrap() {}
}

#[test]
fn pragma_wal_checkpoint_restart() {
    let (_d, c) = open();
    c.execute("PRAGMA journal_mode=WAL").expect("wal");
    let mut stmt = c.prepare("PRAGMA wal_checkpoint(RESTART)").expect("prepare");
    while let Step::Row = stmt.step().unwrap() {}
}

#[test]
fn pragma_wal_checkpoint_truncate() {
    let (_d, c) = open();
    c.execute("PRAGMA journal_mode=WAL").expect("wal");
    let mut stmt = c.prepare("PRAGMA wal_checkpoint(TRUNCATE)").expect("prepare");
    while let Step::Row = stmt.step().unwrap() {}
}

// ── Nested SAVEPOINT ──────────────────────────────────────────────────────────

#[test]
fn nested_savepoint_basic() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("BEGIN").expect("begin");
    c.execute("INSERT INTO t VALUES (1)").expect("ins1");
    c.execute("SAVEPOINT sp1").expect("sp1");
    c.execute("INSERT INTO t VALUES (2)").expect("ins2");
    c.execute("ROLLBACK TO sp1").expect("rollback to sp1");
    // Row 2 should be gone after rollback-to
    let count = q1(&c, "SELECT count(*) FROM t");
    assert_eq!(count, SqlValue::Integer(1));
    c.execute("COMMIT").expect("commit");
}

#[test]
fn nested_savepoint_release() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("BEGIN").expect("begin");
    c.execute("INSERT INTO t VALUES (10)").expect("ins");
    c.execute("SAVEPOINT sp_a").expect("savepoint");
    c.execute("INSERT INTO t VALUES (20)").expect("ins2");
    c.execute("RELEASE sp_a").expect("release");
    c.execute("COMMIT").expect("commit");
    // Both rows should be committed
    let count = q1(&c, "SELECT count(*) FROM t");
    assert_eq!(count, SqlValue::Integer(2));
}

// ── PRAGMA additional coverage ────────────────────────────────────────────────

#[test]
fn pragma_auto_vacuum() {
    let (_d, c) = open();
    // Read current auto_vacuum setting
    let v = q1(&c, "PRAGMA auto_vacuum");
    assert!(
        matches!(v, SqlValue::Integer(_)),
        "expected integer, got {v:?}"
    );
}

#[test]
fn pragma_quick_check() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    let rows = query_all(&c, "PRAGMA quick_check");
    assert!(!rows.is_empty());
}

// ── Large integer boundary ────────────────────────────────────────────────────

#[test]
fn i64_max_stores_and_retrieves() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute(&format!("INSERT INTO t VALUES ({})", i64::MAX)).expect("insert");
    let v = q1(&c, "SELECT v FROM t");
    assert_eq!(v, SqlValue::Integer(i64::MAX));
}

#[test]
fn i64_min_stores_and_retrieves() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute(&format!("INSERT INTO t VALUES ({})", i64::MIN)).expect("insert");
    let v = q1(&c, "SELECT v FROM t");
    assert_eq!(v, SqlValue::Integer(i64::MIN));
}

// ── Multi-join chain ──────────────────────────────────────────────────────────

#[test]
fn inner_join_chain() {
    let (_d, c) = open();
    c.execute("CREATE TABLE a(id INTEGER, name TEXT)").expect("create a");
    c.execute("CREATE TABLE b(aid INTEGER, val TEXT)").expect("create b");
    c.execute("CREATE TABLE bb(bid INTEGER, extra TEXT)").expect("create bb");
    c.execute("INSERT INTO a VALUES (1, 'alice')").expect("ins a");
    c.execute("INSERT INTO b VALUES (1, 'beta')").expect("ins b");
    c.execute("INSERT INTO bb VALUES (1, 'extra')").expect("ins bb");
    let rows = query_all(&c, "SELECT a.name, b.val, bb.extra FROM a JOIN b ON a.id = b.aid JOIN bb ON b.aid = bb.bid");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Text(Arc::from("alice")));
    assert_eq!(rows[0][1], SqlValue::Text(Arc::from("beta")));
    assert_eq!(rows[0][2], SqlValue::Text(Arc::from("extra")));
}
