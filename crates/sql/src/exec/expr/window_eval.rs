//! Window function evaluation.
//!
//! Detects `OVER (...)` calls in a SELECT projection and computes their
//! per-row values from a materialized row set. Supports the SQLite
//! window-function surface: ROW_NUMBER, RANK, DENSE_RANK, NTILE,
//! LAG / LEAD, FIRST_VALUE / LAST_VALUE / NTH_VALUE, PERCENT_RANK,
//! CUME_DIST, and aggregate-OVER (SUM/COUNT/AVG/MIN/MAX/TOTAL).
//!
//! Frame defaults follow SQL standard:
//!   * ORDER BY present, no frame -> RANGE BETWEEN UNBOUNDED PRECEDING
//!     AND CURRENT ROW
//!   * No ORDER BY, no frame      -> RANGE BETWEEN UNBOUNDED PRECEDING
//!     AND UNBOUNDED FOLLOWING (entire partition)
//!
//! Execution is single-threaded and operates on already-materialized
//! `SqlRow` values; partitions / ordering / frame bounds are computed
//! purely in-memory.

use std::cmp::Ordering;
use std::sync::Arc;

use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, FunctionArguments, OrderByExpr, SelectItem,
    WindowFrameBound, WindowFrameUnits, WindowSpec, WindowType,
};

use crate::error::{Error, Result};
use crate::value::{SqlValue, compare_values};

use super::SqlRow;
use super::eval_scalar;

/// Returns `true` if any projection item contains a function call carrying
/// an `OVER (...)` clause.
pub(crate) fn projection_has_window(items: &[SelectItem]) -> bool {
    items.iter().any(|item| match item {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            expr_has_window(expr)
        }
        _ => false,
    })
}

pub(crate) fn expr_has_window(expr: &Expr) -> bool {
    match expr {
        Expr::Function(func) => func.over.is_some(),
        Expr::BinaryOp { left, right, .. } => expr_has_window(left) || expr_has_window(right),
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) | Expr::Cast { expr, .. } => {
            expr_has_window(expr)
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            operand.as_deref().is_some_and(expr_has_window)
                || conditions.iter().any(|w| {
                    expr_has_window(&w.condition) || expr_has_window(&w.result)
                })
                || else_result.as_deref().is_some_and(expr_has_window)
        }
        _ => false,
    }
}

/// Evaluate every window function call in `projection` against `rows`.
/// Returns a `Vec` of projected rows. Non-window items are evaluated as
/// scalars per-row; window items get their per-row value from window
/// computation.
pub(crate) fn evaluate_window_functions(
    rows: &[SqlRow],
    projection: &[SelectItem],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(rows.len());
    // Precompute every window-function call's per-row result. We index by
    // (proj_item_idx, position-within-expr) by walking each projection
    // expression and replacing every Expr::Function-with-OVER with a
    // placeholder pulled from `window_values[item][row_idx]`.
    let mut window_values: Vec<Vec<Vec<SqlValue>>> = Vec::with_capacity(projection.len());
    for item in projection {
        let exprs = collect_window_calls(item);
        let mut per_call: Vec<Vec<SqlValue>> = Vec::with_capacity(exprs.len());
        for call in &exprs {
            per_call.push(eval_window_call(call, rows, bindings)?);
        }
        window_values.push(per_call);
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let mut projected = Vec::with_capacity(projection.len());
        for (item_idx, item) in projection.iter().enumerate() {
            let value = match item {
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                    // Wildcards just pass through every base column.
                    for v in row.values()? {
                        projected.push(v);
                    }
                    continue;
                }
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                    let mut counter = 0usize;
                    eval_with_window_values(
                        expr,
                        row,
                        bindings,
                        &window_values[item_idx],
                        row_idx,
                        &mut counter,
                    )?
                }
            };
            projected.push(value);
        }
        out.push(projected);
    }
    Ok(out)
}

/// Walk an expression and collect every windowed `Expr::Function` call in
/// the order they appear (left-to-right DFS).
fn collect_window_calls(item: &SelectItem) -> Vec<Expr> {
    let mut out = Vec::new();
    match item {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            collect_window_calls_in(expr, &mut out)
        }
        _ => {}
    }
    out
}

