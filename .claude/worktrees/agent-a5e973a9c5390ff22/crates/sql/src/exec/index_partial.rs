//! Phase-11 SQL-D A6: planner-side support for partial indexes.
//!
//! The kernel stores the partial-index predicate as verbatim SQL. The
//! planner is allowed to use the index only when the query's WHERE
//! provably implies the index's WHERE — we accept identical predicates
//! (after a whitespace/case-folding normalisation) as the conservative
//! base case. Richer implication (e.g. `a > 5` implies `a > 0`) is a
//! follow-up; the safety floor is "never advertise a partial index when
//! the row may have been filtered out at write time".

use redlinedb_kernel::catalog::IndexDef;
use sqlparser::ast::Expr;

use crate::exec::index_predicate::normalise_predicate;

/// Returns `true` when the query's WHERE provably implies the index's
/// partial-WHERE. Currently exact-match (modulo normalisation); the
/// query must contain the index predicate as one of its top-level AND
/// conjuncts.
pub(crate) fn query_implies_index_predicate(
    selection: &Option<Expr>,
    index: &IndexDef,
) -> bool {
    let Some(pred_sql) = index.predicate_sql.as_deref() else {
        return true;
    };
    let Some(query) = selection.as_ref() else {
        return false;
    };
    let needle = normalise_predicate(pred_sql);
    for conjunct in flatten_top_level_and(query) {
        if normalise_predicate(&conjunct.to_string()) == needle {
            return true;
        }
    }
    false
}

fn flatten_top_level_and(expr: &Expr) -> Vec<&Expr> {
    let mut out = Vec::new();
    fn walk<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
        match expr {
            Expr::BinaryOp {
                left,
                op: sqlparser::ast::BinaryOperator::And,
                right,
            } => {
                walk(left, out);
                walk(right, out);
            }
            Expr::Nested(inner) => walk(inner, out),
            other => out.push(other),
        }
    }
    walk(expr, &mut out);
    out
}

/// Try to plan an expression-source index access for a query like
/// `WHERE expr(col) = value`. Returns the leading expression's match
/// when the query's WHERE contains an `expr = const` (or `const = expr`)
/// conjunct whose left/right matches the index's leading expression key
/// verbatim (modulo whitespace/case normalisation).
pub(crate) fn try_match_expression_index(
    table: &redlinedb_kernel::catalog::TableDef,
    index: &IndexDef,
    selection: &Option<Expr>,
    bindings: &[Option<crate::value::SqlValue>],
) -> Option<ExpressionIndexMatch> {
    let first = index.keys.first()?;
    let expr_sql = match &first.source {
        redlinedb_kernel::catalog::IndexKeySource::Expression { sql, .. } => sql.as_ref(),
        _ => return None,
    };
    if index.predicate_sql.is_some() {
        // Only honour a partial expression index when the query also
        // proves the predicate; keep planner safe.
        if !query_implies_index_predicate(selection, index) {
            return None;
        }
    }
    let needle = normalise_predicate(expr_sql);
    let conjuncts = match selection {
        Some(s) => flatten_top_level_and(s),
        None => return None,
    };
    for conjunct in conjuncts {
        if let Expr::BinaryOp {
            left,
            op: sqlparser::ast::BinaryOperator::Eq,
            right,
        } = strip_nested(conjunct)
        {
            let (target_expr, value_expr) =
                if normalise_predicate(&strip_nested(left).to_string()) == needle {
                    (left.as_ref(), right.as_ref())
                } else if normalise_predicate(&strip_nested(right).to_string()) == needle {
                    (right.as_ref(), left.as_ref())
                } else {
                    continue;
                };
            let value = eval_constant(value_expr, bindings)?;
            if matches!(value, crate::value::SqlValue::Null) {
                continue;
            }
            let _ = target_expr;
            // Only the leading-key expression is matched — multi-key
            // expression indexes need follow-up work (each subsequent
            // key needs its own conjunct).
            if index.keys.len() != 1 {
                continue;
            }
            let _ = table;
            return Some(ExpressionIndexMatch {
                leading_value: value,
                predicate_str: format!("{expr_sql} = ?"),
            });
        }
    }
    None
}

pub(crate) struct ExpressionIndexMatch {
    pub(crate) leading_value: crate::value::SqlValue,
    pub(crate) predicate_str: String,
}

fn strip_nested(expr: &Expr) -> &Expr {
    let mut current = expr;
    while let Expr::Nested(inner) = current {
        current = inner;
    }
    current
}

fn eval_constant(
    expr: &Expr,
    bindings: &[Option<crate::value::SqlValue>],
) -> Option<crate::value::SqlValue> {
    use crate::value::SqlValue;
    use sqlparser::ast::{UnaryOperator, Value};
    if let Expr::Value(v) = expr
        && let Some(name) = crate::parser::bind::as_bind_name(&v.value)
    {
        return crate::parser::bind::resolve_positional(name, bindings);
    }
    match expr {
        Expr::Value(v) => Some(match &v.value {
            Value::Null => SqlValue::Null,
            Value::Boolean(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
            Value::Number(n, _) => n
                .parse()
                .ok()
                .map(SqlValue::Integer)
                .unwrap_or(SqlValue::Null),
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                SqlValue::Text(std::sync::Arc::from(s.as_str()))
            }
            _ => return None,
        }),
        Expr::Nested(inner) => eval_constant(inner, bindings),
        Expr::UnaryOp { op, expr } => {
            let value = eval_constant(expr, bindings)?;
            match op {
                UnaryOperator::Minus => match value {
                    SqlValue::Integer(v) => Some(SqlValue::Integer(-v)),
                    SqlValue::Real(v) => Some(SqlValue::Real(-v)),
                    _ => None,
                },
                UnaryOperator::Plus => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}
