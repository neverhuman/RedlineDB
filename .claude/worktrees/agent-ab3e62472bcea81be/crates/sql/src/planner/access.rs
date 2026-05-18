use super::*;

// One arg per planner-side input the access-path resolver might
// need; flattening into a struct would scatter call sites without
// shrinking the contract.
#[allow(clippy::too_many_arguments)]
pub(crate) fn choose_access_path(
    conn: &Connection,
    table: &Arc<TableDef>,
    _projection: &[SelectItem],
    selection: &Option<Expr>,
    _order_by: &[OrderByExpr],
    rowid: Option<RowId>,
    bindings: &[Option<SqlValue>],
    _table_stats: Option<&TableStats>,
    _optimizer: &OptimizerConfig,
) -> AccessPath {
    // Order matters and mirrors the executor in `exec.rs`:
    //   1. The integer-PK rowid alias (if the predicate is `id = ?` on
    //      a rowid table) is the cheapest path; it lands in
    //      `RowIdGet`.
    //   2. Lane C: a leading-prefix index probe (point or range).
    //   3. Default path: a heap scan. The planner stays conservative — it
    //      ONLY advertises an index path when the executor will
    //      actually consume one, so EXPLAIN never lies about the
    //      physical plan.
    if let Some(rowid) = rowid {
        return AccessPath::RowIdGet { rowid };
    }
    if let Some(matched) =
        crate::exec::index_access::try_match_index_access(conn.engine(), table, selection, bindings)
    {
        return match matched.kind {
            crate::exec::index_access::IndexProbeKind::PointLookup => {
                AccessPath::IndexPointLookup {
                    index: matched.index,
                    predicates: matched.predicates,
                }
            }
            crate::exec::index_access::IndexProbeKind::RangeScan => AccessPath::IndexRangeScan {
                index: matched.index,
                predicates: matched.predicates,
            },
        };
    }
    AccessPath::TableScan
}

/// Conservatism guard for `AccessPath`. The planner is required to
/// only advertise paths the executor can satisfy; this check is a
/// compile-time-style enumeration that fires in debug builds if a
/// new variant is added without a matching executor arm in
/// `exec.rs::execute_select` and `exec/index_access.rs`.
///
/// Variants that ARE consumable today:
/// - `TableScan`          (executor: `collect_table_rowids`)
/// - `RowIdGet`           (executor: `selection_rowid_eq` fast path)
/// - `IndexPointLookup`   (executor: `execute_index_point_lookup`)
/// - `IndexRangeScan`     (executor: `execute_index_range_scan`)
///
/// Variants that are deliberately NOT advertised this round (Wave 4):
/// - `CoveringIndexScan`  — scheduled for a later wave
/// - `MultiIndexOr`       — scheduled for a later wave
/// - `MultiIndexAnd`      — scheduled for a later wave
///
/// If you add a new `AccessPath` variant, decide whether the executor
/// can consume it today. If yes, extend `execute_select` and add a
/// match arm here (returning `true`). If no, leave the planner unable
/// to emit it.
#[cfg(debug_assertions)]
pub(crate) fn access_path_is_consumable_by_executor(access: &AccessPath) -> bool {
    match access {
        AccessPath::TableScan
        | AccessPath::RowIdGet { .. }
        | AccessPath::IndexPointLookup { .. }
        | AccessPath::IndexRangeScan { .. } => true,
        AccessPath::CoveringIndexScan { .. }
        | AccessPath::MultiIndexOr { .. }
        | AccessPath::MultiIndexAnd { .. } => false,
    }
}

pub(crate) fn best_index_for_table(
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    _table_stats: Option<&TableStats>,
) -> Option<Arc<IndexDef>> {
    let candidate_column = selection
        .as_ref()
        .and_then(|expr| first_indexable_column(expr, table))
        .or_else(|| {
            order_by
                .first()
                .and_then(|order| order_expr_column(order, table))
        });
    let candidate_column = candidate_column?;
    for index in &table.indexes {
        if index
            .keys
            .first()
            .is_some_and(|key| key.ordinal as usize == candidate_column)
        {
            return Some(Arc::new(index.clone()));
        }
    }
    None
}

pub(crate) fn first_indexable_column(expr: &Expr, table: &TableDef) -> Option<usize> {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            if matches!(
                op,
                BinaryOperator::Eq
                    | BinaryOperator::Gt
                    | BinaryOperator::GtEq
                    | BinaryOperator::Lt
                    | BinaryOperator::LtEq
            ) {
                column_ordinal(left, table).or_else(|| column_ordinal(right, table))
            } else if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                first_indexable_column(left, table).or_else(|| first_indexable_column(right, table))
            } else {
                None
            }
        }
        Expr::Nested(expr) => first_indexable_column(expr, table),
        _ => None,
    }
}

