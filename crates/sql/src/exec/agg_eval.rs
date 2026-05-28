use super::*;
use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    static GROUP_EVAL_CACHE: RefCell<Option<ahash::AHashMap<usize, SqlValue>>> = const {
        RefCell::new(None)
    };
    static GROUP_AGG_CACHE: RefCell<Option<ahash::AHashMap<String, SqlValue>>> = const {
        RefCell::new(None)
    };
}

struct GroupEvalCacheGuard;

impl Drop for GroupEvalCacheGuard {
    fn drop(&mut self) {
        GROUP_EVAL_CACHE.with(|cache| {
            *cache.borrow_mut() = None;
        });
    }
}

struct GroupEvalScope {
    eval_prev: Option<ahash::AHashMap<usize, SqlValue>>,
    agg_prev: Option<ahash::AHashMap<String, SqlValue>>,
}

impl Drop for GroupEvalScope {
    fn drop(&mut self) {
        GROUP_EVAL_CACHE.with(|cache| {
            *cache.borrow_mut() = self.eval_prev.take();
        });
        GROUP_AGG_CACHE.with(|cache| {
            *cache.borrow_mut() = self.agg_prev.take();
        });
    }
}

pub(super) fn with_group_eval_cache<T>(f: impl FnOnce() -> T) -> T {
    let eval_prev =
        GROUP_EVAL_CACHE.with(|cache| cache.borrow_mut().replace(ahash::AHashMap::new()));
    let agg_prev = GROUP_AGG_CACHE.with(|cache| cache.borrow_mut().replace(ahash::AHashMap::new()));
    let _scope = GroupEvalScope {
        eval_prev,
        agg_prev,
    };
    f()
}

struct GroupCaseEvaluator<'group, 'ctx> {
    group: &'group [SqlRow],
    first_context: Option<&'ctx RowContext<'ctx>>,
    bindings: &'group [Option<SqlValue>],
}

impl<'group, 'ctx> CaseEvaluator for GroupCaseEvaluator<'group, 'ctx> {
    fn eval_case_expr(&mut self, expr: &Expr) -> Result<SqlValue> {
        eval_group_scalar_with_ctx(expr, self.group, self.first_context, self.bindings)
    }
}

pub(super) fn project_group_row(
    projection: &[SelectItem],
    group: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let first = group.first();
    let first_context = first.map(|row| row.context());
    let mut out = Vec::new();
    for item in projection {
        match item {
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                if let Some(row) = first {
                    out.extend(row.values()?);
                }
            }
            SelectItem::UnnamedExpr(expr) => out.push(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context.as_ref(),
                bindings,
            )?),
            SelectItem::ExprWithAlias { expr, .. } => out.push(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context.as_ref(),
                bindings,
            )?),
        }
    }
    Ok(out)
}

