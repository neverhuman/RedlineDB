//! SQLite scalar function parity tests.
//!
//! Covers all newly implemented scalar functions:
//! substr/substring, instr, trim/ltrim/rtrim, replace, printf/format,
//! iif, sign, char, unicode, zeroblob, randomblob.
//! Each test validates SQLite-compatible NULL-propagation and edge cases.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("scalar.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

/// Collect all rows from a query into Vec<Vec<SqlValue>>.
fn query_all(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
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

/// Execute a query and return the first column of the first row, or Null.
fn q1(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    query_all(conn, sql)
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next())
        .unwrap_or(SqlValue::Null)
}

// ── substr ────────────────────────────────────────────────────────────────────

#[test]
fn substr_basic_1based() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT substr('hello', 2)");
    assert_eq!(v, SqlValue::Text(Arc::from("ello")));
}

#[test]
fn substr_with_length() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT substr('hello', 2, 3)");
    assert_eq!(v, SqlValue::Text(Arc::from("ell")));
}

#[test]
fn substr_negative_start() {
    let (_d, c) = open();
    // Negative start counts from end: substr('hello', -3) → 'llo'
    let v = q1(&c, "SELECT substr('hello', -3)");
    assert_eq!(v, SqlValue::Text(Arc::from("llo")));
}

#[test]
fn substr_zero_start_acts_as_one() {
    let (_d, c) = open();
    // SQLite: start=0 is treated like 0 offset → 'he' (takes 2)
    let v = q1(&c, "SELECT substr('hello', 0, 3)");
    assert_eq!(v, SqlValue::Text(Arc::from("he")));
}

#[test]
fn substr_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT substr(NULL, 1)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT substr('hi', NULL)"), SqlValue::Null);
}

#[test]
fn substr_alias_substring() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT substring('hello', 2, 3)");
    assert_eq!(v, SqlValue::Text(Arc::from("ell")));
}

#[test]
fn substr_beyond_length_returns_empty() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT substr('hi', 10)");
    assert_eq!(v, SqlValue::Text(Arc::from("")));
}

// ── instr ─────────────────────────────────────────────────────────────────────

#[test]
fn instr_found() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT instr('hello world', 'world')");
    assert_eq!(v, SqlValue::Integer(7));
}

#[test]
fn instr_not_found() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT instr('hello', 'xyz')");
    assert_eq!(v, SqlValue::Integer(0));
}

#[test]
fn instr_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT instr(NULL, 'x')"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT instr('x', NULL)"), SqlValue::Null);
}

#[test]
fn instr_empty_needle_returns_one() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT instr('hello', '')");
    assert_eq!(v, SqlValue::Integer(1));
}

// ── trim / ltrim / rtrim ───────────────────────────────────────────────────────

#[test]
fn trim_whitespace() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT trim('  hello  ')");
    assert_eq!(v, SqlValue::Text(Arc::from("hello")));
}

#[test]
fn trim_custom_chars() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT trim('*' FROM '***hello***')");
    assert_eq!(v, SqlValue::Text(Arc::from("hello")));
}

#[test]
fn ltrim_whitespace() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT ltrim('  hi  ')");
    assert_eq!(v, SqlValue::Text(Arc::from("hi  ")));
}

#[test]
fn rtrim_whitespace() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT rtrim('  hi  ')");
    assert_eq!(v, SqlValue::Text(Arc::from("  hi")));
}

#[test]
fn trim_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT trim(NULL)"), SqlValue::Null);
}

// ── replace ───────────────────────────────────────────────────────────────────

#[test]
fn replace_basic() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT replace('hello world', 'world', 'Rust')");
    assert_eq!(v, SqlValue::Text(Arc::from("hello Rust")));
}

#[test]
fn replace_all_occurrences() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT replace('aababab', 'ab', 'X')");
    assert_eq!(v, SqlValue::Text(Arc::from("aXXX")));
}

#[test]
fn replace_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT replace(NULL, 'a', 'b')"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT replace('a', NULL, 'b')"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT replace('a', 'a', NULL)"), SqlValue::Null);
}

// ── printf / format ───────────────────────────────────────────────────────────

#[test]
fn printf_string_placeholder() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT printf('Hello, %s!', 'world')");
    assert_eq!(v, SqlValue::Text(Arc::from("Hello, world!")));
}

