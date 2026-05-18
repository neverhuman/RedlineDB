use super::super::*;

pub(crate) fn select_requires_aggregation(plan: &crate::statement::SelectPlan) -> bool {
    !plan.group_by.is_empty()
        || plan.having.as_ref().is_some_and(expr_contains_aggregate)
        || plan.projection.iter().any(select_item_contains_aggregate)
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
            let is_builtin = matches!(
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
            );
            // Registered aggregate UDFs route through the grouped evaluator
            // exactly like built-in aggregates so `SELECT my_agg(col) FROM t
            // GROUP BY k` works without parser/planner changes.
            is_builtin
                || crate::udf::is_registered_aggregate(&name)
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
