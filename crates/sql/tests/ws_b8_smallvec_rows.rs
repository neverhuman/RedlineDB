//! WS-B8 Part 1: SmallVec inline row representation.
//!
//! Verifies the inline-stack-allocated row buffer used by the vec
//! `filter_batch` path: small rows (<= 8 columns) stay on the stack;
//! wider rows spill to the heap. The end-to-end smoke confirms that the
//! SELECT pipeline still produces correct results through the swap.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use smallvec::SmallVec;
use tempfile::tempdir;

/// Mirrors the buffer type used inside `exec::vec::select::filter_batch`.
type RowBuffer = SmallVec<[SqlValue; 8]>;

#[test]
fn small_row_three_cols_stays_inline() {
    let mut row: RowBuffer = SmallVec::with_capacity(3);
    row.push(SqlValue::Integer(1));
    row.push(SqlValue::Integer(2));
    row.push(SqlValue::Integer(3));
    assert_eq!(row.len(), 3);
    assert!(!row.spilled(), "3-column row must not spill to heap");
}

#[test]
fn row_at_inline_capacity_does_not_spill() {
    let mut row: RowBuffer = SmallVec::new();
    for i in 0..8 {
        row.push(SqlValue::Integer(i));
    }
    assert_eq!(row.len(), 8);
    assert!(!row.spilled(), "8-column row (= inline capacity) must not spill");
}

#[test]
fn wide_row_twelve_cols_spills_to_heap() {
    let mut row: RowBuffer = SmallVec::new();
    for i in 0..12 {
        row.push(SqlValue::Integer(i));
    }
    assert_eq!(row.len(), 12);
    assert!(row.spilled(), "12-column row must spill to heap");
}

#[test]
fn select_smoke_through_vec_filter_path() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ws_b8.db");
    let db = Database::create(&path, DbOptions::default()).expect("create");
    let conn: Arc<Connection> = db.connect();

    conn.execute("CREATE TABLE t (a INTEGER, b INTEGER, c INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 10, 100), (2, 20, 200), (3, 30, 300)")
        .expect("insert");

    let mut stmt = conn
        .prepare("SELECT a, b, c FROM t WHERE b > 15 ORDER BY a")
        .expect("prepare");
    let mut out: Vec<Vec<SqlValue>> = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let n = stmt.column_count();
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            row.push(stmt.column_value(i).expect("col").clone());
        }
        out.push(row);
    }
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[0],
        vec![
            SqlValue::Integer(2),
            SqlValue::Integer(20),
            SqlValue::Integer(200),
        ]
    );
    assert_eq!(
        out[1],
        vec![
            SqlValue::Integer(3),
            SqlValue::Integer(30),
            SqlValue::Integer(300),
        ]
    );
}
