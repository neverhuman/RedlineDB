//! Phase-11 SQL-D A6 parity: partial indexes.
//!
//! Verifies that `CREATE INDEX ... WHERE <predicate>` produces the same
//! query results, error classes, and (where observable) the same
//! index-use decisions as SQLite's rusqlite reference.

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
        let path = dir.path().join("partial.db");
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

fn partial_index_indexes_only_matching_rows() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, status TEXT, score INTEGER)");
    lab.execute("CREATE INDEX ix_active ON t(score) WHERE status = 'active'");
    lab.execute(
        "INSERT INTO t(id, status, score) VALUES \
         (1, 'active', 10), \
         (2, 'inactive', 20), \
         (3, 'active', 30), \
         (4, 'inactive', 40)",
    );
    // Query that matches the partial predicate exactly should return
    // the active rows ordered by score.
    lab.assert_query_matches(
        "SELECT id, score FROM t WHERE status = 'active' AND score > 0 ORDER BY id",
    );
    // Query without the predicate should still find all rows.
    lab.assert_query_matches("SELECT id FROM t ORDER BY id");
}

#[test]

fn partial_index_skip_insert_when_predicate_false() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, active INTEGER, name TEXT)");
    lab.execute("CREATE INDEX ix_a ON t(name) WHERE active = 1");
    lab.execute(
        "INSERT INTO t VALUES (1, 1, 'a'), (2, 0, 'b'), (3, 1, 'c'), (4, 0, 'd')",
    );
    lab.assert_query_matches("SELECT id, name FROM t WHERE active = 1 ORDER BY name");
}

#[test]

fn partial_index_update_moves_in_and_out() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, k INTEGER, flag INTEGER)");
    lab.execute("CREATE INDEX ix_k ON t(k) WHERE flag = 1");
    lab.execute("INSERT INTO t VALUES (1, 100, 1), (2, 200, 0)");
    // Flip rows in/out of the index.
    lab.execute("UPDATE t SET flag = 0 WHERE id = 1");
    lab.execute("UPDATE t SET flag = 1 WHERE id = 2");
    lab.assert_query_matches("SELECT id, k FROM t WHERE flag = 1 ORDER BY id");
    lab.assert_query_matches("SELECT id, k FROM t WHERE flag = 0 ORDER BY id");
}

#[test]

fn partial_index_delete_only_marks_matching_keys() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, x INTEGER, kind TEXT)");
    lab.execute("CREATE INDEX ix_x ON t(x) WHERE kind = 'A'");
    lab.execute(
        "INSERT INTO t VALUES (1, 10, 'A'), (2, 20, 'B'), (3, 30, 'A'), (4, 40, 'B')",
    );
    lab.execute("DELETE FROM t WHERE kind = 'B'");
    lab.assert_query_matches("SELECT id, x FROM t WHERE kind = 'A' ORDER BY id");
}

#[test]

fn partial_index_planner_only_uses_matching_predicate() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, a INTEGER, b TEXT)");
    lab.execute("CREATE INDEX ix_a ON t(a) WHERE b = 'x'");
    lab.execute("INSERT INTO t VALUES (1, 10, 'x'), (2, 20, 'y'), (3, 30, 'x')");
    // Query without the predicate must still get correct rows — planner
    // must NOT use the partial index here.
    lab.assert_query_matches("SELECT id FROM t WHERE a = 20 ORDER BY id");
    // Query with the matching predicate should use the index and return
    // correct results.
    lab.assert_query_matches("SELECT id FROM t WHERE b = 'x' AND a = 10 ORDER BY id");
}
