//! WS-A2f-rewrite: opt-in pre-parse rewrite of
//! `DELETE/UPDATE ... [WHERE ...] [ORDER BY ...] LIMIT n [OFFSET m]` into
//! the `WHERE rowid IN (SELECT rowid FROM t ...)` subquery form.
//!
//! Default-OFF behaviour mirrors the SQLite autoconf amalgamation (which
//! rejects the syntax regardless of the `-DSQLITE_ENABLE_UPDATE_DELETE_LIMIT`
//! compile flag) so parity case 00220 stays clean. With
//! `PRAGMA redline_dml_order_limit_rewrite = ON` the same SQL succeeds and
//! produces the SQLite-spec semantics (delete/update exactly N rows by the
//! ORDER BY order).

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a2f_rewrite.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn seed_t(c: &Arc<Connection>) {
    c.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)")
        .expect("create");
    c.execute("INSERT INTO t VALUES (1, 10), (2, 20), (3, 30), (4, 40), (5, 50)")
        .expect("insert");
}

fn scalar_i64(c: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = c.prepare(sql).expect("prepare");
    stmt.step().expect("step");
    match stmt.column_value(0).expect("col 0") {
        SqlValue::Integer(v) => *v,
        other => panic!("expected Integer, got {other:?}"),
    }
}

#[test]
fn delete_order_limit_errors_without_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    let res = c.execute("DELETE FROM t ORDER BY id LIMIT 1");
    res.expect_err("expected rejection without PRAGMA opt-in");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 5);
}

#[test]
fn delete_limit_errors_without_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    let res = c.execute("DELETE FROM t LIMIT 1");
    res.expect_err("expected rejection without PRAGMA opt-in");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 5);
}

#[test]
fn delete_order_limit_succeeds_with_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    c.execute("DELETE FROM t ORDER BY id LIMIT 1")
        .expect("rewrite-enabled delete");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 4);
    // ORDER BY id ASC => lowest id (1) removed.
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 1"), 0);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 2"), 1);
}

#[test]
fn delete_order_desc_limit_picks_highest_row() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    c.execute("DELETE FROM t ORDER BY id DESC LIMIT 2")
        .expect("rewrite-enabled delete");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 3);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 5"), 0);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 4"), 0);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 3"), 1);
}

#[test]
fn delete_where_order_limit_keeps_predicate() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    // Predicate selects ids 2..=4 (v in {20,30,40}); ORDER BY v LIMIT 1
    // picks v=20 (id=2).
    c.execute("DELETE FROM t WHERE v > 15 AND v < 45 ORDER BY v LIMIT 1")
        .expect("rewrite-enabled delete");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 2"), 0);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 3"), 1);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 4);
}

#[test]
fn delete_limit_offset_with_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    // Skip first row (id=1), delete next one (id=2).
    c.execute("DELETE FROM t ORDER BY id LIMIT 1 OFFSET 1")
        .expect("rewrite-enabled delete");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 1"), 1);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 2"), 0);
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t"), 4);
}

#[test]
fn update_order_limit_errors_without_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    let res = c.execute("UPDATE t SET v = 999 ORDER BY id LIMIT 1");
    res.expect_err("expected rejection without PRAGMA opt-in");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE v = 999"), 0);
}

#[test]
fn update_order_limit_succeeds_with_pragma() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    c.execute("UPDATE t SET v = 999 ORDER BY id LIMIT 2")
        .expect("rewrite-enabled update");
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE v = 999"), 2);
    assert_eq!(
        scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 1 AND v = 999"),
        1
    );
    assert_eq!(
        scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 2 AND v = 999"),
        1
    );
    assert_eq!(
        scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 3 AND v = 30"),
        1
    );
}

#[test]
fn update_where_order_limit_keeps_predicate() {
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    c.execute("UPDATE t SET v = 111 WHERE v >= 20 ORDER BY v DESC LIMIT 1")
        .expect("rewrite-enabled update");
    assert_eq!(
        scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE id = 5 AND v = 111"),
        1
    );
    assert_eq!(scalar_i64(&c, "SELECT COUNT(*) FROM t WHERE v = 111"), 1);
}

#[test]
fn pragma_toggles_off_again() {
    // The PRAGMA gates the *parse* step. Once a DML statement has been
    // cached as the rewritten form, subsequent identical-text executes
    // reuse the cached template regardless of the current PRAGMA state
    // (the prepared-statement cache is keyed on SQL text + schema/stats
    // epochs only). To re-exercise the parse path after toggling OFF we
    // use a textually distinct statement so cache lookup misses.
    let (_d, c) = open();
    seed_t(&c);
    c.execute("PRAGMA redline_dml_order_limit_rewrite = ON")
        .expect("pragma on");
    c.execute("DELETE FROM t ORDER BY id LIMIT 1")
        .expect("rewrite-enabled delete");
    c.execute("PRAGMA redline_dml_order_limit_rewrite = OFF")
        .expect("pragma off");
    let res = c.execute("DELETE FROM t ORDER BY v LIMIT 1");
    res.expect_err("expected rejection after PRAGMA off (fresh SQL)");
}
