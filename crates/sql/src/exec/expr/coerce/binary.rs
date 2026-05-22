use super::super::*;

pub(crate) fn eval_binary(
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if matches!(op, BinaryOperator::Match) {
        let pattern = eval_scalar(right, row, bindings)?;
        return match_result(left, SqlValue::Null, pattern, row);
    }
    let collation = match collation_from_expr(left) {
        Some(c) => Some(c),
        None => collation_from_expr(right).or_else(|| declared_collation(row, left)),
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
        BinaryOperator::Match => match_result(left, left_value, right_value, row)?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported binary op {other:?}"
            )));
        }
    })
}

fn match_result(
    left_expr: &Expr,
    left_value: SqlValue,
    pattern: SqlValue,
    row: &RowContext<'_>,
) -> Result<SqlValue> {
    if matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let needle = value_to_string(&pattern).to_ascii_lowercase();
    crate::exec::expr::json_dispatch::set_current_match_term(Some(needle.clone()));
    let haystack = match left_expr {
        Expr::Identifier(ident) => {
            row_values_for_table_name(row, &ident.value).unwrap_or_else(|| vec![left_value])
        }
        _ => vec![left_value],
    };
    let matched = haystack.iter().any(|value| {
        value_to_string(value)
            .to_ascii_lowercase()
            .contains(&needle)
    });
    Ok(SqlValue::Integer(if matched { 1 } else { 0 }))
}

fn row_values_for_table_name(row: &RowContext<'_>, name: &str) -> Option<Vec<SqlValue>> {
    match row {
        RowContext::Table(table) if table.table.name.eq_ignore_ascii_case(name) => {
            Some(table.values.clone())
        }
        RowContext::Joined(rows) => rows.iter().find_map(|joined| {
            let table_name = joined
                .alias
                .as_ref()
                .map(|alias| alias.as_ref())
                .unwrap_or(joined.table.name.as_ref());
            if table_name.eq_ignore_ascii_case(name) {
                joined.row.as_ref().map(|row| row.values.clone())
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn declared_collation(row: &RowContext<'_>, expr: &Expr) -> Option<crate::collation::Collation> {
    let (qualifier, name) = match expr {
        Expr::Identifier(ident) => (None, ident.value.as_str()),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
            (Some(parts[0].value.as_str()), parts[1].value.as_str())
        }
        Expr::Nested(inner) => return declared_collation(row, inner),
        _ => return None,
    };
    let table = match row {
        RowContext::Table(table) => Some(table.table.as_ref()),
        RowContext::Joined(rows) => rows.iter().find_map(|joined| {
            let table_name = joined
                .alias
                .as_ref()
                .map(|alias| alias.as_ref())
                .unwrap_or(joined.table.name.as_ref());
            if qualifier.is_none_or(|q| table_name.eq_ignore_ascii_case(q)) {
                Some(joined.table.as_ref())
            } else {
                None
            }
        }),
        _ => None,
    }?;
    let _ = table
        .columns
        .iter()
        .find(|column| column.folded.eq_ignore_ascii_case(name))?;
    let sql = table.normalized_sql.as_deref()?.to_ascii_lowercase();
    let needle = name.to_ascii_lowercase();
    if sql.split(',').any(|part| {
        let part = part.rsplit('(').next().unwrap_or(part).trim();
        (part.starts_with(&(needle.clone() + " ")) || part.starts_with(&(needle.clone() + "\t")))
            && part.contains("collate nocase")
    }) {
        Some(crate::collation::Collation::NoCase)
    } else {
        None
    }
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
