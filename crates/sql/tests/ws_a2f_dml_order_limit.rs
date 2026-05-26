//! Phase 5 WS-A2f: DML `ORDER BY` / `LIMIT` / `OFFSET` parity.
//!
//! SQLite accepts `DELETE FROM t WHERE ... ORDER BY x LIMIT n` and the
//! equivalent UPDATE shape; RedlineDB used to reject both at the parser.
//! These tests pin the new behaviour:
//!   * DELETE deletes exactly the rows it would select with the same
//!     WHERE / ORDER BY / LIMIT shape.
//!   * UPDATE updates exactly that subset (sqlparser 0.61 parses
//!     `UPDATE ... LIMIT n` but NOT `UPDATE ... ORDER BY ...`, so the
//!     UPDATE order-by surface is exercised only via the LIMIT-only
//!     shape until a parser pre-rewrite lands).
//!   * The legacy `DELETE FROM t WHERE x = y` path is untouched.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a2f.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    Box::leak(Box::new(dir));
    conn
}

fn collect_int_pairs(conn: &Arc<Connection>, sql: &str) -> Vec<(i64, i64)> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let a = match stmt.column_value(0).expect("col0").clone() {
            SqlValue::Integer(n) => n,
            other => panic!("col0 not Integer: {other:?}"),
        };
        let b = match stmt.column_value(1).expect("col1").clone() {
            SqlValue::Integer(n) => n,
            other => panic!("col1 not Integer: {other:?}"),
        };
        out.push((a, b));
    }
    out
}

fn seed_ten(conn: &Arc<Connection>) {
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, x INTEGER, v INTEGER)")
        .expect("ddl");
    for id in 1..=10 {
        // x = 11 - id so id=1 has the largest x (10), id=10 the smallest (1).
        let x = 11 - id;
        let v = id * 10;
        conn.execute(&format!(
            "INSERT INTO t(id, x, v) VALUES ({id}, {x}, {v})"
        ))
        .expect("insert");
    }
}

#[test]
fn delete_order_by_limit_removes_smallest_id_among_predicate_rows() {
    let conn = open();
    seed_ten(&conn);
    // All ten rows have id < 100; ordering by id ASC and LIMIT 3 must
    // delete rows id=1,2,3.
    let affected = conn
        .execute("DELETE FROM t WHERE id < 100 ORDER BY id LIMIT 3")
        .expect("delete order limit");
    assert_eq!(affected, 3, "DELETE should affect exactly 3 rows");
    let remaining = collect_int_pairs(&conn, "SELECT id, v FROM t ORDER BY id");
    let ids: Vec<i64> = remaining.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec![4, 5, 6, 7, 8, 9, 10]);
}

#[test]
fn delete_order_by_limit_descending_removes_largest_id() {
    let conn = open();
    seed_ten(&conn);
    let affected = conn
        .execute("DELETE FROM t ORDER BY id DESC LIMIT 2")
        .expect("delete order desc limit");
    assert_eq!(affected, 2);
    let ids: Vec<i64> = collect_int_pairs(&conn, "SELECT id, v FROM t ORDER BY id")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn delete_no_order_by_still_works_for_legacy_path() {
    let conn = open();
    seed_ten(&conn);
    let affected = conn
        .execute("DELETE FROM t WHERE id = 5")
        .expect("delete by pk");
    assert_eq!(affected, 1);
    let ids: Vec<i64> = collect_int_pairs(&conn, "SELECT id, v FROM t ORDER BY id")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 6, 7, 8, 9, 10]);
}

#[test]
fn update_limit_only_updates_capped_row_count() {
    // sqlparser 0.61's SQLite dialect parses `UPDATE ... LIMIT n` (no
    // ORDER BY). Without ORDER BY the row pick is implementation-defined,
    // so we only assert the cap and the fact that surviving rows are
    // either fully updated (v=99) or untouched.
    let conn = open();
    seed_ten(&conn);
    let affected = conn
        .execute("UPDATE t SET v = 99 WHERE x > 0 LIMIT 2")
        .expect("update limit");
    assert_eq!(affected, 2, "UPDATE should affect exactly 2 rows");
    let rows = collect_int_pairs(&conn, "SELECT id, v FROM t ORDER BY id");
    let touched = rows.iter().filter(|(_, v)| *v == 99).count();
    let untouched = rows.iter().filter(|(_, v)| *v != 99).count();
    assert_eq!(touched, 2);
    assert_eq!(untouched, 8);
}

#[test]
fn delete_order_by_secondary_key_breaks_ties_deterministically() {
    let conn = open();
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, k INTEGER, v INTEGER)")
        .expect("ddl");
    // Three rows share k=1, distinguishable only by id.
    conn.execute("INSERT INTO t VALUES (1, 1, 10), (2, 1, 20), (3, 1, 30), (4, 2, 40)")
        .expect("seed");
    let affected = conn
        .execute("DELETE FROM t ORDER BY k ASC, id DESC LIMIT 2")
        .expect("delete tie-break");
    // Among k=1 rows, id DESC picks ids 3 and 2 first. Two are deleted,
    // leaving id=1 (k=1) and id=4 (k=2).
    assert_eq!(affected, 2);
    let ids: Vec<i64> = collect_int_pairs(&conn, "SELECT id, v FROM t ORDER BY id")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![1, 4]);
}

#[test]
fn delete_order_by_returning_emits_only_the_window() {
    let conn = open();
    seed_ten(&conn);
    // sqlparser 0.61 parses DELETE in the order WHERE → RETURNING →
    // ORDER BY → LIMIT, so the RETURNING clause comes before ORDER BY in
    // the surface syntax we accept.
    let mut stmt = conn
        .prepare("DELETE FROM t WHERE id < 100 RETURNING id ORDER BY id LIMIT 2")
        .expect("prepare returning");
    let mut emitted = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        match stmt.column_value(0).expect("col0").clone() {
            SqlValue::Integer(n) => emitted.push(n),
            other => panic!("col0 not int: {other:?}"),
        }
    }
    drop(stmt);
    assert_eq!(emitted, vec![1, 2]);
    let count = conn
        .prepare("SELECT count(*) FROM t")
        .and_then(|mut s| {
            s.step()?;
            Ok(match s.column_value(0).expect("c").clone() {
                SqlValue::Integer(n) => n,
                other => panic!("not int: {other:?}"),
            })
        })
        .expect("count");
    assert_eq!(count, 8);
}
