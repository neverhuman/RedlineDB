//! End-to-end SQL coverage for the phase-10 Lane V1 vector surface:
//!
//! * `VECTOR(d)` / `VECTOR(d, f32)` column types parse and persist.
//! * INSERT validates dimension via the auto-emitted CHECK constraint.
//! * `vector(...)`, `vector_dims(...)`, and `vector_distance_*` evaluate.
//! * `<=>` is overloaded to cosine distance when both operands are vectors,
//!   so `ORDER BY embedding <=> ? LIMIT k` returns ascending-distance rows.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open_database() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-vector.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

fn vector_blob_literal(values: &[f32]) -> String {
    // Build the same JSON-array literal that `vector('[...]')` consumes.
    let mut s = String::from("[");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&v.to_string());
    }
    s.push(']');
    s
}

#[test]
fn create_table_vector_basic_parses() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, embedding VECTOR(4))")
        .expect("create with VECTOR(4)");
    conn.execute("CREATE TABLE t2(id INTEGER PRIMARY KEY, embedding VECTOR(8, f32))")
        .expect("create with VECTOR(8, f32)");
}

#[test]
fn create_table_vector_zero_dim_rejected() {
    let (_dir, conn) = open_database();
    let err = conn
        .execute("CREATE TABLE t(id INTEGER, embedding VECTOR(0))")
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("VECTOR"), "got: {msg}");
}

#[test]
fn create_table_vector_unsupported_kind_rejected() {
    let (_dir, conn) = open_database();
    let err = conn
        .execute("CREATE TABLE t(id INTEGER, embedding VECTOR(4, i8))")
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("VECTOR"), "got: {msg}");
}

#[test]
fn insert_vector_with_correct_dim_succeeds() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(3))")
        .unwrap();
    let lit = vector_blob_literal(&[1.0, 2.0, 3.0]);
    let sql = format!("INSERT INTO t VALUES (1, vector('{lit}'))");
    conn.execute(&sql).expect("insert correct-dim vector");
}

#[test]
fn insert_vector_with_wrong_dim_rejected() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(3))")
        .unwrap();
    let lit = vector_blob_literal(&[1.0, 2.0, 3.0, 4.0]);
    let sql = format!("INSERT INTO t VALUES (1, vector('{lit}'))");
    let err = conn.execute(&sql).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_ascii_lowercase().contains("check")
            || msg.to_ascii_lowercase().contains("constraint"),
        "expected CHECK violation, got: {msg}"
    );
}

#[test]
fn insert_non_vector_into_vector_column_rejected() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(3))")
        .unwrap();
    // Plain integer is a clear type mismatch — the BlobLen sentinel ensures
    // this fails the auto-CHECK.
    let err = conn.execute("INSERT INTO t VALUES (1, 7)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_ascii_lowercase().contains("check")
            || msg.to_ascii_lowercase().contains("constraint"),
        "expected CHECK violation, got: {msg}"
    );
}

#[test]
fn insert_null_into_nullable_vector_column_succeeds() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(3))")
        .unwrap();
    conn.execute("INSERT INTO t VALUES (1, NULL)")
        .expect("Null is allowed unless NOT NULL");
}

#[test]
fn vector_dims_returns_dimension() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(4))")
        .unwrap();
    let lit = vector_blob_literal(&[1.0, 2.0, 3.0, 4.0]);
    conn.execute(&format!("INSERT INTO t VALUES (1, vector('{lit}'))"))
        .unwrap();
    let mut stmt = conn.prepare("SELECT vector_dims(e) FROM t").unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    assert_eq!(stmt.column_i64(0).unwrap(), 4);
    assert_eq!(stmt.step().unwrap(), Step::Done);
}

#[test]
fn vector_distance_l2_matches_expected() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, a VECTOR(3), b VECTOR(3))")
        .unwrap();
    let a_lit = vector_blob_literal(&[1.0, 2.0, 3.0]);
    let b_lit = vector_blob_literal(&[4.0, 6.0, 3.0]);
    conn.execute(&format!(
        "INSERT INTO t VALUES (1, vector('{a_lit}'), vector('{b_lit}'))"
    ))
    .unwrap();
    let mut stmt = conn
        .prepare("SELECT vector_distance_l2(a, b) FROM t")
        .unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    // Squared L2 = (1-4)^2 + (2-6)^2 + (3-3)^2 = 9 + 16 + 0 = 25.
    let d = stmt.column_f64(0).unwrap();
    assert!((d - 25.0).abs() < 1e-6, "d = {d}");
}

