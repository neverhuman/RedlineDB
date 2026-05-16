//! Unit tests for the scalar submodule split. These cover small
//! pure-function behaviour that is otherwise only exercised end-to-end
//! via the SQL integration tests.

use super::*;

#[test]
fn row_width_sums_value_sizes() {
    let row = vec![
        SqlValue::Null,
        SqlValue::Integer(42),
        SqlValue::Real(3.14),
        SqlValue::Text(Arc::from("hello")),
        SqlValue::Blob(Arc::from(&b"abc"[..])),
    ];
    // null=0, int=8, real=8, "hello"=5, b"abc"=3
    assert_eq!(row_width(&row), 0 + 8 + 8 + 5 + 3);
}

#[test]
fn value_to_string_handles_all_variants() {
    assert_eq!(value_to_string(&SqlValue::Null), "");
    assert_eq!(value_to_string(&SqlValue::Integer(-7)), "-7");
    assert_eq!(value_to_string(&SqlValue::Text(Arc::from("ok"))), "ok");
    assert_eq!(
        value_to_string(&SqlValue::Blob(Arc::from(&b"hi"[..]))),
        "hi"
    );
}

#[test]
fn scalar_to_usize_clamps_negatives_to_zero() {
    assert_eq!(scalar_to_usize(&SqlValue::Integer(-5)).unwrap(), 0);
    assert_eq!(scalar_to_usize(&SqlValue::Integer(12)).unwrap(), 12);
    assert_eq!(scalar_to_usize(&SqlValue::Real(-1.0)).unwrap(), 0);
    assert_eq!(scalar_to_usize(&SqlValue::Real(2.7)).unwrap(), 2);
    assert_eq!(scalar_to_usize(&SqlValue::Null).unwrap(), 0);
    assert!(scalar_to_usize(&SqlValue::Text(Arc::from("x"))).is_err());
}

#[test]
fn key_values_equal_respects_length_and_values() {
    let a = vec![SqlValue::Integer(1), SqlValue::Text(Arc::from("a"))];
    let b = vec![SqlValue::Integer(1), SqlValue::Text(Arc::from("a"))];
    let c = vec![SqlValue::Integer(1), SqlValue::Text(Arc::from("b"))];
    assert!(key_values_equal(&a, &b));
    assert!(!key_values_equal(&a, &c));
    assert!(!key_values_equal(&a, &a[..1]));
}

#[test]
fn negate_propagates_null_and_rejects_text() {
    assert!(matches!(negate(SqlValue::Null).unwrap(), SqlValue::Null));
    assert!(matches!(
        negate(SqlValue::Integer(3)).unwrap(),
        SqlValue::Integer(-3)
    ));
    assert!(negate(SqlValue::Text(Arc::from("x"))).is_err());
}

#[test]
fn numeric_value_parses_text_and_blob() {
    assert_eq!(numeric_value(&SqlValue::Null).unwrap(), 0.0);
    assert_eq!(numeric_value(&SqlValue::Integer(7)).unwrap(), 7.0);
    assert!((numeric_value(&SqlValue::Real(1.5)).unwrap() - 1.5).abs() < 1e-9);
    assert!(
        (numeric_value(&SqlValue::Text(Arc::from(" 2.5 "))).unwrap() - 2.5).abs() < 1e-9
    );
    assert!(numeric_value(&SqlValue::Text(Arc::from("nope"))).is_err());
}