fn collect_window_calls_in(expr: &Expr, out: &mut Vec<Expr>) {
    match expr {
        Expr::Function(func) if func.over.is_some() => {
            out.push(expr.clone());
        }
        Expr::Function(_) => {}
        Expr::BinaryOp { left, right, .. } => {
            collect_window_calls_in(left, out);
            collect_window_calls_in(right, out);
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) | Expr::Cast { expr, .. } => {
            collect_window_calls_in(expr, out)
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(op) = operand.as_deref() {
                collect_window_calls_in(op, out);
            }
            for when in conditions {
                collect_window_calls_in(&when.condition, out);
                collect_window_calls_in(&when.result, out);
            }
            if let Some(er) = else_result.as_deref() {
                collect_window_calls_in(er, out);
            }
        }
        _ => {}
    }
}

/// Evaluate an expression where windowed Expr::Function nodes are
/// replaced by precomputed per-row window values (in DFS order).
fn eval_with_window_values(
    expr: &Expr,
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
    window_values: &[Vec<SqlValue>],
    row_idx: usize,
    counter: &mut usize,
) -> Result<SqlValue> {
    match expr {
        Expr::Function(func) if func.over.is_some() => {
            let idx = *counter;
            *counter += 1;
            Ok(window_values[idx][row_idx].clone())
        }
        Expr::BinaryOp { left, op, right } => {
            let l = eval_with_window_values(left, row, bindings, window_values, row_idx, counter)?;
            let r = eval_with_window_values(right, row, bindings, window_values, row_idx, counter)?;
            // Reuse the scalar binary evaluator by routing through eval_scalar
            // on a constructed expression.
            let ctx = row.context();
            super::coerce::eval_binary(
                &Expr::Value(sqlparser::ast::ValueWithSpan {
                    value: sqlparser::ast::Value::Number(value_to_sql_number(&l), false),
                    span: sqlparser::tokenizer::Span::empty(),
                }),
                op,
                &Expr::Value(sqlparser::ast::ValueWithSpan {
                    value: sqlparser::ast::Value::Number(value_to_sql_number(&r), false),
                    span: sqlparser::tokenizer::Span::empty(),
                }),
                &ctx,
                bindings,
            )
            .or_else(|_| {
                // Fallback: do simple numeric/text combine for + - * /
                fallback_binary_op(op, l, r)
            })
        }
        Expr::Nested(inner) => {
            eval_with_window_values(inner, row, bindings, window_values, row_idx, counter)
        }
        Expr::Cast { expr, .. } => {
            eval_with_window_values(expr, row, bindings, window_values, row_idx, counter)
        }
        _ => eval_scalar(expr, &row.context(), bindings),
    }
}

fn fallback_binary_op(
    op: &sqlparser::ast::BinaryOperator,
    left: SqlValue,
    right: SqlValue,
) -> Result<SqlValue> {
    use sqlparser::ast::BinaryOperator as B;
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let l = to_real(&left);
    let r = to_real(&right);
    let out = match op {
        B::Plus => SqlValue::Real(l + r),
        B::Minus => SqlValue::Real(l - r),
        B::Multiply => SqlValue::Real(l * r),
        B::Divide => {
            if r == 0.0 {
                SqlValue::Null
            } else {
                SqlValue::Real(l / r)
            }
        }
        other => {
            return Err(Error::UnsupportedSql(format!(
                "windowed combination operator unsupported: {other:?}"
            )));
        }
    };
    Ok(out)
}

fn value_to_sql_number(value: &SqlValue) -> String {
    match value {
        SqlValue::Integer(n) => n.to_string(),
        SqlValue::Real(n) => n.to_string(),
        SqlValue::Text(s) => s.to_string(),
        SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
        SqlValue::Null => "0".to_string(),
    }
}

fn to_real(value: &SqlValue) -> f64 {
    match value {
        SqlValue::Integer(n) => *n as f64,
        SqlValue::Real(n) => *n,
        SqlValue::Text(s) => s.parse().unwrap_or(0.0),
        SqlValue::Blob(b) => String::from_utf8_lossy(b).parse().unwrap_or(0.0),
        SqlValue::Null => 0.0,
    }
}

