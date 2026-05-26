//! Phase 5 WS-A1: index fast paths must NOT silently ignore residual
//! WHERE conjuncts.
//!
//! Before this gate, `SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ? AND ?
//! AND status = 'active'` would route through the index-count fast path
//! using only the BETWEEN range, dropping `status='active'` entirely and
//! returning the count of EVERY row in the tenant range.
//!
//! These tests pin the correctness gate so future fast-path additions
//! cannot regress the invariant.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a1.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    // Keep the TempDir alive by leaking it for the test's duration;
    // the db file goes away when the process exits.
    Box::leak(Box::new(dir));
    conn
}

fn one_int(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let v = stmt.column_value(0).expect("col").clone();
    match v {
        SqlValue::Integer(n) => n,
        other => panic!("expected Integer, got {other:?}"),
    }
}

#[test]
fn count_index_range_does_not_ignore_residual_predicate() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, tenant INTEGER, status TEXT)")
        .expect("ddl");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("idx");
    // 10 rows: tenant 1..=10, status alternating active/idle.
    for i in 1..=10 {
        let status = if i % 2 == 0 { "active" } else { "idle" };
        conn.execute(&format!(
            "INSERT INTO kv(tenant, status) VALUES ({i}, '{status}')"
        ))
        .expect("insert");
    }

    // Sanity: tenants in [3,7] are {3,4,5,6,7}; even => 'active' => {4,6} = 2.
    // Use range comparison form (>= and <=) — this shape goes through
    // the planner WITHOUT folding into the index-count fast path,
    // so it gives us the predicate-correct baseline.
    let baseline = one_int(
        &conn,
        "SELECT COUNT(*) FROM kv WHERE tenant >= 3 AND tenant <= 7 AND status = 'active'",
    );
    assert_eq!(baseline, 2, "tenants 4,6 are active in [3,7] range");

    // Now the index-fast-path query (BETWEEN + residual status). The
    // fast path used to drop the residual and return 5 (all in
    // tenant range). With the gate it must fall back to scan and
    // return the same 2 as the baseline.
    let fast = one_int(
        &conn,
        "SELECT COUNT(*) FROM kv WHERE tenant BETWEEN 3 AND 7 AND status = 'active'",
    );
    assert_eq!(
        fast, 2,
        "index-count fast path must not drop residual predicate `status='active'`"
    );
}

#[test]
fn covering_scan_does_not_ignore_residual_predicate() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, tenant INTEGER, status TEXT)")
        .expect("ddl");
    // Covering index on (tenant, status) so the covering fast path
    // can fire on SELECT tenant FROM kv WHERE tenant = ? AND ...
    conn.execute("CREATE INDEX kv_ts ON kv(tenant, status)")
        .expect("idx");
    for i in 1..=5 {
        conn.execute(&format!(
            "INSERT INTO kv(tenant, status) VALUES (1, 'active')"
        ))
        .expect("insert row");
        let _ = i;
    }
    // 5 rows all (tenant=1, status='active'). Plus 5 (tenant=1, status='idle').
    for _ in 0..5 {
        conn.execute("INSERT INTO kv(tenant, status) VALUES (1, 'idle')")
            .expect("insert idle");
    }

    // The 1-arg projection of `tenant` is covering for INDEX(tenant, status).
    // WHERE tenant = 1 is the index probe; status = 'active' is residual.
    // Before WS-A1, covering fast path would emit ALL 10 rows (ignoring
    // status). After WS-A1, must fall back and return only 5.
    let mut stmt = conn
        .prepare("SELECT tenant FROM kv WHERE tenant = 1 AND status = 'active'")
        .expect("prepare");
    let mut rows = 0;
    while let Step::Row = stmt.step().expect("step") {
        rows += 1;
    }
    assert_eq!(
        rows, 5,
        "covering fast path must not drop residual predicate `status='active'`"
    );
}
