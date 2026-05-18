use super::super::*;

pub(crate) fn row_width(row: &[SqlValue]) -> usize {
    row.iter().map(row_width_value).sum()
}

pub(crate) fn row_width_value(value: &SqlValue) -> usize {
    match value {
        SqlValue::Null => 0,
        SqlValue::Integer(_) | SqlValue::Real(_) => 8,
        SqlValue::Text(value) => value.len(),
        SqlValue::Blob(value) => value.len(),
    }
}

// Lane VE: the pre-VE SQL-A in-place row-sort path was replaced by
// `vec::SpillSort` + top-K heap, so this helper has no current
// caller outside tests. Marked allow(dead_code) instead of removed
// to keep the SQL-A surface intact.
#[allow(dead_code)]
pub(crate) fn compare_row_ordering(
    left: &SqlRow,
    right: &SqlRow,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<Ordering> {
    for order in order_by {
        let collation = collation_from_expr(&order.expr);
        let left_value = eval_scalar(&order.expr, &left.context(), bindings)?;
        let right_value = eval_scalar(&order.expr, &right.context(), bindings)?;
        let mut ord = match collation.and_then(|c| c.compare_values(&left_value, &right_value)) {
            Some(o) => o,
            None => compare_values(&left_value, &right_value),
        };
        if matches!(order.options.asc, Some(false)) {
            ord = ord.reverse();
        }
        if ord != Ordering::Equal {
            return Ok(ord);
        }
    }
    Ok(Ordering::Equal)
}

pub(crate) fn scalar_to_usize(value: &SqlValue) -> Result<usize> {
    match value {
        SqlValue::Integer(v) => Ok((*v).max(0) as usize),
        SqlValue::Real(v) => Ok((*v).max(0.0) as usize),
        SqlValue::Null => Ok(0),
        _ => Err(Error::DatatypeMismatch),
    }
}
