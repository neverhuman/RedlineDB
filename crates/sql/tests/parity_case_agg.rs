//! SQLite parity coverage for CASE expressions containing aggregates.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("case_agg.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
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

fn query_all(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let ncols = stmt.column_count();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let row = (0..ncols)
            .map(|i| stmt.column_value(i).expect("col").clone())
            .collect();
        rows.push(row);
    }
    rows
}

fn sqlite_query_all(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("sqlite prepare");
    let ncols = stmt.column_count();
    let mut rows = Vec::new();
    let mut query = stmt.query([]).expect("sqlite query");
    while let Some(row) = query.next().expect("sqlite next") {
        let current = (0..ncols)
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
            .unwrap_or_else(|err| panic!("sqlite setup failed for {statement:?}: {err}"));
    }
    let sqlite_rows = sqlite_query_all(&sqlite, sql);
    let redline_rows = query_all(conn, sql);
    assert_eq!(
        redline_rows, sqlite_rows,
        "rows differ for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
    );
}

const SETUP: &[&str] = &[
    "CREATE TABLE t(id INTEGER, grp TEXT, v INTEGER)",
    "INSERT INTO t VALUES (1, 'a', 1), (2, 'a', NULL), (3, 'a', 2), (4, 'b', NULL), (5, 'b', NULL)",
];

const NO_SETUP: &[&str] = &[];

#[test]
fn searched_case_with_count_star() {
    let (_d, c) = open();
    for statement in SETUP {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        SETUP,
        "SELECT CASE WHEN count(*) > 4 THEN 'many' ELSE 'few' END FROM t",
    );
}

#[test]
fn grouped_case_with_sum_and_avg() {
    let (_d, c) = open();
    for statement in SETUP {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        SETUP,
        "SELECT grp, CASE WHEN sum(v) IS NULL THEN 'all-null' WHEN avg(v) >= 1.5 THEN 'high' ELSE 'low' END FROM t GROUP BY grp ORDER BY grp",
    );
}

#[test]
fn simple_case_with_aggregate_operand() {
    let (_d, c) = open();
    for statement in SETUP {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        SETUP,
        "SELECT CASE count(*) WHEN 0 THEN 'empty' WHEN 5 THEN 'five' ELSE 'other' END FROM t",
    );
}

#[test]
fn repeated_aggregate_calls_in_case() {
    let (_d, c) = open();
    for statement in SETUP {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        SETUP,
        "SELECT CASE WHEN count(*) = 5 THEN count(*) WHEN count(*) = 6 THEN count(*) + 10 ELSE count(*) + 100 END FROM t",
    );
}

#[test]
fn null_sensitive_case_branch_matches_sqlite() {
    let (_d, c) = open();
    for statement in SETUP {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        SETUP,
        "SELECT CASE sum(v) WHEN NULL THEN 'matched-null' ELSE 'fallback' END FROM t WHERE grp = 'b'",
    );
}

#[test]
fn simple_case_null_operand_uses_else_branch_like_sqlite() {
    let (_d, c) = open();
    assert_matches_sqlite(
        &c,
        NO_SETUP,
        "SELECT CASE NULL WHEN 1 THEN 'one' WHEN 2 THEN 'two' ELSE 'fallback' END",
    );
}

#[test]
fn searched_case_skips_null_conditions_like_sqlite() {
    let (_d, c) = open();
    assert_matches_sqlite(
        &c,
        NO_SETUP,
        "SELECT CASE WHEN NULL THEN 'nope' WHEN 0 THEN 'also-nope' WHEN 1 THEN 'hit' ELSE 'fallback' END",
    );
}