/// Compute the per-row value sequence for a single windowed function call.
fn eval_window_call(
    expr: &Expr,
    rows: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let Expr::Function(func) = expr else {
        return Err(Error::UnsupportedSql(
            "expected function call for window evaluation".to_owned(),
        ));
    };
    let Some(WindowType::WindowSpec(window)) = &func.over else {
        return Err(Error::UnsupportedSql(
            "named windows are not supported".to_owned(),
        ));
    };

    // Partition rows by PARTITION BY keys.
    let partitions = partition_rows(rows, &window.partition_by, bindings)?;
    // Resolve effective frame per spec defaults.
    let frame = resolve_frame(window);

    let func_name = func.name.to_string().to_ascii_lowercase();
    let args = function_args(func);

    let mut results = vec![SqlValue::Null; rows.len()];
    for partition in &partitions {
        // Compute peer groups by ORDER BY keys for ranking / RANGE frames.
        let sorted = order_partition(partition, rows, &window.order_by, bindings)?;
        // Map sorted-position -> original row index in `rows`.
        let order_index_map: Vec<usize> = sorted.iter().map(|(idx, _)| *idx).collect();
        // Peer-group ids per sorted position (rows with equal ORDER-BY keys).
        let peer_ids: Vec<usize> = if window.order_by.is_empty() {
            // No order: every row is its own peer for purposes of RANK,
            // but DENSE_RANK should return 1 for all rows (SQLite behavior).
            vec![0; sorted.len()]
        } else {
            assign_peer_ids(&sorted, rows, &window.order_by, bindings)?
        };

        for (sorted_pos, (row_idx, _row_ref)) in sorted.iter().enumerate() {
            let value = compute_function_for_row(
                &func_name,
                &args,
                rows,
                &order_index_map,
                &peer_ids,
                sorted_pos,
                &frame,
                window,
                bindings,
            )?;
            results[*row_idx] = value;
        }
    }
    Ok(results)
}

