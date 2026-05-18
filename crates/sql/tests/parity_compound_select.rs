//! Compound-SELECT parity (Workstream A2): UNION / UNION ALL /
//! INTERSECT / EXCEPT against the rusqlite oracle.
//!
//! Uses the A1 harness from `tests/parity_oracle/harness.rs` for the
//! happy-path tests, and hand-asserts the error-class cases (type / arity
//! mismatch) directly so the diagnostic surface stays close to the parser
//! / executor that emits them.

#[path = "parity_oracle/harness.rs"]
mod harness;

use redlinedb_sql::{Database, DbOptions};

const SETUP: &str = "
    CREATE TABLE a(v INTEGER);
    CREATE TABLE b(v INTEGER);
    INSERT INTO a VALUES (1), (2), (3);
    INSERT INTO b VALUES (3), (4), (5);
";

#[test]
fn union_all_keeps_duplicates() {
    harness::assert_parity(&format!(
        "{SETUP} SELECT v FROM a UNION ALL SELECT v FROM b ORDER BY v"
    ));
}

#[test]
fn union_distinct_dedups() {
    harness::assert_parity(&format!(
        "{SETUP} SELECT v FROM a UNION SELECT v FROM b ORDER BY v"
    ));
}

#[test]
fn intersect_returns_common_rows() {
    let sql = "
        CREATE TABLE a(v INTEGER);
        CREATE TABLE b(v INTEGER);
        INSERT INTO a VALUES (1), (2), (3);
        INSERT INTO b VALUES (2), (3), (4);
        SELECT v FROM a INTERSECT SELECT v FROM b ORDER BY v";
    harness::assert_parity(sql);
}

#[test]
fn except_returns_left_only_rows() {
    let sql = "
        CREATE TABLE a(v INTEGER);
        CREATE TABLE b(v INTEGER);
        INSERT INTO a VALUES (1), (2), (3);
        INSERT INTO b VALUES (2);
        SELECT v FROM a EXCEPT SELECT v FROM b ORDER BY v";
    harness::assert_parity(sql);
}

#[test]
fn union_all_order_by_position_matches_fuzz_iter_110() {
    // Regression for target/proof/sqlite-full-parity/fuzz-divergence.txt
    // seed=7 iter=110: ORDER BY 1 after UNION ALL was a no-op because the
    // bare integer literal evaluated to the constant `Integer(1)` per row
    // instead of being treated as a 1-based output-column reference.
    let sql = "
        CREATE TABLE t1(id INTEGER, x INTEGER);
        CREATE TABLE t2(id INTEGER, price INTEGER);
        INSERT INTO t1 VALUES (1, 50), (2, 40), (3, 30);
        INSERT INTO t2 VALUES (4, 1), (5, 2), (6, 7);
        SELECT id FROM t1 WHERE x > 6 UNION ALL SELECT id FROM t2 WHERE price < 6 ORDER BY 1";
    harness::assert_parity(sql);
}

#[test]
fn three_way_union_all_concatenates() {
    let sql = "
        CREATE TABLE a(v INTEGER);
        CREATE TABLE b(v INTEGER);
        CREATE TABLE c(v INTEGER);
        INSERT INTO a VALUES (1);
        INSERT INTO b VALUES (2);
        INSERT INTO c VALUES (3);
        SELECT v FROM a UNION ALL SELECT v FROM b UNION ALL SELECT v FROM c ORDER BY v";
    harness::assert_parity(sql);
}

#[test]
fn column_count_mismatch_errors() {
    // Both engines must reject this. We use the harness so the error class
    // (Generic / SyntaxError) must agree.
    let sql = "
        CREATE TABLE a(x INTEGER, y INTEGER);
        CREATE TABLE b(x INTEGER);
        INSERT INTO a VALUES (1, 10);
        INSERT INTO b VALUES (2);
        SELECT x, y FROM a UNION SELECT x FROM b";
    let oracle = harness::run_oracle(sql);
    let redline = harness::run_redline(sql);
    assert!(
        oracle.err_class.is_some(),
        "rusqlite must reject arity mismatch, got rows: {:?}",
        oracle.rows
    );
    assert!(
        redline.err_class.is_some(),
        "redline must reject arity mismatch, got rows: {:?}",
        redline.rows
    );
}

#[test]
fn redline_handles_compound_via_database_handle() {
    // End-to-end smoke against `Database::create` so we know the path
    // through the public API (not just the parity harness) works.
    let dir = tempfile::tempdir().expect("tempdir");
    let db =
        Database::create(dir.path().join("compound.db"), DbOptions::default()).expect("create db");
    let conn = db.connect();
    conn.execute("CREATE TABLE x(a INTEGER); INSERT INTO x VALUES (1),(2),(3);")
        .expect("setup");
    conn.execute("CREATE TABLE y(a INTEGER); INSERT INTO y VALUES (2),(3),(4);")
        .expect("setup");

    let mut stmt = conn
        .prepare("SELECT a FROM x INTERSECT SELECT a FROM y ORDER BY a")
        .expect("prepare intersect");
    let mut rows = Vec::new();
    while let redlinedb_sql::Step::Row = stmt.step().expect("step") {
        rows.push(stmt.column_value(0).expect("col").clone());
    }
    assert_eq!(rows.len(), 2, "expected two rows of intersection");
}
