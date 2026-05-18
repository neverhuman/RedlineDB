//! Phase-11 SQL-D A6 parity: expression indexes.
//!
//! Verifies `CREATE INDEX ix ON t(expr(col))` matches SQLite's behavior
//! for indexed reads, INSERT / UPDATE / DELETE maintenance, and query
//! results when the matching `WHERE expr(col) = value` form appears.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

fn to_sql_value(val: RuValue) -> SqlValue {
    match val {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(i) => SqlValue::Integer(i),
        RuValue::Real(f) => SqlValue::Real(f),
        RuValue::Text(s) => SqlValue::Text(Arc::from(s)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

struct Lab {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Lab {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("expr.db");
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        let redline = db.connect();
        let sqlite = rusqlite::Connection::open_in_memory().expect("rusqlite open");
        Self {
            _dir: dir,
            redline,
            sqlite,
        }
    }

    fn execute(&self, sql: &str) {
        self.sqlite.execute_batch(sql).expect("sqlite execute");
        self.redline.execute(sql).expect("redline execute");
    }

    fn query_redline(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        let mut stmt = self.redline.prepare(sql).expect("prepare");
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

    fn query_sqlite(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        let mut stmt = self.sqlite.prepare(sql).expect("rusqlite prepare");
        let ncols = stmt.column_count();
        let mut q = stmt.query([]).expect("query");
        let mut out = Vec::new();
        while let Some(row) = q.next().expect("next") {
            let mut current = Vec::with_capacity(ncols);
            for i in 0..ncols {
                let v: RuValue = row.get(i).expect("get");
                current.push(to_sql_value(v));
            }
            out.push(current);
        }
        out
    }

    fn assert_query_matches(&self, sql: &str) {
        let a = self.query_sqlite(sql);
        let b = self.query_redline(sql);
        assert_eq!(a, b, "divergence on `{sql}`\nsqlite: {a:?}\nredline: {b:?}");
    }
}

#[test]

fn expression_index_lower_lookup() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)");
    lab.execute("CREATE INDEX ix_lname ON t(lower(name))");
    lab.execute("INSERT INTO t VALUES (1, 'Alice'), (2, 'bob'), (3, 'CAROL'), (4, 'Dave')");
    lab.assert_query_matches("SELECT id, name FROM t WHERE lower(name) = 'bob' ORDER BY id");
    lab.assert_query_matches("SELECT id FROM t WHERE lower(name) = 'carol' ORDER BY id");
}

#[test]

fn expression_index_update_recomputes_key() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)");
    lab.execute("CREATE INDEX ix_lname ON t(lower(name))");
    lab.execute("INSERT INTO t VALUES (1, 'ZAP'), (2, 'WOW')");
    lab.execute("UPDATE t SET name = 'foo' WHERE id = 1");
    lab.assert_query_matches("SELECT id, name FROM t WHERE lower(name) = 'foo' ORDER BY id");
    lab.assert_query_matches("SELECT id FROM t WHERE lower(name) = 'zap' ORDER BY id");
}

#[test]

fn expression_index_delete_removes_key() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)");
    lab.execute("CREATE INDEX ix_lname ON t(lower(name))");
    lab.execute("INSERT INTO t VALUES (1, 'Hello'), (2, 'World')");
    lab.execute("DELETE FROM t WHERE id = 1");
    lab.assert_query_matches("SELECT id FROM t WHERE lower(name) = 'hello' ORDER BY id");
    lab.assert_query_matches("SELECT id FROM t WHERE lower(name) = 'world' ORDER BY id");
}

#[test]

fn expression_index_fallback_to_scan_when_pred_form_does_not_match() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)");
    lab.execute("CREATE INDEX ix_lname ON t(lower(name))");
    lab.execute("INSERT INTO t VALUES (1, 'Alpha'), (2, 'Beta')");
    // Plain `name = 'Alpha'` cannot use the lower(name) index — but the
    // results must still match.
    lab.assert_query_matches("SELECT id FROM t WHERE name = 'Alpha' ORDER BY id");
}
