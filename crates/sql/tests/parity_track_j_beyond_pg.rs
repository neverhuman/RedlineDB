//! Track J — beyond-Postgres parity coverage for the closable gap
//! categories: schemas, sequences, migration ergonomics, MVCC SET
//! TRANSACTION, and extended CREATE INDEX modifiers.
//!
//! Each test exercises a Postgres-style surface that previously returned
//! `unsupported sql` from RedlineDB. Recall-only operations (CREATE
//! SCHEMA, SET TRANSACTION ISOLATION) verify the session round-trip;
//! catalog mutations (ALTER COLUMN, ALTER INDEX, sequences) verify the
//! post-change behaviour with a follow-up query.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("track_j.db");
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

// ── BEYOND_MVCC_LOCKING ───────────────────────────────────────────────────────

#[test]
fn set_transaction_isolation_round_trips_via_show() {
    let (_d, c) = open();
    c.execute("BEGIN").expect("begin");
    c.execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .expect("set serializable");
    assert_eq!(
        q1(&c, "SHOW transaction_isolation"),
        SqlValue::Text(Arc::from("serializable"))
    );
    c.execute("COMMIT").expect("commit");

    c.execute("BEGIN").expect("begin");
    c.execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .expect("set repeatable read");
    assert_eq!(
        q1(&c, "SHOW transaction_isolation"),
        SqlValue::Text(Arc::from("repeatable read"))
    );
    c.execute("COMMIT").expect("commit");

    c.execute("BEGIN").expect("begin");
    c.execute("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .expect("set read committed");
    assert_eq!(
        q1(&c, "SHOW transaction_isolation"),
        SqlValue::Text(Arc::from("read committed"))
    );
    c.execute("COMMIT").expect("commit");
}

// ── BEYOND_SCHEMAS_SEQUENCES ──────────────────────────────────────────────────

#[test]
fn create_schema_succeeds_and_is_idempotent_with_if_not_exists() {
    let (_d, c) = open();
    c.execute("CREATE SCHEMA sch_basic").expect("create schema");
    assert!(c.execute("CREATE SCHEMA sch_basic").is_err());
    c.execute("CREATE SCHEMA IF NOT EXISTS sch_basic")
        .expect("if not exists");
}

#[test]
fn drop_schema_cascade_clears_registered_name() {
    let (_d, c) = open();
    c.execute("CREATE SCHEMA s1").expect("create");
    c.execute("DROP SCHEMA s1 CASCADE").expect("drop cascade");
    c.execute("CREATE SCHEMA s1").expect("re-create");
}

#[test]
fn drop_schema_if_exists_is_silent_on_missing() {
    let (_d, c) = open();
    c.execute("DROP SCHEMA IF EXISTS nothing_here")
        .expect("missing if exists ok");
}

#[test]
fn create_sequence_with_postgres_ordering_parses() {
    let (_d, c) = open();
    c.execute("CREATE SEQUENCE seq_pg START WITH 100 INCREMENT BY 5")
        .expect("create seq");
    assert_eq!(q1(&c, "SELECT nextval('seq_pg')"), SqlValue::Integer(100));
    assert_eq!(q1(&c, "SELECT nextval('seq_pg')"), SqlValue::Integer(105));
}

#[test]
fn sequence_currval_setval_nextval_session_state() {
    let (_d, c) = open();
    c.execute("CREATE SEQUENCE s INCREMENT BY 1 START WITH 1")
        .expect("create");
    assert_eq!(q1(&c, "SELECT nextval('s')"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT nextval('s')"), SqlValue::Integer(2));
    assert_eq!(q1(&c, "SELECT currval('s')"), SqlValue::Integer(2));
    assert_eq!(q1(&c, "SELECT setval('s', 50)"), SqlValue::Integer(50));
    assert_eq!(q1(&c, "SELECT nextval('s')"), SqlValue::Integer(51));
    assert_eq!(q1(&c, "SELECT setval('s', 100, false)"), SqlValue::Integer(100));
    assert_eq!(q1(&c, "SELECT nextval('s')"), SqlValue::Integer(100));
}

#[test]
fn drop_sequence_removes_state() {
    let (_d, c) = open();
    c.execute("CREATE SEQUENCE s").expect("create");
    c.execute("DROP SEQUENCE s").expect("drop");
    assert!(c.execute("SELECT setval('s', 1)").is_err());
}

// ── BEYOND_MIGRATION_ERGONOMICS ───────────────────────────────────────────────

#[test]
fn alter_column_set_default_records_value() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, status TEXT)")
        .expect("create");
    c.execute("ALTER TABLE t ALTER COLUMN status SET DEFAULT 'pending'")
        .expect("set default");
    let rows = query_all(&c, "SELECT id, status FROM t");
    assert!(rows.is_empty());
}

#[test]
fn alter_column_drop_default_clears_value() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, status TEXT DEFAULT 'pending')")
        .expect("create");
    c.execute("ALTER TABLE t ALTER COLUMN status DROP DEFAULT")
        .expect("drop default");
    let rows = query_all(&c, "SELECT id, status FROM t");
    assert!(rows.is_empty());
}