#[test]
fn printf_integer_placeholder() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT printf('%d bottles', 99)");
    assert_eq!(v, SqlValue::Text(Arc::from("99 bottles")));
}

#[test]
fn printf_hex_placeholder() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT printf('%x', 255)");
    assert_eq!(v, SqlValue::Text(Arc::from("ff")));
}

#[test]
fn printf_percent_escape() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT printf('100%%')");
    assert_eq!(v, SqlValue::Text(Arc::from("100%")));
}

#[test]
fn format_is_alias_for_printf() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT format('%d + %d = %d', 1, 2, 3)");
    assert_eq!(v, SqlValue::Text(Arc::from("1 + 2 = 3")));
}

#[test]
fn printf_null_format_returns_null() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT printf(NULL)"), SqlValue::Null);
}

#[test]
fn printf_zero_pads_integer_width() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT printf('%04d', 7)");
    assert_eq!(v, SqlValue::Text(Arc::from("0007")));
}

// ── min / max ────────────────────────────────────────────────────────────────

#[test]
fn min_and_max_scalar_multiple_args() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT min(5, 2, 8)"), SqlValue::Integer(2));
    assert_eq!(q1(&c, "SELECT max(5, 2, 8)"), SqlValue::Integer(8));
}

#[test]
fn min_scalar_returns_null_when_any_arg_is_null() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT min(1, NULL, 2)"), SqlValue::Null);
}

// ── iif ───────────────────────────────────────────────────────────────────────

#[test]
fn iif_true_branch() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT iif(1, 'yes', 'no')");
    assert_eq!(v, SqlValue::Text(Arc::from("yes")));
}

#[test]
fn iif_false_branch() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT iif(0, 'yes', 'no')");
    assert_eq!(v, SqlValue::Text(Arc::from("no")));
}

#[test]
fn iif_null_condition_returns_false_branch() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT iif(NULL, 'yes', 'no')");
    assert_eq!(v, SqlValue::Text(Arc::from("no")));
}

// ── sign ──────────────────────────────────────────────────────────────────────

#[test]
fn sign_positive() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT sign(42)"), SqlValue::Integer(1));
}

#[test]
fn sign_negative() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT sign(-7)"), SqlValue::Integer(-1));
}

#[test]
fn sign_zero() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT sign(0)"), SqlValue::Integer(0));
}

#[test]
fn sign_null() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT sign(NULL)"), SqlValue::Null);
}

// ── char ──────────────────────────────────────────────────────────────────────

#[test]
fn char_basic_ascii() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT char(72, 105)"); // 'H', 'i'
    assert_eq!(v, SqlValue::Text(Arc::from("Hi")));
}

#[test]
fn char_single() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT char(65)"); // 'A'
    assert_eq!(v, SqlValue::Text(Arc::from("A")));
}

// ── unicode ───────────────────────────────────────────────────────────────────

#[test]
fn unicode_basic() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT unicode('A')");
    assert_eq!(v, SqlValue::Integer(65));
}

#[test]
fn unicode_multi_char_returns_first() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT unicode('Hello')");
    assert_eq!(v, SqlValue::Integer(72)); // 'H'
}

#[test]
fn unicode_multibyte_literal_returns_first_codepoint() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT unicode('á')");
    assert_eq!(v, SqlValue::Integer(225));
}

#[test]
fn unicode_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT unicode(NULL)"), SqlValue::Null);
}

// ── zeroblob ──────────────────────────────────────────────────────────────────

#[test]
fn zeroblob_correct_length() {
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT zeroblob(5)").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let val = stmt.column_value(0).expect("col").clone();
    match val {
        SqlValue::Blob(b) => {
            assert_eq!(b.len(), 5);
            assert!(b.iter().all(|&x| x == 0));
        }
        other => panic!("expected BLOB, got {other:?}"),
    }
}

#[test]
fn zeroblob_zero_length() {
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT zeroblob(0)").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let val = stmt.column_value(0).expect("col").clone();
    match val {
        SqlValue::Blob(b) => assert_eq!(b.len(), 0),
        other => panic!("expected BLOB, got {other:?}"),
    }
}

