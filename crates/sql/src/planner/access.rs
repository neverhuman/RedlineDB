use super::*;

mod projection;

pub(crate) use projection::*;

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
    table_hint: Option<&crate::statement::TableAccessHint>,
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
    if let Some(rowid) = rowid
        && !matches!(table_hint, Some(crate::statement::TableAccessHint::NotIndexed))
    {
        return AccessPath::RowIdGet { rowid };
    }
    if let Some(matched) = crate::exec::index_access::try_match_index_access_hinted(
        conn.engine(),
        table,
        selection,
        bindings,
        table_hint,
    ) {
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
    let candidate_column = match selection
        .as_ref()
        .and_then(|expr| first_indexable_column(expr, table))
    {
        Some(col) => Some(col),
        None => order_by
            .first()
            .and_then(|order| order_expr_column(order, table)),
    };
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
                match column_ordinal(left, table) {
                    Some(o) => Some(o),
                    None => column_ordinal(right, table),
                }
            } else if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                match first_indexable_column(left, table) {
                    Some(o) => Some(o),
                    None => first_indexable_column(right, table),
                }
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
