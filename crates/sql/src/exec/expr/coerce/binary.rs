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
    let collation = match (collation_from_expr(left), collation_from_expr(right)) {
        (Some(c), _) => Some(c),
        (None, Some(c)) => Some(c),
        (None, None) => declared_collation(row, left),
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
        BinaryOperator::Minus => match try_json_delete(&left_value, &right_value) {
            Some(v) => v,
            None => arithmetic(
                left_value,
                right_value,
                |a, b| Some(a.wrapping_sub(b)),
                |a, b| Some(a - b),
            )?,
        },
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
            } else if let Some(merged) = try_json_concat(&left_value, &right_value) {
                merged
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
        BinaryOperator::AtArrow => crate::json::jsonb::op_at_arrow(&left_value, &right_value)?,
        BinaryOperator::ArrowAt => crate::json::jsonb::op_arrow_at(&left_value, &right_value)?,
        BinaryOperator::HashArrow => crate::json::jsonb::op_hash_arrow(&left_value, &right_value)?,
        BinaryOperator::HashLongArrow => {
            crate::json::jsonb::op_hash_long_arrow(&left_value, &right_value)?
        }
        BinaryOperator::HashMinus => crate::json::jsonb::op_hash_minus(&left_value, &right_value)?,
        BinaryOperator::AtAt => crate::json::jsonb::op_at_at(&left_value, &right_value)?,
        BinaryOperator::AtQuestion => {
            crate::json::jsonb::op_at_question(&left_value, &right_value)?
        }
        BinaryOperator::Question => crate::json::jsonb::op_question(&left_value, &right_value)?,
        BinaryOperator::QuestionPipe => crate::json::jsonb::op_question_any(
            &left_value,
            std::slice::from_ref(&right_value),
        )?,
        BinaryOperator::QuestionAnd => crate::json::jsonb::op_question_all(
            &left_value,
            std::slice::from_ref(&right_value),
        )?,
        BinaryOperator::Regexp => regexp_result(left_value, right_value, false)?,
        BinaryOperator::Match => match_result(left, left_value, right_value, row)?,
        // Postgres POSIX-regex match operators
        // (https://www.postgresql.org/docs/16/functions-matching.html#FUNCTIONS-POSIX-REGEXP).
        // `~`  matches case-sensitive, `~*` case-insensitive, `!~` / `!~*`
        // are the negated forms. Internally we delegate to the same `regex`
        // crate that backs `REGEXP`; the `(?i)` flag prefix yields case
        // insensitivity without recompiling against a different feature set.
        BinaryOperator::PGRegexMatch => {
            pg_regex_result(left_value, right_value, false, false)?
        }
        BinaryOperator::PGRegexIMatch => {
            pg_regex_result(left_value, right_value, false, true)?
        }
        BinaryOperator::PGRegexNotMatch => {
            pg_regex_result(left_value, right_value, true, false)?
        }
        BinaryOperator::PGRegexNotIMatch => {
            pg_regex_result(left_value, right_value, true, true)?
        }
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

/// Evaluator for Postgres POSIX-regex match operators (`~`, `~*`, `!~`,
/// `!~*`). The case-insensitive flag is implemented by lower-casing both
/// haystack and pattern before delegating to the shared regex engine —
/// the `regex` crate is built without the `unicode-case` feature here,
/// so a `(?i)` prefix would refuse Unicode haystacks. Pre-folding via
/// Rust's `to_lowercase` covers the same ASCII range Postgres' default
/// C-locale `~*` does (and a wider Unicode range on top). NULL on either
/// side propagates.
pub(crate) fn pg_regex_result(
    value: SqlValue,
    pattern: SqlValue,
    negated: bool,
    case_insensitive: bool,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern_str = value_to_string(&pattern);
    let (effective_text, effective_pattern) = if case_insensitive {
        (text.to_lowercase(), pattern_str.to_lowercase())
    } else {
        (text.to_string(), pattern_str.to_string())
    };
    let matched = crate::regexp::regex_match(&effective_text, &effective_pattern)?;
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

/// If both operands look like JSON containers (object or array), return
/// their PostgreSQL `jsonb || jsonb` merge. Returns `None` for any other
/// shape so the caller falls back to text concatenation.
fn try_json_concat(left: &SqlValue, right: &SqlValue) -> Option<SqlValue> {
    use serde_json::Value;
    let left_text = match left {
        SqlValue::Text(s) => s.as_ref(),
        _ => return None,
    };
    let right_text = match right {
        SqlValue::Text(s) => s.as_ref(),
        _ => return None,
    };
    let left_doc: Value = serde_json::from_str(left_text.trim()).ok()?;
    let right_doc: Value = serde_json::from_str(right_text.trim()).ok()?;
    // Only switch to JSON semantics when at least one side is a container
    // — otherwise plain `'foo' || 'bar'` would mis-concatenate as an array.
    if !matches!(left_doc, Value::Object(_) | Value::Array(_))
        && !matches!(right_doc, Value::Object(_) | Value::Array(_))
    {
        return None;
    }
    let merged = crate::json::jsonb::op_concat(&left_doc, &right_doc);
    Some(crate::json::jsonb::jsonb_text(&merged))
}

/// PostgreSQL `jsonb - text|int` — strip a key (or array index, with
/// negative-from-end support). Returns `None` when the LHS does not look
/// like a JSON container, so arithmetic subtraction continues to handle
/// numeric pairs.
fn try_json_delete(left: &SqlValue, right: &SqlValue) -> Option<SqlValue> {
    use serde_json::Value;
    let SqlValue::Text(text) = left else {
        return None;
    };
    let doc: Value = serde_json::from_str(text.trim()).ok()?;
    if !matches!(doc, Value::Object(_) | Value::Array(_)) {
        return None;
    }
    let updated = match right {
        SqlValue::Text(key) => crate::json::jsonb::op_delete_key(&doc, key.as_ref()),
        SqlValue::Integer(idx) => crate::json::jsonb::op_delete_index(&doc, *idx),
        _ => return None,
    };
    Some(crate::json::jsonb::jsonb_text(&updated))
}
