use super::super::agg_eval::{
    eval_group_scalar_with_ctx, project_group_row, with_group_eval_cache,
};
use super::super::vec::hash_agg::{AggKind, HashAggregator, encode_group_key_bytes};
use super::super::*;
use super::order::{eval_group_key, sort_groups_by_order_by, sort_projected_rows_by_order_by};

/// A4: minimum filtered-row count to attempt the WS-C2 one-pass HashAggregator
/// route. Below this threshold the projection-classification cost outweighs
/// the win and the legacy materialised group path is faster. Tune via the
/// micro-bench in W9; correctness is path-independent (one-pass and
/// materialised paths produce byte-identical output, asserted by the
/// differential test).
pub(crate) const ONE_PASS_GROUP_THRESHOLD: usize = 16;

pub(crate) fn execute_grouped_select(
    plan: &crate::statement::SelectPlan,
    rows: Vec<SqlRow>,
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Vec<Vec<SqlValue>>> {
    let mut filtered = Vec::with_capacity(rows.len());
    for row in rows {
        if selection_passes(&plan.selection, &row, bindings)? {
            filtered.push(row);
        }
    }

    // WS-C2: one-pass routing through HashAggregator when the projection
    // shape is compatible (GROUP BY + bare built-in aggregates, no DISTINCT,
    // no exotic aggregates). Falls back to the materialised group path on
    // None.
    //
    // A4: only attempt the one-pass routing for inputs large enough to amortise
    // the projection classification + HashAggregator setup. Below the
    // threshold the legacy materialised group path is faster, so the routing
    // attempt is pure overhead on tiny benchmark queries. The cutoff was
    // picked from W0 ranked-CSV evidence: cases with ≤ ONE_PASS_GROUP_THRESHOLD
    // post-filter rows make up most of the per-statement overhead tax in the
    // parity corpus. Both code paths produce byte-identical output (one-pass
    // is a fast-path, not a different algorithm) — guarded by the differential
    // test in `crates/sql/tests/agg_group_one_pass_threshold.rs`.
    if filtered.len() >= ONE_PASS_GROUP_THRESHOLD
        && let Some(routed) = try_one_pass_grouped(plan, &filtered, bindings, limit, offset, memory)?
    {
        return Ok(routed);
    }

    if plan.distinct && plan.group_by.is_empty() {
        let mut out = Vec::with_capacity(filtered.len());
        let memory_bytes = filtered.iter().try_fold(0usize, |acc, row| {
            row.values().map(|values| acc + row_width(&values))
        })?;
        memory.request(memory_bytes)?;
        for row in filtered {
            let first_context = Some(row.context());
            let projected = with_group_eval_cache(|| -> Result<Option<Vec<SqlValue>>> {
                if let Some(having) = &plan.having
                    && !is_truthy(&eval_group_scalar_with_ctx(
                        having,
                        std::slice::from_ref(&row),
                        first_context.as_ref(),
                        bindings,
                    )?)
                {
                    return Ok(None);
                }
                Ok(Some(project_row(&plan.projection, &row, bindings)?))
            })?;
            if let Some(projected) = projected {
                out.push(projected);
            }
        }
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    let groups = if plan.group_by.is_empty() {
        vec![filtered]
    } else {
        let mut index_by_key: ahash::AHashMap<Vec<u8>, usize> =
            ahash::AHashMap::with_capacity(filtered.len());
        let mut groups: Vec<Vec<SqlRow>> = Vec::with_capacity(filtered.len());
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

    let mut out = Vec::with_capacity(groups.len());
    if plan.distinct {
        for group in groups {
            let first_context = group.first().map(|row| row.context());
            if group.is_empty()
                && !plan
                    .projection
                    .iter()
                    .any(super::select::select_item_contains_aggregate)
            {
                continue;
            }
            let projected = with_group_eval_cache(|| -> Result<Option<Vec<SqlValue>>> {
                if let Some(having) = &plan.having
                    && !is_truthy(&eval_group_scalar_with_ctx(
                        having,
                        &group,
                        first_context.as_ref(),
                        bindings,
                    )?)
                {
                    return Ok(None);
                }
                Ok(Some(project_group_row(&plan.projection, &group, bindings)?))
            })?;
            if let Some(projected) = projected {
                out.push(projected);
            }
        }
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    let mut surviving_groups: Vec<&[SqlRow]> = Vec::with_capacity(groups.len());
    for group in &groups {
        let first_context = group.first().map(|row| row.context());
        if group.is_empty()
            && !plan
                .projection
                .iter()
                .any(super::select::select_item_contains_aggregate)
        {
            continue;
        }
        let projected = with_group_eval_cache(|| -> Result<Option<Vec<SqlValue>>> {
            if let Some(having) = &plan.having
                && !is_truthy(&eval_group_scalar_with_ctx(
                    having,
                    group,
                    first_context.as_ref(),
                    bindings,
                )?)
            {
                return Ok(None);
            }
            Ok(Some(project_group_row(&plan.projection, group, bindings)?))
        })?;
        let Some(projected) = projected else {
            continue;
        };
        out.push(projected);
        surviving_groups.push(group.as_slice());
    }

    if !plan.order_by.is_empty() {
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

// ----------------------------------------------------------------------
// WS-C2: one-pass grouped aggregation via HashAggregator
// ----------------------------------------------------------------------

#[derive(Clone)]
struct AggSlot {
    kind: AggKind,
    // None ⇒ COUNT(*); otherwise the bare column-ref expression we eval per row.
    arg: Option<Expr>,
}

fn try_one_pass_grouped(
    plan: &crate::statement::SelectPlan,
    filtered: &[SqlRow],
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Option<Vec<Vec<SqlValue>>>> {
    if plan.distinct || !plan.distinct_on.is_empty() {
        return Ok(None);
    }
    if plan.group_by.is_empty() {
        // Scalar (no GROUP BY) goes through the existing path; the
        // single-row-output / empty-input semantics there are not yet
        // mirrored in the one-pass router.
        return Ok(None);
    }

    let mut slots: Vec<AggSlot> = Vec::new();
    let mut item_specs: Vec<ProjectionItem> = Vec::with_capacity(plan.projection.len());
    for item in &plan.projection {
        let expr = match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => return Ok(None),
        };
        let Some(spec) = classify_expr(expr, &mut slots)? else {
            return Ok(None);
        };
        item_specs.push(spec);
    }
    let mut having_spec: Option<ProjectionItem> = None;
    if let Some(having) = &plan.having {
        let Some(spec) = classify_expr(having, &mut slots)? else {
            return Ok(None);
        };
        having_spec = Some(spec);
    }
    // Classify ORDER BY expressions up-front so sorting can reuse slot
    // values without re-aggregating per group (the old code evaluates
    // over the full group; with a single representative row that would
    // collapse COUNT(*) to 1).
    let mut order_specs: Vec<(ProjectionItem, bool)> = Vec::with_capacity(plan.order_by.len());
    for order in &plan.order_by {
        // ORDER BY <int> position handled via column resolution below.
        if let Expr::Value(v) = &order.expr
            && let sqlparser::ast::Value::Number(s, _) = &v.value
            && let Ok(pos) = s.parse::<usize>()
            && pos > 0
            && pos <= item_specs.len()
        {
            // Reference the projection-output column directly via a
            // ProjectionRef slot wrapping the underlying spec.
            // We can't deep-borrow item_specs here without cloning, so
            // re-classify the matching projection expr.
            let item_expr = match &plan.projection[pos - 1] {
                SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
                _ => return Ok(None),
            };
            let Some(spec) = classify_expr(item_expr, &mut slots)? else {
                return Ok(None);
            };
            order_specs.push((spec, matches!(order.options.asc, Some(false))));
            continue;
        }
        // ORDER BY <alias> → resolve to the projection's expression.
        if let Expr::Identifier(ident) = &order.expr {
            let target = ident.value.as_str();
            let mut resolved = false;
            for item in &plan.projection {
                if let SelectItem::ExprWithAlias { alias, expr } = item
                    && alias.value.eq_ignore_ascii_case(target)
                {
                    let Some(spec) = classify_expr(expr, &mut slots)? else {
                        return Ok(None);
                    };
                    order_specs.push((spec, matches!(order.options.asc, Some(false))));
                    resolved = true;
                    break;
                }
            }
            if resolved {
                continue;
            }
            // Bare identifier — fall through to normal classify below
            // (column ref against the first row).
        }
        let Some(spec) = classify_expr(&order.expr, &mut slots)? else {
            return Ok(None);
        };
        order_specs.push((spec, matches!(order.options.asc, Some(false))));
    }

    let agg_kinds: Vec<AggKind> = slots.iter().map(|s| s.kind).collect();
    let mut hash_agg = HashAggregator::new(
        agg_kinds,
        memory.work_mem_bytes,
        memory.max_spill_bytes,
        memory.spill_root().to_path_buf(),
    );
    let mut first_row_index_by_key: ahash::AHashMap<Vec<u8>, usize> =
        ahash::AHashMap::with_capacity(filtered.len().min(1024));

    let mut arg_values: Vec<SqlValue> = vec![SqlValue::Null; slots.len()];
    for (row_idx, row) in filtered.iter().enumerate() {
        let key = eval_group_key(&plan.group_by, row, bindings)?;
        let key_bytes = encode_group_key_bytes(&key)?;
        first_row_index_by_key.entry(key_bytes).or_insert(row_idx);
        let ctx = row.context();
        for (i, slot) in slots.iter().enumerate() {
            arg_values[i] = match &slot.arg {
                None => SqlValue::Null,
                Some(arg_expr) => eval_scalar(arg_expr, &ctx, bindings)?,
            };
        }
        hash_agg.observe(key, &arg_values)?;
    }

    let finalised = hash_agg.finalize()?;

    let mut out: Vec<Vec<SqlValue>> = Vec::with_capacity(finalised.len());
    let mut surviving_row_indices: Vec<usize> = Vec::with_capacity(finalised.len());
    let mut per_row_agg_values: Vec<Vec<SqlValue>> = Vec::with_capacity(finalised.len());
    for (key_vec, agg_values) in finalised {
        let key_bytes = encode_group_key_bytes(&key_vec)?;
        let Some(&first_row_idx) = first_row_index_by_key.get(&key_bytes) else {
            continue;
        };
        let first_row = &filtered[first_row_idx];
        let ctx = first_row.context();
        if let Some(spec) = &having_spec {
            let v = eval_projection_item(spec, &agg_values, &ctx, bindings)?;
            if !is_truthy(&v) {
                continue;
            }
        }
        let mut row_out = Vec::with_capacity(item_specs.len());
        for spec in &item_specs {
            row_out.push(eval_projection_item(spec, &agg_values, &ctx, bindings)?);
        }
        out.push(row_out);
        surviving_row_indices.push(first_row_idx);
        per_row_agg_values.push(agg_values);
    }

    if !order_specs.is_empty() {
        // Precompute the ORDER BY key tuple per surviving group so the
        // sort comparator is O(n log n) scalar compares.
        let mut order_keys: Vec<Vec<SqlValue>> = Vec::with_capacity(out.len());
        for (idx, &first_row_idx) in surviving_row_indices.iter().enumerate() {
            let first_row = &filtered[first_row_idx];
            let agg_values_for_row = &per_row_agg_values[idx];
            let ctx = first_row.context();
            let mut tup = Vec::with_capacity(order_specs.len());
            for (spec, _) in &order_specs {
                tup.push(eval_projection_item(
                    spec,
                    agg_values_for_row,
                    &ctx,
                    bindings,
                )?);
            }
            order_keys.push(tup);
        }
        // A11: pair-sort avoids the N Vec clones the previous indices-sort
        // path emitted. We zip each output row with its precomputed
        // order-key tuple, sort the pair vector in place by the key, then
        // unzip the rows back. Zero clones, two contiguous Vec allocations
        // total instead of N small ones.
        debug_assert_eq!(out.len(), order_keys.len());
        let mut paired: Vec<(Vec<SqlValue>, Vec<SqlValue>)> =
            out.into_iter().zip(order_keys.into_iter()).collect();
        paired.sort_by(|a, b| {
            for (idx, (_, desc)) in order_specs.iter().enumerate() {
                let mut ord = compare_values(&a.1[idx], &b.1[idx]);
                if *desc {
                    ord = ord.reverse();
                }
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });
        out = paired.into_iter().map(|(row, _)| row).collect();
    }

    Ok(Some(out.into_iter().skip(offset).take(limit).collect()))
}

enum ProjectionItem {
    /// Bare aggregate call: emit slot value as-is.
    AggSlot(usize),
    /// Non-aggregate expression: eval against representative row context.
    Scalar(Expr),
}

fn eval_projection_item(
    spec: &ProjectionItem,
    agg_values: &[SqlValue],
    ctx: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    match spec {
        ProjectionItem::AggSlot(idx) => Ok(agg_values[*idx].clone()),
        ProjectionItem::Scalar(expr) => eval_scalar(expr, ctx, bindings),
    }
}

/// Classify a projection / HAVING expression for one-pass compatibility.
/// Returns `Ok(Some(_))` when the expression is either a bare aggregate
/// call or fully aggregate-free. Returns `Ok(None)` when it mixes
/// aggregates with surrounding ops (`SUM(v) + 1`, `CASE WHEN COUNT(*)>0`),
/// which need the per-group evaluator.
fn classify_expr(expr: &Expr, slots: &mut Vec<AggSlot>) -> Result<Option<ProjectionItem>> {
    if let Some(slot) = classify_as_aggregate(expr, slots)? {
        return Ok(Some(ProjectionItem::AggSlot(slot)));
    }
    if expr_contains_aggregate(expr) {
        return Ok(None);
    }
    Ok(Some(ProjectionItem::Scalar(expr.clone())))
}

/// If `expr` is a bare supported built-in aggregate call, register (or
/// reuse) a slot and return its index. Returns `Ok(None)` for non-aggregate
/// expressions, and also for aggregates we don't yet route (GROUP_CONCAT,
/// DISTINCT-args, FILTER, registered UDF aggregates, complex arg exprs).
fn classify_as_aggregate(expr: &Expr, slots: &mut Vec<AggSlot>) -> Result<Option<usize>> {
    let Expr::Function(func) = expr else {
        return Ok(None);
    };
    let name = func.name.to_string().to_ascii_lowercase();
    let kind_hint = match name.as_str() {
        "count" => None,
        "sum" => Some(AggKind::Sum),
        "avg" => Some(AggKind::Avg),
        "min" => Some(AggKind::Min),
        "max" => Some(AggKind::Max),
        _ => return Ok(None),
    };
    if crate::udf::is_registered_aggregate(&name) {
        return Ok(None);
    }
    if func.filter.is_some()
        || !func.within_group.is_empty()
        || func.over.is_some()
        || func.null_treatment.is_some()
    {
        return Ok(None);
    }
    let FunctionArguments::List(list) = &func.args else {
        return Ok(None);
    };
    if !list.clauses.is_empty() {
        return Ok(None);
    }
    if matches!(
        list.duplicate_treatment,
        Some(sqlparser::ast::DuplicateTreatment::Distinct)
    ) {
        return Ok(None);
    }

    let (resolved_kind, slot_arg) = match name.as_str() {
        "count" => {
            if list.args.len() != 1 {
                return Ok(None);
            }
            match &list.args[0] {
                FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => (AggKind::CountStar, None),
                FunctionArg::Unnamed(FunctionArgExpr::Expr(arg)) => {
                    if !is_bare_column_ref(arg) {
                        return Ok(None);
                    }
                    (AggKind::Count, Some(arg.clone()))
                }
                _ => return Ok(None),
            }
        }
        _ => {
            if list.args.len() != 1 {
                return Ok(None);
            }
            let FunctionArg::Unnamed(FunctionArgExpr::Expr(arg)) = &list.args[0] else {
                return Ok(None);
            };
            if !is_bare_column_ref(arg) {
                return Ok(None);
            }
            (
                kind_hint.expect("non-count kind resolved"),
                Some(arg.clone()),
            )
        }
    };

    for (i, slot) in slots.iter().enumerate() {
        if slot.kind == resolved_kind && slot.arg == slot_arg {
            return Ok(Some(i));
        }
    }
    slots.push(AggSlot {
        kind: resolved_kind,
        arg: slot_arg,
    });
    Ok(Some(slots.len() - 1))
}

/// True if `expr` is a column reference we can safely pass to
/// `eval_scalar` per row. Restricting to bare references keeps the MVP
/// scope tight; complex args (`SUM(a + b)`) fall back to the old path.
fn is_bare_column_ref(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) => true,
        Expr::CompoundIdentifier(parts) => !parts.is_empty(),
        Expr::Nested(inner) => is_bare_column_ref(inner),
        _ => false,
    }
}
