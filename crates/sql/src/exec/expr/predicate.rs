use super::*;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct SubqueryCacheKey {
    ast_addr: usize,
    schema_epoch: u64,
    stats_epoch: u64,
    optimizer_hash: u64,
}

thread_local! {
    static SUBQUERY_TEMPLATE_CACHE: RefCell<HashMap<SubqueryCacheKey, PreparedTemplate>> =
        RefCell::new(HashMap::new());
    static IN_SUBQUERY_ROW_CACHE: RefCell<HashMap<SubqueryCacheKey, Vec<Vec<SqlValue>>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn clear_subquery_template_cache() {
    SUBQUERY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().clear());
    IN_SUBQUERY_ROW_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn truthy_opt(value: &SqlValue) -> Option<bool> {
    match value {
        SqlValue::Null => None,
        _ => Some(is_truthy(value)),
    }
}

pub(crate) trait CaseEvaluator {
    fn eval_case_expr(&mut self, expr: &Expr) -> Result<SqlValue>;
}

pub(crate) fn eval_case<E>(
    operand: Option<&Expr>,
    conditions: &[sqlparser::ast::CaseWhen],
    else_result: Option<&Expr>,
    evaluator: &mut E,
) -> Result<SqlValue>
where
    E: CaseEvaluator,
{
    if let Some(operand) = operand {
        let operand = evaluator.eval_case_expr(operand)?;
        if matches!(operand, SqlValue::Null) {
            return match else_result {
                Some(expr) => evaluator.eval_case_expr(expr),
                None => Ok(SqlValue::Null),
            };
        }
        for when in conditions {
            let condition = evaluator.eval_case_expr(&when.condition)?;
            if matches!(condition, SqlValue::Null) {
                continue;
            }
            if compare_values(&operand, &condition) == Ordering::Equal {
                return evaluator.eval_case_expr(&when.result);
            }
        }
    } else {
        for when in conditions {
            let condition = evaluator.eval_case_expr(&when.condition)?;
            if !matches!(condition, SqlValue::Null) && is_truthy(&condition) {
                return evaluator.eval_case_expr(&when.result);
            }
        }
    }
    match else_result {
        Some(expr) => evaluator.eval_case_expr(expr),
        None => Ok(SqlValue::Null),
    }
}

pub(crate) fn eval_subquery_value(
    subquery: &sqlparser::ast::Query,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let rows = evaluate_subquery_rows(subquery, row, bindings)?;
    match rows.as_slice() {
        [] => Ok(SqlValue::Null),
        [row] if row.len() == 1 => Ok(row[0].clone()),
        [row] if row.is_empty() => Ok(SqlValue::Null),
        _ => Err(Error::UnsupportedSql(
            "scalar subquery must return exactly one row and one column".to_owned(),
        )),
    }
}

fn bind_subquery(conn: &Connection, subquery: &sqlparser::ast::Query) -> Result<PreparedTemplate> {
    let key = subquery_cache_key(conn, subquery);
    if let Some(template) = SUBQUERY_TEMPLATE_CACHE.with(|cache| cache.borrow().get(&key).cloned())
    {
        return Ok(template);
    }
    let schema =
        current_tx_schema_snapshot(conn).unwrap_or_else(|| conn.engine().schema_snapshot());
    let template = crate::parser::bind_query(
        conn,
        schema,
        conn.schema_epoch(),
        "<subquery>",
        subquery.clone(),
    )?;
    SUBQUERY_TEMPLATE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, template.clone());
    });
    Ok(template)
}

fn subquery_cache_key(conn: &Connection, subquery: &sqlparser::ast::Query) -> SubqueryCacheKey {
    SubqueryCacheKey {
        ast_addr: subquery as *const sqlparser::ast::Query as usize,
        schema_epoch: conn.schema_epoch().0,
        stats_epoch: conn.stats_epoch().0,
        optimizer_hash: conn.optimizer_hash(),
    }
}

/// Evaluate a subquery, pushing the caller's row onto the correlated-scope
/// stack so qualified references (`outer.col`) resolve through
/// `lookup_correlated`. The row snapshot is dropped automatically once
/// the subquery returns.
pub(crate) fn evaluate_subquery_rows(
    subquery: &sqlparser::ast::Query,
    outer_row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    let owned = outer_row.to_owned_row();
    crate::exec::with_outer_row(owned, || {
        materialize_prepared_rows(conn, &template, bindings)
    })
}