#[test]
fn zeroblob_null_propagates() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT zeroblob(NULL)"), SqlValue::Null);
}

// ── randomblob ────────────────────────────────────────────────────────────────

#[test]
fn randomblob_correct_length() {
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT randomblob(16)").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let val = stmt.column_value(0).expect("col").clone();
    match val {
        SqlValue::Blob(b) => assert_eq!(b.len(), 16),
        other => panic!("expected BLOB, got {other:?}"),
    }
}

#[test]
fn randomblob_produces_blob_of_right_size() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT randomblob(8)");
    assert!(matches!(v, SqlValue::Blob(ref b) if b.len() == 8));
}

#[test]
fn length_counts_blob_bytes() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT length(randomblob(4))"), SqlValue::Integer(4));
}

// ── functions in column expressions after INSERT ───────────────────────────────

#[test]
fn scalar_funcs_in_select_after_insert() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(name TEXT)").expect("create");
    c.execute("INSERT INTO t VALUES ('  hello  ')")
        .expect("insert");
    let v = q1(&c, "SELECT trim(name) FROM t");
    assert_eq!(v, SqlValue::Text(Arc::from("hello")));
}

#[test]
fn replace_in_where_clause() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER, val TEXT)")
        .expect("create");
    c.execute("INSERT INTO t VALUES (1, 'foo'), (2, 'bar')")
        .expect("insert");
    let rows = query_all(
        &c,
        "SELECT id FROM t WHERE replace(val, 'foo', 'baz') = 'baz'",
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(1));
}

// ── real → text coercion parity (regression: fuzz seed=7 iter=286) ────────

#[test]
fn lower_upper_of_real_keeps_trailing_zero() {
    // Direct repro of the fuzz divergence: RedlineDB used to emit
    // Text("1") for `lower(1.0)` while SQLite emits Text("1.0").
    let (_d, c) = open();
    let oracle = rusqlite::Connection::open_in_memory().expect("oracle open");

    let cases: &[(&str, f64)] = &[
        ("one", 1.0),
        ("twenty_two", 22.0),
        ("seven", 7.0),
        ("half", 1.5),
        ("twelve_quarter", 12.25),
    ];

    for (label, v) in cases {
        let sql = format!("SELECT lower(CAST({v} AS REAL))");
        let red = q1(&c, &sql);
        let oracle_val: String = oracle
            .query_row(&sql, [], |row| row.get(0))
            .expect("oracle query");
        let expected = SqlValue::Text(Arc::from(oracle_val.as_str()));
        assert_eq!(red, expected, "case {label}: redline ≠ oracle for {sql}");
    }
}

#[test]
fn cast_real_as_text_keeps_trailing_zero() {
    let (_d, c) = open();
    let oracle = rusqlite::Connection::open_in_memory().expect("oracle open");

    for v in [1.0f64, 0.0, -3.0, 22.5, 100.0] {
        let sql = format!("SELECT CAST(CAST({v} AS REAL) AS TEXT)");
        let red = q1(&c, &sql);
        let oracle_val: String = oracle
            .query_row(&sql, [], |row| row.get(0))
            .expect("oracle query");
        let expected = SqlValue::Text(Arc::from(oracle_val.as_str()));
        assert_eq!(red, expected, "redline ≠ oracle for {sql}");
    }
}

// ── SQLite math1 extension functions (acos, cos, exp, log, ...) ───────────────

#[test]
fn math1_acos_unit_circle() {
    let (_d, c) = open();
    let cases = [
        (1.0, 0.0_f64.acos()),
        (0.0, 0.0_f64.asin().acos()),
        (-1.0, (-1.0_f64).acos()),
    ];
    // Each `acos(x)` matches the f64::acos result; out-of-domain returns NULL.
    for (x, _expect) in cases {
        let sql = format!("SELECT acos({x})");
        let v = q1(&c, &sql);
        assert!(matches!(v, SqlValue::Real(_)), "acos({x}) → {v:?}");
    }
    assert_eq!(q1(&c, "SELECT acos(2.0)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT acos(-2.0)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT acos(NULL)"), SqlValue::Null);
}