pub(super) fn eval_group_scalar_with_ctx(
    expr: &Expr,
    group: &[SqlRow],
    first_context: Option<&RowContext<'_>>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let cache_key = expr as *const Expr as usize;
    let mut cache_guard = None;
    let cached = GROUP_EVAL_CACHE.with(|cache| {
        let mut slot = cache.borrow_mut();
        if slot.is_none() {
            *slot = Some(ahash::AHashMap::new());
            cache_guard = Some(GroupEvalCacheGuard);
        }
        slot.as_ref()
            .and_then(|cache| cache.get(&cache_key).cloned())
    });
    if let Some(value) = cached {
        return Ok(value);
    }
    let result = if !expr_contains_aggregate(expr) {
        match first_context {
            Some(ctx) => eval_scalar(expr, ctx, bindings),
            None => eval_scalar(expr, &RowContext::Empty, bindings),
        }
    } else {
        match expr {
            Expr::Function(func) => {
                eval_group_function_or_scalar(func, group, first_context, bindings)
            }
            Expr::BinaryOp { left, op, right } => {
                let left_value = eval_group_scalar_with_ctx(left, group, first_context, bindings)?;
                let right_value =
                    eval_group_scalar_with_ctx(right, group, first_context, bindings)?;
                Ok(match op {
                    BinaryOperator::And => {
                        match (truthy_opt(&left_value), truthy_opt(&right_value)) {
                            (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
                            (Some(true), Some(true)) => SqlValue::Integer(1),
                            _ => SqlValue::Null,
                        }
                    }
                    BinaryOperator::Or => match (truthy_opt(&left_value), truthy_opt(&right_value))
                    {
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
                        compare_binary(left_value, right_value, |o| o == Ordering::Equal)?
                    }
                    BinaryOperator::NotEq | BinaryOperator::Spaceship => {
                        compare_binary(left_value, right_value, |o| o != Ordering::Equal)?
                    }
                    BinaryOperator::Gt => {
                        compare_binary(left_value, right_value, |o| o == Ordering::Greater)?
                    }
                    BinaryOperator::GtEq => {
                        compare_binary(left_value, right_value, |o| o != Ordering::Less)?
                    }
                    BinaryOperator::Lt => {
                        compare_binary(left_value, right_value, |o| o == Ordering::Less)?
                    }
                    BinaryOperator::LtEq => {
                        compare_binary(left_value, right_value, |o| o != Ordering::Greater)?
                    }
                    BinaryOperator::StringConcat => {
                        if matches!(left_value, SqlValue::Null)
                            || matches!(right_value, SqlValue::Null)
                        {
                            SqlValue::Null
                        } else {
                            SqlValue::Text(Arc::from(format!(
                                "{}{}",
                                value_to_string(&left_value),
                                value_to_string(&right_value)
                            )))
                        }
                    }
                    other => {
                        return Err(Error::UnsupportedSql(format!(
                            "unsupported binary op {other:?}"
                        )));
                    }
                })
            }
            Expr::UnaryOp { op, expr } => {
                let value = eval_group_scalar_with_ctx(expr, group, first_context, bindings)?;
                match op {
                    UnaryOperator::Not => match truthy_opt(&value) {
                        Some(v) => Ok(SqlValue::Integer(if !v { 1 } else { 0 })),
                        None => Ok(SqlValue::Null),
                    },
                    UnaryOperator::Minus => negate(value),
                    UnaryOperator::Plus => Ok(value),
                    _ => Err(Error::UnsupportedSql(format!(
                        "unsupported unary op {op:?}"
                    ))),
                }
            }
            Expr::Nested(expr) => eval_group_scalar_with_ctx(expr, group, first_context, bindings),
            Expr::Cast {
                expr, data_type, ..
            } => cast_value(
                eval_group_scalar_with_ctx(expr, group, first_context, bindings)?,
                data_type,
            ),
            Expr::Between {
                expr,
                negated,
                low,
                high,
            } => {
                let value = eval_group_scalar_with_ctx(expr, group, first_context, bindings)?;
                let low = eval_group_scalar_with_ctx(low, group, first_context, bindings)?;
                let high = eval_group_scalar_with_ctx(high, group, first_context, bindings)?;
                if matches!(value, SqlValue::Null)
                    || matches!(low, SqlValue::Null)
                    || matches!(high, SqlValue::Null)
                {
                    Ok(SqlValue::Null)
                } else {
                    let mut ok = compare_values(&value, &low) != Ordering::Less
                        && compare_values(&value, &high) != Ordering::Greater;
                    if *negated {
                        ok = !ok;
                    }
                    Ok(SqlValue::Integer(if ok { 1 } else { 0 }))
                }
            }
            Expr::InList {
                expr,
                list,
                negated,
            } => {
                let value = eval_group_scalar_with_ctx(expr, group, first_context, bindings)?;
                if matches!(value, SqlValue::Null) {
                    Ok(SqlValue::Null)
                } else {
                    let mut found = false;
                    let mut saw_null = false;
                    for item in list {
                        let candidate =
                            eval_group_scalar_with_ctx(item, group, first_context, bindings)?;
                        match candidate {
                            SqlValue::Null => saw_null = true,
                            _ if compare_values(&value, &candidate) == Ordering::Equal => {
                                found = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                    let mut ok = found;
                    if *negated {
                        ok = !ok;
                    }
                    if !ok && saw_null {
                        Ok(SqlValue::Null)
                    } else {
                        Ok(SqlValue::Integer(if ok { 1 } else { 0 }))
                    }
                }
            }
            Expr::IsNull(expr) => Ok(SqlValue::Integer(
                if matches!(
                    eval_group_scalar_with_ctx(expr, group, first_context, bindings)?,
                    SqlValue::Null
                ) {
                    1
                } else {
                    0
                },
            )),
            Expr::IsNotNull(expr) => Ok(SqlValue::Integer(
                if !matches!(
                    eval_group_scalar_with_ctx(expr, group, first_context, bindings)?,
                    SqlValue::Null
                ) {
                    1
                } else {
                    0
                },
            )),
            Expr::IsTrue(expr) => Ok(sql_truth_result(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context,
                bindings,
            )?)),
            Expr::IsNotTrue(expr) => Ok(sql_truth_result_not(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context,
                bindings,
            )?)),
            Expr::IsFalse(expr) => Ok(sql_false_result(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context,
                bindings,
            )?)),
            Expr::IsNotFalse(expr) => Ok(sql_false_result_not(eval_group_scalar_with_ctx(
                expr,
                group,
                first_context,
                bindings,
            )?)),
            Expr::IsUnknown(expr) => Ok(SqlValue::Integer(
                if matches!(
                    eval_group_scalar_with_ctx(expr, group, first_context, bindings)?,
                    SqlValue::Null
                ) {
                    1
                } else {
                    0
                },
            )),
            Expr::IsNotUnknown(expr) => Ok(SqlValue::Integer(
                if !matches!(
                    eval_group_scalar_with_ctx(expr, group, first_context, bindings)?,
                    SqlValue::Null
                ) {
                    1
                } else {
                    0
                },
            )),
            Expr::Case {
                operand,
                conditions,
                else_result,
                ..
            } => {
                let mut evaluator = GroupCaseEvaluator {
                    group,
                    first_context,
                    bindings,
                };
                eval_case(
                    operand.as_deref(),
                    conditions,
                    else_result.as_deref(),
                    &mut evaluator,
                )
            }
            _ => Err(Error::UnsupportedSql(
                "aggregate expressions in this query are not supported".to_owned(),
            )),
        }
    }?;
    GROUP_EVAL_CACHE.with(|cache| {
        if let Some(slot) = cache.borrow_mut().as_mut() {
            slot.insert(cache_key, result.clone());
        }
    });
    drop(cache_guard);
    Ok(result)
}

fn eval_group_function_or_scalar(
    func: &sqlparser::ast::Function,
    group: &[SqlRow],
    first_context: Option<&RowContext<'_>>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let name = func.name.to_string().to_ascii_lowercase();
    if is_aggregate_call(func, &name) {
        return eval_group_function(func, group, bindings);
    }

    let FunctionArguments::List(list) = &func.args else {
        return Err(Error::UnsupportedSql(
            "unsupported aggregate function call form".to_owned(),
        ));
    };
    let mut values = Vec::with_capacity(list.args.len());
    for arg in &list.args {
        match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                values.push(eval_group_scalar_with_ctx(
                    expr,
                    group,
                    first_context,
                    bindings,
                )?);
            }
            _ => {
                return Err(Error::UnsupportedSql(
                    "unsupported function argument".to_owned(),
                ));
            }
        }
    }

    crate::exec::expr::json_dispatch::eval_scalar_function_values(&name, values)
}

