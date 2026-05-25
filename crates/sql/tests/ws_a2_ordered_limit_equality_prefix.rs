//! Phase 5 WS-A2: ORDER BY satisfaction must recognize equality-pinned
//! leading index columns.
//!
//! Before this change, `SELECT k FROM kv WHERE tenant=? ORDER BY k LIMIT 1`
//! on `INDEX(tenant, k)` would NOT recognize that the cursor already
//! walks in k-order across the tenant slice, and would fall through to
//! a full sort. After WS-A2 the ordered-limit / covering fast path
//! fires and only walks until the first matching row.
//!
//! This test pins the semantic — same result either way — and the
//! correctness invariant: the rows returned must be sorted by k.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a2.db");
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
fn order_by_secondary_column_with_equality_leading_uses_index() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, tenant INTEGER, k INTEGER, v TEXT)")
        .expect("ddl");
    conn.execute("CREATE INDEX kv_tk ON kv(tenant, k)").expect("idx");
    // Two tenants, k values interleaved out-of-order
    for (tenant, k, v) in [
        (1, 30, "a"), (1, 10, "b"), (1, 20, "c"),
        (2, 50, "d"), (2, 40, "e"),
        (1, 5,  "f"),
    ] {
        conn.execute(&format!(
            "INSERT INTO kv(tenant, k, v) VALUES ({tenant}, {k}, '{v}')"
        ))
        .expect("insert");
    }

    // Tenant=1 has k in {5,10,20,30}. ORDER BY k LIMIT 1 must yield 5.
    let one = collect_int_col(
        &conn,
        "SELECT k FROM kv WHERE tenant = 1 ORDER BY k LIMIT 1",
        0,
    );
    assert_eq!(one, vec![5], "ORDER BY secondary index column over equality-pinned leading");

    // ORDER BY k LIMIT 3 → smallest 3 k values for tenant=1.
    let three = collect_int_col(
        &conn,
        "SELECT k FROM kv WHERE tenant = 1 ORDER BY k LIMIT 3",
        0,
    );
    assert_eq!(three, vec![5, 10, 20]);

    // Without LIMIT, the order must still be correct.
    let all = collect_int_col(
        &conn,
        "SELECT k FROM kv WHERE tenant = 1 ORDER BY k",
        0,
    );
    assert_eq!(all, vec![5, 10, 20, 30]);
}

#[test]
fn order_by_non_aligned_falls_back() {
    // Negative case: ORDER BY a column that does NOT align with the
    // remaining index keys after the equality prefix must still
    // produce correct results (via the sort fallback).
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, tenant INTEGER, k INTEGER, v INTEGER)")
        .expect("ddl");
    conn.execute("CREATE INDEX kv_tk ON kv(tenant, k)").expect("idx");
    for (tenant, k, v) in [(1, 10, 99), (1, 20, 50), (1, 30, 75)] {
        conn.execute(&format!(
            "INSERT INTO kv(tenant, k, v) VALUES ({tenant}, {k}, {v})"
        ))
        .expect("insert");
    }
    // ORDER BY v is NOT satisfiable by INDEX(tenant, k) — must sort.
    // Result must still be correct: v ascending => 50, 75, 99.
    let by_v = collect_int_col(
        &conn,
        "SELECT v FROM kv WHERE tenant = 1 ORDER BY v",
        0,
    );
    assert_eq!(by_v, vec![50, 75, 99]);
}
