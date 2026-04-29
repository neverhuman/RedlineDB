use super::*;

pub(super) fn select_requires_aggregation(plan: &crate::statement::SelectPlan) -> bool {
    !plan.group_by.is_empty()
        || plan.having.as_ref().is_some_and(expr_contains_aggregate)
        || plan.projection.iter().any(select_item_contains_aggregate)
}

fn select_item_contains_aggregate(item: &SelectItem) -> bool {
    match item {
        SelectItem::UnnamedExpr(expr) => expr_contains_aggregate(expr),
        SelectItem::ExprWithAlias { expr, .. } => expr_contains_aggregate(expr),
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
    }
}

fn expr_contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Function(func) => {
            let name = func.name.to_string().to_ascii_lowercase();
            matches!(name.as_str(), "count" | "sum" | "avg" | "min" | "max")
        }
        Expr::BinaryOp { left, right, .. } => {
            expr_contains_aggregate(left) || expr_contains_aggregate(right)
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) | Expr::Cast { expr, .. } => {
            expr_contains_aggregate(expr)
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            expr_contains_aggregate(expr)
                || expr_contains_aggregate(low)
                || expr_contains_aggregate(high)
        }
        Expr::InList { expr, list, .. } => {
            expr_contains_aggregate(expr) || list.iter().any(expr_contains_aggregate)
        }
        Expr::IsNull(expr)
        | Expr::IsNotNull(expr)
        | Expr::IsTrue(expr)
        | Expr::IsNotTrue(expr)
        | Expr::IsFalse(expr)
        | Expr::IsNotFalse(expr)
        | Expr::IsUnknown(expr)
        | Expr::IsNotUnknown(expr) => expr_contains_aggregate(expr),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            operand.as_deref().is_some_and(expr_contains_aggregate)
                || conditions.iter().any(|when| {
                    expr_contains_aggregate(&when.condition)
                        || expr_contains_aggregate(&when.result)
                })
                || else_result.as_deref().is_some_and(expr_contains_aggregate)
        }
        _ => false,
    }
}