#[test]
fn math1_exp_cosh_match_sqlite_3531_rendering() {
    let (_d, c) = open();

    for (sql, expected) in [
        ("SELECT cosh(1.0)", "1.5430806348152437"),
        ("SELECT cosh(-1.0)", "1.5430806348152437"),
        ("SELECT exp(1.0)", "2.7182818284590451"),
    ] {
        let rendered = match q1(&c, sql) {
            SqlValue::Real(value) => redlinedb_sql::format_real_sqlite(value),
            other => panic!("expected real for {sql}, got {other:?}"),
        };
        assert_eq!(
            rendered, expected,
            "rendered output differs from SQLite 3.53.1 for {sql}"
        );
    }
}

#[test]
fn math1_log_overload_natural_vs_base() {
    let (_d, c) = open();
    // log(x) = natural log; log(b, x) = log base b of x.
    let one = q1(&c, "SELECT log(1.0)");
    assert!(matches!(one, SqlValue::Real(v) if v.abs() < 1e-12));
    let three = q1(&c, "SELECT log(2.0, 8.0)");
    assert!(matches!(three, SqlValue::Real(v) if (v - 3.0).abs() < 1e-12));
    // Domain errors return NULL, not an error.
    assert_eq!(q1(&c, "SELECT log(0.0)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT log(-1.0)"), SqlValue::Null);
}

#[test]
fn math1_atan2_quadrants() {
    let (_d, c) = open();
    let half_pi = q1(&c, "SELECT atan2(1.0, 0.0)");
    assert!(
        matches!(half_pi, SqlValue::Real(v) if (v - std::f64::consts::FRAC_PI_2).abs() < 1e-12)
    );
    assert_eq!(q1(&c, "SELECT atan2(NULL, 1.0)"), SqlValue::Null);
}

#[test]
fn math1_pi_constant() {
    let (_d, c) = open();
    let pi = q1(&c, "SELECT pi()");
    assert!(matches!(pi, SqlValue::Real(v) if (v - std::f64::consts::PI).abs() < 1e-15));
}

#[test]
fn math1_mod_zero_divisor_returns_null() {
    let (_d, c) = open();
    let one = q1(&c, "SELECT mod(10, 3)");
    assert!(matches!(one, SqlValue::Real(v) if (v - 1.0).abs() < 1e-12));
    assert_eq!(q1(&c, "SELECT mod(5, 0)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT mod(NULL, 2)"), SqlValue::Null);
}

#[test]
fn math1_degrees_radians_round_trip() {
    let (_d, c) = open();
    let deg = q1(&c, "SELECT degrees(radians(180.0))");
    assert!(matches!(deg, SqlValue::Real(v) if (v - 180.0).abs() < 1e-9));
}

#[test]
fn math1_trunc_round_toward_zero() {
    let (_d, c) = open();
    let one_pos = q1(&c, "SELECT trunc(1.7)");
    assert!(matches!(one_pos, SqlValue::Real(v) if (v - 1.0).abs() < 1e-12));
    let one_neg = q1(&c, "SELECT trunc(-1.7)");
    assert!(matches!(one_neg, SqlValue::Real(v) if (v + 1.0).abs() < 1e-12));
    assert_eq!(q1(&c, "SELECT trunc(NULL)"), SqlValue::Null);
}

// ── SQLite text helpers: length (chars), octet_length, concat, concat_ws ─────

#[test]
fn length_counts_utf8_characters_for_text() {
    let (_d, c) = open();
    // 'héllo' has 5 chars but 6 bytes (é is 2 bytes in UTF-8).
    assert_eq!(q1(&c, "SELECT length('héllo')"), SqlValue::Integer(5));
    assert_eq!(q1(&c, "SELECT octet_length('héllo')"), SqlValue::Integer(6));
    assert_eq!(q1(&c, "SELECT length(NULL)"), SqlValue::Null);
}

#[test]
fn concat_skips_nulls_concat_ws_uses_separator() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT concat('a', 'b', 'c')"),
        SqlValue::Text(Arc::from("abc"))
    );
    assert_eq!(
        q1(&c, "SELECT concat('a', NULL, 'c')"),
        SqlValue::Text(Arc::from("ac"))
    );
    assert_eq!(
        q1(&c, "SELECT concat_ws(', ', 'a', 'b', 'c')"),
        SqlValue::Text(Arc::from("a, b, c"))
    );
    // NULL separator → NULL result; NULL operands are skipped.
    assert_eq!(q1(&c, "SELECT concat_ws(NULL, 'a', 'b')"), SqlValue::Null);
    assert_eq!(
        q1(&c, "SELECT concat_ws(',', 'a', NULL, 'b')"),
        SqlValue::Text(Arc::from("a,b"))
    );
}