fn is_aggregate_call(func: &sqlparser::ast::Function, name: &str) -> bool {
    if matches!(name, "min" | "max") {
        return function_arg_count(func) == Some(1);
    }
    is_builtin_aggregate_name(name) || crate::udf::is_registered_aggregate(name)
}

fn function_arg_count(func: &sqlparser::ast::Function) -> Option<usize> {
    let FunctionArguments::List(list) = &func.args else {
        return None;
    };
    Some(list.args.len())
}

fn is_builtin_aggregate_name(name: &str) -> bool {
    matches!(
        name,
        "count"
            | "sum"
            | "avg"
            | "median"
            | "percentile_cont"
            | "min"
            | "max"
            | "group_concat"
            | "string_agg"
            | "total"
            | "json_group_array"
            | "json_group_object"
    )
}

fn aggregate_order_by(func: &sqlparser::ast::Function) -> Result<Option<&[OrderByExpr]>> {
    let FunctionArguments::List(list) = &func.args else {
        return Ok(None);
    };
    let mut order_by = None;
    for clause in &list.clauses {
        match clause {
            FunctionArgumentClause::OrderBy(items) => order_by = Some(items.as_slice()),
            other => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported aggregate function clause: {other:?}"
                )));
            }
        }
    }
    Ok(order_by)
}

