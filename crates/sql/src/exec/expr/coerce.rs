//! Type coercion, comparison helpers, and binary-operator evaluation.
//!
//! This file owns:
//!   * `eval_binary` — the dispatcher for `Expr::BinaryOp`
//!   * NULL-aware comparison helpers (`compare_binary*`, `is_distinct`)
//!   * SQL truthiness predicates (`sql_truth_*`, `sql_false_*`)
//!   * `cast_value` and the SQLite-style numeric prefix parsers
//!   * `regexp_result` (used by both `BinaryOperator::Regexp` and the
//!     scalar `regexp(...)` function)
//!
//! All cross-file callers go through `pub(super)` exports.

use super::*;

pub(super) fn eval_binary(
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let collation = match collation_from_expr(left) {
        Some(c) => Some(c),
        None => collation_from_expr(right),
    };
    let left_value = eval_scalar(left, row, bindings)?;
    let right_value = eval_scalar(right, row, bindings)?;
    let compare_with_collation = |a: SqlValue, b: SqlValue, accept: fn(Ordering) -> bool| {
        compare_binary_with(a, b, accept, collation.clone())
    };
    Ok(match op {
        BinaryOperator::And => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
            (Some(true), Some(true)) => SqlValue::Integer(1),
            _ => SqlValue::Null,
        },
        BinaryOperator::Or => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(true), _) | (_, Some(true)) => SqlValue::Integer(1),
            (Some(false), Some(false)) => SqlValue::Integer(0),
            _ => SqlValue::Null,
        },
        BinaryOperator::Plus => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_add(b)),
            |a, b| Some(a + b),
        )?,
        BinaryOperator::Minus => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_sub(b)),
            |a, b| Some(a - b),
        )?,
        BinaryOperator::Multiply => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_mul(b)),
            |a, b| Some(a * b),
        )?,
        BinaryOperator::Divide => arithmetic(
            left_value,
            right_value,
            |a, b| if b == 0 { None } else { a.checked_div(b) },
            |a, b| if b == 0.0 { None } else { Some(a / b) },
        )?,
        BinaryOperator::Modulo => arithmetic(
            left_value,
            right_value,
            |a, b| if b == 0 { None } else { a.checked_rem(b) },
            |a, b| if b == 0.0 { None } else { Some(a % b) },
        )?,
        BinaryOperator::Eq => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Equal)?
        }
        BinaryOperator::Spaceship => match try_vector_pair(&left_value, &right_value) {
            Some((a, b)) => vector_distance_to_value(VectorOpMetric::Cosine, &a, &b)?,
            None => compare_with_collation(left_value, right_value, |o| o != Ordering::Equal)?,
        },
        BinaryOperator::NotEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Equal)?
        }
        BinaryOperator::Gt => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Greater)?
        }
        BinaryOperator::GtEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Less)?
        }
        BinaryOperator::Lt => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Less)?
        }
        BinaryOperator::LtEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Greater)?
        }
        BinaryOperator::StringConcat => {
            if matches!(left_value, SqlValue::Null) || matches!(right_value, SqlValue::Null) {
                SqlValue::Null
            } else {
                SqlValue::Text(Arc::from(format!(
                    "{}{}",
                    value_to_string(&left_value),
                    value_to_string(&right_value)
                )))
            }
        }
        BinaryOperator::Arrow => crate::json::scalar::arrow_json(&left_value, &right_value)?,
        BinaryOperator::LongArrow => crate::json::scalar::arrow_sql(&left_value, &right_value)?,
        BinaryOperator::Regexp => regexp_result(left_value, right_value, false)?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported binary op {other:?}"
            )));
        }
    })
}

/// Lane SQL-D: SQLite-style `value REGEXP pattern`. NULL on either side
/// propagates; an invalid pattern errors out.
pub(crate) fn regexp_result(value: SqlValue, pattern: SqlValue, negated: bool) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern_str = value_to_string(&pattern);
    let matched = crate::regexp::regex_match(&text, &pattern_str)?;
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

