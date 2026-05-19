use super::*;

use std::collections::BTreeSet;

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
        .collect::<BTreeSet<_>>();
    projection.iter().all(|item| match item {
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            projection_expr_covered(table, &covered_ordinals, expr)
        }
    })
}

pub(crate) fn predicates_for_index(selection: &Option<Expr>) -> Vec<String> {
    match selection.as_ref().map(|expr| vec![expr_to_string(expr)]) {
        Some(v) => v,
        None => Vec::new(),
    }
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
                || function_args_contain_aggregate(func)
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

fn function_args_contain_aggregate(func: &sqlparser::ast::Function) -> bool {
    let FunctionArguments::List(list) = &func.args else {
        return false;
    };
    list.args.iter().any(|arg| match arg {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))
        | FunctionArg::Named {
            arg: FunctionArgExpr::Expr(expr),
            ..
        }
        | FunctionArg::ExprNamed {
            arg: FunctionArgExpr::Expr(expr),
            ..
        } => expr_contains_aggregate(expr),
        _ => false,
    })
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
