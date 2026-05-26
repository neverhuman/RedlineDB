//! WS-B7: JSON1 fast path through JSONB bytes.
//!
//! Verifies that the scalar JSON readers (`json_extract`, `json_type`,
//! `json_array_length`, `json_valid`) walk a `SqlValue::Blob` carrying the
//! kernel's JSONB preamble directly via the byte-level path bytecode,
//! producing values identical to the existing `serde_json` path for the
//! equivalent JSON TEXT input.

use std::sync::Arc;

use redlinedb_kernel::json::encode;
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use serde_json::json;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ws_b7.db");
    let db = Database::create(&path, DbOptions::default()).expect("create");
    let conn = db.connect();
    (dir, conn)
}

fn run_one(conn: &Arc<Connection>, sql: &str, blob: &[u8]) -> Vec<SqlValue> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    stmt.bind_blob(1, blob.to_vec()).expect("bind");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        for i in 0..ncols {
            out.push(stmt.column_value(i).expect("col").clone());
        }
    }
    out
}

fn run_text(conn: &Arc<Connection>, sql: &str, text: &str) -> Vec<SqlValue> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    stmt.bind_text(1, text).expect("bind");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        for i in 0..ncols {
            out.push(stmt.column_value(i).expect("col").clone());
        }
    }
    out
}

#[test]
fn json_extract_on_jsonb_blob_returns_scalar() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"a": 42, "b": "hi"}));
    let rows = run_one(&conn, "SELECT json_extract(?, '$.a')", &blob);
    assert_eq!(rows, vec![SqlValue::Integer(42)]);
}

#[test]
fn json_extract_on_jsonb_blob_returns_text() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"a": 42, "b": "hi"}));
    let rows = run_one(&conn, "SELECT json_extract(?, '$.b')", &blob);
    assert_eq!(rows, vec![SqlValue::Text(Arc::from("hi"))]);
}

#[test]
fn json_extract_nested_and_array() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"users": [{"name": "ada"}, {"name": "lin"}]}));
    let rows = run_one(&conn, "SELECT json_extract(?, '$.users[1].name')", &blob);
    assert_eq!(rows, vec![SqlValue::Text(Arc::from("lin"))]);
}

#[test]
fn json_extract_missing_path_returns_null() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"a": 1}));
    let rows = run_one(&conn, "SELECT json_extract(?, '$.missing')", &blob);
    assert_eq!(rows, vec![SqlValue::Null]);
}

#[test]
fn json_extract_blob_matches_text_form() {
    // Differential: the JSONB byte-walker must produce results identical to
    // the serde_json TEXT path for the same logical document/path pair.
    let (_dir, conn) = open();
    let doc = json!({"a": [1, 2, {"b": "x"}], "c": null, "d": true});
    let text = serde_json::to_string(&doc).unwrap();
    let blob = encode(&doc);

    for path in ["$.a[0]", "$.a[2].b", "$.c", "$.d", "$.missing"] {
        let sql = format!("SELECT json_extract(?, '{}')", path);
        let from_blob = run_one(&conn, &sql, &blob);
        let from_text = run_text(&conn, &sql, &text);
        assert_eq!(from_blob, from_text, "diverged for path {path}");
    }
}

#[test]
fn json_type_on_jsonb_blob() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"a": [1, 2, 3], "b": "x", "c": 3.14, "d": null}));
    assert_eq!(
        run_one(&conn, "SELECT json_type(?, '$.a')", &blob),
        vec![SqlValue::Text(Arc::from("array"))]
    );
    assert_eq!(
        run_one(&conn, "SELECT json_type(?, '$.b')", &blob),
        vec![SqlValue::Text(Arc::from("text"))]
    );
    assert_eq!(
        run_one(&conn, "SELECT json_type(?, '$.c')", &blob),
        vec![SqlValue::Text(Arc::from("real"))]
    );
    assert_eq!(
        run_one(&conn, "SELECT json_type(?, '$.d')", &blob),
        vec![SqlValue::Text(Arc::from("null"))]
    );
    assert_eq!(
        run_one(&conn, "SELECT json_type(?)", &blob),
        vec![SqlValue::Text(Arc::from("object"))]
    );
}

#[test]
fn json_array_length_on_jsonb_blob() {
    let (_dir, conn) = open();
    let blob = encode(&json!({"a": [10, 20, 30, 40]}));
    assert_eq!(
        run_one(&conn, "SELECT json_array_length(?, '$.a')", &blob),
        vec![SqlValue::Integer(4)]
    );
    // Top-level array.
    let top = encode(&json!([1, 2, 3]));
    assert_eq!(
        run_one(&conn, "SELECT json_array_length(?)", &top),
        vec![SqlValue::Integer(3)]
    );
    // Non-array target returns 0 (matches SQLite + the serde_json path).
    assert_eq!(
        run_one(&conn, "SELECT json_array_length(?, '$.a[0]')", &blob),
        vec![SqlValue::Integer(0)]
    );
}

#[test]
fn json_valid_on_jsonb_blob() {
    let (_dir, conn) = open();
    let good = encode(&json!({"a": 1}));
    assert_eq!(
        run_one(&conn, "SELECT json_valid(?)", &good),
        vec![SqlValue::Integer(1)]
    );
    // Malformed: preamble looks like JSONB but the node is truncated.
    let bad = vec![redlinedb_kernel::json::MAGIC, redlinedb_kernel::json::FORMAT_VERSION, 0x07, 0x99];
    assert_eq!(
        run_one(&conn, "SELECT json_valid(?)", &bad),
        vec![SqlValue::Integer(0)]
    );
}

#[test]
fn columnar_blob_round_trip_extracts_correctly() {
    // End-to-end through the storage layer: insert a JSONB blob, select it
    // back through json_extract.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(c BLOB)").expect("create");
    let blob = encode(&json!({"score": 99, "tags": ["a", "b"]}));
    let mut ins = conn
        .prepare("INSERT INTO t(c) VALUES (?)")
        .expect("prepare insert");
    ins.bind_blob(1, blob.clone()).expect("bind");
    while let Step::Row = ins.step().expect("step") {}

    let mut q = conn
        .prepare("SELECT json_extract(c, '$.score'), json_extract(c, '$.tags[1]') FROM t")
        .expect("prepare select");
    let mut got: Vec<Vec<SqlValue>> = Vec::new();
    while let Step::Row = q.step().expect("step") {
        let row: Vec<SqlValue> = (0..q.column_count())
            .map(|i| q.column_value(i).expect("col").clone())
            .collect();
        got.push(row);
    }
    assert_eq!(
        got,
        vec![vec![
            SqlValue::Integer(99),
            SqlValue::Text(Arc::from("b"))
        ]]
    );
}
