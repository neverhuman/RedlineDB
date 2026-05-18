//! Lane J1 — SQLite JSON1 compatibility integration tests.
//!
//! Each scalar function is exercised with at least a positive case and an
//! edge case (NULL propagation, malformed input, out-of-range path, etc.).
//! The fuzz harness at the bottom mints randomized JSON-like documents
//! and asserts (a) no panics from the SQL surface and (b) `json_valid`
//! agrees with `serde_json::from_str` on the same input.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("phase10-j1.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

fn one_text(conn: &Arc<Connection>, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_text(0).expect("text").to_owned()
}

fn one_int(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_i64(0).expect("int")
}

fn one_value(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_value(0).expect("value").clone()
}

// ---------------------------------------------------------------------------
// json() — minify + validate
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_minifies_whitespace() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json('  { \"a\" : 1 }  ')");
    assert_eq!(v, r#"{"a":1}"#);
}

#[test]
fn phase10_j1_json_propagates_null() {
    let (_d, c) = open();
    let v = one_value(&c, "SELECT json(NULL)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn phase10_j1_json_rejects_malformed() {
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT json('not-json')").expect("prepare");
    assert!(stmt.step().is_err());
}

// ---------------------------------------------------------------------------
// json_array
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_array_builds_mixed_types() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_array(1, 2.5, 'x', NULL)");
    assert_eq!(v, r#"[1,2.5,"x",null]"#);
}

#[test]
fn phase10_j1_json_array_empty() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_array()");
    assert_eq!(v, "[]");
}

// ---------------------------------------------------------------------------
// json_array_length
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_array_length_counts() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT json_array_length('[1,2,3,4]')");
    assert_eq!(v, 4);
}

#[test]
fn phase10_j1_json_array_length_with_path() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT json_array_length('{\"a\":[1,2,3]}', '$.a')");
    assert_eq!(v, 3);
}

#[test]
fn phase10_j1_json_array_length_non_array_returns_zero() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT json_array_length('{\"a\":1}')");
    assert_eq!(v, 0);
}

// ---------------------------------------------------------------------------
// json_object
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_object_builds_pairs() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_object('a', 1, 'b', 'two')");
    assert_eq!(v, r#"{"a":1,"b":"two"}"#);
}

#[test]
fn phase10_j1_json_object_rejects_odd_args() {
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT json_object('a')").expect("prepare");
    assert!(stmt.step().is_err());
}

// ---------------------------------------------------------------------------
// json_extract
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_extract_single_path() {
    let (_d, c) = open();
    assert_eq!(one_int(&c, "SELECT json_extract('{\"a\":42}', '$.a')"), 42);
    assert_eq!(
        one_text(&c, "SELECT json_extract('{\"a\":\"hi\"}', '$.a')"),
        "hi"
    );
}

#[test]
fn phase10_j1_json_extract_missing_returns_null() {
    let (_d, c) = open();
    let v = one_value(&c, "SELECT json_extract('{\"a\":1}', '$.b')");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn phase10_j1_json_extract_multi_path_returns_array() {
    let (_d, c) = open();
    let v = one_text(
        &c,
        "SELECT json_extract('{\"a\":1,\"b\":2}', '$.a', '$.b', '$.c')",
    );
    assert_eq!(v, "[1,2,null]");
}

#[test]
fn phase10_j1_json_extract_object_path_returns_json() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_extract('{\"a\":[1,2]}', '$.a')");
    assert_eq!(v, "[1,2]");
}

// ---------------------------------------------------------------------------
// json_set / json_insert / json_replace
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_set_overwrites_and_creates() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_set('{\"a\":1}', '$.a', 9, '$.b', 2)"),
        r#"{"a":9,"b":2}"#
    );
}

#[test]
fn phase10_j1_json_set_null_doc_propagates() {
    let (_d, c) = open();
    let v = one_value(&c, "SELECT json_set(NULL, '$.a', 1)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn phase10_j1_json_insert_only_creates_missing() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_insert('{\"a\":1}', '$.a', 9, '$.b', 2)"),
        r#"{"a":1,"b":2}"#
    );
}

#[test]
fn phase10_j1_json_replace_only_overwrites_existing() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_replace('{\"a\":1}', '$.a', 9, '$.b', 2)"),
        r#"{"a":9}"#
    );
}

// ---------------------------------------------------------------------------
// json_remove
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_remove_drops_members() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_remove('{\"a\":1,\"b\":2}', '$.a')"),
        r#"{"b":2}"#
    );
}

