//! Expression evaluation for the SQL executor.
//!
//! This module is the dispatcher between AST nodes and concrete evaluation
//! helpers. The bulk of the implementation is split across sibling files:
//!
//!   * `scalar`        — arithmetic / string / null helpers, vector and
//!                       datetime helpers, row-context plumbing
//!   * `coerce`        — type coercion, comparison, binary-operator eval
//!   * `json_dispatch` — function-call dispatcher (delegates JSON funcs
//!                       to `crate::json::scalar`)
//!   * `window`        — window-function execution stubs (parsed-only)
//!
//! Submodules import shared symbols via `use super::*`. To keep that
//! ergonomic, this `mod.rs` glob-re-exports the items defined in the
//! sibling files (with `pub(super)` so the surface area outside `expr/`
//! does not change).

use super::*;

pub(crate) mod coerce;
pub(crate) mod json_dispatch;
pub(crate) mod scalar;
pub(crate) mod window;

// Glob-re-export every symbol from the sibling files so each sibling's
// `use super::*` (combined with the `use super::*` re-exporting `exec`'s
// own items above) sees the full pre-split surface area. This keeps the
// split a pure rename refactor — call sites in this `mod.rs` and inside
// the siblings reach unqualified helpers through these re-exports.
pub(crate) use coerce::*;
use json_dispatch::eval_function;
pub(crate) use scalar::*;

pub(crate) fn project_row(
    projection: &[SelectItem],
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    if projection.is_empty() {
        return row.values();
    }

    let mut out = Vec::new();
    for item in projection {
        match item {
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                out.extend(row.values()?);
            }
            SelectItem::UnnamedExpr(expr) => out.push(eval_scalar(expr, &row.context(), bindings)?),
            SelectItem::ExprWithAlias { expr, .. } => {
                out.push(eval_scalar(expr, &row.context(), bindings)?)
            }
        }
    }
    Ok(out)
}

