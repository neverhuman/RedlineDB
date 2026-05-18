//! Phase-10 Lane SQL-A correctness regressions.
//!
//! Each test in this file pins down a specific SQLite-divergence wrong-result
//! bug from the audit. The expected behavior matches SQLite (3.x).

use std::sync::Arc;

use redlinedb_sql::{Database, DbOptions, SqlValue, Step};
use tempfile::tempdir;

fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-sql-phase10-sqla.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

#[allow(dead_code)]
fn run_scalar(sql: &str) -> SqlValue {
    let (_dir, conn) = open_database();
    let mut stmt = conn.prepare(sql).expect("prepare scalar");
    assert_eq!(stmt.step().expect("step scalar"), Step::Row);
    stmt.column_value(0).expect("scalar value").clone()
}

#[allow(dead_code)]
fn assert_null(value: &SqlValue) {
    assert!(
        matches!(value, SqlValue::Null),
        "expected NULL, got {:?}",
        value
    );
}

#[allow(dead_code)]
fn assert_int(value: &SqlValue, expected: i64) {
    match value {
        SqlValue::Integer(v) => assert_eq!(*v, expected, "integer value mismatch"),
        other => panic!("expected Integer({expected}), got {other:?}"),
    }
}

#[allow(dead_code)]
fn assert_real(value: &SqlValue, expected: f64) {
    match value {
        SqlValue::Real(v) => {
            assert!(
                (v - expected).abs() < 1e-9,
                "real value mismatch: got {v}, expected {expected}"
            );
        }
        other => panic!("expected Real({expected}), got {other:?}"),
    }
}

#[allow(dead_code)]
fn assert_text(value: &SqlValue, expected: &str) {
    match value {
        SqlValue::Text(v) => assert_eq!(v.as_ref(), expected, "text value mismatch"),
        other => panic!("expected Text({expected:?}), got {other:?}"),
    }
}

mod phase10_sqla_correctness {
    use super::*;

    // --- Bug 1: SELECT ALL must preserve duplicates --------------------------

