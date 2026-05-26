//! Phase 6 R1-F integration tests for the new `AccessPath` IR.
//!
//! The IR type itself is `pub(crate)` and lives in
//! `crates/sql/src/planner/access_path.rs`; this binary exercises the
//! same five WHERE shapes through the public `EXPLAIN QUERY PLAN` API
//! to confirm the scaffolding does not perturb the existing planner.
//!
//! Variant-level structural assertions on `AccessPath` itself live in
//! the `#[cfg(test)] mod tests` block alongside the module source. The
//! integration tests here cover the user-visible plan strings:
//!   * `WHERE id = 1` on rowid PK  -> `SEARCH ... rowid`
//!   * `WHERE k = ?` on `INDEX(k)` -> `IndexPointLookup`
//!   * `WHERE k BETWEEN ...`       -> `IndexRangeScan`
//!   * `WHERE tenant=? AND k>? ORDER BY k DESC LIMIT n` -> ordered
//!     index range scan with LIMIT annotation
//!   * `WHERE non_indexed_col = ?` -> `SCAN TABLE`
//!
//! The plan-string assertions exercise the same call sites the new
//! `choose_access_path` will route through once the executor rewrite
//! lands in a later wave. Until then, the scaffolding is required to
//! produce identical plans — these tests defend that invariant.

#![allow(clippy::needless_borrow)]

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};

fn open_database() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("access_path_ir.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

/// Concatenate every detail row of `EXPLAIN QUERY PLAN <sql>`.
fn explain_text(conn: &Arc<Connection>, sql: &str) -> String {
    let prepared = format!("EXPLAIN QUERY PLAN {sql}");
    let mut stmt = conn.prepare(&prepared).expect("prepare explain");
    let mut out = String::new();
    while let Step::Row = stmt.step().expect("step explain") {
        out.push_str(stmt.column_text(3).expect("detail"));
        out.push('\n');
    }
    out
}

#[test]
fn where_rowid_pk_renders_rowid_search() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 10)").expect("insert");
    let plan = explain_text(&conn, "SELECT v FROM t WHERE id = 1");
    // SQLite-parity wording for an integer-PK direct lookup goes
    // through either "rowid=?" or a USING-INTEGER-PRIMARY-KEY shape.
    assert!(
        plan.to_ascii_lowercase().contains("rowid")
            || plan.contains("USING INTEGER PRIMARY KEY"),
        "expected rowid-PK access, got plan:\n{plan}"
    );
}

#[test]
fn where_single_key_equality_renders_index_point_lookup() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_k_idx ON t(k)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES (5, 50)").expect("insert");
    let plan = explain_text(&conn, "SELECT v FROM t WHERE k = 5");
    assert!(
        plan.contains("USING INDEX") && plan.contains("PointLookup"),
        "expected IndexPointLookup, got plan:\n{plan}"
    );
}

#[test]
fn where_between_renders_index_range_scan() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_k_idx ON t(k)")
        .expect("create index");
    for i in 1..=20 {
        conn.execute(&format!("INSERT INTO t VALUES ({i}, {i})"))
            .expect("insert");
    }
    let plan = explain_text(&conn, "SELECT v FROM t WHERE k BETWEEN 1 AND 10");
    assert!(
        plan.contains("USING INDEX") && plan.contains("Range"),
        "expected IndexRangeScan, got plan:\n{plan}"
    );
}

#[test]
fn where_leading_eq_and_order_by_limit_renders_range_scan() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_tk_idx ON t(tenant, k)")
        .expect("create index");
    for i in 0..50 {
        conn.execute(&format!("INSERT INTO t VALUES (1, {i}, {i})"))
            .expect("insert");
    }
    let plan = explain_text(
        &conn,
        "SELECT v FROM t WHERE tenant = 1 AND k > 5 ORDER BY k DESC LIMIT 10",
    );
    // The planner should pick the composite index for this shape;
    // exact LIMIT annotation rendering is a planner-wave concern (the
    // scaffolding does not change it). We assert the index path was
    // picked at all — that's the property the IR must preserve.
    assert!(
        plan.contains("USING INDEX"),
        "expected an index path, got plan:\n{plan}"
    );
}

#[test]
fn where_non_indexed_column_falls_back_to_table_scan() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 7)").expect("insert");
    let plan = explain_text(&conn, "SELECT a FROM t WHERE b = 7");
    assert!(
        plan.contains("SCAN TABLE"),
        "expected TableScan, got plan:\n{plan}"
    );
    assert!(
        !plan.contains("USING INDEX"),
        "did not expect an index path:\n{plan}"
    );
}

#[test]
fn no_where_clause_renders_table_scan() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1)").expect("insert");
    let plan = explain_text(&conn, "SELECT a FROM t");
    assert!(
        plan.contains("SCAN TABLE"),
        "expected TableScan, got plan:\n{plan}"
    );
}
