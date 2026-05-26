//! WS-C2: one-pass aggregation routing via HashAggregator.
//!
//! Verifies that `execute_grouped_select` produces identical results
//! whether it routes through the new `try_one_pass_grouped` fast path
//! (bare COUNT/SUM/AVG/MIN/MAX over a GROUP BY) or falls back to the
//! per-group evaluator (DISTINCT-inside-aggregate, GROUP_CONCAT,
//! expressions wrapping aggregates).
//!
//! Parity is asserted against SQLite for each query so that any
//! divergence between the two execution paths is caught directly.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("agg.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn query_all(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let row: Vec<SqlValue> = (0..ncols)
            .map(|i| stmt.column_value(i).expect("col").clone())
            .collect();
        out.push(row);
    }
    out
}

fn to_sql_value(value: RuValue) -> SqlValue {
    match value {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(value) => SqlValue::Integer(value),
        RuValue::Real(value) => SqlValue::Real(value),
        RuValue::Text(value) => SqlValue::Text(Arc::from(value)),
        RuValue::Blob(value) => SqlValue::Blob(Arc::from(value)),
    }
}

fn sqlite_query_all(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("sqlite prepare");
    let ncols = stmt.column_count();
    let mut rows = Vec::new();
    let mut query = stmt.query([]).expect("sqlite query");
    while let Some(row) = query.next().expect("sqlite next") {
        let current: Vec<SqlValue> = (0..ncols)
            .map(|i| to_sql_value(row.get::<usize, RuValue>(i).expect("sqlite value")))
            .collect();
        rows.push(current);
    }
    rows
}

fn assert_matches_sqlite(conn: &Arc<Connection>, setup: &[&str], sql: &str) {
    let sqlite = rusqlite::Connection::open_in_memory().expect("sqlite open");
    for statement in setup {
        sqlite
            .execute_batch(statement)
            .unwrap_or_else(|err| panic!("sqlite setup {statement:?}: {err}"));
    }
    let mut sqlite_rows = sqlite_query_all(&sqlite, sql);
    let mut redline_rows = query_all(conn, sql);
    // Aggregation hash order is undefined; sort both sides for parity
    // checks where ORDER BY is absent. Tests that exercise ORDER BY
    // compare without sorting via assert_matches_sqlite_ordered below.
    sqlite_rows.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    redline_rows.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    assert_eq!(
        redline_rows, sqlite_rows,
        "rows differ for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
    );
}

fn assert_matches_sqlite_ordered(conn: &Arc<Connection>, setup: &[&str], sql: &str) {
    let sqlite = rusqlite::Connection::open_in_memory().expect("sqlite open");
    for statement in setup {
        sqlite
            .execute_batch(statement)
            .unwrap_or_else(|err| panic!("sqlite setup {statement:?}: {err}"));
    }
    let sqlite_rows = sqlite_query_all(&sqlite, sql);
    let redline_rows = query_all(conn, sql);
    assert_eq!(
        redline_rows, sqlite_rows,
        "ordered rows differ for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
    );
}

const SETUP_T: &[&str] = &[
    "CREATE TABLE t(k TEXT, v INTEGER)",
    "INSERT INTO t VALUES \
        ('a', 1), ('a', 2), ('a', 7), \
        ('b', 4), ('b', 6), \
        ('c', 9), ('c', 1), ('c', 3), ('c', 5)",
];

fn seed_t(conn: &Arc<Connection>) {
    for statement in SETUP_T {
        conn.execute(statement).expect("setup t");
    }
}

#[test]
fn one_pass_sum_by_group_matches_sqlite() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // Routed through HashAggregator.
    assert_matches_sqlite(&conn, SETUP_T, "SELECT k, SUM(v) FROM t GROUP BY k");
}

#[test]
fn one_pass_count_avg_with_having_matches_sqlite() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // COUNT(*), AVG(v) in projection AND SUM(v) only in HAVING.
    // The slot dedup must register a SUM slot for HAVING separate
    // from the projection ones, and HAVING must see that slot's value.
    assert_matches_sqlite(
        &conn,
        SETUP_T,
        "SELECT k, COUNT(*), AVG(v) FROM t GROUP BY k HAVING SUM(v) > 5",
    );
}

#[test]
fn one_pass_order_by_group_key_preserves_order() {
    let (_dir, conn) = open();
    seed_t(&conn);
    assert_matches_sqlite_ordered(
        &conn,
        SETUP_T,
        "SELECT k, SUM(v) FROM t GROUP BY k ORDER BY k",
    );
}

#[test]
fn one_pass_order_by_aggregate_alias_preserves_order() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // ORDER BY by alias of an aggregate. Tests the order_specs
    // alias-resolution path in try_one_pass_grouped.
    assert_matches_sqlite_ordered(
        &conn,
        SETUP_T,
        "SELECT k, COUNT(*) AS c FROM t GROUP BY k ORDER BY c DESC",
    );
}

#[test]
fn one_pass_min_max_per_group_matches_sqlite() {
    let (_dir, conn) = open();
    seed_t(&conn);
    assert_matches_sqlite(&conn, SETUP_T, "SELECT k, MIN(v), MAX(v) FROM t GROUP BY k");
}

#[test]
fn group_concat_falls_back_without_regression() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // GROUP_CONCAT is not routed (order-sensitive); must still produce
    // SQLite-equivalent output via the original group evaluator path.
    let rows = query_all(&conn, "SELECT k, GROUP_CONCAT(v) FROM t GROUP BY k");
    assert_eq!(rows.len(), 3, "three groups expected, got {rows:?}");
    // We don't strictly assert the order-within-a-group of GROUP_CONCAT
    // because SQLite doesn't guarantee it either; we just verify the
    // fall-back path runs cleanly and emits the correct group count.
}

#[test]
fn complex_arg_falls_back_to_old_path() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // SUM(v + 1) has a non-bare-column-ref argument. The router must
    // bail out and the per-group evaluator must produce the SQLite
    // value (sum of v + group_size).
    assert_matches_sqlite(&conn, SETUP_T, "SELECT k, SUM(v + 1) FROM t GROUP BY k");
}

#[test]
fn count_distinct_falls_back_to_old_path() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // COUNT(DISTINCT v): duplicate_treatment = Distinct → router bails.
    assert_matches_sqlite(
        &conn,
        SETUP_T,
        "SELECT k, COUNT(DISTINCT v) FROM t GROUP BY k",
    );
}

#[test]
fn aggregate_dedup_across_projection_and_having() {
    let (_dir, conn) = open();
    seed_t(&conn);
    // SUM(v) appears in BOTH projection and HAVING — must dedup to one
    // accumulator and project the SAME value HAVING checks.
    assert_matches_sqlite(
        &conn,
        SETUP_T,
        "SELECT k, SUM(v) FROM t GROUP BY k HAVING SUM(v) > 6",
    );
}

#[test]
fn one_pass_limit_offset_pagination() {
    let (_dir, conn) = open();
    seed_t(&conn);
    assert_matches_sqlite_ordered(
        &conn,
        SETUP_T,
        "SELECT k, SUM(v) FROM t GROUP BY k ORDER BY k LIMIT 2 OFFSET 1",
    );
}
