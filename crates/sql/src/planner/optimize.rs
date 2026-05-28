use super::*;

pub(crate) fn choose_join_kind(
    left_rows: f64,
    right_rows: f64,
    right_table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> JoinKind {
    let has_indexable_equality = selection
        .as_ref()
        .is_some_and(|expr| join_has_indexable_equality(expr, right_table, bindings));
    let has_equality = selection
        .as_ref()
        .is_some_and(|expr| join_has_equality(expr, right_table, bindings));
    ActivePlannerPolicy::choose_join_kind(JoinChoice {
        left_rows,
        right_rows,
        has_indexable_equality,
        has_equality,
        has_selection: selection.is_some(),
    })
}

pub(crate) fn join_has_equality(
    expr: &Expr,
    table: &Arc<TableDef>,
    bindings: &[Option<SqlValue>],
) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            if matches!(op, BinaryOperator::Eq) {
                join_operand_references_table(left, table)
                    || join_operand_references_table(right, table)
                    || eval_constant(left, bindings).is_some()
                    || eval_constant(right, bindings).is_some()
            } else if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                join_has_equality(left, table, bindings)
                    || join_has_equality(right, table, bindings)
            } else {
                false
            }
        }
        Expr::Nested(inner) => join_has_equality(inner, table, bindings),
        _ => false,
    }
}

pub(crate) fn join_has_indexable_equality(
    expr: &Expr,
    table: &Arc<TableDef>,
    bindings: &[Option<SqlValue>],
) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            if matches!(op, BinaryOperator::Eq) {
                let ordinal_opt = match join_table_column_ordinal(left, table) {
                    Some(o) => Some(o),
                    None => join_table_column_ordinal(right, table),
                };
                if let Some(ordinal) = ordinal_opt {
                    return table.indexes.iter().any(|index| {
                        index
                            .keys
                            .first()
                            .is_some_and(|key| key.ordinal as usize == ordinal)
                    });
                }
                eval_constant(left, bindings).is_some() || eval_constant(right, bindings).is_some()
            } else if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                join_has_indexable_equality(left, table, bindings)
                    || join_has_indexable_equality(right, table, bindings)
            } else {
                false
            }
        }
        Expr::Nested(inner) => join_has_indexable_equality(inner, table, bindings),
        _ => false,
    }
}

pub(crate) fn join_operand_references_table(expr: &Expr, table: &TableDef) -> bool {
    join_table_column_ordinal(expr, table).is_some()
}

pub(crate) fn join_table_column_ordinal(expr: &Expr, table: &TableDef) -> Option<usize> {
    match expr {
        Expr::Identifier(ident) => column_ordinal_for_table(&ident.value, table),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .and_then(|ident| column_ordinal_for_table(&ident.value, table)),
        Expr::Nested(inner) => join_table_column_ordinal(inner, table),
        _ => None,
    }
}

pub(crate) fn wrap_aggregate(input: PhysicalPlan, plan: &SelectPlan) -> PhysicalPlan {
    let input_rows = input.cost.rows;
    let input_width = input.cost.width;
    let input_total = input.cost.total;
    let mut node = PhysicalPlan::new(ActivePlannerPolicy::aggregate_kind(AggregateChoice {
        input_rows,
        group_cols: plan.group_by.len(),
        ordered: has_group_by_ordering(plan),
    }));
    node.children = vec![input];
    node.estimated_rows = estimate_group_rows(input_rows, plan.group_by.len());
    node.cost = Cost {
        startup: CPU_OPERATOR_COST,
        total: CPU_OPERATOR_COST + input_total + node.estimated_rows * CPU_TUPLE_COST,
        rows: node.estimated_rows,
        width: input_width,
        memory_bytes: if matches!(node.kind, PhysicalKind::HashAggregate) {
            (input_rows.max(1.0) * input_width.max(1.0)) as usize
        } else {
            0
        },
        spill_bytes: 0,
    };
    node.memory_budget = node.cost.memory_bytes;
    node.projected_columns = projected_columns_from_projection(&plan.projection);
    node.access_predicates = plan.group_by.iter().map(expr_to_string).collect();
    if matches!(node.kind, PhysicalKind::StreamingAggregate) {
        node.output_order = plan.group_by.iter().map(expr_to_string).collect();
    }
    node
}

pub(crate) fn wrap_ordering(
    input: PhysicalPlan,
    plan: &SelectPlan,
    _bindings: &[Option<SqlValue>],
    _optimizer: &OptimizerConfig,
) -> PhysicalPlan {
    let limit_small = plan
        .limit
        .as_ref()
        .and_then(|expr| eval_constant(expr, &[]))
        .and_then(|value| match value {
            SqlValue::Integer(v) if v > 0 => Some(v as usize),
            _ => None,
        })
        .unwrap_or(usize::MAX);
    // Lane VE: a small `LIMIT` lets the executor use the fixed-size top-K
    // heap instead of a full sort. The threshold matches
    // `vec::TOPK_LIMIT_THRESHOLD` to keep planner-vs-executor decisions in
    // lockstep.
    let kind = ActivePlannerPolicy::ordering_kind(SortChoice {
        limit: (limit_small != usize::MAX).then_some(limit_small),
    });
    let mut node = PhysicalPlan::new(kind);
    node.children = vec![input];
    node.output_order = plan
        .order_by
        .iter()
        .map(|order| expr_to_string(&order.expr))
        .collect();
    node.estimated_rows = node.children[0].estimated_rows;
    node.cost = Cost {
        startup: CPU_OPERATOR_COST,
        total: node.children[0].cost.total + (node.estimated_rows * CPU_TUPLE_COST),
        rows: node.estimated_rows,
        width: node.children[0].cost.width,
        memory_bytes: if kind == PhysicalKind::TopN {
            limit_small.saturating_mul(node.children[0].cost.width as usize)
        } else {
            (node.estimated_rows * node.children[0].cost.width.max(1.0)) as usize
        },
        spill_bytes: 0,
    };
    node.memory_budget = node.cost.memory_bytes;
    node
}

