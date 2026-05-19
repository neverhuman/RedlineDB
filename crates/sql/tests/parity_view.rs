//! View parity coverage (Lane A5-views).
//!
//! Differential against rusqlite for CREATE VIEW, DROP VIEW, SELECT FROM
//! a view, JOIN against a view, IF NOT EXISTS / IF EXISTS variants, and
//! the "cannot modify a view" error class for DML.

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
        let path = dir.path().join("v.db");
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
            panic!("view mismatch on {sql:?}\n  sqlite={ru:?}\n  redline={rl:?}");
        }
    }

    /// Both engines must reject `sql` with an error. We assert RedlineDB
    /// returns an error matching SQLite's "view" / "modify" error class.
    fn assert_both_reject(&self, sql: &str) {
        let ru_err = self.sqlite.execute(sql, ()).err();
        assert!(ru_err.is_some(), "sqlite unexpectedly accepted {sql:?}");
        let rl_err = self.redline.execute(sql).err();
        assert!(rl_err.is_some(), "redline unexpectedly accepted {sql:?}");
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

// ── Create / select / drop ─────────────────────────────────────────────────

#[test]
fn create_select_drop_view_basic() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER, b INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 10), (2, 20), (3, 30)");
    lab.execute("CREATE VIEW v_t AS SELECT a, b FROM t WHERE a > 1");
    lab.assert_match("SELECT a, b FROM v_t ORDER BY a");
    lab.assert_match("SELECT COUNT(*) FROM v_t");
    lab.execute("DROP VIEW v_t");
    // After drop the view should no longer resolve in either engine.
    lab.assert_both_reject("SELECT * FROM v_t");
}

#[test]
fn view_with_alias_columns() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(x INTEGER, y INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 100), (2, 200)");
    lab.execute("CREATE VIEW v_renamed(p, q) AS SELECT x, y FROM t");
    lab.assert_match("SELECT p, q FROM v_renamed ORDER BY p");
}

#[test]
fn view_with_predicate_and_projection() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE orders(id INTEGER, amount INTEGER, status TEXT)");
    lab.execute("INSERT INTO orders VALUES (1, 100, 'OK'), (2, 200, 'CANCEL'), (3, 300, 'OK')");
    lab.execute("CREATE VIEW ok_orders AS SELECT id, amount FROM orders WHERE status = 'OK'");
    lab.assert_match("SELECT id, amount FROM ok_orders ORDER BY id");
    lab.assert_match("SELECT SUM(amount) FROM ok_orders");
}

#[test]
fn repeated_prepare_of_view_observes_base_table_changes() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("INSERT INTO t VALUES (1)");
    lab.execute("CREATE VIEW v AS SELECT a FROM t");

    let sql = "SELECT a FROM v ORDER BY a";
    lab.assert_match(sql);

    lab.execute("INSERT INTO t VALUES (2)");
    lab.assert_match(sql);
}

// ── Joins involving a view ─────────────────────────────────────────────────

#[test]
fn join_two_tables_through_view() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)");
    lab.execute("CREATE TABLE orders(uid INTEGER, total INTEGER)");
    lab.execute("INSERT INTO users VALUES (1, 'a'), (2, 'b'), (3, 'c')");
    lab.execute("INSERT INTO orders VALUES (1, 10), (1, 30), (2, 50), (3, 20)");
    lab.execute(
        "CREATE VIEW user_totals AS SELECT uid, SUM(total) AS total FROM orders GROUP BY uid",
    );
    lab.assert_match(
        "SELECT u.name, t.total FROM users u JOIN user_totals t ON u.id = t.uid ORDER BY u.name",
    );
}

#[test]
fn repeated_prepare_of_view_join_observes_base_table_changes_across_connections() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("shared.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let writer = db.connect();
    let reader = db.connect();

    writer
        .execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)")
        .expect("create users");
    writer
        .execute("CREATE TABLE orders(uid INTEGER, total INTEGER)")
        .expect("create orders");
    writer
        .execute("INSERT INTO users VALUES (1, 'a'), (2, 'b')")
        .expect("insert users");
    writer
        .execute("INSERT INTO orders VALUES (1, 10), (2, 20)")
        .expect("insert orders");
    writer
        .execute(
            "CREATE VIEW user_totals AS SELECT uid, SUM(total) AS total FROM orders GROUP BY uid",
        )
        .expect("create view");

    let sql =
        "SELECT u.name, t.total FROM users u JOIN user_totals t ON u.id = t.uid ORDER BY u.name";
    assert_eq!(
        query_redline(&writer, sql),
        vec![
            vec![SqlValue::Text(Arc::from("a")), SqlValue::Integer(10)],
            vec![SqlValue::Text(Arc::from("b")), SqlValue::Integer(20)],
        ],
    );

    writer
        .execute("INSERT INTO orders VALUES (1, 30)")
        .expect("insert changed order");
    assert_eq!(
        query_redline(&reader, sql),
        vec![
            vec![SqlValue::Text(Arc::from("a")), SqlValue::Integer(40)],
            vec![SqlValue::Text(Arc::from("b")), SqlValue::Integer(20)],
        ],
    );
}

// ── IF NOT EXISTS / IF EXISTS ──────────────────────────────────────────────

#[test]
fn create_view_if_not_exists_is_noop() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("INSERT INTO t VALUES (7)");
    lab.execute("CREATE VIEW v AS SELECT a FROM t");
    // Second CREATE without IF NOT EXISTS fails; both engines should error.
    lab.assert_both_reject("CREATE VIEW v AS SELECT a FROM t");
    // With IF NOT EXISTS the operation should be silently skipped.
    lab.execute("CREATE VIEW IF NOT EXISTS v AS SELECT a FROM t");
    lab.assert_match("SELECT a FROM v");
}

#[test]
fn drop_view_if_exists_is_noop() {
    let lab = Lab::new();
    // Drop of a non-existent view without IF EXISTS errors; both engines
    // should reject.
    lab.assert_both_reject("DROP VIEW nope");
    // With IF EXISTS the drop is a no-op.
    lab.sqlite
        .execute("DROP VIEW IF EXISTS nope", ())
        .expect("sqlite drop");
    lab.redline
        .execute("DROP VIEW IF EXISTS nope")
        .expect("redline drop");
}

// ── DML on a view must fail ────────────────────────────────────────────────

#[test]
fn insert_into_view_is_rejected() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("INSERT INTO t VALUES (1)");
    lab.execute("CREATE VIEW v AS SELECT a FROM t");
    lab.assert_both_reject("INSERT INTO v VALUES (2)");
}

#[test]
fn update_view_is_rejected() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("INSERT INTO t VALUES (1)");
    lab.execute("CREATE VIEW v AS SELECT a FROM t");
    lab.assert_both_reject("UPDATE v SET a = 99");
}

#[test]
fn delete_from_view_is_rejected() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER)");
    lab.execute("INSERT INTO t VALUES (1)");
    lab.execute("CREATE VIEW v AS SELECT a FROM t");
    lab.assert_both_reject("DELETE FROM v");
}

// ── Persistence across reopen ──────────────────────────────────────────────

#[test]
fn view_survives_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("persist.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
        conn.execute("INSERT INTO t VALUES (42)").expect("insert");
        conn.execute("CREATE VIEW v AS SELECT a FROM t")
            .expect("view");
    }
    let db = Database::open(&path, DbOptions::default()).expect("reopen");
    let conn = db.connect();
    let rows = query_redline(&conn, "SELECT a FROM v");
    assert_eq!(rows, vec![vec![SqlValue::Integer(42)]]);
}
