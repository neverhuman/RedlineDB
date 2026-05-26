//! WS-A7b: recursive CTE arena-backed worktable + AHashSet UNION dedup.
//!
//! Coverage:
//! - UNION DISTINCT recursive CTE at scale (1000+ rows): verifies the
//!   AHashSet dedup keeps unique-row counts correct and that the
//!   former O(n²) linear `row_in` is gone (must complete in <1s).
//! - UNION ALL recursive CTE: verifies duplicates are preserved
//!   (dedup is skipped entirely when union_all is true).
//! - LIMIT push-down (WS-A7) still short-circuits on top of the new
//!   arena/frontier representation.

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
fn union_distinct_recursive_cte_scales() {
    // 1500 distinct rows. With the old O(n²) dedup (`row_in` linear
    // scan against `accumulated` plus a full `accumulated.clone()` per
    // iteration) this would take ~1500*1500=2.25M comparisons just on
    // dedup, plus 1500 vector clones; the AHashSet keyed on encoded
    // bytes turns each insert into O(1) amortized.
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7b_dist.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();

    let sql = "WITH RECURSIVE c(n) AS (\
                 SELECT 1 UNION SELECT n+1 FROM c WHERE n<1500\
               ) SELECT count(*) FROM c";

    let start = Instant::now();
    let rows = run_query(&conn, sql);
    let elapsed = start.elapsed();

    assert_eq!(rows.len(), 1, "count(*) returns one row");
    match rows[0].first() {
        Some(SqlValue::Integer(n)) => assert_eq!(*n, 1500, "expected 1500 unique rows"),
        other => panic!("expected Integer count, got {other:?}"),
    }
    assert!(
        elapsed < Duration::from_secs(1),
        "WS-A7b: 1500-row UNION DISTINCT recursive CTE must run in <1s; took {elapsed:?}"
    );
}

#[test]
fn union_distinct_dedup_actually_dedups() {
    // The recursive arm's `JOIN dups` emits two candidate next-rows
    // for every current row (n+1 and n+1 again). UNION (no ALL) must
    // collapse them so the final result is 1..=20, exactly 20 rows.
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7b_dup.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();
    conn.execute("CREATE TABLE dups(k INTEGER)")
        .expect("create dups");
    conn.execute("INSERT INTO dups VALUES (1), (1)")
        .expect("insert dups");

    let sql = "WITH RECURSIVE c(n) AS (\
                 SELECT 1 UNION \
                 SELECT c.n + dups.k FROM c, dups WHERE c.n<20\
               ) SELECT count(*) FROM c";

    let rows = run_query(&conn, sql);
    match rows[0].first() {
        Some(SqlValue::Integer(n)) => assert_eq!(*n, 20, "expected 20 unique rows after dedup"),
        other => panic!("expected Integer count, got {other:?}"),
    }
}

#[test]
fn union_all_preserves_duplicates() {
    // UNION ALL must skip dedup entirely. Anchor emits (1, 'a'), each
    // recursive iteration emits two rows with the same n+1, so by
    // termination we have a strictly larger multiset than the DISTINCT
    // shape would produce.
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7b_all.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();

    let sql = "WITH RECURSIVE c(n) AS (\
                 SELECT 1 UNION ALL \
                 SELECT n+1 FROM c WHERE n<5\
               ) SELECT count(*) FROM c";

    let rows = run_query(&conn, sql);
    match rows[0].first() {
        // Standard recursive expansion: 1, 2, 3, 4, 5 — five rows.
        // UNION ALL just means no dedup is applied; here the recursion
        // produces no duplicates by shape, so count matches DISTINCT.
        Some(SqlValue::Integer(n)) => assert_eq!(*n, 5, "expected 5 rows"),
        other => panic!("expected Integer count, got {other:?}"),
    }

    // Now a shape where UNION ALL actually retains duplicates: every
    // row in the previous frontier emits two copies of the next n,
    // doubling the working-set growth versus UNION DISTINCT.
    let sql_dup = "WITH RECURSIVE c(n) AS (\
                     VALUES(1),(1) UNION ALL \
                     SELECT n+1 FROM c WHERE n<3\
                   ) SELECT n FROM c ORDER BY n";
    let rows = run_query(&conn, sql_dup);
    let values: Vec<i64> = rows
        .iter()
        .map(|r| match r.first() {
            Some(SqlValue::Integer(n)) => *n,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect();
    // Anchor: 1,1. Iter 1: each 1 -> 2, so 2,2 added; frontier=[2,2].
    // Iter 2: each 2 -> 3, so 3,3 added; frontier=[3,3].
    // Iter 3: WHERE n<3 filters out everything, terminate.
    // Accumulated multiset: {1,1,2,2,3,3}.
    assert_eq!(values, vec![1, 1, 2, 2, 3, 3]);
}

#[test]
fn limit_pushdown_still_applies_with_arena() {
    // Mirrors `ws_a7_recursive_cte_limit::unbounded_recursive_cte_with_outer_limit`
    // to confirm WS-A7's LIMIT push-down still short-circuits over the
    // new frontier-range representation.
    let dir = tempdir().expect("temp dir");
    let db = Database::create(&dir.path().join("ws_a7b_limit.db"), DbOptions::default())
        .expect("create db");
    let conn = db.connect();

    let sql = "WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c) \
               SELECT n FROM c LIMIT 12";

    let start = Instant::now();
    let rows = run_query(&conn, sql);
    let elapsed = start.elapsed();

    assert_eq!(rows.len(), 12);
    for (i, row) in rows.iter().enumerate() {
        let expected = (i as i64) + 1;
        match row.first() {
            Some(SqlValue::Integer(n)) => assert_eq!(*n, expected, "row {i}"),
            other => panic!("row {i}: expected Integer({expected}), got {other:?}"),
        }
    }
    assert!(
        elapsed < Duration::from_millis(200),
        "WS-A7b: LIMIT push-down must still short-circuit; took {elapsed:?}"
    );
}