fn rows_in_aggregate_order<'a>(
    func: &sqlparser::ast::Function,
    group: &'a [SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<&'a SqlRow>> {
    let Some(order_by) = aggregate_order_by(func)? else {
        let mut rows = Vec::with_capacity(group.len());
        for row in group {
            if row_passes_aggregate_filter(func, row, bindings)? {
                rows.push(row);
            }
        }
        return Ok(rows);
    };

    let mut keyed = Vec::with_capacity(group.len());
    for (idx, row) in group.iter().enumerate() {
        if !row_passes_aggregate_filter(func, row, bindings)? {
            continue;
        }
        let ctx = row.context();
        let mut keys = Vec::with_capacity(order_by.len());
        for order in order_by {
            keys.push(eval_scalar(&order.expr, &ctx, bindings)?);
        }
        keyed.push((idx, row, keys));
    }

    keyed.sort_by(|(left_idx, _, left_keys), (right_idx, _, right_keys)| {
        for (idx, order) in order_by.iter().enumerate() {
            let ord = compare_values(&left_keys[idx], &right_keys[idx]);
            if ord != Ordering::Equal {
                return if matches!(order.options.asc, Some(false)) {
                    ord.reverse()
                } else {
                    ord
                };
            }
        }
        left_idx.cmp(right_idx)
    });

    Ok(keyed.into_iter().map(|(_, row, _)| row).collect())
}