pub(crate) fn selection_passes(
    selection: &Option<Expr>,
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<bool> {
    match selection {
        Some(expr) => Ok(is_truthy(&eval_scalar(expr, &row.context(), bindings)?)),
        None => Ok(true),
    }
}

pub(crate) fn compare_rows(left: &[SqlValue], right: &[SqlValue]) -> Ordering {
    for (l, r) in left.iter().zip(right.iter()) {
        let ord = compare_values(l, r);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    left.len().cmp(&right.len())
}

pub(crate) fn eval_scalar(
    expr: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    Ok(match expr {
        Expr::Value(v) => match &v.value {
            Value::Null => SqlValue::Null,
            Value::Boolean(v) => SqlValue::Integer(if *v { 1 } else { 0 }),
            Value::Number(n, _) => parse_number(n)?,
            Value::SingleQuotedString(s)
            | Value::DoubleQuotedString(s)
            | Value::EscapedStringLiteral(s)
            | Value::TripleSingleQuotedString(s)
            | Value::TripleDoubleQuotedString(s)
            | Value::UnicodeStringLiteral(s)
            | Value::SingleQuotedRawStringLiteral(s)
            | Value::DoubleQuotedRawStringLiteral(s)
            | Value::TripleSingleQuotedRawStringLiteral(s)
            | Value::TripleDoubleQuotedRawStringLiteral(s) => SqlValue::Text(Arc::from(s.as_str())),
            Value::SingleQuotedByteStringLiteral(s)
            | Value::DoubleQuotedByteStringLiteral(s)
            | Value::TripleSingleQuotedByteStringLiteral(s)
            | Value::TripleDoubleQuotedByteStringLiteral(s) => {
                SqlValue::Blob(Arc::from(s.as_bytes()))
            }
            Value::HexStringLiteral(s) => SqlValue::Blob(hex_string_to_bytes(s)?),
            Value::DollarQuotedString(s) => SqlValue::Text(Arc::from(s.value.as_str())),
            Value::Placeholder(name) => resolve_binding(name, bindings)?,
            other => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported SQL literal: {other:?}"
                )));
            }
        },
        Expr::Identifier(ident) => lookup_column(row, &ident.value)?,
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [ident] => lookup_column(row, &ident.value)?,
            [qualifier, ident] => lookup_qualified_column(row, &qualifier.value, &ident.value)?,
            _ => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported identifier: {parts:?}"
                )));
            }
        },
        Expr::Nested(expr) => eval_scalar(expr, row, bindings)?,
        Expr::UnaryOp { op, expr } => {
            let value = eval_scalar(expr, row, bindings)?;
            match op {
                UnaryOperator::Not => match truthy_opt(&value) {
                    Some(v) => SqlValue::Integer(if !v { 1 } else { 0 }),
                    None => SqlValue::Null,
                },
                UnaryOperator::Minus => negate(value)?,
                UnaryOperator::Plus => value,
                _ => {
                    return Err(Error::UnsupportedSql(format!(
                        "unsupported unary op {op:?}"
                    )));
                }
            }
        }
        Expr::BinaryOp { left, op, right } => eval_binary(left, op, right, row, bindings)?,
        Expr::Collate { expr, collation } => {
            // The COLLATE wrapper is transparent for value evaluation; the
            // collation only affects comparisons performed in eval_binary or
            // ORDER BY. Validate the name here so unknown collations error.
            let name = collation.to_string();
            if crate::collation::Collation::parse(&name).is_none() {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported collation: {name}"
                )));
            }
            eval_scalar(expr, row, bindings)?
        }
        Expr::Cast {
            expr, data_type, ..
        } => cast_value(eval_scalar(expr, row, bindings)?, data_type)?,
        Expr::Function(func) => eval_function(func, row, bindings)?,
        Expr::Like {
            negated,
            any,
            expr,
            pattern,
            escape_char,
        } => {
            if *any {
                return Err(Error::UnsupportedSql(
                    "LIKE ANY is not supported".to_owned(),
                ));
            }
            let value = eval_scalar(expr, row, bindings)?;
            let pattern = eval_scalar(pattern, row, bindings)?;
            like_result(value, pattern, *negated, escape_char.clone(), true)?
        }
        Expr::ILike {
            negated,
            any,
            expr,
            pattern,
            escape_char,
        } => {
            if *any {
                return Err(Error::UnsupportedSql(
                    "ILIKE ANY is not supported".to_owned(),
                ));
            }
            let value = eval_scalar(expr, row, bindings)?;
            let pattern = eval_scalar(pattern, row, bindings)?;
            like_result(value, pattern, *negated, escape_char.clone(), true)?
        }
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => {
            let value = eval_scalar(expr, row, bindings)?;
            let low = eval_scalar(low, row, bindings)?;
            let high = eval_scalar(high, row, bindings)?;
            if matches!(value, SqlValue::Null)
                || matches!(low, SqlValue::Null)
                || matches!(high, SqlValue::Null)
            {
                SqlValue::Null
            } else {
                let mut ok = compare_values(&value, &low) != Ordering::Less
                    && compare_values(&value, &high) != Ordering::Greater;
                if *negated {
                    ok = !ok;
                }
                SqlValue::Integer(if ok { 1 } else { 0 })
            }
        }
        Expr::InList {
            expr,
            list,
            negated,
        } => {
            let value = eval_scalar(expr, row, bindings)?;
            if matches!(value, SqlValue::Null) {
                SqlValue::Null
            } else {
                // SQLite semantics: compute the base IN as TRUE / FALSE / NULL,
                // then apply NOT only on TRUE / FALSE — NULL must propagate
                // through unchanged.
                //   `5 NOT IN (1, NULL)`  → NULL  (cannot prove 5 != NULL)
                //   `1 NOT IN (1, NULL)`  → FALSE (we found a match)
                //   `5 NOT IN (1, 2, 3)`  → TRUE  (no NULL, no match)
                let mut found = false;
                let mut saw_null = false;
                for item in list {
                    let candidate = eval_scalar(item, row, bindings)?;
                    match candidate {
                        SqlValue::Null => saw_null = true,
                        _ if compare_values(&value, &candidate) == Ordering::Equal => {
                            found = true;
                            break;
                        }
                        _ => {}
                    }
                }
                let base_in: Option<bool> = if found {
                    Some(true)
                } else if saw_null {
                    None
                } else {
                    Some(false)
                };
                match (base_in, *negated) {
                    (Some(b), false) => SqlValue::Integer(if b { 1 } else { 0 }),
                    (Some(b), true) => SqlValue::Integer(if !b { 1 } else { 0 }),
                    (None, _) => SqlValue::Null,
                }
            }
        }
        Expr::Exists { subquery, negated } => {
            let rows = evaluate_subquery_rows(subquery, bindings)?;
            let exists = !rows.is_empty();
            SqlValue::Integer(if exists ^ *negated { 1 } else { 0 })
        }
        Expr::InSubquery {
            expr,
            subquery,
            negated,
        } => {
            let value = eval_scalar(expr, row, bindings)?;
            if matches!(value, SqlValue::Null) {
                SqlValue::Null
            } else {
                let rows = evaluate_subquery_rows(subquery, bindings)?;
                let mut found = false;
                let mut saw_null = false;
                for row in rows {
                    if row.len() != 1 {
                        return Err(Error::UnsupportedSql(
                            "IN subquery must return exactly one column".to_owned(),
                        ));
                    }
                    let candidate = row.into_iter().next().unwrap_or(SqlValue::Null);
                    match candidate {
                        SqlValue::Null => saw_null = true,
                        _ if compare_values(&value, &candidate) == Ordering::Equal => {
                            found = true;
                            break;
                        }
                        _ => {}
                    }
                }
                // Same NOT-IN-with-NULL semantics as InList: compute base IN as
                // TRUE / FALSE / NULL, then apply NOT only on TRUE / FALSE.
                let base_in: Option<bool> = if found {
                    Some(true)
                } else if saw_null {
                    None
                } else {
                    Some(false)
                };
                match (base_in, *negated) {
                    (Some(b), false) => SqlValue::Integer(if b { 1 } else { 0 }),
                    (Some(b), true) => SqlValue::Integer(if !b { 1 } else { 0 }),
                    (None, _) => SqlValue::Null,
                }
            }
        }
        Expr::Subquery(subquery) => {
            let rows = evaluate_subquery_rows(subquery, bindings)?;
            match rows.as_slice() {
                [] => SqlValue::Null,
                [row] if row.len() == 1 => row[0].clone(),
                [row] if row.is_empty() => SqlValue::Null,
                _ => {
                    return Err(Error::UnsupportedSql(
                        "scalar subquery must return exactly one row and one column".to_owned(),
                    ));
                }
            }
        }
        Expr::IsNull(expr) => SqlValue::Integer(
            if matches!(eval_scalar(expr, row, bindings)?, SqlValue::Null) {
                1
            } else {
                0
            },
        ),
        Expr::IsNotNull(expr) => SqlValue::Integer(
            if !matches!(eval_scalar(expr, row, bindings)?, SqlValue::Null) {
                1
            } else {
                0
            },
        ),
        Expr::IsTrue(expr) => sql_truth_result(eval_scalar(expr, row, bindings)?),
        Expr::IsNotTrue(expr) => sql_truth_result_not(eval_scalar(expr, row, bindings)?),
        Expr::IsFalse(expr) => sql_false_result(eval_scalar(expr, row, bindings)?),
        Expr::IsNotFalse(expr) => sql_false_result_not(eval_scalar(expr, row, bindings)?),
        Expr::IsUnknown(expr) => SqlValue::Integer(
            if matches!(eval_scalar(expr, row, bindings)?, SqlValue::Null) {
                1
            } else {
                0
            },
        ),
        Expr::IsNotUnknown(expr) => SqlValue::Integer(
            if !matches!(eval_scalar(expr, row, bindings)?, SqlValue::Null) {
                1
            } else {
                0
            },
        ),
        Expr::IsDistinctFrom(left, right) => {
            let left = eval_scalar(left, row, bindings)?;
            let right = eval_scalar(right, row, bindings)?;
            SqlValue::Integer(if is_distinct(&left, &right) { 1 } else { 0 })
        }
        Expr::IsNotDistinctFrom(left, right) => {
            let left = eval_scalar(left, row, bindings)?;
            let right = eval_scalar(right, row, bindings)?;
            SqlValue::Integer(if !is_distinct(&left, &right) { 1 } else { 0 })
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => eval_case(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            row,
            bindings,
        )?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported expression: {other:?}"
            )));
        }
    })
}

