//! High-level SQLite parity traceability tests.
//!
//! This suite is intentionally small. It records the bundled `rusqlite`
//! reference build, checks representative passing behavior against that
//! reference, and keeps major full-SQLite gaps executable until they are
//! implemented.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

struct Harness {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Harness {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let db = Database::create(dir.path().join("parity.db"), DbOptions::default())
            .expect("create redline db");
        let sqlite = rusqlite::Connection::open_in_memory().expect("open sqlite");
        Self {
            _dir: dir,
            redline: db.connect(),
            sqlite,
        }
    }

    fn execute_both(&self, sql: &str) {
        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|err| panic!("sqlite setup failed for {sql:?}: {err}"));
        self.redline
            .execute(sql)
            .unwrap_or_else(|err| panic!("redline setup failed for {sql:?}: {err:?}"));
    }

    fn assert_query_matches(&self, sql: &str) {
        let sqlite_rows = query_sqlite(&self.sqlite, sql);
        let redline_rows = query_redline(&self.redline, sql);
        assert_eq!(
            redline_rows, sqlite_rows,
            "query mismatch for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
        );
    }

    fn assert_sqlite_accepts_redline_rejects(&self, setup: &[&str], sql: &str) {
        for stmt in setup {
            self.execute_both(stmt);
        }

        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|err| panic!("sqlite should accept {sql:?}: {err}"));

        let redline_result = redline_accepts(&self.redline, sql);
        assert!(
            redline_result.is_err(),
            "redline unexpectedly accepted known full-parity gap: {sql}"
        );
    }

    fn assert_sqlite_result_diff_or_redline_rejects(&self, setup: &[&str], sql: &str) {
        for stmt in setup {
            self.sqlite
                .execute_batch(stmt)
                .unwrap_or_else(|err| panic!("sqlite setup failed for {stmt:?}: {err}"));
            if self.redline.execute(stmt).is_err() {
                return;
            }
        }

        let sqlite_rows = query_sqlite(&self.sqlite, sql);
        let redline_rows = match try_query_redline(&self.redline, sql) {
            Ok(rows) => rows,
            Err(_) => return,
        };
        assert_ne!(
            redline_rows, sqlite_rows,
            "redline unexpectedly matched known full-parity gap: {sql}"
        );
    }
}

fn query_sqlite(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|err| panic!("sqlite prepare failed for {sql:?}: {err}"));
    let columns = stmt.column_count();
    let mut rows = stmt.query([]).expect("sqlite query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("sqlite next") {
        let mut values = Vec::with_capacity(columns);
        for idx in 0..columns {
            let value: RuValue = row.get(idx).expect("sqlite value");
            values.push(to_sql_value(value));
        }
        out.push(values);
    }
    out
}

fn query_redline(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    try_query_redline(conn, sql).unwrap_or_else(|err| {
        panic!("redline query failed for {sql:?}: {err:?}");
    })
}

fn try_query_redline(
    conn: &Arc<Connection>,
    sql: &str,
) -> Result<Vec<Vec<SqlValue>>, redlinedb_sql::Error> {
    let mut stmt = conn.prepare(sql)?;
    let columns = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step()? {
        let mut values = Vec::with_capacity(columns);
        for idx in 0..columns {
            values.push(stmt.column_value(idx)?.clone());
        }
        out.push(values);
    }
    Ok(out)
}

fn redline_accepts(conn: &Arc<Connection>, sql: &str) -> Result<(), redlinedb_sql::Error> {
    let trimmed = sql.trim_start();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("SELECT") || upper.starts_with("WITH") {
        let mut stmt = conn.prepare(sql)?;
        while let Step::Row = stmt.step()? {}
        return Ok(());
    }
    conn.execute(sql).map(|_| ())
}

fn to_sql_value(value: RuValue) -> SqlValue {
    match value {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(value) => SqlValue::Integer(value),
        RuValue::Real(value) => SqlValue::Real(value),
        RuValue::Text(value) => SqlValue::Text(Arc::from(value)),
        RuValue::Blob(value) => SqlValue::Blob(Arc::from(value)),
    }
}

#[test]
fn reference_build_metadata_is_available() {
    let conn = rusqlite::Connection::open_in_memory().expect("open sqlite");
    let version: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .expect("sqlite version");
    let compile_options: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA compile_options")
            .expect("compile options");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("compile option rows");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("compile option values")
    };

    println!("rusqlite_crate_version=0.37.0");
    println!("sqlite_version={version}");
    println!("compile_options:");
    for option in &compile_options {
        println!("{option}");
    }

    assert!(!version.trim().is_empty(), "empty sqlite_version()");
    assert!(
        !compile_options.is_empty(),
        "bundled SQLite reported no compile options"
    );
}

#[test]
fn representative_sqlite_supported_rows_match_reference() {
    let harness = Harness::new();
    harness.execute_both("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, n INTEGER)");
    harness.execute_both(
        "INSERT INTO t(id, name, n) VALUES (1, ' Ada ', 10), (2, 'Grace', NULL), (3, NULL, 5)",
    );

    harness.assert_query_matches(
        "SELECT id, trim(name), substr(name, 2, 2), coalesce(n, 99) FROM t ORDER BY id",
    );
    harness.assert_query_matches("SELECT count(*), count(n), total(n) FROM t");
    harness.assert_query_matches(
        "SELECT id FROM t WHERE id IN (SELECT id FROM t WHERE n IS NOT NULL) ORDER BY id",
    );
}

#[test]
fn known_full_sqlite_parity_gaps_are_explicit_failures() {
    // CTE (`WITH`) is now supported — see `parity_cte.rs`.
    // Window functions (`OVER`) are now supported — see `parity_window.rs`.
    // CREATE VIEW is now supported (Lane A5-views) — see `parity_view.rs`.
    Harness::new().assert_sqlite_accepts_redline_rejects(
        &["CREATE TABLE t(a INTEGER)"],
        "CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET a = a + 1; END",
    );
    Harness::new().assert_sqlite_result_diff_or_redline_rejects(
        &[
            "CREATE TABLE g(a INTEGER, b INTEGER GENERATED ALWAYS AS (a + 1) STORED)",
            "INSERT INTO g(a) VALUES (4)",
        ],
        "SELECT a, b FROM g",
    );
    Harness::new().assert_sqlite_accepts_redline_rejects(
        &["CREATE TABLE t(a INTEGER)"],
        "CREATE INDEX idx_partial ON t(a) WHERE a > 0",
    );
    Harness::new().assert_sqlite_accepts_redline_rejects(
        &["CREATE TABLE t(a INTEGER)"],
        "CREATE INDEX idx_expr ON t((a + 1))",
    );
}