pub(super) fn execute_grouped_select(
    plan: &crate::statement::SelectPlan,
    rows: Vec<SqlRow>,
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Vec<Vec<SqlValue>>> {
    let mut filtered = Vec::new();
    for row in rows {
        if selection_passes(&plan.selection, &row, bindings)? {
            filtered.push(row);
        }
    }

    if plan.distinct && plan.group_by.is_empty() {
        let mut out = Vec::with_capacity(filtered.len());
        let memory_bytes = filtered.iter().try_fold(0usize, |acc, row| {
            row.values().map(|values| acc + row_width(&values))
        })?;
        memory.request(memory_bytes)?;
        for row in filtered {
            let first_context = Some(row.context());
            if let Some(having) = &plan.having
                && !is_truthy(&eval_group_scalar_with_ctx(
                    having,
                    std::slice::from_ref(&row),
                    first_context.as_ref(),
                    bindings,
                )?)
            {
                continue;
            }
            out.push(project_row(&plan.projection, &row, bindings)?);
        }
        // Sort + dedup on full projected-row equality so DISTINCT eliminates
        // duplicates.
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        // Then if ORDER BY was requested, re-sort the deduped projection by
        // the requested keys so DESC / ASC and explicit column references
        // are honored. ORDER BY here resolves against output columns.
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    let groups = if plan.group_by.is_empty() {
        vec![filtered]
    } else {
        // Lane VE hash-aggregation: replace the O(n^2) linear-find group
        // build with an encoded-key HashMap. Insertion order is preserved
        // via a parallel `Vec<usize>` so EXPLAIN-stable orderings remain
        // unchanged (callers that need ORDER BY apply their own sort).
        use std::collections::HashMap;
        let mut index_by_key: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut groups: Vec<Vec<SqlRow>> = Vec::new();
        for row in filtered {
            let key = eval_group_key(&plan.group_by, &row, bindings)?;
            let key_bytes = vec::hash_agg::encode_group_key_bytes(&key)?;
            match index_by_key.get(&key_bytes) {
                Some(&idx) => groups[idx].push(row),
                None => {
                    index_by_key.insert(key_bytes, groups.len());
                    groups.push(vec![row]);
                }
            }
        }
        groups
    };

    let memory_bytes = groups.iter().try_fold(0usize, |acc, group| {
        let group_bytes = group.iter().try_fold(0usize, |group_acc, row| {
            row.values().map(|values| group_acc + row_width(&values))
        })?;
        Ok::<usize, Error>(acc + group_bytes)
    })?;
    memory.request(memory_bytes)?;

    let mut out = Vec::new();
    if plan.distinct {
        for group in groups {
            let first_context = group.first().map(|row| row.context());
            if group.is_empty() && !plan.projection.iter().any(select_item_contains_aggregate) {
                continue;
            }
            if let Some(having) = &plan.having
                && !is_truthy(&eval_group_scalar_with_ctx(
                    having,
                    &group,
                    first_context.as_ref(),
                    bindings,
                )?)
            {
                continue;
            }
            out.push(project_group_row(&plan.projection, &group, bindings)?);
        }
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    // Track which groups survived HAVING so we can re-evaluate ORDER BY keys
    // against the same group contexts that produced each projected row.
    let mut surviving_groups: Vec<&[SqlRow]> = Vec::with_capacity(groups.len());
    for group in &groups {
        let first_context = group.first().map(|row| row.context());
        if group.is_empty() && !plan.projection.iter().any(select_item_contains_aggregate) {
            continue;
        }
        if let Some(having) = &plan.having
            && !is_truthy(&eval_group_scalar_with_ctx(
                having,
                group,
                first_context.as_ref(),
                bindings,
            )?)
        {
            continue;
        }
        out.push(project_group_row(&plan.projection, group, bindings)?);
        surviving_groups.push(group.as_slice());
    }

    if !plan.order_by.is_empty() {
        // Evaluate ORDER BY keys against the same grouped row contexts used
        // for projection so aggregate aliases (e.g. `COUNT(*) AS c ... ORDER
        // BY c DESC`) and aggregate functions in ORDER BY both work and
        // ASC / DESC is honored.
        sort_groups_by_order_by(
            &mut out,
            &surviving_groups,
            &plan.projection,
            &plan.order_by,
            bindings,
        )?;
    }

    Ok(out.into_iter().skip(offset).take(limit).collect())
}

/// Reorder `projected` rows in-place by evaluating `order_by` against the
/// grouped row contexts that produced them. Output column aliases declared in
/// `projection` are resolved before falling through to direct evaluation
/// (which handles aggregates and group-by columns alike).
fn sort_groups_by_order_by(
    projected: &mut [Vec<SqlValue>],
    groups: &[&[SqlRow]],
    projection: &[SelectItem],
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<()> {
    if projected.len() != groups.len() {
        // Defensive: if projection arity drifts, fall back to a stable
        // projected-row compare. Should not happen in current call sites.
        projected.sort_by(|l, r| compare_rows(l, r));
        return Ok(());
    }
    let mut keys: Vec<Vec<SqlValue>> = Vec::with_capacity(groups.len());
    for group in groups {
        let mut row_keys = Vec::with_capacity(order_by.len());
        for order in order_by {
            row_keys.push(eval_grouped_order_key(
                &order.expr,
                group,
                projection,
                bindings,
            )?);
        }
        keys.push(row_keys);
    }
    let mut indices: Vec<usize> = (0..projected.len()).collect();
    indices.sort_by(|&a, &b| {
        for (idx, order) in order_by.iter().enumerate() {
            let mut ord = compare_values(&keys[a][idx], &keys[b][idx]);
            if matches!(order.options.asc, Some(false)) {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
    let mut sorted: Vec<Vec<SqlValue>> = Vec::with_capacity(projected.len());
    for idx in indices {
        sorted.push(projected[idx].clone());
    }
    projected.clone_from_slice(&sorted);
    Ok(())
}

fn eval_grouped_order_key(
    expr: &Expr,
    group: &[SqlRow],
    projection: &[SelectItem],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    // ORDER BY may reference projection aliases by name. If the order
    // expression is a bare identifier matching an aliased projection, evaluate
    // the underlying aliased expression instead — this is what makes
    // `COUNT(*) AS c ... ORDER BY c DESC` work.
    if let Expr::Identifier(ident) = expr {
        for item in projection {
            if let SelectItem::ExprWithAlias { alias, expr } = item
                && alias.value.eq_ignore_ascii_case(&ident.value)
            {
                return eval_grouped_expr(expr, group, bindings);
            }
        }
    }
    eval_grouped_expr(expr, group, bindings)
}

fn eval_grouped_expr(
    expr: &Expr,
    group: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let first_context = group.first().map(|row| row.context());
    eval_group_scalar_with_ctx(expr, group, first_context.as_ref(), bindings)
}

/// Sort `projected` rows in-place by `order_by`. Order expressions are
/// resolved against the projected output columns (by alias, or by the
/// rendered name of an unaliased projection expression). This supports
/// `SELECT DISTINCT x ... ORDER BY x DESC` after dedup.
fn sort_projected_rows_by_order_by(
    projected: &mut [Vec<SqlValue>],
    projection: &[SelectItem],
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<()> {
    enum OrderResolution {
        Column(usize),
        Constant(SqlValue),
    }

    let mut recipes: Vec<(OrderResolution, bool)> = Vec::with_capacity(order_by.len());
    for order in order_by {
        let desc = matches!(order.options.asc, Some(false));
        let resolved = resolve_order_against_projection(&order.expr, projection)?;
        let resolution = match resolved {
            Some(idx) => OrderResolution::Column(idx),
            None => {
                // Treat literal positional ORDER BY 1, 2, ... as 1-based
                // column index when the literal fits.
                if let Expr::Value(v) = &order.expr
                    && let sqlparser::ast::Value::Number(s, _) = &v.value
                    && let Ok(pos) = s.parse::<usize>()
                    && pos > 0
                    && pos <= projection_output_arity(projection)
                {
                    OrderResolution::Column(pos - 1)
                } else {
                    let ctx = RowContext::Empty;
                    OrderResolution::Constant(eval_scalar(&order.expr, &ctx, bindings)?)
                }
            }
        };
        recipes.push((resolution, desc));
    }

    projected.sort_by(|a, b| {
        for (recipe, desc) in &recipes {
            let (lv, rv) = match recipe {
                OrderResolution::Column(idx) => {
                    let lv = a.get(*idx).cloned().unwrap_or(SqlValue::Null);
                    let rv = b.get(*idx).cloned().unwrap_or(SqlValue::Null);
                    (lv, rv)
                }
                OrderResolution::Constant(value) => (value.clone(), value.clone()),
            };
            let mut ord = compare_values(&lv, &rv);
            if *desc {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
    Ok(())
}

/// If `expr` is a bare identifier matching the alias of one of `projection`'s
/// items (or the rendered name of an unaliased simple projection), return
/// the corresponding output-column index.
fn resolve_order_against_projection(
    expr: &Expr,
    projection: &[SelectItem],
) -> Result<Option<usize>> {
    let target = match expr {
        Expr::Identifier(ident) => ident.value.as_str(),
        Expr::CompoundIdentifier(parts) if parts.len() == 1 => parts[0].value.as_str(),
        _ => return Ok(None),
    };
    let mut idx = 0usize;
    for item in projection {
        match item {
            SelectItem::ExprWithAlias { alias, .. } => {
                if alias.value.eq_ignore_ascii_case(target) {
                    return Ok(Some(idx));
                }
                idx += 1;
            }
            SelectItem::UnnamedExpr(expr) => {
                if matches_simple_identifier(expr, target) {
                    return Ok(Some(idx));
                }
                idx += 1;
            }
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                // Wildcards expand to an unknown number of columns at this
                // layer; bail out.
                return Ok(None);
            }
        }
    }
    Ok(None)
}

fn projection_output_arity(projection: &[SelectItem]) -> usize {
    let mut arity = 0usize;
    for item in projection {
        match item {
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => return usize::MAX,
            _ => arity += 1,
        }
    }
    arity
}

fn matches_simple_identifier(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Identifier(ident) => ident.value.eq_ignore_ascii_case(target),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .is_some_and(|p| p.value.eq_ignore_ascii_case(target)),
        Expr::Nested(inner) => matches_simple_identifier(inner, target),
        _ => false,
    }
}

fn eval_group_key(
    group_by: &[Expr],
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let mut out = Vec::with_capacity(group_by.len());
    for expr in group_by {
        out.push(eval_scalar(expr, &row.context(), bindings)?);
    }
    Ok(out)
}

fn project_group_row(
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

fn eval_group_scalar_with_ctx(
    expr: &Expr,
    group: &[SqlRow],
    first_context: Option<&RowContext<'_>>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if !expr_contains_aggregate(expr) {
        return match first_context {
            Some(ctx) => eval_scalar(expr, ctx, bindings),
            None => eval_scalar(expr, &RowContext::Empty, bindings),
        };
    }
    match expr {
        Expr::Function(func) => eval_group_function(func, group, bindings),
        Expr::BinaryOp { left, op, right } => {
            let left_value = eval_group_scalar_with_ctx(left, group, first_context, bindings)?;
            let right_value = eval_group_scalar_with_ctx(right, group, first_context, bindings)?;
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
                    if matches!(left_value, SqlValue::Null) || matches!(right_value, SqlValue::Null)
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
                UnaryOperator::Minus => negate_value(value),
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
        Expr::Case { .. } => Err(Error::UnsupportedSql(
            "aggregate expressions in CASE are not supported".to_owned(),
        )),
        _ => Err(Error::UnsupportedSql(
            "aggregate expressions in this query are not supported".to_owned(),
        )),
    }
}

fn eval_group_function(
    func: &sqlparser::ast::Function,
    group: &[SqlRow],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let name = func.name.to_string().to_ascii_lowercase();
    match name.as_str() {
        "count" => {
            if let FunctionArguments::List(list) = &func.args {
                if list.args.len() == 1
                    && matches!(
                        list.args[0],
                        FunctionArg::Unnamed(FunctionArgExpr::Wildcard)
                    )
                {
                    return Ok(SqlValue::Integer(group.len() as i64));
                }
                let mut count = 0i64;
                for row in group {
                    let ctx = row.context();
                    let mut include = true;
                    for arg in &list.args {
                        if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) = arg
                            && matches!(eval_scalar(expr, &ctx, bindings)?, SqlValue::Null)
                        {
                            include = false;
                        }
                    }
                    if include {
                        count += 1;
                    }
                }
                Ok(SqlValue::Integer(count))
            } else {
                Ok(SqlValue::Integer(group.len() as i64))
            }
        }
        "sum" => {
            let mut total_i: i64 = 0;
            let mut total_r: f64 = 0.0;
            let mut saw_real = false;
            let mut saw_value = false;
            for row in group {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    match eval_scalar(expr, &ctx, bindings)? {
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
            for row in group {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    match eval_scalar(expr, &ctx, bindings)? {
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
        "min" | "max" => {
            let mut best: Option<SqlValue> = None;
            for row in group {
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
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported aggregate function: {name}"
        ))),
    }
}

fn negate_value(value: SqlValue) -> Result<SqlValue> {
    match value {
        SqlValue::Integer(v) => Ok(SqlValue::Integer(-v)),
        SqlValue::Real(v) => Ok(SqlValue::Real(-v)),
        SqlValue::Null => Ok(SqlValue::Null),
        _ => Err(Error::DatatypeMismatch),
    }
}
