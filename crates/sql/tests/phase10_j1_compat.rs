//! Lane J1 — SQLite JSON1 integration coverage.
//!
//! Deterministic JSON behavior is checked against the bundled rusqlite oracle.
//! The fuzz tests at the bottom are robustness checks for RedlineDB's SQL
//! surface; they are intentionally not the parity proof.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

#[path = "parity_oracle/harness.rs"]
mod harness;

fn assert_json_parity(sql: &str) {
    harness::assert_parity(sql);
}

fn assert_json_error_parity(sql: &str) {
    harness::check_parity(sql).expect("expected matching JSON error class");
}

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("phase10-j1.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

// ---------------------------------------------------------------------------
// json() — minify + validate
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_minifies_whitespace() {
    assert_json_parity(r#"SELECT json('  { "a" : 1 }  ')"#);
}

#[test]
fn phase10_j1_json_propagates_null() {
    assert_json_parity("SELECT json(NULL)");
}

#[test]
fn phase10_j1_json_rejects_malformed() {
    assert_json_error_parity("SELECT json('not-json')");
}

// ---------------------------------------------------------------------------
// json_array
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_array_builds_mixed_types() {
    assert_json_parity("SELECT json_array(1, 2.5, 'x', NULL)");
}

#[test]
fn phase10_j1_json_array_empty() {
    assert_json_parity("SELECT json_array()");
}

// ---------------------------------------------------------------------------
// json_array_length
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_array_length_counts() {
    assert_json_parity("SELECT json_array_length('[1,2,3,4]')");
}

#[test]
fn phase10_j1_json_array_length_with_path() {
    assert_json_parity(r#"SELECT json_array_length('{"a":[1,2,3]}', '$.a')"#);
}

#[test]
fn phase10_j1_json_array_length_non_array_returns_zero() {
    assert_json_parity(r#"SELECT json_array_length('{"a":1}')"#);
}

// ---------------------------------------------------------------------------
// json_object
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_object_builds_pairs() {
    assert_json_parity("SELECT json_object('a', 1, 'b', 'two')");
}

#[test]
fn phase10_j1_json_object_rejects_odd_args() {
    assert_json_error_parity("SELECT json_object('a')");
}

// ---------------------------------------------------------------------------
// json_extract
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_extract_single_path() {
    assert_json_parity(r#"SELECT json_extract('{"a":42}', '$.a')"#);
    assert_json_parity(r#"SELECT json_extract('{"a":"hi"}', '$.a')"#);
}

#[test]
fn phase10_j1_json_extract_missing_returns_null() {
    assert_json_parity(r#"SELECT json_extract('{"a":1}', '$.b')"#);
}

#[test]
fn phase10_j1_json_extract_multi_path_returns_array() {
    assert_json_parity(r#"SELECT json_extract('{"a":1,"b":2}', '$.a', '$.b', '$.c')"#);
}

#[test]
fn phase10_j1_json_extract_object_path_returns_json() {
    assert_json_parity(r#"SELECT json_extract('{"a":[1,2]}', '$.a')"#);
}

// ---------------------------------------------------------------------------
// json_set / json_insert / json_replace
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_set_overwrites_and_creates() {
    assert_json_parity(r#"SELECT json_set('{"a":1}', '$.a', 9, '$.b', 2)"#);
}

#[test]
fn phase10_j1_json_set_null_doc_propagates() {
    assert_json_parity("SELECT json_set(NULL, '$.a', 1)");
}

#[test]
fn phase10_j1_json_insert_only_creates_missing() {
    assert_json_parity(r#"SELECT json_insert('{"a":1}', '$.a', 9, '$.b', 2)"#);
}

#[test]
fn phase10_j1_json_replace_only_overwrites_existing() {
    assert_json_parity(r#"SELECT json_replace('{"a":1}', '$.a', 9, '$.b', 2)"#);
}

// ---------------------------------------------------------------------------
// json_remove
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_remove_drops_members() {
    assert_json_parity(r#"SELECT json_remove('{"a":1,"b":2}', '$.a')"#);
}

#[test]
fn phase10_j1_json_remove_missing_path_is_noop() {
    assert_json_parity(r#"SELECT json_remove('{"a":1}', '$.missing')"#);
}

#[test]
fn phase10_j1_json_remove_array_index() {
    assert_json_parity("SELECT json_remove('[1,2,3,4]', '$[1]')");
}

// ---------------------------------------------------------------------------
// json_patch (RFC 7396)
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_patch_merges_and_deletes() {
    assert_json_parity(r#"SELECT json_patch('{"a":1,"b":2}', '{"b":null,"c":3}')"#);
}

#[test]
fn phase10_j1_json_patch_recursive() {
    assert_json_parity(r#"SELECT json_patch('{"a":{"x":1,"y":2}}', '{"a":{"y":null,"z":9}}')"#);
}

// ---------------------------------------------------------------------------
// json_type
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_type_named() {
    for doc in [
        "'null'", "'true'", "'false'", "'1'", "'3.14'", r#"'"x"'"#, "'[1]'", "'{}'",
    ] {
        assert_json_parity(&format!("SELECT json_type({doc})"));
    }
}

#[test]
fn phase10_j1_json_type_with_path() {
    assert_json_parity(r#"SELECT json_type('{"a":[1]}', '$.a')"#);
    assert_json_parity(r#"SELECT json_type('{"a":1}', '$.missing')"#);
}

// ---------------------------------------------------------------------------
// json_valid
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_valid_recognizes_input() {
    assert_json_parity("SELECT json_valid('[1,2,3]')");
    assert_json_parity("SELECT json_valid('not-json')");
}

#[test]
fn phase10_j1_json_valid_null_is_null() {
    assert_json_parity("SELECT json_valid(NULL)");
}

// ---------------------------------------------------------------------------
// json_quote
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_json_quote_text_and_numbers() {
    assert_json_parity("SELECT json_quote('hi')");
    assert_json_parity("SELECT json_quote(42)");
}

#[test]
fn phase10_j1_json_quote_null_returns_null_literal() {
    assert_json_parity("SELECT json_quote(NULL)");
}

// ---------------------------------------------------------------------------
// -> and ->> operators
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_arrow_returns_json() {
    assert_json_parity(r#"SELECT '{"a":[1,2]}' -> '$.a'"#);
}

#[test]
fn phase10_j1_long_arrow_returns_sql_value() {
    assert_json_parity(r#"SELECT '{"a":42}' ->> '$.a'"#);
}

#[test]
fn phase10_j1_arrow_shorthand_path() {
    assert_json_parity(r#"SELECT '{"a":7}' ->> 'a'"#);
}

// ---------------------------------------------------------------------------
// Path parser surface (root, dotted, indexed, unicode, malformed)
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_path_root_returns_whole_doc() {
    assert_json_parity("SELECT json_extract('[1,2]', '$')");
}

#[test]
fn phase10_j1_path_unicode_key() {
    assert_json_parity(r#"SELECT json_extract('{"café":1}', '$.café')"#);
}

#[test]
fn phase10_j1_path_quoted_member_with_dot() {
    assert_json_parity(r#"SELECT json_extract('{"a.b":5}', '$."a.b"')"#);
}

#[test]
fn phase10_j1_path_malformed_errors() {
    assert_json_error_parity("SELECT json_extract('{}', 'no-dollar')");
}

#[test]
fn phase10_j1_path_nested_array_object() {
    assert_json_parity(r#"SELECT json_extract('{"x":[{"y":99}]}', '$.x[0].y')"#);
}

#[test]
fn phase10_j1_path_append_token_extends_array() {
    assert_json_parity("SELECT json_set('[1,2]', '$[#]', 3)");
}

// ---------------------------------------------------------------------------
// json_extract returns SQL NULL for json 'null' literal
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_extract_json_null_literal_returns_sql_null() {
    assert_json_parity("SELECT json_extract('null', '$')");
}

// ---------------------------------------------------------------------------
// Round-trip with stored TEXT
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_round_trip_through_table() {
    assert_json_parity(
        r#"CREATE TABLE docs(id INTEGER PRIMARY KEY, body TEXT);
           INSERT INTO docs(body) VALUES ('{"a":[10,20,30]}');
           SELECT json_extract(body, '$.a[2]'), json_array_length(body, '$.a') FROM docs"#,
    );
}

// ---------------------------------------------------------------------------
// Fuzz harness — robustness only. These tests assert no panics and local
// `json_valid` agreement with serde_json on the same input.
// ---------------------------------------------------------------------------

#[test]
fn phase10_j1_fuzz_validity_matches_serde_json() {
    let (_d, c) = open();

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
            let pool = b"{}[]\"',:0123456789truefalsenull. -_abcXYZ\\";
            let b = pool[(next() as usize) % pool.len()];
            s.push(b as char);
        }
        let sql_safe = s.replace('\'', "''");

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
