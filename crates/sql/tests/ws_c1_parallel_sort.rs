//! Phase 5 WS-C1: in-memory spill-sort buffer uses Rayon's `par_sort_by`
//! when the buffer exceeds `PARALLEL_SORT_THRESHOLD` (64 KiB rows).
//!
//! Correctness pin only — no benchmark. Insert 100K rows in non-sorted
//! order, run ORDER BY, assert the row count and ascending order. The
//! comparator (`SortDirection::compare_values` -> `value::compare_values`)
//! is pure, so parallel execution must yield the same result as serial.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_c1.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    Box::leak(Box::new(dir));
    conn
}

#[test]
fn parallel_sort_threshold_preserves_correctness() {
    let conn = open();
    conn.execute("CREATE TABLE t (k INTEGER, v INTEGER)")
        .expect("ddl");

    const N: i64 = 100_000;

    conn.execute("BEGIN").expect("begin");
    // Insert in shuffled order so the sort actually has work to do.
    // Deterministic LCG-style mixing keeps the test reproducible.
    let mut stmt = conn
        .prepare("INSERT INTO t(k, v) VALUES (?1, ?2)")
        .expect("prepare");
    for i in 0..N {
        let k = (i.wrapping_mul(2_654_435_761)) % N;
        stmt.reset().expect("reset");
        stmt.clear_bindings();
        stmt.bind_i64(1, k).expect("bind k");
        stmt.bind_i64(2, i).expect("bind v");
        while let Step::Row = stmt.step().expect("step") {}
    }
    drop(stmt);
    conn.execute("COMMIT").expect("commit");

    // Row count check.
    let mut count_stmt = conn.prepare("SELECT COUNT(*) FROM t").expect("prepare");
    let mut count: i64 = -1;
    while let Step::Row = count_stmt.step().expect("step") {
        match count_stmt.column_value(0).expect("col").clone() {
            SqlValue::Integer(n) => count = n,
            other => panic!("expected Integer, got {other:?}"),
        }
    }
    drop(count_stmt);
    assert_eq!(count, N, "row count mismatch");

    // Ascending order check — also forces the in-memory sort path.
    let mut order_stmt = conn
        .prepare("SELECT k FROM t ORDER BY k ASC")
        .expect("prepare");
    let mut prev: Option<i64> = None;
    let mut seen: i64 = 0;
    while let Step::Row = order_stmt.step().expect("step") {
        let k = match order_stmt.column_value(0).expect("col").clone() {
            SqlValue::Integer(v) => v,
            other => panic!("expected Integer, got {other:?}"),
        };
        if let Some(p) = prev {
            assert!(
                p <= k,
                "ORDER BY ascending invariant violated: {p} > {k} at row {seen}"
            );
        }
        prev = Some(k);
        seen += 1;
    }
    assert_eq!(seen, N, "ORDER BY scan returned wrong row count");
}
