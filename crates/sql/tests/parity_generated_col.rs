//! Phase-11 SQL-D A6 parity: GENERATED ALWAYS AS (expr) STORED/VIRTUAL.
//!
//! Verifies STORED + VIRTUAL semantics match SQLite, including
//! recomputation on UPDATE-of-input and the SQLite write-block error
//! ("cannot INSERT into generated column" / "cannot UPDATE generated
//! column").

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
        let path = dir.path().join("gen.db");
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

fn generated_stored_basic_insert_select() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         b INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a + b) STORED)",
    );
    lab.execute("INSERT INTO t(id, a, b) VALUES (1, 10, 20), (2, 5, 6)");
    lab.assert_query_matches("SELECT id, a, b, c FROM t ORDER BY id");
}

#[test]

fn generated_virtual_basic_insert_select() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         b INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a * b) VIRTUAL)",
    );
    lab.execute("INSERT INTO t(id, a, b) VALUES (1, 3, 4), (2, 7, 8)");
    lab.assert_query_matches("SELECT id, a, b, c FROM t ORDER BY id");
}

#[test]

fn generated_stored_recomputes_on_update() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         b INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a + b) STORED)",
    );
    lab.execute("INSERT INTO t(id, a, b) VALUES (1, 10, 20)");
    lab.execute("UPDATE t SET a = 100 WHERE id = 1");
    lab.assert_query_matches("SELECT id, a, b, c FROM t");
}

#[test]

fn generated_virtual_reflects_update() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         b INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a * 2) VIRTUAL)",
    );
    lab.execute("INSERT INTO t(id, a, b) VALUES (1, 5, 0)");
    lab.execute("UPDATE t SET a = 9 WHERE id = 1");
    lab.assert_query_matches("SELECT id, c FROM t");
}

#[test]

fn generated_stored_write_block_explicit_insert() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a + 1) STORED)",
    );
    // Both engines must reject writes that name a generated column.
    let res_ru = lab
        .sqlite
        .execute("INSERT INTO t(id, a, c) VALUES (1, 1, 99)", []);
    let res_rl = lab
        .redline
        .execute("INSERT INTO t(id, a, c) VALUES (1, 1, 99)");
    assert!(
        res_ru.is_err(),
        "sqlite should reject writing generated col"
    );
    assert!(
        res_rl.is_err(),
        "redline should reject writing generated col"
    );
}

#[test]

fn generated_virtual_write_block_explicit_update() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a + 1) VIRTUAL)",
    );
    lab.execute("INSERT INTO t(id, a) VALUES (1, 5)");
    let res_ru = lab.sqlite.execute("UPDATE t SET c = 99 WHERE id = 1", []);
    let res_rl = lab.redline.execute("UPDATE t SET c = 99 WHERE id = 1");
    assert!(
        res_ru.is_err(),
        "sqlite should reject updating generated col"
    );
    assert!(
        res_rl.is_err(),
        "redline should reject updating generated col"
    );
}

#[test]

fn generated_implicit_insert_skips_generated_columns() {
    let lab = Lab::new();
    lab.execute(
        "CREATE TABLE t(\
         id INTEGER PRIMARY KEY, \
         a INTEGER, \
         c INTEGER GENERATED ALWAYS AS (a + 100) STORED)",
    );
    // Without an explicit column list, both engines must allow the
    // INSERT and only provide values for non-generated columns.
    lab.execute("INSERT INTO t VALUES (1, 5), (2, 7)");
    lab.assert_query_matches("SELECT id, a, c FROM t ORDER BY id");
}