pub(crate) fn estimate_group_rows(input_rows: f64, group_by_len: usize) -> f64 {
    if group_by_len == 0 {
        return 1.0;
    }
    let divisor = (group_by_len as f64 * 2.0).max(1.0);
    (input_rows / divisor).clamp(1.0, input_rows.max(1.0))
}

pub(crate) fn wrap_project(input: PhysicalPlan, plan: &SelectPlan) -> PhysicalPlan {
    let mut node = PhysicalPlan::new(PhysicalKind::Project);
    node.children = vec![input];
    node.projected_columns = projected_columns_from_projection(&plan.projection);
    node.estimated_rows = node.children[0].estimated_rows;
    node.cost = Cost {
        startup: CPU_OPERATOR_COST,
        total: node.children[0].cost.total + node.estimated_rows * CPU_TUPLE_COST,
        rows: node.estimated_rows,
        width: node.children[0].cost.width,
        memory_bytes: 0,
        spill_bytes: 0,
    };
    node
}

pub(crate) fn wrap_limit(input: PhysicalPlan, plan: &SelectPlan) -> PhysicalPlan {
    wrap_limit_with_conn(None, input, plan)
}

/// R2-C: optional `Connection`-aware variant of `wrap_limit`. When the
/// PRAGMA `redline_planner_use_access_path = ON` is set, the planner
/// consults the `AccessPath` IR's `hard_limit()` accessor to decide
/// LIMIT pushdown into an ordered index range scan, rather than the
/// ad-hoc `IndexScan + RangeScan + output_order non-empty` match below.
/// The non-PRAGMA path is byte-for-byte identical to v4.0.3.
pub(crate) fn wrap_limit_with_conn(
    conn: Option<&Connection>,
    input: PhysicalPlan,
    plan: &SelectPlan,
) -> PhysicalPlan {
    let input_rows = input.cost.rows;
    let input_width = input.cost.width;
    let input_total = input.cost.total;
    // Phase 11 W1-D: when the LIMIT directly wraps an IndexScan whose
    // output order satisfies the SELECT's ORDER BY, propagate the
    // numeric limit down into the leaf so EXPLAIN renders the
    // early-stop annotation. The executor honors the same fact
    // independently via `try_ordered_index_limit_path` — this is the
    // planner-side annotation so that EXPLAIN output reflects the
    // physical plan the executor will run.
    let limit_n = plan.limit.as_ref().and_then(|expr| match expr {
        Expr::Value(v) => match &v.value {
            sqlparser::ast::Value::Number(n, _) => n.parse::<usize>().ok(),
            _ => None,
        },
        _ => None,
    });
    let mut input = input;
    // R2-C: PRAGMA-ON path. Consult the IR directly — does this access
    // path support a hard limit? `AccessPath::hard_limit()` already
    // enforces the residual-empty + order_satisfies safety checks, so
    // we can trust its answer without re-deriving the conditions.
    if planner_use_access_path()
        && let Some(conn) = conn
        && let Some(n) = limit_n
        && limit_annotatable_index_scan_mut(&mut input).is_some()
    {
        if let SelectSource::Table(table) = &plan.source {
            let ir = choose_access_path_ir(
                conn.engine(),
                table,
                &plan.selection,
                &[],
                plan.table_hint.as_ref(),
                &plan.order_by,
                Some(n),
            );
            if let Some(k) = ir.hard_limit() {
                if let Some(index_scan) = limit_annotatable_index_scan_mut(&mut input) {
                    index_scan.ordered_index_scan_limit = Some(k);
                }
            }
        }
    } else if let (PhysicalKind::IndexScan, Some(n)) = (input.kind, limit_n)
        && !input.output_order.is_empty()
        && input
            .index_probe_kind
            .map(|k| k == "RangeScan")
            .unwrap_or(false)
    {
        // Default-OFF path: v4.0.3 ad-hoc match. Preserves byte-for-byte
        // parity when the PRAGMA is not set.
        input.ordered_index_scan_limit = Some(n);
    }
    let mut node = PhysicalPlan::new(PhysicalKind::Limit);
    node.children = vec![input];
    node.estimated_rows = input_rows;
    node.cost = Cost {
        startup: 0.0,
        total: input_total,
        rows: input_rows,
        width: input_width,
        memory_bytes: 0,
        spill_bytes: 0,
    };
    node.relation = Some(match plan.limit.as_ref().map(expr_to_string) {
        Some(s) => s,
        None => "LIMIT".to_owned(),
    });
    node
}

fn limit_annotatable_index_scan_mut(input: &mut PhysicalPlan) -> Option<&mut PhysicalPlan> {
    match input.kind {
        PhysicalKind::IndexScan => Some(input),
        // Projection preserves row count and order. This is the common
        // SELECT-col path because `build_select_plan` wraps projection
        // before LIMIT.
        PhysicalKind::Project => match input.children.as_mut_slice() {
            [child] if matches!(child.kind, PhysicalKind::IndexScan) => Some(child),
            _ => None,
        },
        _ => None,
    }
}
