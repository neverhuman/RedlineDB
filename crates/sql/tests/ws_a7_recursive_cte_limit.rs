//! WS-A7: recursive CTE LIMIT pushdown.
//!
//! Without the pushdown, an unbounded recursive CTE (no terminating
//! WHERE) runs until the hard 10_000-iteration cap before the outer
//! LIMIT is applied. With the pushdown the recursion stops as soon as
//! the outer LIMIT (plus OFFSET) rows have been collected, mirroring
//! SQLite's `pushDownWhere`/LIMIT-into-worktable optimization.

use redlinedb_sql::{Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn run_query(conn: &Arc<redlinedb_sql::Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let cols = stmt.column_count();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let mut current = Vec::with_capacity(cols);
        for i in 0..cols {
            current.push(stmt.column_value(i).expect("col").clone());
        }
        rows.push(current);
    }
    rows
}

#[test]
fn unbounded_recursive_cte_with_outer_limit() {
    // No WHERE in the recursive arm. Without LIMIT pushdown this
    // would hit RECURSIVE_CTE_ITERATION_LIMIT (10_000) and error.
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();
    let sql = "WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c) \
               SELECT n FROM c LIMIT 10";

    let start = Instant::now();
    let rows = run_query(&conn, sql);
    let elapsed = start.elapsed();

    assert_eq!(rows.len(), 10, "expected 10 rows, got {}", rows.len());
    for (i, row) in rows.iter().enumerate() {
        let expected = (i as i64) + 1;
        match row.first() {
            Some(SqlValue::Integer(n)) => assert_eq!(*n, expected, "row {i}"),
            other => panic!("row {i}: expected Integer({expected}), got {other:?}"),
        }
    }
    assert!(
        elapsed < Duration::from_millis(100),
        "WS-A7: recursive CTE with LIMIT should short-circuit fast; took {elapsed:?}"
    );
}

#[test]
fn unbounded_recursive_cte_with_limit_offset() {
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7_off.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();
    let sql = "WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c) \
               SELECT n FROM c LIMIT 5 OFFSET 3";

    let rows = run_query(&conn, sql);
    assert_eq!(rows.len(), 5, "expected 5 rows with OFFSET 3");
    let values: Vec<i64> = rows
        .iter()
        .map(|r| match r.first() {
            Some(SqlValue::Integer(n)) => *n,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect();
    assert_eq!(values, vec![4, 5, 6, 7, 8]);
}

#[test]
fn bounded_recursive_cte_unaffected() {
    // Sanity: a recursion with a terminating WHERE still produces
    // the right rows (the pushdown should only short-circuit early
    // when the outer LIMIT is reached first; in this case the inner
    // WHERE terminates earlier than the LIMIT, so the LIMIT is just
    // a no-op slice).
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7_bounded.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();
    let sql = "WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c WHERE n<4) \
               SELECT n FROM c LIMIT 100";

    let rows = run_query(&conn, sql);
    let values: Vec<i64> = rows
        .iter()
        .map(|r| match r.first() {
            Some(SqlValue::Integer(n)) => *n,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect();
    assert_eq!(values, vec![1, 2, 3, 4]);
}
