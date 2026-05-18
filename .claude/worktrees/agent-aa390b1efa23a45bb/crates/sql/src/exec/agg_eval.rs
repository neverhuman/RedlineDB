use super::*;
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
        // SQLite total(X) — NULL-safe sum: returns 0.0 when all values are NULL
        // (unlike sum() which returns NULL). Always returns a real.
        "total" => {
            let mut acc = 0.0f64;
            for row in group {
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
            for row in group {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let val = eval_scalar(expr, &ctx, bindings)?;
                    if !matches!(val, SqlValue::Null) {
                        parts.push(value_to_string(&val));
                    }
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
            for row in group {
                let ctx = row.context();
                if let FunctionArguments::List(list) = &func.args
                    && let Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) =
                        list.args.first()
                {
                    let val = eval_scalar(expr, &ctx, bindings)?;
                    arr.push(sql_to_json_value(&val));
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
            for row in group {
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
                        obj.insert(value_to_string(&key), sql_to_json_value(&val));
                    }
                }
            }
            let json = serde_json::Value::Object(obj);
            Ok(SqlValue::Text(Arc::from(json.to_string())))
        }
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported aggregate function: {name}"
        ))),
    }
}
