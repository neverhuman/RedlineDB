//! Trigger parity coverage (Lane A5-triggers).
//!
//! Differential against rusqlite for AFTER INSERT, AFTER UPDATE with
//! `UPDATE OF` filtering, AFTER DELETE cascade chains, BEFORE-style
//! WHEN-predicate filtering, and the SQLite recursion cap.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

struct Lab {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Lab {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("tr.db");
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        Self {
            _dir: dir,
            redline: db.connect(),
            sqlite: rusqlite::Connection::open_in_memory().expect("open in memory"),
        }
    }

    fn execute(&self, sql: &str) {
        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|e| panic!("sqlite setup failed for {sql:?}: {e}"));
        self.redline
            .execute(sql)
            .unwrap_or_else(|e| panic!("redline setup failed for {sql:?}: {e:?}"));
    }

    fn assert_match(&self, sql: &str) {
        let ru = query_sqlite(&self.sqlite, sql);
        let rl = query_redline(&self.redline, sql);
        if ru != rl {
            panic!("trigger mismatch on {sql:?}\n  sqlite={ru:?}\n  redline={rl:?}");
        }
    }
}

fn query_sqlite(c: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = c.prepare(sql).expect("prepare");
    let cols = stmt.column_count();
    let mut rows = stmt.query([]).expect("query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("next") {
        let mut current = Vec::with_capacity(cols);
        for i in 0..cols {
            let v: RuValue = row.get(i).expect("get");
            current.push(to_sql(v));
        }
        out.push(current);
    }
    out
}

fn query_redline(c: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = c
        .prepare(sql)
        .unwrap_or_else(|e| panic!("redline prepare failed for {sql:?}: {e:?}"));
    let cols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let mut row = Vec::with_capacity(cols);
        for i in 0..cols {
            row.push(stmt.column_value(i).expect("col").clone());
        }
        out.push(row);
    }
    out
}

fn to_sql(v: RuValue) -> SqlValue {
    match v {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(n) => SqlValue::Integer(n),
        RuValue::Real(r) => SqlValue::Real(r),
        RuValue::Text(s) => SqlValue::Text(Arc::from(s)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

// ── AFTER INSERT: NEW row mirrored into an audit table ──────────────────────

#[test]
fn after_insert_writes_audit_row() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER, b TEXT)");
    lab.execute("CREATE TABLE audit(a INTEGER, b TEXT)");
    lab.execute(
        "CREATE TRIGGER copy AFTER INSERT ON t FOR EACH ROW \
         BEGIN INSERT INTO audit(a, b) VALUES (NEW.a, NEW.b); END",
    );
    lab.execute("INSERT INTO t VALUES (1, 'x')");
    lab.execute("INSERT INTO t VALUES (2, 'y')");
    lab.assert_match("SELECT a, b FROM audit ORDER BY a");
}

// ── AFTER UPDATE with UPDATE OF column filter ──────────────────────────────

#[test]
fn after_update_of_column_filters_firing() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER, b INTEGER)");
    lab.execute("CREATE TABLE log(old_a INTEGER, new_a INTEGER)");
    lab.execute(
        "CREATE TRIGGER on_a AFTER UPDATE OF a ON t FOR EACH ROW \
         BEGIN INSERT INTO log VALUES (OLD.a, NEW.a); END",
    );
    lab.execute("INSERT INTO t VALUES (1, 100)");
    // Update b only — trigger should NOT fire.
    lab.execute("UPDATE t SET b = 999 WHERE a = 1");
    // Update a — trigger should fire.
    lab.execute("UPDATE t SET a = 42 WHERE a = 1");
    lab.assert_match("SELECT old_a, new_a FROM log ORDER BY old_a");
}

// ── AFTER DELETE: cascade-style chain via trigger ──────────────────────────

#[test]
fn after_delete_cascades_to_child_via_trigger() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE parent(id INTEGER, name TEXT)");
    lab.execute("CREATE TABLE child(parent_id INTEGER, info TEXT)");
    lab.execute(
        "CREATE TRIGGER del_children AFTER DELETE ON parent FOR EACH ROW \
         BEGIN DELETE FROM child WHERE parent_id = OLD.id; END",
    );
    lab.execute("INSERT INTO parent VALUES (1, 'a'), (2, 'b')");
    lab.execute("INSERT INTO child VALUES (1, 'kid_a1'), (1, 'kid_a2'), (2, 'kid_b1')");
    lab.execute("DELETE FROM parent WHERE id = 1");
    lab.assert_match("SELECT parent_id, info FROM child ORDER BY parent_id, info");
    lab.assert_match("SELECT id FROM parent ORDER BY id");
}

// ── WHEN predicate filtering ───────────────────────────────────────────────

#[test]
fn when_predicate_skips_body_for_false_rows() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("CREATE TABLE big(a INTEGER)");
    lab.execute(
        "CREATE TRIGGER large AFTER INSERT ON t FOR EACH ROW \
         WHEN NEW.a > 10 \
         BEGIN INSERT INTO big VALUES (NEW.a); END",
    );
    lab.execute("INSERT INTO t VALUES (1), (5), (10), (11), (100)");
    lab.assert_match("SELECT a FROM big ORDER BY a");
}

// ── Recursive trigger cap ──────────────────────────────────────────────────

#[test]
fn recursive_trigger_terminates_at_cap() {
    // Trigger that re-inserts into the same table; without the cap this
    // would loop forever. Both engines should error out.
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute(
        "CREATE TRIGGER recur AFTER INSERT ON t FOR EACH ROW \
         BEGIN INSERT INTO t VALUES (NEW.a + 1); END",
    );
    // Enable SQLite's recursive triggers so the comparison is fair.
    lab.sqlite
        .execute("PRAGMA recursive_triggers = ON", ())
        .expect("sqlite pragma");
    let ru_err = lab.sqlite.execute("INSERT INTO t VALUES (1)", ()).err();
    let rl_err = lab.redline.execute("INSERT INTO t VALUES (1)").err();
    assert!(ru_err.is_some(), "sqlite expected recursion cap error");
    assert!(rl_err.is_some(), "redline expected recursion cap error");
}

// ── DROP TRIGGER ──────────────────────────────────────────────────────────

#[test]
fn drop_trigger_removes_firing() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("CREATE TABLE seen(a INTEGER)");
    lab.execute(
        "CREATE TRIGGER mirror AFTER INSERT ON t FOR EACH ROW \
         BEGIN INSERT INTO seen VALUES (NEW.a); END",
    );
    lab.execute("INSERT INTO t VALUES (1)");
    lab.execute("DROP TRIGGER mirror");
    lab.execute("INSERT INTO t VALUES (2)");
    lab.assert_match("SELECT a FROM seen ORDER BY a");
}

// ── Trigger persists across reopen ────────────────────────────────────────

#[test]
fn trigger_survives_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("p.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
        conn.execute("CREATE TABLE seen(a INTEGER)")
            .expect("create");
        conn.execute(
            "CREATE TRIGGER mirror AFTER INSERT ON t FOR EACH ROW \
             BEGIN INSERT INTO seen VALUES (NEW.a); END",
        )
        .expect("create trigger");
    }
    let db = Database::open(&path, DbOptions::default()).expect("reopen");
    let conn = db.connect();
    conn.execute("INSERT INTO t VALUES (42)").expect("insert");
    let rows = query_redline(&conn, "SELECT a FROM seen");
    assert_eq!(rows, vec![vec![SqlValue::Integer(42)]]);
}