pub(crate) fn truthy_opt(value: &SqlValue) -> Option<bool> {
    match value {
        SqlValue::Null => None,
        _ => Some(is_truthy(value)),
    }
}

fn eval_case(
    operand: Option<&Expr>,
    conditions: &[sqlparser::ast::CaseWhen],
    else_result: Option<&Expr>,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if let Some(operand) = operand {
        let operand = eval_scalar(operand, row, bindings)?;
        for when in conditions {
            let condition = eval_scalar(&when.condition, row, bindings)?;
            if matches!(condition, SqlValue::Null) {
                continue;
            }
            if compare_values(&operand, &condition) == Ordering::Equal {
                return eval_scalar(&when.result, row, bindings);
            }
        }
    } else {
        for when in conditions {
            let condition = eval_scalar(&when.condition, row, bindings)?;
            if !matches!(condition, SqlValue::Null) && is_truthy(&condition) {
                return eval_scalar(&when.result, row, bindings);
            }
        }
    }
    match else_result {
        Some(expr) => eval_scalar(expr, row, bindings),
        None => Ok(SqlValue::Null),
    }
}

fn evaluate_subquery_rows(
    subquery: &sqlparser::ast::Query,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let schema = conn.engine().schema_snapshot();
    let template = crate::parser::bind_query(
        conn,
        schema,
        conn.schema_epoch(),
        "<subquery>",
        subquery.clone(),
    )?;
    materialize_prepared_rows(conn, &template, bindings)
}
