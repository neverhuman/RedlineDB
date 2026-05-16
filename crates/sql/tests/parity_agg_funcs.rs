//! SQLite aggregate function parity tests.
//!
//! Covers: group_concat, string_agg, total, json_group_array, json_group_object.

use std::sync::Arc;
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
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

fn q1(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    query_all(conn, sql)
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next())
        .unwrap_or(SqlValue::Null)
}

fn setup_words(conn: &Arc<Connection>) {
    conn.execute("CREATE TABLE words(w TEXT, grp INTEGER)").expect("create");
    conn.execute(
        "INSERT INTO words VALUES ('alpha', 1), ('beta', 1), ('gamma', 2), (NULL, 1), ('delta', 2)",
    )
    .expect("insert");
}

// ── group_concat ──────────────────────────────────────────────────────────────

#[test]
fn group_concat_basic_default_separator() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES ('a'), ('b'), ('c')").expect("insert");
    let v = q1(&c, "SELECT group_concat(v) FROM t");
    // group_concat order is implementation-defined without ORDER BY; check set equality
    let s = match v { SqlValue::Text(s) => s, other => panic!("expected text, got {other:?}") };
    let mut parts: Vec<&str> = s.split(',').collect();
    parts.sort_unstable();
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn group_concat_custom_separator() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES ('x'), ('y'), ('z')").expect("insert");
    let v = q1(&c, "SELECT group_concat(v, ' | ') FROM t");
    let s = match v { SqlValue::Text(s) => s, other => panic!("expected text, got {other:?}") };
    let mut parts: Vec<&str> = s.split(" | ").collect();
    parts.sort_unstable();
    assert_eq!(parts, vec!["x", "y", "z"]);
}

#[test]
fn group_concat_skips_nulls() {
    let (_d, c) = open();
    setup_words(&c);
    // grp=1 has: alpha, beta, NULL — NULL should be skipped
    let v = q1(&c, "SELECT group_concat(w) FROM words WHERE grp = 1");
    assert_eq!(v, SqlValue::Text(Arc::from("alpha,beta")));
}

#[test]
fn group_concat_all_null_returns_null() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES (NULL), (NULL)").expect("insert");
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
    let rows = query_all(
        &c,
        "SELECT grp, group_concat(w) FROM words WHERE w IS NOT NULL GROUP BY grp ORDER BY grp",
    );
    assert_eq!(rows.len(), 2, "expected 2 groups");
    assert_eq!(rows[0][0], SqlValue::Integer(1));
    let v1 = match &rows[0][1] {
        SqlValue::Text(s) => s.as_ref().to_owned(),
        v => panic!("expected text, got {v:?}"),
    };
    assert!(v1.contains("alpha") && v1.contains("beta"), "grp1: {v1}");
}

// ── string_agg (alias) ────────────────────────────────────────────────────────

#[test]
fn string_agg_alias_works() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES ('p'), ('q')").expect("insert");
    let v = q1(&c, "SELECT string_agg(v, '-') FROM t");
    let s = match v { SqlValue::Text(s) => s, other => panic!("expected text, got {other:?}") };
    let mut parts: Vec<&str> = s.split('-').collect();
    parts.sort_unstable();
    assert_eq!(parts, vec!["p", "q"]);
}

// ── total ─────────────────────────────────────────────────────────────────────

#[test]
fn total_basic_sum() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n REAL)").expect("create");
    c.execute("INSERT INTO t VALUES (1.0), (2.0), (3.0)").expect("insert");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(6.0));
}

#[test]
fn total_all_null_returns_zero_real() {
    // SQLite: total(X) returns 0.0 for all-NULL groups, unlike sum() which returns NULL.
    let (_d, c) = open();
    c.execute("CREATE TABLE t(n INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (NULL), (NULL)").expect("insert");
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
    c.execute("INSERT INTO t VALUES (10), (NULL), (5)").expect("insert");
    let v = q1(&c, "SELECT total(n) FROM t");
    assert_eq!(v, SqlValue::Real(15.0));
}

// ── json_group_array ──────────────────────────────────────────────────────────

#[test]
fn json_group_array_basic() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1), (2), (3)").expect("insert");
    let v = q1(&c, "SELECT json_group_array(v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    // Order is implementation-defined; check set equality via sorted comparison
    let mut arr: Vec<i64> = parsed.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    arr.sort_unstable();
    assert_eq!(arr, vec![1, 2, 3]);
}

#[test]
fn json_group_array_includes_nulls() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1), (NULL), (3)").expect("insert");
    let v = q1(&c, "SELECT json_group_array(v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    let mut nums: Vec<i64> = arr.iter().filter(|v| !v.is_null()).map(|v| v.as_i64().unwrap()).collect();
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
    c.execute("CREATE TABLE t(k TEXT, v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES ('a', 1), ('b', 2)").expect("insert");
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
    c.execute("CREATE TABLE t(k TEXT, v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES ('a', 1), (NULL, 99)").expect("insert");
    let v = q1(&c, "SELECT json_group_object(k, v) FROM t");
    let s = match v {
        SqlValue::Text(s) => s,
        other => panic!("expected text, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid json");
    assert!(parsed.get("null").is_none(), "should not have 'null' key");
    assert_eq!(parsed["a"], serde_json::json!(1));
}