pub(crate) fn compare_binary(
    left: SqlValue,
    right: SqlValue,
    accept: impl FnOnce(Ordering) -> bool,
) -> Result<SqlValue> {
    compare_binary_with(left, right, accept, None)
}

/// Like `compare_binary` but optionally applies a `Collation` to the
/// comparison. When the collation cannot be applied to the value pair (for
/// example, a number compared to text) we fall through to the default
/// type-precedence comparison.
pub(crate) fn compare_binary_with(
    left: SqlValue,
    right: SqlValue,
    accept: impl FnOnce(Ordering) -> bool,
    collation: Option<crate::collation::Collation>,
) -> Result<SqlValue> {
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let ord = match collation.as_ref().and_then(|c| c.compare_values(&left, &right)) {
        Some(o) => o,
        None => compare_values(&left, &right),
    };
    Ok(SqlValue::Integer(if accept(ord) { 1 } else { 0 }))
}

/// Walk a possibly-`Collate(...)` wrapper to extract the collation choice.
pub(crate) fn collation_from_expr(expr: &Expr) -> Option<crate::collation::Collation> {
    match expr {
        Expr::Collate { collation, .. } => {
            crate::collation::Collation::parse(&collation.to_string())
        }
        Expr::Nested(inner) => collation_from_expr(inner),
        _ => None,
    }
}

pub(crate) fn sql_truth_result(value: SqlValue) -> SqlValue {
    SqlValue::Integer(if is_truthy(&value) { 1 } else { 0 })
}

pub(crate) fn sql_truth_result_not(value: SqlValue) -> SqlValue {
    SqlValue::Integer(if !is_truthy(&value) { 1 } else { 0 })
}

pub(crate) fn sql_false_result(value: SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        other => SqlValue::Integer(if !is_truthy(&other) { 1 } else { 0 }),
    }
}

pub(crate) fn sql_false_result_not(value: SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        other => SqlValue::Integer(if is_truthy(&other) { 1 } else { 0 }),
    }
}

pub(super) fn is_distinct(left: &SqlValue, right: &SqlValue) -> bool {
    matches!(left, SqlValue::Null) != matches!(right, SqlValue::Null)
        || (!matches!(left, SqlValue::Null)
            && !matches!(right, SqlValue::Null)
            && compare_values(left, right) != Ordering::Equal)
}

