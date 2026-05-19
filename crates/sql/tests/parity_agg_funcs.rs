//! SQLite aggregate function parity tests.
//!
//! Covers: group_concat, string_agg, total, json_group_array, json_group_object.

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
            .unwrap_or_else(|err| panic!("sqlite setup failed for {statement:?}: {err}"));
    }
    let sqlite_rows = sqlite_query_all(&sqlite, sql);
    let redline_rows = query_all(conn, sql);
    assert_eq!(
        redline_rows, sqlite_rows,
        "rows differ for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
    );
}

fn q1(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    query_all(conn, sql)
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next())
        .unwrap_or(SqlValue::Null)
}

fn setup_words(conn: &Arc<Connection>) {
    for statement in SETUP_WORDS {
        conn.execute(statement).expect("setup words");
    }
}

const SETUP_WORDS: &[&str] = &[
    "CREATE TABLE words(w TEXT, grp INTEGER)",
    "INSERT INTO words VALUES ('alpha', 1), ('beta', 1), ('gamma', 2), (NULL, 1), ('delta', 2)",
];

// ── group_concat ──────────────────────────────────────────────────────────────

#[test]
fn group_concat_basic_default_separator() {
    let (_d, c) = open();
    let setup = [
        "CREATE TABLE t(v TEXT)",
        "INSERT INTO t VALUES ('b'), ('c'), ('a')",
    ];
    for statement in setup {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(&c, &setup, "SELECT group_concat(v ORDER BY v) FROM t");
}

#[test]
fn group_concat_custom_separator() {
    let (_d, c) = open();
    let setup = [
        "CREATE TABLE t(v TEXT)",
        "INSERT INTO t VALUES ('z'), ('x'), ('y')",
    ];
    for statement in setup {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(
        &c,
        &setup,
        "SELECT group_concat(v, ' | ' ORDER BY v) FROM t",
    );
}

#[test]
fn group_concat_skips_nulls() {
    let (_d, c) = open();
    setup_words(&c);
    assert_matches_sqlite(
        &c,
        SETUP_WORDS,
        "SELECT group_concat(w ORDER BY w) FROM words WHERE grp = 1",
    );
}

#[test]
fn group_concat_all_null_returns_null() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES (NULL), (NULL)")
        .expect("insert");
    let v = q1(&c, "SELECT group_concat(v) FROM t");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn group_concat_empty_table_returns_null() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    let v = q1(&c, "SELECT group_concat(v) FROM t");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn group_concat_with_group_by() {
    let (_d, c) = open();
    setup_words(&c);
    assert_matches_sqlite(
        &c,
        SETUP_WORDS,
        "SELECT grp, group_concat(w ORDER BY w) FROM words WHERE w IS NOT NULL GROUP BY grp ORDER BY grp",
    );
}

// ── string_agg (alias) ────────────────────────────────────────────────────────

#[test]
fn string_agg_alias_works() {
    let (_d, c) = open();
    let setup = [
        "CREATE TABLE t(v TEXT)",
        "INSERT INTO t VALUES ('q'), ('p')",
    ];
    for statement in setup {
        c.execute(statement).expect("setup");
    }
    assert_matches_sqlite(&c, &setup, "SELECT string_agg(v, '-' ORDER BY v) FROM t");
}

// ── total ─────────────────────────────────────────────────────────────────────

#[test]
fn total_basic_sum() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n REAL)").expect("create");
    c.execute("INSERT INTO t VALUES (1.0), (2.0), (3.0)")
        .expect("insert");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(6.0));
}

#[test]
fn total_all_null_returns_zero_real() {
    // SQLite: total(X) returns 0.0 for all-NULL groups, unlike sum() which returns NULL.
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (NULL), (NULL)")
        .expect("insert");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(0.0));
}

#[test]
fn total_empty_table_returns_zero_real() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n INTEGER)").expect("create");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(0.0));
}

#[test]
fn total_vs_sum_null_difference() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (NULL)").expect("insert");
    let sum_v = q1(&c, "SELECT sum(n) FROM t");
    let total_v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(sum_v, SqlValue::Null);
    assert_eq!(total_v, SqlValue::Real(0.0));
}

#[test]
fn total_skips_null_values() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (10), (NULL), (5)")
        .expect("insert");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(15.0));
}

// ── json_group_array ──────────────────────────────────────────────────────────

#[test]
fn json_group_array_basic() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1), (2), (3)")
        .expect("insert");
    let v = q1(&c, "SELECT json_group_array(v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    // Order is implementation-defined; check set equality via sorted comparison
    let mut arr: Vec<i64> = parsed
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    arr.sort_unstable();
    assert_eq!(arr, vec![1, 2, 3]);
}

#[test]
fn json_group_array_includes_nulls() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1), (NULL), (3)")
        .expect("insert");
    let v = q1(&c, "SELECT json_group_array(v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    let mut nums: Vec<i64> = arr
        .iter()
        .filter(|v| !v.is_null())
        .map(|v| v.as_i64().unwrap())
        .collect();
    nums.sort_unstable();
    assert_eq!(nums, vec![1, 3]);
    assert!(arr.iter().any(|v| v.is_null()), "null should be present");
}

#[test]
fn json_group_array_empty_table() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    let v = q1(&c, "SELECT json_group_array(v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    assert_eq!(parsed, serde_json::json!([]));
}

// ── json_group_object ─────────────────────────────────────────────────────────

#[test]
fn json_group_object_basic() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(k TEXT, v INTEGER)")
        .expect("create");
    c.execute("INSERT INTO t VALUES ('a', 1), ('b', 2)")
        .expect("insert");
    let v = q1(&c, "SELECT json_group_object(k, v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    assert_eq!(parsed["a"], serde_json::json!(1));
    assert_eq!(parsed["b"], serde_json::json!(2));
}

#[test]
fn json_group_object_skips_null_keys() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(k TEXT, v INTEGER)")
        .expect("create");
    c.execute("INSERT INTO t VALUES ('a', 1), (NULL, 99)")
        .expect("insert");
    let v = q1(&c, "SELECT json_group_object(k, v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    assert!(parsed.get("null").is_none(), "should not have 'null' key");
    assert_eq!(parsed["a"], serde_json::json!(1));
}