fn row_passes_aggregate_filter(
    func: &sqlparser::ast::Function,
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<bool> {
    let Some(filter) = &func.filter else {
        return Ok(true);
    };
    let ctx = row.context();
    Ok(is_truthy(&eval_scalar(filter, &ctx, bindings)?))
}

/// True if the function call is `<name>(DISTINCT ...)`.
fn is_distinct_call(func: &sqlparser::ast::Function) -> bool {
    if let FunctionArguments::List(list) = &func.args {
        matches!(
            list.duplicate_treatment,
            Some(sqlparser::ast::DuplicateTreatment::Distinct)
        )
    } else {
        false
    }
}

/// If `expr` is a `COLLATE` wrapper, return the named collation.
fn expr_collation(expr: &Expr) -> Option<crate::collation::Collation> {
    if let Expr::Collate { collation, .. } = expr {
        let name = collation.to_string();
        crate::collation::Collation::parse(&name)
    } else {
        None
    }
}

/// Build a deduplication key for a single aggregate value, honoring a
/// collation (e.g. `count(DISTINCT x COLLATE NOCASE)` should treat
/// `'a'` and `'A'` as equal). Returns `None` for NULL.
fn distinct_key(
    value: &SqlValue,
    collation: Option<&crate::collation::Collation>,
) -> Result<Option<Vec<u8>>> {
    if matches!(value, SqlValue::Null) {
        return Ok(None);
    }
    let normalised = match (value, collation) {
        (SqlValue::Text(s), Some(crate::collation::Collation::NoCase)) => {
            SqlValue::Text(Arc::from(s.to_ascii_lowercase()))
        }
        (SqlValue::Text(s), Some(crate::collation::Collation::RTrim)) => {
            SqlValue::Text(Arc::from(s.trim_end_matches(' ').to_owned()))
        }
        _ => value.clone(),
    };
    let key = vec::hash_agg::encode_group_key_bytes(&[normalised])?;
    Ok(Some(key))
}

fn eval_group_function(
    func: &sqlparser::ast::Function,
    group: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    // Phase 4.2: borrow the function name into a stack buffer for the
    // dispatch match instead of allocating + lowercasing the full
    // ObjectName Display form. Mirrors the Phase 1.3 fast path in
    // scalar function dispatch.
    let mut name_scratch = [0u8; crate::exec::expr::json_dispatch::FN_NAME_STACK];
    let borrowed_name =
        crate::exec::expr::json_dispatch::simple_function_name_lower(func, &mut name_scratch);
    let owned_name;
    let name: &str = match borrowed_name {
        Some(s) => s,
        None => {
            owned_name = func.name.to_string().to_ascii_lowercase();
            owned_name.as_str()
        }
    };

    // Phase 4.2: aggregate_cache_key renders the full function AST
    // exactly once, lowercases it in place, and uses the same lowercased
    // String as both the volatility check input and the cache key.
    // Replaces the prior double-render (cacheable check + cache_key
    // each called `func.to_string()` separately).
    let cache_key = aggregate_cache_key(func);
    if let Some(cache_key) = &cache_key
        && let Some(value) = GROUP_AGG_CACHE.with(|cache| {
            cache
                .borrow()
                .as_ref()
                .and_then(|cache| cache.get(cache_key).cloned())
        })
    {
        return Ok(value);
    }
    let result = match name {
        "count" => {
            if let FunctionArguments::List(list) = &func.args {
                if list.args.len() == 1
                    && matches!(
                        list.args[0],
                        FunctionArg::Unnamed(FunctionArgExpr::Wildcard)
                    )
                {
                    let mut count = 0i64;
                    for row in group {
                        if row_passes_aggregate_filter(func, row, bindings)? {
                            count += 1;
                        }
                    }
                    Ok(SqlValue::Integer(count))
                } else {
                    let distinct = matches!(
                        list.duplicate_treatment,
                        Some(sqlparser::ast::DuplicateTreatment::Distinct)
                    );
                    // Per-argument collation: `count(DISTINCT x COLLATE NOCASE)`
                    // dedupes by case-insensitive text key.
                    let collations: Vec<Option<crate::collation::Collation>> = list
                        .args
                        .iter()
                        .map(|a| match a {
                            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                                expr_collation(expr)
                            }
                            _ => None,
                        })
                        .collect();
                    let mut seen = HashSet::new();
                    let mut count = 0i64;
                    for row in group {
                        if !row_passes_aggregate_filter(func, row, bindings)? {
                            continue;
                        }
                        let ctx = row.context();
                        let mut include = true;
                        let mut values = Vec::new();
                        for arg in &list.args {
                            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) = arg {
                                let value = eval_scalar(expr, &ctx, bindings)?;
                                if matches!(value, SqlValue::Null) {
                                    include = false;
                                } else {
                                    values.push(value);
                                }
                            }
                        }
                        if include {
                            if distinct {
                                // Build a per-arg dedup key honoring each
                                // argument's collation wrapper.
                                let mut normalised: Vec<SqlValue> =
                                    Vec::with_capacity(values.len());
                                for (idx, val) in values.iter().enumerate() {
                                    normalised.push(
                                        match (val, collations.get(idx).and_then(|c| c.as_ref())) {
                                            (
                                                SqlValue::Text(s),
                                                Some(crate::collation::Collation::NoCase),
                                            ) => SqlValue::Text(Arc::from(s.to_ascii_lowercase())),
                                            (
                                                SqlValue::Text(s),
                                                Some(crate::collation::Collation::RTrim),
                                            ) => SqlValue::Text(Arc::from(
                                                s.trim_end_matches(' ').to_owned(),
                                            )),
                                            _ => val.clone(),
                                        },
                                    );
                                }
                                let key = vec::hash_agg::encode_group_key_bytes(&normalised)?;
                                if !seen.insert(key) {
                                    continue;
                                }
                            }
                            count += 1;
                        }
                    }
                    Ok(SqlValue::Integer(count))
                }
            } else {
                let mut count = 0i64;
                for row in group {
                    if row_passes_aggregate_filter(func, row, bindings)? {
                        count += 1;
                    }
                }
                Ok(SqlValue::Integer(count))
            }
        }
        "sum" => {
            let mut total_i: i64 = 0;
            let mut total_r: f64 = 0.0;
            let mut saw_real = false;
            let mut saw_value = false;
            let distinct = is_distinct_call(func);
            let mut seen: HashSet<Vec<u8>> = HashSet::new();
            let collation = if let FunctionArguments::List(list) = &func.args {
                list.args.first().and_then(|a| match a {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => expr_collation(expr),
                    _ => None,
                })
            } else {
                None
            };
            for row in group {
                if !row_passes_aggregate_filter(func, row, bindings)? {
                    continue;
                }
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let value = eval_scalar(expr, &ctx, bindings)?;
                    if distinct {
                        match distinct_key(&value, collation.as_ref())? {
                            None => continue,
                            Some(key) => {
                                if !seen.insert(key) {
                                    continue;
                                }
                            }
                        }
                    }
                    match value {
                        SqlValue::Null => {}
                        SqlValue::Integer(v) if !saw_real => {
                            total_i += v;
                            saw_value = true;
                        }
                        SqlValue::Integer(v) => {
                            total_r += v as f64;
                            saw_value = true;
                        }
                        SqlValue::Real(v) => {
                            if !saw_real {
                                total_r = total_i as f64;
                                saw_real = true;
                            }
                            total_r += v;
                            saw_value = true;
                        }
                        other => {
                            let real = value_to_string(&other)
                                .trim()
                                .parse::<f64>()
                                .map_err(|_| Error::DatatypeMismatch)?;
                            if !saw_real {
                                total_r = total_i as f64;
                                saw_real = true;
                            }
                            total_r += real;
                            saw_value = true;
                        }
                    }
                }
            }
            if !saw_value {
                Ok(SqlValue::Null)
            } else if saw_real {
                Ok(canonicalize(SqlValue::Real(total_r)))
            } else {
                Ok(SqlValue::Integer(total_i))
            }
        }
        "avg" => {
            let mut count = 0i64;
            let mut sum = 0.0f64;
            let distinct = is_distinct_call(func);
            let mut seen: HashSet<Vec<u8>> = HashSet::new();
            let collation = if let FunctionArguments::List(list) = &func.args {
                list.args.first().and_then(|a| match a {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => expr_collation(expr),
                    _ => None,
                })
            } else {
                None
            };
            for row in group {
                if !row_passes_aggregate_filter(func, row, bindings)? {
                    continue;
                }
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let value = eval_scalar(expr, &ctx, bindings)?;
                    if distinct {
                        match distinct_key(&value, collation.as_ref())? {
                            None => continue,
                            Some(key) => {
                                if !seen.insert(key) {
                                    continue;
                                }
                            }
                        }
                    }
                    match value {
                        SqlValue::Null => {}
                        SqlValue::Integer(v) => {
                            sum += v as f64;
                            count += 1;
                        }
                        SqlValue::Real(v) => {
                            sum += v;
                            count += 1;
                        }
                        other => {
                            sum += value_to_string(&other)
                                .trim()
                                .parse::<f64>()
                                .map_err(|_| Error::DatatypeMismatch)?;
                            count += 1;
                        }
                    }
                }
            }
            if count == 0 {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Real(sum / count as f64))
            }
        }
        "median" | "percentile_cont" => {
            let percentile = if name == "median" {
                0.5
            } else {
                percentile_argument(func, group, bindings)?
            };
            if !(0.0..=1.0).contains(&percentile) {
                return Err(Error::DatatypeMismatch);
            }

            let mut values = Vec::new();
            for row in group {
                if !row_passes_aggregate_filter(func, row, bindings)? {
                    continue;
                }
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let value = eval_scalar(expr, &ctx, bindings)?;
                    if !matches!(value, SqlValue::Null) {
                        values.push(numeric_aggregate_value(value)?);
                    }
                }
            }
            if values.is_empty() {
                return Ok(SqlValue::Null);
            }
            values.sort_by(|left, right| left.total_cmp(right));
            let rank = percentile * (values.len().saturating_sub(1) as f64);
            let lower = rank.floor() as usize;
            let upper = rank.ceil() as usize;
            let fraction = rank - lower as f64;
            let result = if lower == upper {
                values[lower]
            } else {
                values[lower] + (values[upper] - values[lower]) * fraction
            };
            Ok(SqlValue::Real(result))
        }
        "min" | "max" => {
            let mut best: Option<SqlValue> = None;
            for row in group {
                if !row_passes_aggregate_filter(func, row, bindings)? {
                    continue;
                }
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let value = eval_scalar(expr, &ctx, bindings)?;
                    if matches!(value, SqlValue::Null) {
                        continue;
                    }
                    best = match best {
                        None => Some(value),
                        Some(current) => {
                            let ord = compare_values(&value, &current);
                            if (name == "min" && ord == Ordering::Less)
                                || (name == "max" && ord == Ordering::Greater)
                            {
                                Some(value)
                            } else {
                                Some(current)
                            }
                        }
                    };
                }
            }
            Ok(best.unwrap_or(SqlValue::Null))
        }
        // SQLite total(X) — NULL-safe sum: returns 0.0 when all values are NULL
        // (unlike sum() which returns NULL). Always returns a real.
        "total" => {
            let mut acc = 0.0f64;
            for row in group {
                if !row_passes_aggregate_filter(func, row, bindings)? {
                    continue;
                }
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    match eval_scalar(expr, &ctx, bindings)? {
                        SqlValue::Null => {}
                        SqlValue::Integer(v) => acc += v as f64,
                        SqlValue::Real(v) => acc += v,
                        other => {
                            acc += value_to_string(&other).trim().parse::<f64>().unwrap_or(0.0);
                        }
                    }
                }
            }
            Ok(SqlValue::Real(acc))
        }
        // SQLite group_concat(X) / group_concat(X, sep) — concatenates
        // non-NULL values with sep (default ','). string_agg(X, sep) is an alias.
        "group_concat" | "string_agg" => {
            let sep = if let FunctionArguments::List(list) = &func.args
                && list.args.len() >= 2
            {
                if let FunctionArg::Unnamed(FunctionArgExpr::Expr(sep_expr)) = &list.args[1] {
                    // Evaluate the separator once from any row (it's a constant expr).
                    let ctx = group
                        .first()
                        .map(|r| r.context())
                        .unwrap_or(RowContext::Empty);
                    let sep_val = eval_scalar(sep_expr, &ctx, bindings)?;
                    value_to_string(&sep_val)
                } else {
                    ",".to_owned()
                }
            } else {
                ",".to_owned()
            };
            let mut parts: Vec<String> = Vec::new();
            let distinct = is_distinct_call(func);
            let mut seen: HashSet<Vec<u8>> = HashSet::new();
            let collation = if let FunctionArguments::List(list) = &func.args {
                list.args.first().and_then(|a| match a {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => expr_collation(expr),
                    _ => None,
                })
            } else {
                None
            };
            for row in rows_in_aggregate_order(func, group, bindings)? {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let val = eval_scalar(expr, &ctx, bindings)?;
                    if matches!(val, SqlValue::Null) {
                        continue;
                    }
                    if distinct {
                        match distinct_key(&val, collation.as_ref())? {
                            None => continue,
                            Some(key) => {
                                if !seen.insert(key) {
                                    continue;
                                }
                            }
                        }
                    }
                    parts.push(value_to_string(&val));
                }
            }
            if parts.is_empty() {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Text(Arc::from(parts.join(&sep))))
            }
        }
        // json_group_array(X) — collects non-NULL values into a JSON array.
        "json_group_array" => {
            use crate::json::scalar::sql_to_json_value;
            let mut arr = Vec::new();
            for row in rows_in_aggregate_order(func, group, bindings)? {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let val = eval_scalar(expr, &ctx, bindings)?;
                    arr.push(sql_to_json_value(&val)?);
                }
            }
            let json = serde_json::Value::Array(arr);
            Ok(SqlValue::Text(Arc::from(json.to_string())))
        }
        // json_group_object(K, V) — builds a JSON object from key/value pairs.
        "json_group_object" => {
            use crate::json::scalar::sql_to_json_value;
            use serde_json::Map;
            let mut obj: Map<String, serde_json::Value> = Map::new();
            for row in rows_in_aggregate_order(func, group, bindings)? {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && list.args.len() >= 2
                {
                    let key = if let FunctionArg::Unnamed(FunctionArgExpr::Expr(k)) = &list.args[0]
                    {
                        eval_scalar(k, &ctx, bindings)?
                    } else {
                        SqlValue::Null
                    };
                    let val = if let FunctionArg::Unnamed(FunctionArgExpr::Expr(v)) = &list.args[1]
                    {
                        eval_scalar(v, &ctx, bindings)?
                    } else {
                        SqlValue::Null
                    };
                    if !matches!(key, SqlValue::Null) {
                        obj.insert(value_to_string(&key), sql_to_json_value(&val)?);
                    }
                }
            }
            let json = serde_json::Value::Object(obj);
            Ok(SqlValue::Text(Arc::from(json.to_string())))
        }
        _ => {
            // Unknown name — try the registered UDF aggregate registry.
            // We materialize each group row's single-argument-list values
            // into a row vector so the C-callback xStep sees them with the
            // declared arity. NULL-skip semantics match SQLite: an
            // aggregate UDF receives every row (including those where its
            // argument evaluates to NULL); it's the UDF's responsibility
            // to choose whether to ignore them. This matches SQLite's
            // documented contract for `sqlite3_create_function*`.
            let rows: Vec<Vec<SqlValue>> = {
                let mut out = Vec::with_capacity(group.len());
                let arg_exprs: Vec<&Expr> = match &func.args {
                    FunctionArguments::List(list) => list
                        .args
                        .iter()
                        .filter_map(|arg| match arg {
                            FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => Some(e),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                for row in group {
                    let ctx = row.context();
                    let mut row_values = Vec::with_capacity(arg_exprs.len());
                    for expr in &arg_exprs {
                        row_values.push(eval_scalar(expr, &ctx, bindings)?);
                    }
                    out.push(row_values);
                }
                out
            };
            let db = crate::udf::current_db();
            match crate::udf::call_registered_aggregate(db, name, &rows) {
                Some(Ok(v)) => Ok(v),
                Some(Err(msg)) => Err(Error::UnsupportedSql(msg)),
                None => Err(Error::UnsupportedSql(format!(
                    "unsupported aggregate function: {name}"
                ))),
            }
        }
    }?;
    if let Some(cache_key) = cache_key {
        GROUP_AGG_CACHE.with(|cache| {
            if let Some(cache) = cache.borrow_mut().as_mut() {
                cache.insert(cache_key, result.clone());
            }
        });
    }
    Ok(result)
}

/// Phase 4.2: single-render cache-key + cacheability check.
///
/// Old shape allocated `func.to_string()` once in `aggregate_cacheable`
/// (for the volatility check) and again in `eval_group_function` (for
/// the cache key), then lowercased one of them. Both renders walk the
/// full Function AST.
///
/// New shape: render once into a single String, lowercase in place via
/// `make_ascii_lowercase`, then both decide cacheability AND use the
/// lowercased form as the cache key. Returns `None` for any volatile
/// function (random/randomblob/last_insert_rowid/changes/total_changes
/// /current_date/current_time/current_timestamp), `Some(lowercased)`
/// otherwise.
fn aggregate_cache_key(func: &sqlparser::ast::Function) -> Option<String> {
    let mut rendered = func.to_string();
    rendered.make_ascii_lowercase();
    let is_volatile = rendered.contains("random(")
        || rendered.contains("randomblob(")
        || rendered.contains("last_insert_rowid")
        || rendered.contains("changes(")
        || rendered.contains("total_changes(")
        || rendered.contains("current_date")
        || rendered.contains("current_time")
        || rendered.contains("current_timestamp");
    if is_volatile { None } else { Some(rendered) }
}

fn percentile_argument(
    func: &sqlparser::ast::Function,
    group: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<f64> {
    let FunctionArguments::List(list) = &func.args else {
        return Err(Error::UnsupportedSql(
            "percentile_cont requires two arguments".to_owned(),
        ));
    };
    if list.args.len() != 2 {
        return Err(Error::UnsupportedSql(
            "percentile_cont requires two arguments".to_owned(),
        ));
    }
    let FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) = &list.args[1] else {
        return Err(Error::UnsupportedSql(
            "unsupported percentile_cont argument".to_owned(),
        ));
    };
    let ctx = group
        .first()
        .map(|row| row.context())
        .unwrap_or(RowContext::Empty);
    numeric_aggregate_value(eval_scalar(expr, &ctx, bindings)?)
}

fn numeric_aggregate_value(value: SqlValue) -> Result<f64> {
    match value {
        SqlValue::Null => Ok(f64::NAN),
        SqlValue::Integer(v) => Ok(v as f64),
        SqlValue::Real(v) => Ok(v),
        other => value_to_string(&other)
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::DatatypeMismatch),
    }
}
