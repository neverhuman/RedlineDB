use super::agg_eval::*;
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

pub(super) fn expr_contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Function(func) => {
            let name = func.name.to_string().to_ascii_lowercase();
            matches!(
                name.as_str(),
                "count"
                    | "sum"
                    | "avg"
                    | "min"
                    | "max"
                    | "group_concat"
                    | "string_agg"
                    | "total"
                    | "json_group_array"
                    | "json_group_object"
            )
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