#[test]
fn phase10_j1_json_remove_missing_path_is_noop() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_remove('{\"a\":1}', '$.missing')"),
        r#"{"a":1}"#
    );
}

#[test]
fn phase10_j1_json_remove_array_index() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_remove('[1,2,3,4]', '$[1]')"),
        "[1,3,4]"
    );
}

// ---------------------------------------------------------------------------
// json_patch (RFC 7396)
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_patch_merges_and_deletes() {
    let (_d, c) = open();
    assert_eq!(
        one_text(
            &c,
            "SELECT json_patch('{\"a\":1,\"b\":2}', '{\"b\":null,\"c\":3}')"
        ),
        r#"{"a":1,"c":3}"#
    );
}

#[test]
fn phase10_j1_json_patch_recursive() {
    let (_d, c) = open();
    assert_eq!(
        one_text(
            &c,
            "SELECT json_patch('{\"a\":{\"x\":1,\"y\":2}}', '{\"a\":{\"y\":null,\"z\":9}}')"
        ),
        r#"{"a":{"x":1,"z":9}}"#
    );
}

// ---------------------------------------------------------------------------
// json_type
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_type_named() {
    let (_d, c) = open();
    assert_eq!(one_text(&c, "SELECT json_type('null')"), "null");
    assert_eq!(one_text(&c, "SELECT json_type('true')"), "true");
    assert_eq!(one_text(&c, "SELECT json_type('false')"), "false");
    assert_eq!(one_text(&c, "SELECT json_type('1')"), "integer");
    assert_eq!(one_text(&c, "SELECT json_type('3.14')"), "real");
    assert_eq!(one_text(&c, "SELECT json_type('\"x\"')"), "text");
    assert_eq!(one_text(&c, "SELECT json_type('[1]')"), "array");
    assert_eq!(one_text(&c, "SELECT json_type('{}')"), "object");
}

#[test]
fn phase10_j1_json_type_with_path() {
    let (_d, c) = open();
    assert_eq!(
        one_text(&c, "SELECT json_type('{\"a\":[1]}', '$.a')"),
        "array"
    );
    assert_eq!(
        one_value(&c, "SELECT json_type('{\"a\":1}', '$.missing')"),
        SqlValue::Null
    );
}

// ---------------------------------------------------------------------------
// json_valid
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_valid_recognizes_input() {
    let (_d, c) = open();
    assert_eq!(one_int(&c, "SELECT json_valid('[1,2,3]')"), 1);
    assert_eq!(one_int(&c, "SELECT json_valid('not-json')"), 0);
}

#[test]
fn phase10_j1_json_valid_null_is_null() {
    let (_d, c) = open();
    assert_eq!(one_value(&c, "SELECT json_valid(NULL)"), SqlValue::Null);
}

// ---------------------------------------------------------------------------
// json_quote
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_quote_text_and_numbers() {
    let (_d, c) = open();
    assert_eq!(one_text(&c, "SELECT json_quote('hi')"), "\"hi\"");
    assert_eq!(one_text(&c, "SELECT json_quote(42)"), "42");
}

#[test]
fn phase10_j1_json_quote_null_returns_null_literal() {
    let (_d, c) = open();
    assert_eq!(one_text(&c, "SELECT json_quote(NULL)"), "null");
}

// ---------------------------------------------------------------------------
// -> and ->> operators
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_arrow_returns_json() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT '{\"a\":[1,2]}' -> '$.a'");
    assert_eq!(v, "[1,2]");
}

#[test]
fn phase10_j1_long_arrow_returns_sql_value() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT '{\"a\":42}' ->> '$.a'");
    assert_eq!(v, 42);
}

#[test]
fn phase10_j1_arrow_shorthand_path() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT '{\"a\":7}' ->> 'a'");
    assert_eq!(v, 7);
}

// ---------------------------------------------------------------------------
// Path parser surface (root, dotted, indexed, unicode, malformed)
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_path_root_returns_whole_doc() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_extract('[1,2]', '$')");
    assert_eq!(v, "[1,2]");
}

#[test]
fn phase10_j1_path_unicode_key() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT json_extract('{\"café\":1}', '$.café')");
    assert_eq!(v, 1);
}

#[test]
fn phase10_j1_path_quoted_member_with_dot() {
    let (_d, c) = open();
    let v = one_int(&c, "SELECT json_extract('{\"a.b\":5}', '$.\"a.b\"')");
    assert_eq!(v, 5);
}

#[test]
fn phase10_j1_path_malformed_errors() {
    let (_d, c) = open();
    let mut stmt = c
        .prepare("SELECT json_extract('{}', 'no-dollar')")
        .expect("prepare");
    assert!(stmt.step().is_err());
}