fn function_args(func: &sqlparser::ast::Function) -> Vec<Expr> {
    match &func.args {
        FunctionArguments::List(list) => list
            .args
            .iter()
            .filter_map(|arg| match arg {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => Some(e.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn partition_rows(
    rows: &[SqlRow],
    partition_by: &[Expr],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<usize>>> {
    if partition_by.is_empty() {
        return Ok(vec![(0..rows.len()).collect()]);
    }
    let mut keys: Vec<Vec<SqlValue>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut key = Vec::with_capacity(partition_by.len());
        for expr in partition_by {
            key.push(eval_scalar(expr, &row.context(), bindings)?);
        }
        keys.push(key);
    }
    let mut groups: Vec<(Vec<SqlValue>, Vec<usize>)> = Vec::new();
    'outer: for (i, key) in keys.iter().enumerate() {
        for (existing_key, members) in &mut groups {
            if rows_equal(existing_key, key) {
                members.push(i);
                continue 'outer;
            }
        }
        groups.push((key.clone(), vec![i]));
    }
    Ok(groups.into_iter().map(|(_, m)| m).collect())
}

fn order_partition(
    partition: &[usize],
    rows: &[SqlRow],
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<(usize, Vec<SqlValue>)>> {
    let mut items: Vec<(usize, Vec<SqlValue>)> = Vec::with_capacity(partition.len());
    for &idx in partition {
        let mut key = Vec::with_capacity(order_by.len());
        for ord in order_by {
            key.push(eval_scalar(&ord.expr, &rows[idx].context(), bindings)?);
        }
        items.push((idx, key));
    }
    if order_by.is_empty() {
        return Ok(items);
    }
    items.sort_by(|a, b| {
        for (i, ord) in order_by.iter().enumerate() {
            let ord_dir = ord.options.asc.unwrap_or(true);
            let nulls_first = ord
                .options
                .nulls_first
                .unwrap_or_else(|| !ord_dir);
            let cmp = compare_with_nulls(&a.1[i], &b.1[i], nulls_first);
            let cmp = if ord_dir { cmp } else { cmp.reverse() };
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        a.0.cmp(&b.0)
    });
    Ok(items)
}

fn assign_peer_ids(
    sorted: &[(usize, Vec<SqlValue>)],
    _rows: &[SqlRow],
    order_by: &[OrderByExpr],
    _bindings: &[Option<SqlValue>],
) -> Result<Vec<usize>> {
    let mut ids = Vec::with_capacity(sorted.len());
    let mut current_id = 0usize;
    for i in 0..sorted.len() {
        if i == 0 {
            ids.push(current_id);
            continue;
        }
        let prev = &sorted[i - 1].1;
        let curr = &sorted[i].1;
        let mut equal = true;
        for (j, _) in order_by.iter().enumerate() {
            if compare_values(&prev[j], &curr[j]) != Ordering::Equal {
                equal = false;
                break;
            }
        }
        if !equal {
            current_id += 1;
        }
        ids.push(current_id);
    }
    Ok(ids)
}

fn compare_with_nulls(a: &SqlValue, b: &SqlValue, nulls_first: bool) -> Ordering {
    match (a, b) {
        (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
        (SqlValue::Null, _) => {
            if nulls_first {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        (_, SqlValue::Null) => {
            if nulls_first {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        _ => compare_values(a, b),
    }
}

fn rows_equal(a: &[SqlValue], b: &[SqlValue]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(l, r)| compare_values(l, r) == Ordering::Equal)
}

#[derive(Clone, Debug)]
struct ResolvedFrame {
    units: WindowFrameUnits,
    start: ResolvedBound,
    end: ResolvedBound,
}

#[derive(Clone, Debug)]
enum ResolvedBound {
    UnboundedPreceding,
    Preceding(i64),
    CurrentRow,
    Following(i64),
    UnboundedFollowing,
}

fn resolve_frame(window: &WindowSpec) -> ResolvedFrame {
    match &window.window_frame {
        Some(frame) => ResolvedFrame {
            units: frame.units,
            start: resolve_bound(&frame.start_bound),
            end: match &frame.end_bound {
                Some(end) => resolve_bound(end),
                None => ResolvedBound::CurrentRow,
            },
        },
        None => {
            if window.order_by.is_empty() {
                // No ORDER BY: entire partition.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::UnboundedFollowing,
                }
            } else {
                // ORDER BY present: RANGE UNBOUNDED PRECEDING -> CURRENT ROW.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::CurrentRow,
                }
            }
        }
    }
}

fn resolve_bound(bound: &WindowFrameBound) -> ResolvedBound {
    match bound {
        WindowFrameBound::CurrentRow => ResolvedBound::CurrentRow,
        WindowFrameBound::Preceding(None) => ResolvedBound::UnboundedPreceding,
        WindowFrameBound::Following(None) => ResolvedBound::UnboundedFollowing,
        WindowFrameBound::Preceding(Some(expr)) => match literal_i64(expr) {
            Some(n) => ResolvedBound::Preceding(n),
            None => ResolvedBound::Preceding(0),
        },
        WindowFrameBound::Following(Some(expr)) => match literal_i64(expr) {
            Some(n) => ResolvedBound::Following(n),
            None => ResolvedBound::Following(0),
        },
    }
}

fn literal_i64(expr: &Expr) -> Option<i64> {
    if let Expr::Value(v) = expr
        && let sqlparser::ast::Value::Number(s, _) = &v.value
    {
        return s.parse::<i64>().ok();
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn compute_function_for_row(
    func_name: &str,
    args: &[Expr],
    rows: &[SqlRow],
    order_index_map: &[usize],
    peer_ids: &[usize],
    sorted_pos: usize,
    frame: &ResolvedFrame,
    window: &WindowSpec,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    match func_name {
        "row_number" => Ok(SqlValue::Integer((sorted_pos + 1) as i64)),
        "rank" => {
            // 1 + number of rows whose peer-id < ours.
            let target = peer_ids[sorted_pos];
            let pre = peer_ids.iter().take_while(|&&id| id < target).count();
            Ok(SqlValue::Integer((pre + 1) as i64))
        }
        "dense_rank" => Ok(SqlValue::Integer((peer_ids[sorted_pos] + 1) as i64)),
        "percent_rank" => {
            let target = peer_ids[sorted_pos];
            let pre = peer_ids.iter().take_while(|&&id| id < target).count() as f64;
            let total = peer_ids.len() as f64;
            let denom = (total - 1.0).max(1.0);
            if total <= 1.0 {
                Ok(SqlValue::Real(0.0))
            } else {
                Ok(SqlValue::Real(pre / denom))
            }
        }
        "cume_dist" => {
            let target = peer_ids[sorted_pos];
            let n = peer_ids.iter().filter(|&&id| id <= target).count() as f64;
            let total = peer_ids.len() as f64;
            Ok(SqlValue::Real(n / total))
        }
        "ntile" => {
            let buckets = match args.first().and_then(|e| literal_i64(e)) {
                Some(n) if n > 0 => n as usize,
                _ => {
                    return Err(Error::UnsupportedSql(
                        "ntile(N) requires a positive integer literal".to_owned(),
                    ));
                }
            };
            let total = order_index_map.len();
            let base = total / buckets;
            let extras = total % buckets;
            // Buckets 1..=extras get base+1 rows; remainder get base.
            let pos = sorted_pos;
            let bucket = if pos < extras * (base + 1) {
                pos / (base + 1) + 1
            } else {
                let after = pos - extras * (base + 1);
                let denom = base.max(1);
                extras + after / denom + 1
            };
            Ok(SqlValue::Integer(bucket as i64))
        }
        "lag" | "lead" => {
            let offset = match args.get(1) {
                Some(e) => literal_i64(e).unwrap_or(1),
                None => 1,
            };
            let default = match args.get(2) {
                Some(e) => eval_scalar(e, &rows[order_index_map[sorted_pos]].context(), bindings)?,
                None => SqlValue::Null,
            };
            let target = if func_name == "lag" {
                sorted_pos as i64 - offset
            } else {
                sorted_pos as i64 + offset
            };
            if target < 0 || target as usize >= order_index_map.len() {
                Ok(default)
            } else {
                let row_idx = order_index_map[target as usize];
                match args.first() {
                    Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings),
                    None => Ok(SqlValue::Null),
                }
            }
        }
        "first_value" => {
            let bounds = frame_bounds(frame, sorted_pos, peer_ids, order_index_map.len());
            let first_pos = bounds.0;
            if first_pos > bounds.1 {
                return Ok(SqlValue::Null);
            }
            let row_idx = order_index_map[first_pos];
            match args.first() {
                Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings),
                None => Ok(SqlValue::Null),
            }
        }
        "last_value" => {
            let bounds = frame_bounds(frame, sorted_pos, peer_ids, order_index_map.len());
            if bounds.0 > bounds.1 {
                return Ok(SqlValue::Null);
            }
            let row_idx = order_index_map[bounds.1];
            match args.first() {
                Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings),
                None => Ok(SqlValue::Null),
            }
        }
        "nth_value" => {
            let n = match args.get(1).and_then(|e| literal_i64(e)) {
                Some(v) if v > 0 => v as usize,
                _ => return Ok(SqlValue::Null),
            };
            let bounds = frame_bounds(frame, sorted_pos, peer_ids, order_index_map.len());
            let target = bounds.0 + n - 1;
            if target > bounds.1 {
                return Ok(SqlValue::Null);
            }
            let row_idx = order_index_map[target];
            match args.first() {
                Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings),
                None => Ok(SqlValue::Null),
            }
        }
        "sum" | "count" | "avg" | "min" | "max" | "total" => {
            let bounds = frame_bounds(frame, sorted_pos, peer_ids, order_index_map.len());
            let mut accumulator = Accumulator::new(func_name);
            for i in bounds.0..=bounds.1 {
                if i >= order_index_map.len() {
                    break;
                }
                let row_idx = order_index_map[i];
                let value = if matches!(func_name, "count")
                    && matches!(
                        args.first(),
                        None | Some(Expr::Identifier(_)) | Some(Expr::CompoundIdentifier(_))
                    )
                    && args.is_empty()
                {
                    // COUNT(*) with no args
                    SqlValue::Integer(1)
                } else {
                    match args.first() {
                        Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings)?,
                        None => SqlValue::Integer(1),
                    }
                };
                accumulator.push(value);
            }
            // Suppress unused warning when window doesn't carry frame defaults
            let _ = window;
            Ok(accumulator.finalize())
        }
        other => Err(Error::UnsupportedSql(format!(
            "window function not supported: {other}"
        ))),
    }
}

/// Compute (start, end) sorted-position bounds for the row at
/// `sorted_pos` under `frame`. End is inclusive. Returns positions
/// clamped into `[0, total-1]`. May return start > end (empty frame).
fn frame_bounds(
    frame: &ResolvedFrame,
    sorted_pos: usize,
    peer_ids: &[usize],
    total: usize,
) -> (usize, usize) {
    let s = match &frame.start {
        ResolvedBound::UnboundedPreceding => 0i64,
        ResolvedBound::Preceding(n) => sorted_pos as i64 - *n,
        ResolvedBound::CurrentRow => match frame.units {
            WindowFrameUnits::Range | WindowFrameUnits::Groups => {
                // First row of the current peer group.
                let target = peer_ids[sorted_pos];
                peer_ids.iter().position(|&id| id == target).unwrap_or(sorted_pos) as i64
            }
            WindowFrameUnits::Rows => sorted_pos as i64,
        },
        ResolvedBound::Following(n) => sorted_pos as i64 + *n,
        ResolvedBound::UnboundedFollowing => total as i64,
    };
    let e = match &frame.end {
        ResolvedBound::UnboundedPreceding => -1i64,
        ResolvedBound::Preceding(n) => sorted_pos as i64 - *n,
        ResolvedBound::CurrentRow => match frame.units {
            WindowFrameUnits::Range | WindowFrameUnits::Groups => {
                // Last row of the current peer group.
                let target = peer_ids[sorted_pos];
                peer_ids
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|&(_, &id)| id == target)
                    .map(|(i, _)| i as i64)
                    .unwrap_or(sorted_pos as i64)
            }
            WindowFrameUnits::Rows => sorted_pos as i64,
        },
        ResolvedBound::Following(n) => sorted_pos as i64 + *n,
        ResolvedBound::UnboundedFollowing => total as i64 - 1,
    };
    let s = s.max(0) as usize;
    let e = if e < 0 { 0 } else { e as usize };
    let e = e.min(total.saturating_sub(1));
    (s, e)
}

struct Accumulator {
    kind: String,
    count: i64,
    sum: f64,
    min: Option<SqlValue>,
    max: Option<SqlValue>,
    saw_null: bool,
    saw_any: bool,
    is_real: bool,
    int_sum: i64,
    int_sum_overflow: bool,
}

impl Accumulator {
    fn new(name: &str) -> Self {
        Self {
            kind: name.to_owned(),
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
            saw_null: false,
            saw_any: false,
            is_real: false,
            int_sum: 0,
            int_sum_overflow: false,
        }
    }
    fn push(&mut self, value: SqlValue) {
        match value {
            SqlValue::Null => {
                self.saw_null = true;
            }
            ref v => {
                self.saw_any = true;
                self.count += 1;
                match v {
                    SqlValue::Integer(n) => {
                        match self.int_sum.checked_add(*n) {
                            Some(s) => self.int_sum = s,
                            None => self.int_sum_overflow = true,
                        }
                        self.sum += *n as f64;
                    }
                    SqlValue::Real(n) => {
                        self.is_real = true;
                        self.sum += *n;
                    }
                    other => {
                        // Best-effort numeric coercion for SUM/AVG.
                        if let Ok(n) = match other {
                            SqlValue::Text(s) => s.parse::<f64>(),
                            SqlValue::Blob(b) => {
                                String::from_utf8_lossy(b).parse::<f64>()
                            }
                            _ => Ok(0.0),
                        } {
                            self.is_real = true;
                            self.sum += n;
                        }
                    }
                }
                match (&self.min, v) {
                    (None, v) => self.min = Some(v.clone()),
                    (Some(cur), v) => {
                        if compare_values(v, cur) == Ordering::Less {
                            self.min = Some(v.clone());
                        }
                    }
                }
                match (&self.max, v) {
                    (None, v) => self.max = Some(v.clone()),
                    (Some(cur), v) => {
                        if compare_values(v, cur) == Ordering::Greater {
                            self.max = Some(v.clone());
                        }
                    }
                }
            }
        }
    }
    fn finalize(self) -> SqlValue {
        match self.kind.as_str() {
            "count" => SqlValue::Integer(self.count),
            "sum" => {
                if !self.saw_any {
                    return SqlValue::Null;
                }
                if self.is_real || self.int_sum_overflow {
                    SqlValue::Real(self.sum)
                } else {
                    SqlValue::Integer(self.int_sum)
                }
            }
            "total" => SqlValue::Real(self.sum),
            "avg" => {
                if self.count == 0 {
                    SqlValue::Null
                } else {
                    SqlValue::Real(self.sum / self.count as f64)
                }
            }
            "min" => self.min.unwrap_or(SqlValue::Null),
            "max" => self.max.unwrap_or(SqlValue::Null),
            _ => SqlValue::Null,
        }
    }
}

/// Unused but referenced for compile-time wiring.
#[allow(dead_code)]
fn _arc_marker(_: Arc<str>) {}