#[test]
fn vector_distance_cosine_zero_for_parallel() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, a VECTOR(3), b VECTOR(3))")
        .unwrap();
    let a_lit = vector_blob_literal(&[1.0, 2.0, 3.0]);
    let b_lit = vector_blob_literal(&[2.0, 4.0, 6.0]);
    conn.execute(&format!(
        "INSERT INTO t VALUES (1, vector('{a_lit}'), vector('{b_lit}'))"
    ))
    .unwrap();
    let mut stmt = conn
        .prepare("SELECT vector_distance_cosine(a, b) FROM t")
        .unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    let d = stmt.column_f64(0).unwrap();
    assert!(d.abs() < 1e-6, "d = {d}");
}

#[test]
fn vector_distance_ip_negates_dot_product() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, a VECTOR(3), b VECTOR(3))")
        .unwrap();
    let a_lit = vector_blob_literal(&[1.0, 2.0, 3.0]);
    let b_lit = vector_blob_literal(&[4.0, 5.0, 6.0]);
    conn.execute(&format!(
        "INSERT INTO t VALUES (1, vector('{a_lit}'), vector('{b_lit}'))"
    ))
    .unwrap();
    let mut stmt = conn
        .prepare("SELECT vector_distance_ip(a, b) FROM t")
        .unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    let d = stmt.column_f64(0).unwrap();
    // Dot = 1*4+2*5+3*6 = 32; metric returns -dot.
    assert!((d + 32.0).abs() < 1e-6, "d = {d}");
}

#[test]
fn order_by_spaceship_returns_ascending_cosine() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(2))")
        .unwrap();
    // Each row's ID encodes its expected rank under cosine distance from
    // [1,0]: parallel (1.0) → orthogonal (0,1) → opposite-ish (-1,0).
    let parallel = vector_blob_literal(&[1.0, 0.0]);
    let orthogonal = vector_blob_literal(&[0.0, 1.0]);
    let antiparallel = vector_blob_literal(&[-1.0, 0.0]);
    conn.execute(&format!(
        "INSERT INTO t VALUES (1, vector('{parallel}')), (2, vector('{orthogonal}')), (3, vector('{antiparallel}'))"
    )).unwrap();
    let query = vector_blob_literal(&[1.0, 0.0]);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id FROM t ORDER BY e <=> vector('{query}') LIMIT 3"
        ))
        .unwrap();
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        ids.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn order_by_l2_distance_returns_ascending() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(2))")
        .unwrap();
    let near = vector_blob_literal(&[0.1, 0.0]);
    let mid = vector_blob_literal(&[1.0, 0.0]);
    let far = vector_blob_literal(&[5.0, 0.0]);
    conn.execute(&format!(
        "INSERT INTO t VALUES (1, vector('{far}')), (2, vector('{near}')), (3, vector('{mid}'))"
    ))
    .unwrap();
    let query = vector_blob_literal(&[0.0, 0.0]);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id FROM t ORDER BY vector_distance_l2(e, vector('{query}')) LIMIT 3"
        ))
        .unwrap();
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        ids.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(ids, vec![2, 3, 1]);
}

#[test]
fn vector_dims_handles_null_input() {
    let (_dir, conn) = open_database();
    let mut stmt = conn.prepare("SELECT vector_dims(NULL)").unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    assert!(matches!(
        stmt.column_value(0).unwrap(),
        redlinedb_sql::SqlValue::Null
    ));
}

#[test]
fn vector_distance_on_null_returns_null() {
    let (_dir, conn) = open_database();
    let lit = vector_blob_literal(&[1.0, 2.0]);
    let mut stmt = conn
        .prepare(&format!("SELECT vector_distance_l2(vector('{lit}'), NULL)"))
        .unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    assert!(matches!(
        stmt.column_value(0).unwrap(),
        redlinedb_sql::SqlValue::Null
    ));
}

#[test]
fn malformed_vector_blob_rejected_at_construction() {
    let (_dir, conn) = open_database();
    let err = conn
        .prepare("SELECT vector('not-a-list')")
        .and_then(|mut stmt| {
            stmt.step()?;
            Ok(())
        })
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.to_ascii_lowercase().contains("vector"), "got: {msg}");
}

#[test]
fn spaceship_falls_back_to_compare_for_non_vector_blobs() {
    // Phase-10 Lane V1 only overloads `<=>` when *both* operands decode as
    // vectors. Non-vector blobs should keep the historical null-safe
    // inequality semantics so existing comparison tests continue to pass.
    let (_dir, conn) = open_database();
    let mut stmt = conn.prepare("SELECT 1 <=> 2").unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Row);
    assert_eq!(stmt.column_i64(0).unwrap(), 1);
}