pub(crate) fn or_children(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Or,
            right,
        } => Some((left.as_ref(), right.as_ref())),
        Expr::Nested(inner) => or_children(inner),
        _ => None,
    }
}

pub(crate) fn and_children(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => Some((left.as_ref(), right.as_ref())),
        Expr::Nested(inner) => and_children(inner),
        _ => None,
    }
}

pub(crate) fn column_ordinal(expr: &Expr, table: &TableDef) -> Option<usize> {
    match expr {
        Expr::Identifier(ident) => table
            .columns
            .iter()
            .position(|column| column.folded.as_ref().eq_ignore_ascii_case(&ident.value)),
        Expr::CompoundIdentifier(parts) => parts.last().and_then(|ident| {
            table
                .columns
                .iter()
                .position(|column| column.folded.as_ref().eq_ignore_ascii_case(&ident.value))
        }),
        _ => None,
    }
}

pub(crate) fn order_expr_column(order: &OrderByExpr, table: &TableDef) -> Option<usize> {
    match &order.expr {
        Expr::Identifier(ident) => table
            .columns
            .iter()
            .position(|column| column.folded.as_ref().eq_ignore_ascii_case(&ident.value)),
        Expr::CompoundIdentifier(parts) => parts.last().and_then(|ident| {
            table
                .columns
                .iter()
                .position(|column| column.folded.as_ref().eq_ignore_ascii_case(&ident.value))
        }),
        _ => None,
    }
}

pub(crate) fn projected_columns_from_projection(projection: &[SelectItem]) -> Vec<String> {
    projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                Some(expr_to_string(expr))
            }
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => None,
        })
        .collect()
}

pub(crate) fn projected_columns_from_select(
    table: &Arc<TableDef>,
    projection: &[SelectItem],
) -> Vec<String> {
    if projection.is_empty() {
        return table
            .columns
            .iter()
            .map(|column| column.name.to_string())
            .collect();
    }
    let projected = projected_columns_from_projection(projection);
    if projected.is_empty() {
        table
            .columns
            .iter()
            .map(|column| column.name.to_string())
            .collect()
    } else {
        projected
    }
}

pub(crate) fn projected_columns_are_covered(
    table: &Arc<TableDef>,
    index: &Arc<IndexDef>,
    projection: &[SelectItem],
) -> bool {
    if projection.is_empty() {
        return false;
    }
    let covered_ordinals = index
        .keys
        .iter()
        .map(|key| key.ordinal as usize)
        .collect::<std::collections::BTreeSet<_>>();
    projection.iter().all(|item| match item {
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            projection_expr_covered(table, &covered_ordinals, expr)
        }
    })
}

pub(crate) fn predicates_for_index(selection: &Option<Expr>) -> Vec<String> {
    selection
        .as_ref()
        .map(|expr| vec![expr_to_string(expr)])
        .unwrap_or_default()
}

pub(crate) fn is_range_predicate(selection: &Option<Expr>) -> bool {
    matches!(
        selection,
        Some(Expr::BinaryOp {
            op: BinaryOperator::Gt
                | BinaryOperator::GtEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq,
            ..
        })
    )
}

pub(crate) fn has_projection_exprs(projection: &[SelectItem]) -> bool {
    projection.iter().any(|item| {
        matches!(
            item,
            SelectItem::UnnamedExpr(_) | SelectItem::ExprWithAlias { .. }
        )
    })
}

pub(crate) fn requires_aggregate(plan: &SelectPlan) -> bool {
    !plan.group_by.is_empty() || plan.projection.iter().any(select_item_contains_aggregate)
}

pub(crate) fn select_item_contains_aggregate(item: &SelectItem) -> bool {
    match item {
        SelectItem::UnnamedExpr(expr) => expr_contains_aggregate(expr),
        SelectItem::ExprWithAlias { expr, .. } => expr_contains_aggregate(expr),
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
    }
}

pub(crate) fn expr_contains_aggregate(expr: &Expr) -> bool {
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

pub(crate) fn has_group_by_ordering(plan: &SelectPlan) -> bool {
    if plan.group_by.is_empty() {
        return false;
    }
    let order_by: Vec<_> = plan
        .order_by
        .iter()
        .map(|item| expr_to_string(&item.expr))
        .collect();
    let group_by: Vec<_> = plan.group_by.iter().map(expr_to_string).collect();
    order_by == group_by
}

pub(crate) fn output_order_satisfies(output_order: &[String], order_by: &[OrderByExpr]) -> bool {
    if order_by.is_empty() {
        return true;
    }
    if output_order.len() < order_by.len() {
        return false;
    }
    output_order
        .iter()
        .zip(order_by.iter())
        .all(|(left, right)| *left == expr_to_string(&right.expr))
}
