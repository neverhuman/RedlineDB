//! Phase-10 smoke additions split out of `sql_smoke.rs` to keep that
//! file under the 2000-LOC active-source cap. These exercise the new
//! `redline_index_check` / `redline_full_check` PRAGMAs (Lane INT),
//! the SQLite JSON1 surface (Lane J1), and the VECTOR/`<=>` surface
//! (Lane V1).

use std::sync::Arc;

use redlinedb_sql::{Database, DbOptions, Step};
use tempfile::tempdir;

fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-sql-smoke-extras.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

#[test]
fn pragma_redline_index_check_matches_engine_api() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES (1, 'one'), (2, 'two')")
        .expect("insert");

    let mut stmt = conn
        .prepare("PRAGMA redline_index_check")
        .expect("prepare pragma");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((
            stmt.column_text(0).expect("index name").to_owned(),
            stmt.column_text(1).expect("status").to_owned(),
        ));
    }
    assert!(
        rows.iter()
            .any(|(name, status)| name == "t_b_idx" && status == "ok"),
        "expected ix t_b_idx ok in {rows:?}"
    );
    for (_, status) in &rows {
        assert_eq!(status, "ok", "all indexes should report ok");
    }
}

#[test]
fn pragma_redline_full_check_returns_per_relation_rows() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES (1, 'one'), (2, 'two'), (3, 'three')")
        .expect("insert");

    let mut stmt = conn
        .prepare("PRAGMA redline_full_check")
        .expect("prepare pragma");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((
            stmt.column_text(0).expect("relation").to_owned(),
            stmt.column_text(1).expect("status").to_owned(),
            stmt.column_i64(2).expect("heap rows"),
            stmt.column_i64(3).expect("index entries"),
            stmt.column_i64(4).expect("heap minus index"),
            stmt.column_i64(5).expect("index minus heap"),
            stmt.column_i64(6).expect("page csum failures"),
            stmt.column_i64(7).expect("lsn violations"),
        ));
    }
    let t_row = rows
        .iter()
        .find(|row| row.0 == "t")
        .expect("t row in full check");
    assert_eq!(t_row.1, "ok");
    assert_eq!(t_row.2, 3, "heap row count");
    assert_eq!(t_row.4, 0, "heap minus index");
    assert_eq!(t_row.5, 0, "index minus heap");
    assert_eq!(t_row.6, 0, "no page csum failures");
    assert_eq!(t_row.7, 0, "no lsn monotonicity violations");
}

#[test]
fn json_text_functions_are_sqlite_compatible_for_core_paths() {
    let (_dir, conn) = open_database();
    let mut stmt = conn
        .prepare(
            "SELECT \
             json_valid('{\"a\":[1,2],\"b\":true}'), \
             json_extract('{\"a\":[1,2],\"b\":true}', '$.a[1]'), \
             json_type('{\"a\":[1,2],\"b\":true}', '$.b'), \
             json_array(1, 'two', NULL), \
             json_object('k', 7), \
             json_quote('x')",
        )
        .expect("prepare json");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("valid"), 1);
    assert_eq!(stmt.column_i64(1).expect("extract"), 2);
    assert_eq!(stmt.column_text(2).expect("type"), "true");
    assert_eq!(stmt.column_text(3).expect("array"), "[1,\"two\",null]");
    assert_eq!(stmt.column_text(4).expect("object"), "{\"k\":7}");
    assert_eq!(stmt.column_text(5).expect("quote"), "\"x\"");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn vector_blob_functions_encode_and_compare_vectors() {
    let (_dir, conn) = open_database();
    let mut stmt = conn
        .prepare(
            "SELECT \
             vector_dims(vector('[1.0, 2.0, 3.0]')), \
             vector_distance_l2(vector('[1, 2]'), vector('[4, 6]')), \
             vector_distance_cosine(vector_from_json('[1,0]'), vector('[0,1]'))",
        )
        .expect("prepare vector");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("dims"), 3);
    assert_eq!(stmt.column_f64(1).expect("l2"), 25.0);
    assert!((stmt.column_f64(2).expect("cosine") - 1.0).abs() < 0.000001);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}