pub(crate) fn evaluate_subquery_exists(
    subquery: &sqlparser::ast::Query,
    outer_row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<bool> {
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    let owned = outer_row.to_owned_row();
    let rows = crate::exec::with_outer_row(owned, || {
        materialize_prepared_rows_limited(conn, &template, bindings, Some(1))
    })?;
    Ok(!rows.is_empty())
}

fn row_values_for_expr(
    expr: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    match expr {
        Expr::Tuple(exprs) => exprs
            .iter()
            .map(|expr| eval_scalar(expr, row, bindings))
            .collect(),
        Expr::Nested(inner) => row_values_for_expr(inner, row, bindings),
        _ => Ok(vec![eval_scalar(expr, row, bindings)?]),
    }
}

fn row_eq(left: &[SqlValue], right: &[SqlValue]) -> Result<Option<bool>> {
    if left.len() != right.len() {
        return Err(Error::UnsupportedSql(format!(
            "row value arity mismatch: {} vs {}",
            left.len(),
            right.len()
        )));
    }
    for (l, r) in left.iter().zip(right.iter()) {
        if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
            return Ok(None);
        }
        match compare_values(l, r) {
            Ordering::Equal => {}
            _ => return Ok(Some(false)),
        }
    }
    Ok(Some(true))
}

pub(crate) fn in_list_result(
    expr: &Expr,
    list: &[Expr],
    negated: bool,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let value = row_values_for_expr(expr, row, bindings)?;
    if value.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let mut found = false;
    let mut saw_null = false;
    for item in list {
        let candidate = row_values_for_expr(item, row, bindings)?;
        match row_eq(&value, &candidate)? {
            Some(true) => {
                found = true;
                break;
            }
            Some(false) => {}
            None => saw_null = true,
        }
    }
    finish_in_result(found, saw_null, negated)
}

pub(crate) fn in_subquery_result(
    expr: &Expr,
    subquery: &sqlparser::ast::Query,
    negated: bool,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let value = row_values_for_expr(expr, row, bindings)?;
    if value.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    if template.output_columns.len() != value.len() {
        return Err(Error::UnsupportedSql(
            "IN subquery must return the same number of columns as the row value".to_owned(),
        ));
    }
    let cache_key = subquery_cache_key(conn, subquery);
    let rows = if in_subquery_is_cacheable(subquery) {
        if let Some(rows) =
            IN_SUBQUERY_ROW_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned())
        {
            rows
        } else {
            let owned = row.to_owned_row();
            let (rows, used_correlated_lookup) = crate::exec::with_outer_row(owned, || {
                crate::exec::with_correlated_lookup_tracking(|| {
                    materialize_prepared_rows(conn, &template, bindings)
                })
            });
            let rows = rows?;
            if !used_correlated_lookup {
                IN_SUBQUERY_ROW_CACHE.with(|cache| {
                    cache.borrow_mut().insert(cache_key, rows.clone());
                });
            }
            rows
        }
    } else {
        let owned = row.to_owned_row();
        crate::exec::with_outer_row(owned, || {
            materialize_prepared_rows(conn, &template, bindings)
        })?
    };
    let mut found = false;
    let mut saw_null = false;
    for row in rows {
        match row_eq(&value, &row)? {
            Some(true) => {
                found = true;
                break;
            }
            Some(false) => {}
            None => saw_null = true,
        }
    }
    finish_in_result(found, saw_null, negated)
}

fn in_subquery_is_cacheable(subquery: &sqlparser::ast::Query) -> bool {
    let rendered = subquery.to_string();
    let lower = rendered.to_ascii_lowercase();
    !rendered.contains('.')
        && !lower.contains("random(")
        && !lower.contains("randomblob(")
        && !lower.contains("last_insert_rowid")
        && !lower.contains("changes(")
        && !lower.contains("total_changes(")
        && !lower.contains("current_date")
        && !lower.contains("current_time")
        && !lower.contains("current_timestamp")
        && !lower.contains("date(")
        && !lower.contains("time(")
        && !lower.contains("datetime(")
        && !lower.contains("julianday(")
        && !lower.contains("unixepoch(")
        && !lower.contains("strftime(")
}

fn finish_in_result(found: bool, saw_null: bool, negated: bool) -> Result<SqlValue> {
    let base_in: Option<bool> = if found {
        Some(true)
    } else if saw_null {
        None
    } else {
        Some(false)
    };
    Ok(match (base_in, negated) {
        (Some(b), false) => SqlValue::Integer(if b { 1 } else { 0 }),
        (Some(b), true) => SqlValue::Integer(if !b { 1 } else { 0 }),
        (None, _) => SqlValue::Null,
    })
}