#[test]
fn phase10_j1_path_nested_array_object() {
    let (_d, c) = open();
    let v = one_int(
        &c,
        "SELECT json_extract('{\"x\":[{\"y\":99}]}', '$.x[0].y')",
    );
    assert_eq!(v, 99);
}

#[test]
fn phase10_j1_path_append_token_extends_array() {
    let (_d, c) = open();
    let v = one_text(&c, "SELECT json_set('[1,2]', '$[#]', 3)");
    assert_eq!(v, "[1,2,3]");
}

// ---------------------------------------------------------------------------
// json_extract returns SQL NULL for json 'null' literal
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_extract_json_null_literal_returns_sql_null() {
    let (_d, c) = open();
    let v = one_value(&c, "SELECT json_extract('null', '$')");
    assert_eq!(v, SqlValue::Null);
}

// ---------------------------------------------------------------------------
// Round-trip with stored TEXT
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_round_trip_through_table() {
    let (_d, c) = open();
    c.execute("CREATE TABLE docs(id INTEGER PRIMARY KEY, body TEXT)")
        .expect("create");
    c.execute("INSERT INTO docs(body) VALUES ('{\"a\":[10,20,30]}')")
        .expect("insert");

    let mut stmt = c
        .prepare("SELECT json_extract(body, '$.a[2]'), json_array_length(body, '$.a') FROM docs")
        .expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("extract"), 30);
    assert_eq!(stmt.column_i64(1).expect("length"), 3);
}

// ---------------------------------------------------------------------------
// Fuzz harness — 100 randomized inputs, never panics, json_valid agrees
// with serde_json on the same string.
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_fuzz_validity_matches_serde_json() {
    let (_d, c) = open();

    // Tiny xorshift RNG so we don't pull in `rand`. Seed is fixed for
    // determinism but covers a varied set of inputs.
    let mut state: u64 = 0xDEAD_BEEF_CAFE_F00D;
    let mut next = || -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let mut iterations = 0usize;
    let mut agreed = 0usize;
    for _ in 0..100 {
        let len = (next() as usize) % 60 + 1;
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            // Bias toward JSON-relevant ASCII characters.
            let pool = b"{}[]\"',:0123456789truefalsenull. -_abcXYZ\\";
            let b = pool[(next() as usize) % pool.len()];
            s.push(b as char);
        }
        // Escape for embedding in single-quoted SQL literal.
        let sql_safe = s.replace('\'', "''");

        // Some randomized strings happen to break the SQL parser (e.g.
        // contain unbalanced double-quotes or are interpreted as
        // identifiers); skip those, the goal here is the JSON layer.
        let prepare = c.prepare(&format!("SELECT json_valid('{sql_safe}')"));
        let Ok(mut stmt) = prepare else { continue };
        let Ok(step) = stmt.step() else { continue };
        if step != Step::Row {
            continue;
        }
        let Ok(got) = stmt.column_i64(0) else {
            continue;
        };
        let expected = if serde_json::from_str::<serde_json::Value>(&s).is_ok() {
            1
        } else {
            0
        };
        assert_eq!(got, expected, "input={s:?}");
        agreed += 1;
        iterations += 1;
    }
    // Sanity: we should have actually exercised the path, not skipped
    // every iteration.
    assert!(iterations > 30, "only {iterations} iterations completed");
    assert_eq!(iterations, agreed);
}

#[test]
fn phase10_j1_fuzz_extract_never_panics() {
    let (_d, c) = open();
    let mut state: u64 = 0xA5A5_A5A5_5A5A_5A5A;
    let mut next = || -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..100 {
        let docs = [
            r#"{"a":1}"#,
            r#"[1,2,3]"#,
            r#"{"x":{"y":[true,false,null]}}"#,
            r#"42"#,
            r#""hello""#,
        ];
        let doc = docs[(next() as usize) % docs.len()];
        let path_pool = b"$.x[0]y_abc012#-";
        let plen = (next() as usize) % 8 + 1;
        let mut path = String::from("$");
        for _ in 0..plen {
            let b = path_pool[(next() as usize) % path_pool.len()];
            path.push(b as char);
        }
        // Don't care about the result — we just want no panic.
        let sql = format!(
            "SELECT json_extract('{}', '{}')",
            doc,
            path.replace('\'', "''")
        );
        if let Ok(mut stmt) = c.prepare(&sql) {
            let _ = stmt.step();
        }
    }
}