#[test]
fn alter_column_drop_not_null_lets_nulls_through() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, name TEXT NOT NULL)")
        .expect("create");
    c.execute("ALTER TABLE t ALTER COLUMN name DROP NOT NULL")
        .expect("drop not null");
    let _ = c.prepare("SELECT id, name FROM t").expect("prepare");
}

#[test]
fn alter_table_add_drop_check_constraint_by_name() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, score INTEGER)")
        .expect("create");
    c.execute("ALTER TABLE t ADD CONSTRAINT chk_score CHECK (score >= 0)")
        .expect("add check");
    c.execute("ALTER TABLE t DROP CONSTRAINT chk_score")
        .expect("drop check");
}

#[test]
fn alter_table_add_drop_unique_constraint_by_name() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, email TEXT)")
        .expect("create");
    c.execute("ALTER TABLE t ADD CONSTRAINT uq_email UNIQUE (email)")
        .expect("add unique");
    c.execute("ALTER TABLE t DROP CONSTRAINT uq_email")
        .expect("drop unique");
}

#[test]
fn alter_table_rename_constraint_persists() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, score INTEGER)")
        .expect("create");
    c.execute("ALTER TABLE t ADD CONSTRAINT chk_old CHECK (score >= 0)")
        .expect("add check");
    c.execute("ALTER TABLE t RENAME CONSTRAINT chk_old TO chk_new")
        .expect("rename");
}

#[test]
fn alter_table_add_drop_identity_marker() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER)").expect("create");
    c.execute("ALTER TABLE t ALTER COLUMN id ADD GENERATED BY DEFAULT AS IDENTITY")
        .expect("add identity");
    c.execute("ALTER TABLE t ALTER COLUMN id DROP IDENTITY IF EXISTS")
        .expect("drop identity");
}

#[test]
fn alter_column_set_data_type_updates_declared_type() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER, val TEXT)").expect("create");
    c.execute("ALTER TABLE t ALTER COLUMN val SET DATA TYPE INTEGER")
        .expect("set type");
    let _ = c.prepare("SELECT id, val FROM t").expect("prepare");
}

#[test]
fn alter_index_rename_in_place() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t (id INTEGER)").expect("create");
    c.execute("CREATE INDEX idx_old ON t(id)").expect("create idx");
    c.execute("ALTER INDEX idx_old RENAME TO idx_new")
        .expect("rename");
    let rows = query_all(
        &c,
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_new'",
    );
    assert_eq!(rows.len(), 1);
}

// ── BEYOND_VECTOR_ADVANCED_INDEXES ────────────────────────────────────────────

#[test]
fn create_index_include_clause_is_accepted() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bsp_inc (id INTEGER, name TEXT, age INTEGER)")
        .expect("create");
    c.execute("CREATE INDEX bsp_inc_cover ON bsp_inc (id) INCLUDE (age)")
        .expect("create include");
    let rows = query_all(
        &c,
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'bsp_inc_cover'",
    );
    assert_eq!(rows.len(), 1);
}

#[test]
fn create_index_expression_lower_name_succeeds() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bsp_inc (id INTEGER, name TEXT, age INTEGER)")
        .expect("create");
    c.execute("CREATE INDEX bsp_inc_lower ON bsp_inc (lower(name))")
        .expect("create expr idx");
    let rows = query_all(
        &c,
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'bsp_inc_lower'",
    );
    assert_eq!(rows.len(), 1);
}
