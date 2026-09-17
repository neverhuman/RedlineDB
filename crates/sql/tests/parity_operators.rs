//! Differential parity tests for operators and DML surfaces that already
//! work but lacked rusqlite-oracle coverage. Locks in the current behaviour
//! so subsequent refactors can't regress us silently.
//!
//! Covered:
//! - `||` string concatenation
//! - `REGEXP` operator + `regexp(pattern, value)` UDF
//! - `LIKE` and `ILIKE` operators (single-value form; `ILIKE ANY` stays a
//!   negative test because that surface is intentionally rejected)
//! - `INSERT ... RETURNING *` and `UPDATE ... RETURNING ...`

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

struct Pair {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Pair {
    fn new() -> Self {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("ops.db");
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let redline = db.connect();
        let sqlite = rusqlite::Connection::open_in_memory().expect("rusqlite open");
        Pair {
            _dir: dir,
            redline,
            sqlite,
        }
    }

    fn execute_both(&self, sql: &str) {
        self.sqlite.execute_batch(sql).expect("sqlite exec");
        self.redline.execute(sql).expect("redline exec");
    }

    fn redline_rows(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        let mut stmt = self.redline.prepare(sql).expect("redline prepare");
        let ncols = stmt.column_count();
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("redline step") {
            let row: Vec<SqlValue> = (0..ncols)
                .map(|i| stmt.column_value(i).expect("redline col").clone())
                .collect();
            rows.push(row);
        }
        rows
    }

    fn sqlite_rows(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        let mut stmt = self.sqlite.prepare(sql).expect("sqlite prepare");
        let ncols = stmt.column_count();
        let mut sqlite_rows = Vec::new();
        let mut query = stmt.query([]).expect("sqlite query");
        while let Some(row) = query.next().expect("sqlite next") {
            let current: Vec<SqlValue> = (0..ncols)
                .map(|i| to_sql_value(row.get::<usize, RuValue>(i).expect("sqlite get")))
                .collect();
            sqlite_rows.push(current);
        }
        sqlite_rows
    }

    fn assert_parity(&self, sql: &str) {
        let rl = self.redline_rows(sql);
        let sl = self.sqlite_rows(sql);
        assert_eq!(rl, sl, "rows differ for: {sql}");
    }
}

// ---------------------------------------------------------------------------
// `||` (string concatenation). Covers the standard text-text case plus the
// SQLite quirks: NULL anywhere in the chain returns NULL, and integer/real
// operands stringify via SQLite's `printf("%g")`-style affinity.
// ---------------------------------------------------------------------------

#[test]
fn parity_concat_text_text() {
    let pair = Pair::new();
    pair.assert_parity("SELECT 'foo' || 'bar'");
}

#[test]
fn parity_concat_text_null_is_null() {
    let pair = Pair::new();
    pair.assert_parity("SELECT 'foo' || NULL");
    pair.assert_parity("SELECT NULL || 'foo'");
}

#[test]
fn parity_concat_integer_coerces_to_text() {
    let pair = Pair::new();
    pair.assert_parity("SELECT 'x=' || 42");
}

#[test]
fn parity_concat_three_way_chain() {
    let pair = Pair::new();
    pair.assert_parity("SELECT 'a' || 'b' || 'c'");
}

// ---------------------------------------------------------------------------
// REGEXP operator + `regexp()` UDF. rusqlite needs the `functions` feature
// for the operator to fire, but it ships a default `regexp()` adapter via
// `Connection::create_scalar_function` — we install a thin wrapper that
// uses the regex crate to mirror RedlineDB's behaviour.
// ---------------------------------------------------------------------------

fn install_sqlite_regexp(conn: &rusqlite::Connection) {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "regexp",
        2,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8,
        |ctx| {
            let pattern: String = ctx.get(0)?;
            let value: String = ctx.get(1)?;
            let re = regex::Regex::new(&pattern).map_err(|err| {
                rusqlite::Error::UserFunctionError(Box::new(std::io::Error::other(err.to_string())))
            })?;
            Ok(re.is_match(&value))
        },
    )
    .expect("install sqlite regexp");
}

#[test]
fn parity_regexp_operator_matches() {
    let pair = Pair::new();
    install_sqlite_regexp(&pair.sqlite);
    pair.assert_parity("SELECT 'hello world' REGEXP '^hello'");
    pair.assert_parity("SELECT 'hello world' REGEXP '^world'");
}

#[test]
fn parity_regexp_function_form_matches() {
    let pair = Pair::new();
    install_sqlite_regexp(&pair.sqlite);
    pair.assert_parity("SELECT regexp('[0-9]+', 'abc123def')");
    pair.assert_parity("SELECT regexp('^foo$', 'foo')");
    pair.assert_parity("SELECT regexp('^foo$', 'foobar')");
}

// ---------------------------------------------------------------------------
// LIKE and ILIKE.
// ---------------------------------------------------------------------------

#[test]
fn parity_like_case_insensitive_ascii() {
    let pair = Pair::new();
    // SQLite's default LIKE is case-insensitive on ASCII letters.
    pair.assert_parity("SELECT 'Hello' LIKE 'hello'");
    pair.assert_parity("SELECT 'Hello' LIKE 'h%'");
}

#[test]
fn redline_ilike_matches_irrespective_of_case() {
    // SQLite proper does not parse `ILIKE`, so this assertion is
    // RedlineDB-only: we accept the Postgres ILIKE operator and treat it as
    // case-insensitive on the ASCII subset.
    let pair = Pair::new();
    let rows = pair.redline_rows("SELECT 'Hello' ILIKE 'hello'");
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
    let rows = pair.redline_rows("SELECT 'WORLD' ILIKE '%or%'");
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
    let rows = pair.redline_rows("SELECT 'xyz' ILIKE 'abc'");
    assert_eq!(rows, vec![vec![SqlValue::Integer(0)]]);
}

#[test]
fn redline_ilike_handles_unicode_case_folding() {
    let pair = Pair::new();
    let rows = pair.redline_rows("SELECT 'Äpfel' ILIKE 'ä%'");
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
}

#[test]
fn ilike_any_is_accepted() {
    // Track G/H landed ILIKE + the ARRAY[..] pre-parse rewrite.
    // `ILIKE ANY(ARRAY[..])` now parses + prepares successfully.
    let pair = Pair::new();
    pair.redline
        .prepare("SELECT 'x' ILIKE ANY (ARRAY['a','x'])")
        .expect("ILIKE ANY accepted");
}

// ---------------------------------------------------------------------------
// RETURNING. Covers INSERT and UPDATE forms; DELETE RETURNING is exercised
// indirectly via the same code path but added here as a third explicit case.
// ---------------------------------------------------------------------------

#[test]
fn parity_insert_returning_star() {
    let pair = Pair::new();
    pair.execute_both("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)");
    pair.assert_parity("INSERT INTO t(a, b) VALUES (1, 'one') RETURNING *");
}

#[test]
fn parity_update_returning_selected_columns() {
    let pair = Pair::new();
    pair.execute_both("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)");
    pair.execute_both("INSERT INTO t VALUES (1, 'one'), (2, 'two')");
    pair.assert_parity("UPDATE t SET b = 'TWO' WHERE a = 2 RETURNING a, b");
}

#[test]
fn parity_delete_returning_row() {
    let pair = Pair::new();
    pair.execute_both("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)");
    pair.execute_both("INSERT INTO t VALUES (1, 'one'), (2, 'two')");
    pair.assert_parity("DELETE FROM t WHERE a = 1 RETURNING a, b");
}
