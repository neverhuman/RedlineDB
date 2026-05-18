use super::super::*;

pub(crate) fn eval_binary(
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
        compare_binary_with(a, b, accept, collation)
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

pub(crate) fn compare_binary_with(
    left: SqlValue,
    right: SqlValue,
    accept: impl FnOnce(Ordering) -> bool,
    collation: Option<crate::collation::Collation>,
) -> Result<SqlValue> {
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let ord = match collation.and_then(|c| c.compare_values(&left, &right)) {
        Some(o) => o,
        None => compare_values(&left, &right),
    };
    Ok(SqlValue::Integer(if accept(ord) { 1 } else { 0 }))
}

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

pub(crate) fn is_distinct(left: &SqlValue, right: &SqlValue) -> bool {
    matches!(left, SqlValue::Null) != matches!(right, SqlValue::Null)
        || (!matches!(left, SqlValue::Null)
            && !matches!(right, SqlValue::Null)
            && compare_values(left, right) != Ordering::Equal)
}
