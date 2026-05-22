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

#[path = "window_eval/accumulator.rs"]
mod accumulator;
#[path = "window_eval/frame.rs"]
mod frame;
#[path = "window_eval/partition.rs"]
mod partition;

use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, FunctionArguments, SelectItem, WindowSpec, WindowType,
};

use crate::error::{Error, Result};
use crate::value::SqlValue;

use super::SqlRow;
use super::{cast_value, eval_scalar};

use accumulator::Accumulator;
use frame::{ResolvedFrame, frame_bounds, literal_i64, resolve_frame};
use partition::{assign_peer_ids, order_partition, partition_rows};

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
                || conditions
                    .iter()
                    .any(|w| expr_has_window(&w.condition) || expr_has_window(&w.result))
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
    let mut window_values: Vec<Vec<Vec<SqlValue>>> = Vec::with_capacity(projection.len());
    for item in projection {
        let calls = collect_window_calls(item);
        let mut per_call: Vec<Vec<SqlValue>> = Vec::with_capacity(calls.len());
        for call in &calls {
            per_call.push(eval_window_call(call, rows, bindings)?);
        }
        window_values.push(per_call);
    }

    let mut out = Vec::with_capacity(rows.len());
    for (row_idx, row) in rows.iter().enumerate() {
        let mut projected = Vec::with_capacity(projection.len());
        for (item_idx, item) in projection.iter().enumerate() {
            match item {
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                    for v in row.values()? {
                        projected.push(v);
                    }
                }
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                    let mut counter = 0usize;
                    projected.push(eval_with_window_values(
                        expr,
                        row,
                        bindings,
                        &window_values[item_idx],
                        row_idx,
                        &mut counter,
                    )?);
                }
            }
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
            direct_binary_op(op, l, r)
        }
        Expr::Nested(inner) => {
            eval_with_window_values(inner, row, bindings, window_values, row_idx, counter)
        }
        Expr::Cast {
            expr, data_type, ..
        } => {
            let value =
                eval_with_window_values(expr, row, bindings, window_values, row_idx, counter)?;
            cast_value(value, data_type)
        }
        _ => eval_scalar(expr, &row.context(), bindings),
    }
}

fn direct_binary_op(
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

    let partitions = partition_rows(rows, &window.partition_by, bindings)?;
    let frame = resolve_frame(window);

    let func_name = func.name.to_string().to_ascii_lowercase();
    let args = function_args(func);

    let mut results = vec![SqlValue::Null; rows.len()];
    for partition in &partitions {
        let sorted = order_partition(partition, rows, &window.order_by, bindings)?;
        let order_index_map: Vec<usize> = sorted.iter().map(|(idx, _)| *idx).collect();
        let peer_ids: Vec<usize> = if window.order_by.is_empty() {
            vec![0; sorted.len()]
        } else {
            assign_peer_ids(&sorted, &window.order_by)
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
        "ntile" => ntile_value(args, order_index_map.len(), sorted_pos),
        "lag" | "lead" => {
            lag_lead_value(func_name, args, rows, order_index_map, sorted_pos, bindings)
        }
        "first_value" => {
            let bounds = frame_bounds(frame, sorted_pos, peer_ids, order_index_map.len());
            if bounds.0 > bounds.1 {
                return Ok(SqlValue::Null);
            }
            let row_idx = order_index_map[bounds.0];
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
            let n = match args.get(1).and_then(literal_i64) {
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
                let value = match args.first() {
                    Some(expr) => eval_scalar(expr, &rows[row_idx].context(), bindings)?,
                    None => SqlValue::Integer(1),
                };
                accumulator.push(value);
            }
            // Reserved hook for future window-spec aware behavior.
            let _ = window;
            Ok(accumulator.finalize())
        }
        other => Err(Error::UnsupportedSql(format!(
            "window function not supported: {other}"
        ))),
    }
}

fn ntile_value(args: &[Expr], total: usize, sorted_pos: usize) -> Result<SqlValue> {
    let buckets = match args.first().and_then(literal_i64) {
        Some(n) if n > 0 => n as usize,
        _ => {
            return Err(Error::UnsupportedSql(
                "ntile(N) requires a positive integer literal".to_owned(),
            ));
        }
    };
    let base = total / buckets;
    let extras = total % buckets;
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

fn lag_lead_value(
    func_name: &str,
    args: &[Expr],
    rows: &[SqlRow],
    order_index_map: &[usize],
    sorted_pos: usize,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
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
