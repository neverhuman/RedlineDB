//! Phase 5 WS-A6: hot-row commutative-delta UPDATE fast path.
//!
//! These tests pin the correctness of the SET-clause shape detector
//! and the optimised single-writer applier. The fast path must:
//!   * produce results IDENTICAL to the legacy `eval_scalar` path,
//!   * gracefully fall back when a trigger / RETURNING / indexed
//!     column / generated column / FK / CHECK / UNIQUE constraint is
//!     present.
//!
//! Scope reduction note: the multi-writer batching coordinator and
//! the WAL `CombinedSemanticDelta` variant from the original spec
//! are not exercised here — they were deferred to a later session.
//! These tests cover only the single-writer optimisation.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> Arc<Connection> {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("ws_a6.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    // Keep the TempDir alive by leaking it for the test's lifetime;
    // the db file vanishes when the process exits.
    Box::leak(Box::new(dir));
    conn
}

fn one_int(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let v = stmt.column_value(0).expect("col").clone();
    match v {
        SqlValue::Integer(n) => n,
        other => panic!("expected Integer, got {other:?}"),
    }
}

fn one_text(conn: &Arc<Connection>, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let v = stmt.column_value(0).expect("col").clone();
    match v {
        SqlValue::Text(s) => s.to_string(),
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn pure_integer_delta_accumulates_correctly() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, counter INTEGER)")
        .expect("ddl");
    conn.execute("INSERT INTO kv(id, counter) VALUES (1, 0)")
        .expect("seed");

    for _ in 0..1000 {
        conn.execute("UPDATE kv SET counter = counter + 1 WHERE id = 1")
            .expect("update");
    }

    assert_eq!(one_int(&conn, "SELECT counter FROM kv WHERE id = 1"), 1000);
}

#[test]
fn mixed_replacement_and_delta_matches_slow_path() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, v TEXT, version INTEGER)")
        .expect("ddl");
    conn.execute("INSERT INTO kv(id, v, version) VALUES (1, 'init', 0)")
        .expect("seed");

    // Mirror the hot_row_update workload: replacement + integer delta.
    for i in 0..1000u64 {
        let mut stmt = conn
            .prepare("UPDATE kv SET v = ?1, version = version + 1 WHERE id = ?2")
            .expect("prepare");
        stmt.bind_text(1, format!("payload-{i}")).expect("bind 1");
        stmt.bind_i64(2, 1).expect("bind 2");
        assert_eq!(stmt.step().expect("step"), Step::Done);
    }

    assert_eq!(one_int(&conn, "SELECT version FROM kv WHERE id = 1"), 1000);
    assert_eq!(
        one_text(&conn, "SELECT v FROM kv WHERE id = 1"),
        "payload-999"
    );
}

#[test]
fn delta_minus_decrements() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, counter INTEGER)")
        .expect("ddl");
    conn.execute("INSERT INTO kv(id, counter) VALUES (1, 500)")
        .expect("seed");
    for _ in 0..100 {
        conn.execute("UPDATE kv SET counter = counter - 3 WHERE id = 1")
            .expect("update");
    }
    // 500 - 300 = 200.
    assert_eq!(one_int(&conn, "SELECT counter FROM kv WHERE id = 1"), 200);
}

#[test]
fn fast_path_disabled_when_returning_clause_present() {
    // RETURNING forces fallback because the user observes the post-image.
    // We verify behaviour is correct, not that a specific path was taken.
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, counter INTEGER)")
        .expect("ddl");
    conn.execute("INSERT INTO kv(id, counter) VALUES (1, 0)")
        .expect("seed");

    let mut stmt = conn
        .prepare("UPDATE kv SET counter = counter + 7 WHERE id = 1 RETURNING counter")
        .expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let v = stmt.column_value(0).expect("col").clone();
    assert_eq!(v, SqlValue::Integer(7));
    assert_eq!(stmt.step().expect("step"), Step::Done);

    assert_eq!(one_int(&conn, "SELECT counter FROM kv WHERE id = 1"), 7);
}

#[test]
fn fast_path_disabled_when_trigger_present() {
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, counter INTEGER)")
        .expect("ddl");
    conn.execute("CREATE TABLE audit (n INTEGER)")
        .expect("ddl audit");
    conn.execute("INSERT INTO kv(id, counter) VALUES (1, 0)")
        .expect("seed");

    // BEFORE trigger that mirrors the new counter into audit. If the
    // fast path ran without firing this, audit would stay empty.
    conn.execute(
        "CREATE TRIGGER bump AFTER UPDATE OF counter ON kv \
         BEGIN INSERT INTO audit(n) VALUES (NEW.counter); END",
    )
    .expect("trigger");

    for _ in 0..5 {
        conn.execute("UPDATE kv SET counter = counter + 1 WHERE id = 1")
            .expect("update");
    }

    assert_eq!(one_int(&conn, "SELECT counter FROM kv WHERE id = 1"), 5);
    assert_eq!(one_int(&conn, "SELECT COUNT(*) FROM audit"), 5);
    assert_eq!(one_int(&conn, "SELECT MAX(n) FROM audit"), 5);
}

#[test]
fn fast_path_disabled_when_set_column_is_indexed() {
    // Indexed write column → fast path must defer to the slow path so
    // `maintain_indexes_on_update` updates the secondary index. We
    // verify the index still answers correctly after the update.
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, score INTEGER)")
        .expect("ddl");
    conn.execute("CREATE INDEX kv_score ON kv(score)")
        .expect("idx");
    conn.execute("INSERT INTO kv(id, score) VALUES (1, 100)")
        .expect("seed");
    for _ in 0..10 {
        conn.execute("UPDATE kv SET score = score + 1 WHERE id = 1")
            .expect("update");
    }
    assert_eq!(one_int(&conn, "SELECT score FROM kv WHERE id = 1"), 110);
    // Verify the index entry tracks the new value (a stale index would
    // miss this exact-match probe).
    assert_eq!(one_int(&conn, "SELECT id FROM kv WHERE score = 110"), 1);
}

#[test]
fn unsupported_expr_shape_falls_back_cleanly() {
    // `counter = counter * 2` is not a Plus/Minus delta — the fast
    // path classifier must reject it and the slow path must compute
    // it correctly.
    let conn = open();
    conn.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, counter INTEGER)")
        .expect("ddl");
    conn.execute("INSERT INTO kv(id, counter) VALUES (1, 3)")
        .expect("seed");
    for _ in 0..5 {
        conn.execute("UPDATE kv SET counter = counter * 2 WHERE id = 1")
            .expect("update");
    }
    // 3 → 6 → 12 → 24 → 48 → 96.
    assert_eq!(one_int(&conn, "SELECT counter FROM kv WHERE id = 1"), 96);
}