pub(crate) fn cast_value(
    value: SqlValue,
    data_type: &sqlparser::ast::DataType,
) -> Result<SqlValue> {
    // SQLite: CAST(NULL AS anything) is NULL.
    if matches!(value, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let type_name = data_type.to_string().to_ascii_lowercase();

    // CAST AS BLOB: text becomes its UTF-8 bytes; numerics become their
    // textual representation as bytes.
    if type_name.contains("blob") {
        return Ok(match value {
            SqlValue::Blob(_) => value,
            SqlValue::Text(s) => SqlValue::Blob(Arc::from(s.as_bytes())),
            other => SqlValue::Blob(Arc::from(value_to_string(&other).into_bytes())),
        });
    }

    if type_name.contains("text") || type_name.contains("char") || type_name.contains("clob") {
        // CAST AS TEXT: numbers become their text form; blobs become UTF-8
        // (lossy if invalid bytes); text is unchanged.
        return Ok(match value {
            SqlValue::Text(_) => value,
            SqlValue::Integer(v) => SqlValue::Text(Arc::from(v.to_string())),
            SqlValue::Real(v) => SqlValue::Text(Arc::from(v.to_string())),
            SqlValue::Blob(v) => {
                SqlValue::Text(Arc::from(String::from_utf8_lossy(&v).into_owned()))
            }
            SqlValue::Null => SqlValue::Null,
        });
    }

    if type_name.contains("real") || type_name.contains("floa") || type_name.contains("doub") {
        return Ok(SqlValue::Real(cast_to_real(&value)));
    }

    if type_name.contains("int") {
        // SQLite truncates real toward zero (not floor / round):
        //   CAST(3.7 AS INTEGER)  → 3
        //   CAST(-3.7 AS INTEGER) → -3
        return Ok(SqlValue::Integer(cast_to_integer(&value)));
    }

    // CAST AS NUMERIC (or any unrecognized type): return integer if it parses
    // cleanly, otherwise real, otherwise the integer prefix.
    if type_name.contains("numeric") {
        return Ok(cast_to_numeric(&value));
    }

    Ok(value)
}

/// SQLite-style implicit text → integer conversion: parse the longest valid
/// integer prefix; if no valid digits, return 0. Real values truncate toward
/// zero; blobs are interpreted as their UTF-8 text.
fn cast_to_integer(value: &SqlValue) -> i64 {
    match value {
        SqlValue::Null => 0,
        SqlValue::Integer(v) => *v,
        // Truncate toward zero — matches SQLite (not floor / round).
        SqlValue::Real(v) => {
            if v.is_nan() {
                0
            } else if *v > i64::MAX as f64 {
                i64::MAX
            } else if *v < i64::MIN as f64 {
                i64::MIN
            } else {
                *v as i64
            }
        }
        SqlValue::Text(s) => parse_integer_prefix(s),
        SqlValue::Blob(b) => parse_integer_prefix(&String::from_utf8_lossy(b)),
    }
}

fn cast_to_real(value: &SqlValue) -> f64 {
    match value {
        SqlValue::Null => 0.0,
        SqlValue::Integer(v) => *v as f64,
        SqlValue::Real(v) => *v,
        SqlValue::Text(s) => parse_real_prefix(s),
        SqlValue::Blob(b) => parse_real_prefix(&String::from_utf8_lossy(b)),
    }
}

fn cast_to_numeric(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Integer(_) | SqlValue::Real(_) => value.clone(),
        SqlValue::Text(t) => parse_numeric_text(t.as_ref()),
        SqlValue::Blob(b) => parse_numeric_text(&String::from_utf8_lossy(b)),
    }
}

fn parse_numeric_text(text: &str) -> SqlValue {
    let trimmed = text.trim();
    if let Ok(v) = trimmed.parse::<i64>() {
        SqlValue::Integer(v)
    } else if let Ok(v) = trimmed.parse::<f64>() {
        SqlValue::Real(v)
    } else {
        SqlValue::Integer(parse_integer_prefix(trimmed))
    }
}

/// Parse the longest valid integer prefix of `s`, mirroring SQLite's
/// `CAST(<text> AS INTEGER)`. Stops at the first non-digit character.
/// Leading whitespace and a single optional sign are accepted.
fn parse_integer_prefix(s: &str) -> i64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    let mut neg = false;
    if let Some(&first) = bytes.first() {
        if first == b'+' {
            idx = 1;
        } else if first == b'-' {
            idx = 1;
            neg = true;
        }
    }
    let start = idx;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == start {
        return 0;
    }
    let digits = &s[start..idx];
    let mag: i64 = digits.parse().unwrap_or(i64::MAX);
    if neg { mag.wrapping_neg() } else { mag }
}

/// Parse the longest valid real-number prefix of `s` (sign, digits,
/// fractional part, optional exponent). Returns 0.0 on no match.
fn parse_real_prefix(s: &str) -> f64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    if let Some(&first) = bytes.first()
        && (first == b'+' || first == b'-')
    {
        idx = 1;
    }
    let mut saw_digit = false;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
        saw_digit = true;
    }
    if idx < bytes.len() && bytes[idx] == b'.' {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
            saw_digit = true;
        }
    }
    if saw_digit && idx < bytes.len() && (bytes[idx] == b'e' || bytes[idx] == b'E') {
        let mut after_e = idx + 1;
        if after_e < bytes.len() && (bytes[after_e] == b'+' || bytes[after_e] == b'-') {
            after_e += 1;
        }
        let exp_start = after_e;
        while after_e < bytes.len() && bytes[after_e].is_ascii_digit() {
            after_e += 1;
        }
        if after_e > exp_start {
            idx = after_e;
        }
    }
    if !saw_digit {
        return 0.0;
    }
    s[..idx].parse::<f64>().unwrap_or(0.0)
}
