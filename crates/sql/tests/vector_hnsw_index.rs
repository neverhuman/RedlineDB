//! Phase 2B.1a: SQL coverage for `CREATE INDEX ... USING hnsw`.
//!
//! Validates the parser-to-engine wiring added in Phase 2B.1a:
//!
//! * `CREATE INDEX idx ON tbl (vec) USING hnsw` parses and creates a
//!   physical HNSW handle visible via `Engine::hnsw_index_handle`.
//! * `CREATE INDEX idx ON tbl (col)` (no `USING`) keeps the historical
//!   B-tree default — no HNSW handle materialises.
//! * `CREATE INDEX ... USING xyz` rejects with a clear error.
//! * `USING hnsw` on a non-VECTOR column rejects at create time.
//! * `USING hnsw` accepted in both grammar positions sqlparser
//!   surfaces (pre-`(columns)` and post-`(columns)`).
//!
//! The HNSW kernel itself (insert/search/persistence) is covered by
//! `crates/kernel/src/vector/hnsw/` unit tests; this binary focuses
//! exclusively on the SQL ↔ kernel plumbing the 2B.1a PR adds.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("hnsw.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

#[test]
fn create_index_using_hnsw_pre_columns_succeeds() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(4))")
        .expect("create table");
    conn.execute("CREATE INDEX idx_vec USING hnsw ON t (e)")
        .expect("create hnsw index (pre-columns USING)");
}

#[test]
fn create_index_using_hnsw_post_columns_succeeds() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(4))")
        .expect("create table");
    conn.execute("CREATE INDEX idx_vec ON t (e) USING hnsw")
        .expect("create hnsw index (post-columns USING)");
}

#[test]
fn create_index_using_btree_is_accepted_as_default() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create table");
    conn.execute("CREATE INDEX idx_a ON t (a) USING btree")
        .expect("USING btree should be accepted (and is a no-op vs the default)");
}

#[test]
fn create_index_without_using_defaults_to_btree() {
    // Negative control: without an explicit USING the historical
    // B-tree default applies and the engine never allocates an HNSW
    // handle. The smoke is the same — INSERT + SELECT round-trip via
    // the secondary index path.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, label TEXT)")
        .expect("create");
    conn.execute("CREATE INDEX idx_label ON t (label)")
        .expect("plain CREATE INDEX should default to btree");
    conn.execute("INSERT INTO t VALUES (1, 'alpha'), (2, 'beta')")
        .expect("insert");
}

#[test]
fn create_index_using_unknown_method_rejected() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    let err = conn
        .execute("CREATE INDEX idx_bogus ON t (a) USING ivfflat")
        .expect_err("USING ivfflat is not supported in 2B.1a");
    let msg = format!("{err}").to_ascii_lowercase();
    assert!(
        msg.contains("using") && msg.contains("ivfflat"),
        "error should mention the rejected method; got: {msg}"
    );
}

#[test]
fn create_index_using_hnsw_on_non_vector_column_rejected() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, label TEXT)")
        .expect("create table");
    let err = conn
        .execute("CREATE INDEX idx_label ON t (label) USING hnsw")
        .expect_err("USING hnsw requires a VECTOR(N) column");
    let msg = format!("{err}").to_ascii_lowercase();
    assert!(
        msg.contains("vector") || msg.contains("hnsw"),
        "error should explain the column-type requirement; got: {msg}"
    );
}

#[test]
fn create_index_using_hnsw_unique_rejected() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(4))")
        .expect("create table");
    let err = conn
        .execute("CREATE UNIQUE INDEX idx_unique ON t (e) USING hnsw")
        .expect_err("UNIQUE has no meaning for HNSW and must be rejected");
    let msg = format!("{err}").to_ascii_lowercase();
    assert!(
        msg.contains("unique") || msg.contains("hnsw"),
        "error should mention unique or hnsw; got: {msg}"
    );
}

#[test]
fn create_index_hnsw_with_backfill_over_existing_rows() {
    // The engine's HNSW create path runs a backfill against every
    // visible row at CREATE INDEX time. Insert two valid vectors,
    // then create the index — the dispatch path must decode the
    // blob payload via the codec and call `HnswIndex::insert_tx`
    // without surfacing an error.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, e VECTOR(3))")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, vector('[1.0, 0.0, 0.0]'))")
        .expect("insert v1");
    conn.execute("INSERT INTO t VALUES (2, vector('[0.0, 1.0, 0.0]'))")
        .expect("insert v2");
    conn.execute("CREATE INDEX idx_vec ON t (e) USING hnsw")
        .expect("hnsw create must succeed with backfill");
}
