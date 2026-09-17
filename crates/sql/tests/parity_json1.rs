//! Differential parity tests for SQLite JSON1 scalar functions.
//!
//! Companion to `phase10_j1_compat.rs`, which exercises each JSON1
//! function against itself only. This file runs the same SQL against both
//! engines and asserts the produced rows agree value-for-value.
//!
//! Known divergences keep the `#[ignore = "..."]` attribute so they show
//! up in `cargo test -- --ignored` and don't silently disappear. Each
//! ignore line carries a short tracked-divergence note so we can revisit.

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
        let path = dir.path().join("json1.db");
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let redline = db.connect();
        let sqlite = rusqlite::Connection::open_in_memory().expect("rusqlite open");
        Pair {
            _dir: dir,
            redline,
            sqlite,
        }
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

    fn execute(&self, sql: &str) {
        self.redline.execute(sql).expect("redline execute");
        self.sqlite.execute_batch(sql).expect("sqlite execute");
    }
}

// ---------------------------------------------------------------------------
// json()
// ---------------------------------------------------------------------------

#[test]
fn parity_json_minify_simple_object() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json('  { "a" : 1 }  ')"#);
}

#[test]
fn parity_json_preserves_unicode() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json('{"name":"café"}')"#);
}

#[test]
fn parity_json_null_input() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json(NULL)");
}

// ---------------------------------------------------------------------------
// json_array / json_array_length
// ---------------------------------------------------------------------------

#[test]
fn parity_json_array_mixed_types() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_array(1, 2.5, 'x', NULL)");
}

#[test]
fn parity_json_array_empty() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_array()");
}

#[test]
fn parity_json_array_length_top_level() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_array_length('[1,2,3,4]')"#);
}

#[test]
fn parity_json_array_length_with_path() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_array_length('{"a":[1,2,3]}', '$.a')"#);
}

#[test]
fn parity_json_array_length_non_array_returns_zero() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_array_length('{"a":1}')"#);
}

// ---------------------------------------------------------------------------
// json_object
// ---------------------------------------------------------------------------

#[test]
fn parity_json_object_simple() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_object('a', 1, 'b', 'two')");
}

#[test]
fn parity_json_object_empty() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_object()");
}

// ---------------------------------------------------------------------------
// json_extract
// ---------------------------------------------------------------------------

#[test]
fn parity_json_extract_root() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_extract('{"a":1,"b":2}', '$')"#);
}

#[test]
fn parity_json_extract_object_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_extract('{"a":1,"b":2}', '$.a')"#);
}

#[test]
fn parity_json_extract_array_index() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_extract('[10,20,30]', '$[1]')"#);
}

#[test]
fn parity_json_extract_missing_path_is_null() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_extract('{"a":1}', '$.missing')"#);
}

// ---------------------------------------------------------------------------
// json_type
// ---------------------------------------------------------------------------

#[test]
fn parity_json_type_each_kind() {
    let pair = Pair::new();
    for (label, doc, path) in [
        ("object", r#"'{"x":1}'"#, "'$'"),
        ("array", "'[1,2]'", "'$'"),
        ("integer", "'1'", "'$'"),
        ("real", "'1.5'", "'$'"),
        ("text", r#"'"hi"'"#, "'$'"),
        ("null", "'null'", "'$'"),
        ("true", "'true'", "'$'"),
        ("false", "'false'", "'$'"),
    ] {
        let sql = format!("SELECT json_type({doc}, {path}) -- {label}");
        pair.assert_parity(&sql);
    }
}

// ---------------------------------------------------------------------------
// json_valid
// ---------------------------------------------------------------------------

#[test]
fn parity_json_valid_well_formed() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_valid('{"a":1}')"#);
}

#[test]
fn parity_json_valid_malformed() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_valid('not-json')");
}

#[test]
fn parity_json_valid_null() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_valid(NULL)");
}

#[test]
fn parity_json_extract_set_official_shape() {
    let pair = Pair::new();
    pair.execute("CREATE TABLE docs(id INT PRIMARY KEY, doc TEXT)");
    pair.execute("INSERT INTO docs VALUES (1, json_object('a',40,'b',json_array(1,2,3)))");
    pair.execute(
        "INSERT INTO docs VALUES (2, json_set(json_object('a',0), '$.a', 41, '$.c', 'x'))",
    );
    pair.assert_parity(
        "SELECT id, json_extract(doc,'$.a'), json_type(doc,'$.b'), json_valid(doc) \
         FROM docs ORDER BY id",
    );
    pair.assert_parity("SELECT json_array_length(json_extract(doc,'$.b')) FROM docs WHERE id=1");
}

// ---------------------------------------------------------------------------
// json_quote
// ---------------------------------------------------------------------------

#[test]
fn parity_json_quote_text() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_quote('hello \"world\"')");
}

#[test]
fn parity_json_quote_integer() {
    let pair = Pair::new();
    pair.assert_parity("SELECT json_quote(42)");
}

#[test]
fn parity_json_quote_null_returns_json_null() {
    let pair = Pair::new();
    // SQLite's json_quote(NULL) returns the literal text "null" (the JSON
    // null), not SQL NULL. Asserting parity covers that surface.
    pair.assert_parity("SELECT json_quote(NULL)");
}

// ---------------------------------------------------------------------------
// json_set / json_insert / json_replace / json_remove
// ---------------------------------------------------------------------------

#[test]
fn parity_json_set_overwrites_existing_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_set('{"a":1}', '$.a', 9)"#);
}

#[test]
fn parity_json_set_creates_missing_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_set('{"a":1}', '$.b', 2)"#);
}

#[test]
fn parity_json_insert_skips_existing_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_insert('{"a":1}', '$.a', 99)"#);
}

#[test]
fn parity_json_replace_skips_missing_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_replace('{"a":1}', '$.b', 2)"#);
}

#[test]
fn parity_json_remove_deletes_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_remove('{"a":1,"b":2}', '$.b')"#);
}

#[test]
fn parity_json_remove_missing_key_is_noop() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_remove('{"a":1}', '$.missing')"#);
}

// ---------------------------------------------------------------------------
// json_patch
// ---------------------------------------------------------------------------

#[test]
fn parity_json_patch_merges_objects() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_patch('{"a":1,"b":2}', '{"b":9,"c":3}')"#);
}

#[test]
fn parity_json_patch_null_removes_key() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT json_patch('{"a":1,"b":2}', '{"b":null}')"#);
}

// ---------------------------------------------------------------------------
// Arrow operators: -> returns JSON, ->> returns SQL primitives.
// ---------------------------------------------------------------------------

#[test]
fn parity_arrow_extracts_json_subtree() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT '{"a":[1,2]}' -> '$.a'"#);
}

#[test]
fn parity_arrow2_extracts_scalar_text() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT '{"a":"hello"}' ->> '$.a'"#);
}

#[test]
fn parity_arrow2_extracts_scalar_integer() {
    let pair = Pair::new();
    pair.assert_parity(r#"SELECT '{"a":42}' ->> '$.a'"#);
}