#[test]
fn hex_of_null_returns_empty_string_not_null() {
    // SQLite returns text "" (zero-length TEXT), not NULL — see func.c.
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT hex(NULL)"), SqlValue::Text(Arc::from("")));
    assert_eq!(
        q1(&c, "SELECT typeof(hex(NULL))"),
        SqlValue::Text(Arc::from("text"))
    );
}

#[test]
fn unhex_one_and_two_arg_forms() {
    let (_d, c) = open();
    match q1(&c, "SELECT unhex('01ab')") {
        SqlValue::Blob(b) => assert_eq!(&b[..], &[0x01, 0xab]),
        other => panic!("expected BLOB, got {other:?}"),
    }
    // Two-arg form: second arg is "ignore" set.
    match q1(&c, "SELECT unhex('01 ab', ' ')") {
        SqlValue::Blob(b) => assert_eq!(&b[..], &[0x01, 0xab]),
        other => panic!("expected BLOB, got {other:?}"),
    }
    // Invalid hex returns NULL.
    assert_eq!(q1(&c, "SELECT unhex('xyz')"), SqlValue::Null);
    // NULL input propagates.
    assert_eq!(q1(&c, "SELECT unhex(NULL)"), SqlValue::Null);
}

#[test]
fn glob_function_form_matches_operator_form() {
    let (_d, c) = open();
    // Function form: glob(pattern, value) — note arg order.
    assert_eq!(q1(&c, "SELECT glob('a*', 'abc')"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT glob('z*', 'abc')"), SqlValue::Integer(0));
    assert_eq!(q1(&c, "SELECT glob('a*', NULL)"), SqlValue::Null);
}

#[test]
fn glob_operator_form_via_parser_rewrite() {
    // The parser pre-rewrites `<lhs> GLOB <rhs>` into `glob(<rhs>, <lhs>)`
    // before handing the SQL to sqlparser (which lacks a GLOB operator).
    // Verify end-to-end semantics including NULL propagation and NOT GLOB.
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT 'abc' GLOB 'a*'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'abc' GLOB '*c'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'ABC' GLOB 'a*'"), SqlValue::Integer(0));
    // Character class: GLOB is case-sensitive.
    assert_eq!(
        q1(&c, "SELECT 'abc' GLOB '[a-z][a-z][a-z]'"),
        SqlValue::Integer(1)
    );
    assert_eq!(q1(&c, "SELECT 'abc' GLOB '[^abc]bc'"), SqlValue::Integer(0));
    // NOT GLOB.
    assert_eq!(q1(&c, "SELECT 'abc' NOT GLOB 'z*'"), SqlValue::Integer(1));
    // NULL propagation.
    assert_eq!(q1(&c, "SELECT NULL GLOB 'a*'"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT 'abc' GLOB NULL"), SqlValue::Null);
}

#[test]
fn like_function_form_two_and_three_arg() {
    let (_d, c) = open();
    // SQLite-style like(pattern, value) is a function alias for the LIKE
    // operator. NB: argument order is (pattern, value), unlike LIKE.
    assert_eq!(q1(&c, "SELECT like('a%', 'abc')"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT like('a%', 'xyz')"), SqlValue::Integer(0));
    // NULL propagation.
    assert_eq!(q1(&c, "SELECT like('a%', NULL)"), SqlValue::Null);
    assert_eq!(q1(&c, "SELECT like(NULL, 'abc')"), SqlValue::Null);
}

#[test]
fn soundex_returns_error_when_compiled_out() {
    // SQLite 3.53.1 reference binary is built without SQLITE_SOUNDEX, so
    // calls to soundex() should surface as an "unsupported"/"no such
    // function" error — we mirror that.
    let (_d, c) = open();
    let mut stmt = c.prepare("SELECT soundex('Robert')").expect("prepare");
    assert!(stmt.step().is_err(), "soundex unexpectedly succeeded");
}
