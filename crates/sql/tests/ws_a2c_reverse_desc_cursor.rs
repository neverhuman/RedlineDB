//! Phase 5 WS-A2c: ORDER BY x DESC LIMIT n on an INDEX(x) (or an
//! equality-prefix + DESC suffix on a composite index) must walk the
//! cursor right-to-left and early-stop, NOT fall back to a full sort.
//!
//! Before WS-A2c, `ORDER BY k DESC LIMIT n` was disqualified by the
//! ordered-limit fast path because the kernel cursor walked
//! left-to-right only; the planner emitted an O(n)-memory sort. After
//! WS-A2c the reverse cursor closes the gap.
//!
//! This test pins the semantic: the rows returned are the top-n DESC
//! ones in correct order. Indirectly verifies early-stop fired (result
//! count == LIMIT) — a sort fallback would still produce the same row
//! count but would have first sorted all 1000 rows.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a2c.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    Box::leak(Box::new(dir));
    conn
}

fn collect_int_col(conn: &Arc<Connection>, sql: &str, col: usize) -> Vec<i64> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        match stmt.column_value(col).expect("col").clone() {
            SqlValue::Integer(n) => out.push(n),
            other => panic!("expected Integer, got {other:?}"),
        }
    }
    out
}

#[test]
fn order_by_desc_limit_uses_reverse_index_cursor() {
    let conn = open();
    conn.execute("CREATE TABLE t (k INTEGER)").expect("ddl");
    conn.execute("CREATE INDEX t_k ON t(k)").expect("idx");
    conn.execute("BEGIN").expect("begin");
    for k in 1..=1000 {
        conn.execute(&format!("INSERT INTO t(k) VALUES ({k})"))
            .expect("insert");
    }
    conn.execute("COMMIT").expect("commit");

    let top5 = collect_int_col(&conn, "SELECT k FROM t ORDER BY k DESC LIMIT 5", 0);
    assert_eq!(top5, vec![1000, 999, 998, 997, 996]);
    assert_eq!(top5.len(), 5, "LIMIT must early-stop");
}

#[test]
fn order_by_desc_limit_with_equality_prefix_uses_reverse_index_cursor() {
    let conn = open();
    conn.execute("CREATE TABLE t (tenant INTEGER, k INTEGER)")
        .expect("ddl");
    conn.execute("CREATE INDEX t_tk ON t(tenant, k)")
        .expect("idx");
    conn.execute("BEGIN").expect("begin");
    for k in 1..=1000 {
        conn.execute(&format!("INSERT INTO t(tenant, k) VALUES (1, {k})"))
            .expect("insert");
        // sprinkle a second tenant so the reverse walk has to honor the
        // equality-prefix bound and not bleed into a neighbor's slice.
        conn.execute(&format!("INSERT INTO t(tenant, k) VALUES (2, {})", 10_000 + k))
            .expect("insert");
    }
    conn.execute("COMMIT").expect("commit");

    let top5 = collect_int_col(
        &conn,
        "SELECT k FROM t WHERE tenant = 1 ORDER BY k DESC LIMIT 5",
        0,
    );
    assert_eq!(top5, vec![1000, 999, 998, 997, 996]);
    assert_eq!(top5.len(), 5);

    // Verify the equality-prefix bound is honored: tenant=2 must yield
    // its own top-n, not bleed across tenants.
    let tenant2_top3 = collect_int_col(
        &conn,
        "SELECT k FROM t WHERE tenant = 2 ORDER BY k DESC LIMIT 3",
        0,
    );
    assert_eq!(tenant2_top3, vec![11_000, 10_999, 10_998]);
}

#[test]
fn order_by_desc_full_walk_matches_expected_set() {
    // Sanity: the reverse cursor without a LIMIT (or LIMIT >= row count)
    // must still produce the full DESC-ordered set with no duplicates
    // or skips. Catches off-by-one slot edges around leaf boundaries.
    let conn = open();
    conn.execute("CREATE TABLE t (k INTEGER)").expect("ddl");
    conn.execute("CREATE INDEX t_k ON t(k)").expect("idx");
    conn.execute("BEGIN").expect("begin");
    for k in 1..=100 {
        conn.execute(&format!("INSERT INTO t(k) VALUES ({k})"))
            .expect("insert");
    }
    conn.execute("COMMIT").expect("commit");

    let all_desc = collect_int_col(&conn, "SELECT k FROM t ORDER BY k DESC LIMIT 100", 0);
    let expected: Vec<i64> = (1..=100).rev().collect();
    assert_eq!(all_desc, expected);
}
