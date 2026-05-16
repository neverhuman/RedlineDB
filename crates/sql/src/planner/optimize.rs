use super::*;

pub(crate) fn choose_join_kind(
    left_rows: f64,
    right_rows: f64,
    right_table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> JoinKind {
    if left_rows <= 16.0 || right_rows <= 16.0 {
        return JoinKind::NestedLoop;
    }
    let Some(expr) = selection else {
        return JoinKind::Cross;
    };
    if join_has_indexable_equality(expr, right_table, bindings)
        && (left_rows <= 256.0 || right_rows <= 1024.0)
    {
        return JoinKind::IndexNestedLoop;
    }
    if join_has_equality(expr, right_table, bindings) {
        return JoinKind::Hash;
    }
    JoinKind::NestedLoop
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
    let mut node = PhysicalPlan::new(if has_group_by_ordering(plan) {
        PhysicalKind::StreamingAggregate
    } else {
        PhysicalKind::HashAggregate
    });
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
    let kind = if limit_small != usize::MAX && limit_small <= crate::exec::vec::TOPK_LIMIT_THRESHOLD
    {
        PhysicalKind::TopN
    } else {
        PhysicalKind::Sort
    };
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
    if let (PhysicalKind::IndexScan, Some(n)) = (input.kind, limit_n)
        && !input.output_order.is_empty()
        && input
            .index_probe_kind
            .map(|k| k == "RangeScan")
            .unwrap_or(false)
    {
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