    #[test]
    fn select_all_preserves_duplicates_basic() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(x INTEGER)").expect("create t");
        conn.execute("INSERT INTO t VALUES (1)").expect("insert 1");
        conn.execute("INSERT INTO t VALUES (1)")
            .expect("insert dup");
        let mut stmt = conn
            .prepare("SELECT ALL x FROM t")
            .expect("prepare select all");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_i64(0).expect("x"));
        }
        assert_eq!(rows.len(), 2, "SELECT ALL must keep duplicates");
        assert_eq!(rows, vec![1, 1]);
    }

    #[test]
    fn select_all_distinguishes_from_distinct() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(x INTEGER)").expect("create t");
        for v in [1, 1, 2, 2, 3] {
            conn.execute(&format!("INSERT INTO t VALUES ({v})"))
                .expect("insert");
        }
        // SELECT ALL should keep all 5 rows.
        let mut stmt = conn.prepare("SELECT ALL x FROM t").expect("prepare all");
        let mut all_rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            all_rows.push(stmt.column_i64(0).expect("x"));
        }
        assert_eq!(all_rows.len(), 5);

        // SELECT DISTINCT should collapse to 3 distinct values.
        let mut stmt = conn
            .prepare("SELECT DISTINCT x FROM t ORDER BY x")
            .expect("prepare distinct");
        let mut distinct_rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            distinct_rows.push(stmt.column_i64(0).expect("x"));
        }
        assert_eq!(distinct_rows, vec![1, 2, 3]);
    }

    // --- Bug 2: NOT IN with NULL --------------------------------------------

    #[test]
    fn not_in_with_null_returns_null_when_no_match() {
        // SQLite: 5 NOT IN (1, NULL) is NULL because we cannot prove 5 != NULL.
        assert_null(&run_scalar("SELECT 5 NOT IN (1, NULL)"));
    }

    #[test]
    fn not_in_with_null_returns_false_when_match() {
        // SQLite: 1 NOT IN (1, NULL) is FALSE (we found a match).
        assert_int(&run_scalar("SELECT 1 NOT IN (1, NULL)"), 0);
    }

    #[test]
    fn in_with_null_regression() {
        // Regression: 1 IN (1, NULL) must remain TRUE (match found).
        assert_int(&run_scalar("SELECT 1 IN (1, NULL)"), 1);
    }

    #[test]
    fn in_with_only_null_is_null() {
        // 5 IN (NULL) is NULL; 5 NOT IN (NULL) is also NULL.
        assert_null(&run_scalar("SELECT 5 IN (NULL)"));
        assert_null(&run_scalar("SELECT 5 NOT IN (NULL)"));
    }

    #[test]
    fn not_in_no_null_returns_true() {
        // 5 NOT IN (1, 2, 3) is TRUE; 1 NOT IN (1, 2, 3) is FALSE.
        assert_int(&run_scalar("SELECT 5 NOT IN (1, 2, 3)"), 1);
        assert_int(&run_scalar("SELECT 1 NOT IN (1, 2, 3)"), 0);
    }

    // --- Bug 3: NULL || 'x' propagates NULL ---------------------------------

    #[test]
    fn null_concat_left_returns_null() {
        assert_null(&run_scalar("SELECT NULL || 'x'"));
    }

    #[test]
    fn null_concat_right_returns_null() {
        assert_null(&run_scalar("SELECT 'a' || NULL"));
    }

    #[test]
    fn concat_normal_strings() {
        // Regression: non-NULL concat still works.
        assert_text(&run_scalar("SELECT 'a' || 'b'"), "ab");
    }

    // --- Bug 4: divide / modulo by zero return NULL --------------------------

    #[test]
    fn integer_divide_by_zero_returns_null() {
        assert_null(&run_scalar("SELECT 1 / 0"));
    }

    #[test]
    fn integer_modulo_by_zero_returns_null() {
        assert_null(&run_scalar("SELECT 5 % 0"));
    }

    #[test]
    fn real_divide_by_zero_returns_null() {
        assert_null(&run_scalar("SELECT 1.0 / 0"));
        assert_null(&run_scalar("SELECT 1.0 / 0.0"));
    }

    #[test]
    fn real_divide_normal_returns_real() {
        // Regression: 7.5 / 2.5 = 3.0.
        assert_real(&run_scalar("SELECT 7.5 / 2.5"), 3.0);
    }

    #[test]
    fn divide_normal_still_works() {
        // Regression: 6 / 2 still returns 3 as integer.
        assert_int(&run_scalar("SELECT 6 / 2"), 3);
    }

    // --- Bug 5: NULL in scalar functions -----------------------------------

    #[test]
    fn length_of_null_is_null() {
        assert_null(&run_scalar("SELECT length(NULL)"));
    }

    #[test]
    fn length_of_text_still_works() {
        assert_int(&run_scalar("SELECT length('abc')"), 3);
    }

    #[test]
    fn lower_upper_of_null_is_null() {
        assert_null(&run_scalar("SELECT lower(NULL)"));
        assert_null(&run_scalar("SELECT upper(NULL)"));
    }

    #[test]
    fn abs_of_null_is_null() {
        assert_null(&run_scalar("SELECT abs(NULL)"));
    }

    #[test]
    fn round_of_null_is_null() {
        assert_null(&run_scalar("SELECT round(NULL)"));
        assert_null(&run_scalar("SELECT round(NULL, 2)"));
        assert_null(&run_scalar("SELECT round(1.5, NULL)"));
    }

    #[test]
    fn coalesce_skips_null_regression() {
        // Regression: coalesce(NULL, 1) → 1.
        assert_int(&run_scalar("SELECT coalesce(NULL, 1)"), 1);
    }

    #[test]
    fn coalesce_returns_first_nonnull_text() {
        assert_text(&run_scalar("SELECT coalesce(NULL, NULL, 'x')"), "x");
    }

    #[test]
    fn ifnull_propagates_when_both_null() {
        assert_null(&run_scalar("SELECT ifnull(NULL, NULL)"));
    }

    // --- Bug 6: CAST is incomplete / wrong ----------------------------------

    #[test]
    fn cast_text_to_integer_no_digits_is_zero() {
        // SQLite: CAST('abc' AS INTEGER) → 0.
        assert_int(&run_scalar("SELECT CAST('abc' AS INTEGER)"), 0);
    }

    #[test]
    fn cast_text_to_integer_prefix_only() {
        // SQLite: CAST('123abc' AS INTEGER) → 123.
        assert_int(&run_scalar("SELECT CAST('123abc' AS INTEGER)"), 123);
    }

    #[test]
    fn cast_null_to_text_is_null() {
        // SQLite: CAST(NULL AS TEXT) → NULL (NOT empty string).
        assert_null(&run_scalar("SELECT CAST(NULL AS TEXT)"));
        assert_null(&run_scalar("SELECT CAST(NULL AS INTEGER)"));
        assert_null(&run_scalar("SELECT CAST(NULL AS REAL)"));
    }

    #[test]
    fn cast_real_to_integer_truncates_toward_zero() {
        // SQLite: CAST(3.7 AS INTEGER) → 3 (NOT 4 / round).
        assert_int(&run_scalar("SELECT CAST(3.7 AS INTEGER)"), 3);
        assert_int(&run_scalar("SELECT CAST(-3.7 AS INTEGER)"), -3);
    }

    #[test]
    fn cast_integer_to_text() {
        assert_text(&run_scalar("SELECT CAST(42 AS TEXT)"), "42");
    }

    // --- Bug 7: GLOB bracket / range / negation ----------------------------

    // GLOB is exposed as a builtin function in this parser; the `x GLOB y`
    // infix form is not yet supported. Use the function form for tests.
    // Argument order in this codebase is `glob(value, pattern)` (NOT the
    // SQLite-standard `glob(pattern, value)` order — see existing test
    // `sqlite_core_functions_cover_round_hex_quote_random_and_glob`).

    #[test]
    fn glob_class_any_of() {
        // 'cat' GLOB 'c[ao]t' → 1 (matches `a`).
        assert_int(&run_scalar("SELECT glob('cat', 'c[ao]t')"), 1);
        // 'cot' GLOB 'c[ao]t' → 1 (matches `o`).
        assert_int(&run_scalar("SELECT glob('cot', 'c[ao]t')"), 1);
        // 'cit' GLOB 'c[ao]t' → 0.
        assert_int(&run_scalar("SELECT glob('cit', 'c[ao]t')"), 0);
    }

    #[test]
    fn glob_class_range() {
        assert_int(&run_scalar("SELECT glob('cot', 'c[a-o]t')"), 1);
        assert_int(&run_scalar("SELECT glob('cpt', 'c[a-o]t')"), 0);
    }

    #[test]
    fn glob_class_negation() {
        // 'c0t' GLOB 'c[!0-9]t' → 0 (negated class excludes digits).
        assert_int(&run_scalar("SELECT glob('c0t', 'c[!0-9]t')"), 0);
        // 'cat' GLOB 'c[!0-9]t' → 1.
        assert_int(&run_scalar("SELECT glob('cat', 'c[!0-9]t')"), 1);
    }

    #[test]
    fn glob_star_and_question_regression() {
        assert_int(&run_scalar("SELECT glob('hello', 'h*o')"), 1);
        assert_int(&run_scalar("SELECT glob('hi', 'h?')"), 1);
        assert_int(&run_scalar("SELECT glob('hi', 'h?i')"), 0);
    }

    // --- Bug 8: ORDER BY after GROUP BY honors keys / DESC -----------------

    #[test]
    fn grouped_order_by_count_desc_via_alias() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(k TEXT, v INTEGER)")
            .expect("create t");
        for (k, v) in [("b", 1), ("a", 1), ("a", 2)] {
            conn.execute(&format!("INSERT INTO t VALUES ('{k}', {v})"))
                .expect("insert");
        }
        let mut stmt = conn
            .prepare("SELECT k, COUNT(*) AS c FROM t GROUP BY k ORDER BY c DESC")
            .expect("prepare grouped order");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            let k = stmt.column_text(0).expect("k").to_owned();
            let c = stmt.column_i64(1).expect("c");
            rows.push((k, c));
        }
        assert_eq!(
            rows,
            vec![("a".to_owned(), 2), ("b".to_owned(), 1)],
            "ORDER BY c DESC after GROUP BY must put the larger count first"
        );
    }

    #[test]
    fn grouped_order_by_count_asc() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(k TEXT)").expect("create t");
        for k in ["a", "a", "a", "b", "b", "c"] {
            conn.execute(&format!("INSERT INTO t VALUES ('{k}')"))
                .expect("insert");
        }
        let mut stmt = conn
            .prepare("SELECT k, COUNT(*) AS c FROM t GROUP BY k ORDER BY c ASC")
            .expect("prepare grouped order asc");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push((
                stmt.column_text(0).expect("k").to_owned(),
                stmt.column_i64(1).expect("c"),
            ));
        }
        assert_eq!(
            rows,
            vec![
                ("c".to_owned(), 1),
                ("b".to_owned(), 2),
                ("a".to_owned(), 3),
            ]
        );
    }

    #[test]
    fn distinct_order_by_desc() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(x INTEGER)").expect("create t");
        for v in [3, 1, 2, 1, 3] {
            conn.execute(&format!("INSERT INTO t VALUES ({v})"))
                .expect("insert");
        }
        let mut stmt = conn
            .prepare("SELECT DISTINCT x FROM t ORDER BY x DESC")
            .expect("prepare distinct desc");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_i64(0).expect("x"));
        }
        assert_eq!(rows, vec![3, 2, 1], "DISTINCT must honor ORDER BY DESC");
    }

    #[test]
    fn grouped_order_by_group_column_desc() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(k TEXT)").expect("create t");
        for k in ["a", "b", "c", "a", "b"] {
            conn.execute(&format!("INSERT INTO t VALUES ('{k}')"))
                .expect("insert");
        }
        let mut stmt = conn
            .prepare("SELECT k FROM t GROUP BY k ORDER BY k DESC")
            .expect("prepare group by k desc");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_text(0).expect("k").to_owned());
        }
        assert_eq!(rows, vec!["c".to_owned(), "b".to_owned(), "a".to_owned()]);
    }

    // Round-trip: combination of fixes — divide-by-zero in WHERE filter must
    // not panic, and propagated NULL must filter out the row.
    #[test]
    fn divide_by_zero_in_where_does_not_panic() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(x INTEGER, y INTEGER)")
            .expect("create t");
        conn.execute("INSERT INTO t VALUES (1, 0)")
            .expect("insert zero divisor");
        conn.execute("INSERT INTO t VALUES (10, 2)")
            .expect("insert ok");
        // The divisor-zero row must not match (x/y produces NULL, NULL > 0
        // is NULL ≠ TRUE, so row filtered out).
        let mut stmt = conn
            .prepare("SELECT x FROM t WHERE x / y > 0")
            .expect("prepare where with division");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_i64(0).expect("x"));
        }
        assert_eq!(rows, vec![10], "only the non-zero-divisor row must match");
    }
}
