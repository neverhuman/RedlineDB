use super::*;

pub(crate) fn bind_query(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    query: Query,
) -> Result<PreparedTemplate> {
    let Query {
        body,
        order_by,
        limit_clause,
        with,
        ..
    } = query;
    if let Some(with) = with {
        // Lane SQL-D phase 10 Tier-2: WITH ... SELECT (CTE) parses but is
        // not yet planner-integrated. We surface a structured error so SQL
        // text round-trips without becoming a parse-error and so future
        // planner work can swap this branch for a materialisation pass.
        let _ = with.cte_tables.len();
        return Err(Error::UnsupportedSql(
            "CTEs (WITH clauses) are parsed-only; execution not yet implemented".to_owned(),
        ));
    }
    match *body {
        SetExpr::Query(query) => {
            if order_by.is_some() || limit_clause.is_some() {
                return Err(Error::UnsupportedSql(
                    "nested query wrappers with ORDER BY or LIMIT are not supported yet".to_owned(),
                ));
            }
            bind_query(conn, schema, schema_epoch, sql, *query)
        }
        SetExpr::SetOperation {
            op,
            set_quantifier,
            left,
            right,
        } => bind_union_all_query(
            UnionAllQueryContext {
                conn,
                schema,
                schema_epoch,
                sql,
                order_by,
                limit_clause,
            },
            op,
            set_quantifier,
            *left,
            *right,
        ),
        SetExpr::Select(select) => {
            let mut params = ParamLayout::default();
            bind_simple_select_query(
                schema,
                schema_epoch,
                sql,
                select,
                order_by,
                limit_clause,
                &mut params,
            )
        }
        _ => Err(Error::UnsupportedSql(
            "only simple SELECT and UNION ALL queries are supported".to_owned(),
        )),
    }
}

pub(crate) struct UnionAllQueryContext<'a> {
    pub conn: &'a Connection,
    pub schema: Arc<SchemaSnapshot>,
    pub schema_epoch: SchemaEpoch,
    pub sql: &'a str,
    pub order_by: Option<sqlparser::ast::OrderBy>,
    pub limit_clause: Option<LimitClause>,
}

pub(crate) fn bind_simple_select_query(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    select: Box<sqlparser::ast::Select>,
    order_by: Option<sqlparser::ast::OrderBy>,
    limit_clause: Option<LimitClause>,
    params: &mut ParamLayout,
) -> Result<PreparedTemplate> {
    let distinct = match select.distinct {
        Some(Distinct::Distinct) => true,
        Some(Distinct::All) => false,
        Some(Distinct::On(_)) => {
            return Err(Error::UnsupportedSql(
                "DISTINCT ON is not supported".to_owned(),
            ));
        }
        None => false,
    };

    let mut projection = Vec::new();
    let mut output_columns = Vec::new();

    let (source, mut selection) = bind_select_from(&schema, select.from, params)?;

    for item in select.projection {
        let item = normalize_select_item(item, params)?;
        match &item {
            SelectItem::Wildcard(_) => push_projection_columns(&source, &mut output_columns),
            SelectItem::QualifiedWildcard(_, _) => {
                push_projection_columns(&source, &mut output_columns)
            }
            SelectItem::UnnamedExpr(expr) => output_columns.push(render_expr_name(expr)),
            SelectItem::ExprWithAlias { alias, .. } => output_columns.push(alias.value.clone()),
        }
        projection.push(item);
    }
    if projection.is_empty() {
        push_projection_columns(&source, &mut output_columns);
    }

    if let Some(expr) = select.selection {
        selection = Some(match selection {
            Some(join_expr) => and_expr(join_expr, normalize_expr(expr, params)?),
            None => normalize_expr(expr, params)?,
        });
    }

    let group_by = match select.group_by {
        GroupByExpr::All(_) => {
            return Err(Error::UnsupportedSql(
                "GROUP BY ALL is not supported".to_owned(),
            ));
        }
        GroupByExpr::Expressions(exprs, modifiers) => {
            if !modifiers.is_empty() {
                return Err(Error::UnsupportedSql(
                    "GROUP BY modifiers are not supported".to_owned(),
                ));
            }
            exprs
                .into_iter()
                .map(|expr| normalize_expr(expr, params))
                .collect::<Result<Vec<_>>>()?
        }
    };

    let having = match select.having {
        Some(expr) => Some(normalize_expr(expr, params)?),
        None => None,
    };

    let order_by = match order_by {
        Some(order_by) => match order_by.kind {
            OrderByKind::Expressions(exprs) => exprs
                .into_iter()
                .map(|expr| {
                    let options = expr.options;
                    let with_fill = expr.with_fill;
                    let expr = normalize_expr(expr.expr, params)?;
                    Ok(OrderByExpr {
                        expr,
                        options,
                        with_fill,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            OrderByKind::All(_) => {
                return Err(Error::UnsupportedSql(
                    "ORDER BY ALL is not supported".to_owned(),
                ));
            }
        },
        None => Vec::new(),
    };

    let (limit, offset) = match limit_clause {
        Some(LimitClause::LimitOffset {
            limit,
            offset,
            limit_by: _,
        }) => {
            let limit = match limit {
                Some(expr) => Some(normalize_expr(expr, params)?),
                None => None,
            };
            let offset = match offset {
                Some(offset) => Some(normalize_expr(offset.value, params)?),
                None => None,
            };
            (limit, offset)
        }
        Some(LimitClause::OffsetCommaLimit { offset, limit }) => (
            Some(normalize_expr(limit, params)?),
            Some(normalize_expr(offset, params)?),
        ),
        None => (None, None),
    };

    if params.count() == 0 {
        scan_sql_parameters(sql, params);
    }
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params.clone(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source,
            distinct,
            projection,
            selection,
            group_by,
            having,
            order_by,
            limit,
            offset,
        }),
    })
}

pub(crate) fn bind_union_all_query(
    ctx: UnionAllQueryContext<'_>,
    op: SetOperator,
    set_quantifier: SetQuantifier,
    left: SetExpr,
    right: SetExpr,
) -> Result<PreparedTemplate> {
    if !matches!(op, SetOperator::Union) || !matches!(set_quantifier, SetQuantifier::All) {
        return Err(Error::UnsupportedSql(
            "only UNION ALL is supported".to_owned(),
        ));
    }

    let left = bind_query(
        ctx.conn,
        Arc::clone(&ctx.schema),
        ctx.schema_epoch,
        ctx.sql,
        Query {
            with: None,
            body: Box::new(left),
            order_by: None,
            limit_clause: None,
            fetch: None,
            locks: Vec::new(),
            for_clause: None,
            settings: None,
            format_clause: None,
            pipe_operators: Vec::new(),
        },
    )?;
    let right = bind_query(
        ctx.conn,
        ctx.schema,
        ctx.schema_epoch,
        ctx.sql,
        Query {
            with: None,
            body: Box::new(right),
            order_by: None,
            limit_clause: None,
            fetch: None,
            locks: Vec::new(),
            for_clause: None,
            settings: None,
            format_clause: None,
            pipe_operators: Vec::new(),
        },
    )?;

    if left.param_layout.count() != 0 || right.param_layout.count() != 0 {
        return Err(Error::UnsupportedSql(
            "UNION ALL with parameters is not supported yet".to_owned(),
        ));
    }

    let mut tail_params = ParamLayout::default();
    let order_by = match ctx.order_by {
        Some(order_by) => match order_by.kind {
            OrderByKind::Expressions(exprs) => exprs
                .into_iter()
                .map(|expr| {
                    let options = expr.options;
                    let with_fill = expr.with_fill;
                    let expr = normalize_expr(expr.expr, &mut tail_params)?;
                    Ok(OrderByExpr {
                        expr,
                        options,
                        with_fill,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            OrderByKind::All(_) => {
                return Err(Error::UnsupportedSql(
                    "ORDER BY ALL is not supported".to_owned(),
                ));
            }
        },
        None => Vec::new(),
    };
    let (limit, offset) = match ctx.limit_clause {
        Some(LimitClause::LimitOffset {
            limit,
            offset,
            limit_by: _,
        }) => {
            let limit = match limit {
                Some(expr) => Some(normalize_expr(expr, &mut tail_params)?),
                None => None,
            };
            let offset = match offset {
                Some(offset) => Some(normalize_expr(offset.value, &mut tail_params)?),
                None => None,
            };
            (limit, offset)
        }
        Some(LimitClause::OffsetCommaLimit { offset, limit }) => (
            Some(normalize_expr(limit, &mut tail_params)?),
            Some(normalize_expr(offset, &mut tail_params)?),
        ),
        None => (None, None),
    };
    if tail_params.count() > 0 {
        return Err(Error::UnsupportedSql(
            "UNION ALL with parameters is not supported yet".to_owned(),
        ));
    }

    let projection = Vec::new();
    let mut output_columns = Vec::new();
    push_projection_columns(
        &SelectSource::CompoundAll(vec![
            match &left.kind {
                PreparedKind::Select(plan) => plan.clone(),
                _ => unreachable!(),
            },
            match &right.kind {
                PreparedKind::Select(plan) => plan.clone(),
                _ => unreachable!(),
            },
        ]),
        &mut output_columns,
    );
    if projection.is_empty() {
        output_columns.clear();
    }

    Ok(PreparedTemplate {
        sql: Arc::from(ctx.sql),
        schema_epoch: ctx.schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source: SelectSource::CompoundAll(vec![
                match left.kind {
                    PreparedKind::Select(plan) => plan,
                    _ => unreachable!(),
                },
                match right.kind {
                    PreparedKind::Select(plan) => plan,
                    _ => unreachable!(),
                },
            ]),
            distinct: false,
            projection,
            selection: None,
            group_by: Vec::new(),
            having: None,
            order_by,
            limit,
            offset,
        }),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_explain(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    analyze: bool,
    query_plan: bool,
    format: Option<AnalyzeFormatKind>,
    statement: SqlStatement,
) -> Result<PreparedTemplate> {
    let inner = Arc::new(super::bind_statement(
        conn,
        Arc::clone(&schema),
        schema_epoch,
        sql,
        statement,
    )?);
    let explain_format = if query_plan {
        crate::statement::ExplainFormat::QueryPlan
    } else {
        match format {
            Some(AnalyzeFormatKind::Keyword(AnalyzeFormat::JSON))
            | Some(AnalyzeFormatKind::Assignment(AnalyzeFormat::JSON)) => {
                crate::statement::ExplainFormat::Json
            }
            _ => crate::statement::ExplainFormat::Text,
        }
    };
    let output_columns = match explain_format {
        crate::statement::ExplainFormat::QueryPlan => Arc::from([
            "id".to_owned(),
            "parent".to_owned(),
            "notused".to_owned(),
            "detail".to_owned(),
        ]),
        crate::statement::ExplainFormat::Text | crate::statement::ExplainFormat::Json => {
            Arc::from(["explain".to_owned()])
        }
    };
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: inner.param_layout.clone(),
        output_columns,
        readonly: true,
        kind: PreparedKind::Explain(crate::statement::ExplainPlan {
            format: explain_format,
            analyze,
            inner,
        }),
    })
}

pub(crate) fn normalize_select_item(
    item: SelectItem,
    params: &mut ParamLayout,
) -> Result<SelectItem> {
    Ok(match item {
        SelectItem::UnnamedExpr(expr) => SelectItem::UnnamedExpr(normalize_expr(expr, params)?),
        SelectItem::ExprWithAlias { expr, alias } => SelectItem::ExprWithAlias {
            expr: normalize_expr(expr, params)?,
            alias,
        },
        other => other,
    })
}

pub(crate) fn normalize_select_projection(
    items: Vec<SelectItem>,
    params: &mut ParamLayout,
) -> Result<Vec<SelectItem>> {
    items
        .into_iter()
        .map(|item| normalize_select_item(item, params))
        .collect()
}

pub(crate) fn returning_output_columns(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    projection: &[SelectItem],
) -> Arc<[String]> {
    let source = SelectSource::Table(Arc::clone(table));
    let mut output_columns = Vec::new();
    for item in projection {
        match item {
            SelectItem::Wildcard(_) => push_projection_columns(&source, &mut output_columns),
            SelectItem::QualifiedWildcard(_, _) => {
                push_projection_columns(&source, &mut output_columns)
            }
            SelectItem::UnnamedExpr(expr) => output_columns.push(render_expr_name(expr)),
            SelectItem::ExprWithAlias { alias, .. } => output_columns.push(alias.value.clone()),
        }
    }
    if output_columns.is_empty() {
        push_projection_columns(&source, &mut output_columns);
    }
    Arc::from(output_columns)
}

pub(crate) fn normalize_expr(expr: Expr, params: &mut ParamLayout) -> Result<Expr> {
    Ok(match expr {
        Expr::Value(v) => match &v.value {
            Value::Placeholder(name) => {
                let name = normalize_placeholder(name, params)?;
                Expr::Value(ValueWithSpan {
                    value: Value::Placeholder(name),
                    span: v.span,
                })
            }
            _ => Expr::Value(v),
        },
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(normalize_expr(*left, params)?),
            op,
            right: Box::new(normalize_expr(*right, params)?),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op,
            expr: Box::new(normalize_expr(*expr, params)?),
        },
        Expr::Nested(expr) => Expr::Nested(Box::new(normalize_expr(*expr, params)?)),
        Expr::Collate { expr, collation } => Expr::Collate {
            expr: Box::new(normalize_expr(*expr, params)?),
            collation,
        },
        Expr::Function(mut func) => {
            normalize_function_args(&mut func.args, params)?;
            normalize_function_args(&mut func.parameters, params)?;
            Expr::Function(func)
        }
        Expr::Like {
            negated,
            any,
            expr,
            pattern,
            escape_char,
        } => Expr::Like {
            negated,
            any,
            expr: Box::new(normalize_expr(*expr, params)?),
            pattern: Box::new(normalize_expr(*pattern, params)?),
            escape_char,
        },
        Expr::ILike {
            negated,
            any,
            expr,
            pattern,
            escape_char,
        } => Expr::ILike {
            negated,
            any,
            expr: Box::new(normalize_expr(*expr, params)?),
            pattern: Box::new(normalize_expr(*pattern, params)?),
            escape_char,
        },
        Expr::Cast {
            expr,
            data_type,
            kind,
            format,
            array,
        } => Expr::Cast {
            expr: Box::new(normalize_expr(*expr, params)?),
            data_type,
            kind,
            format,
            array,
        },
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Expr::Between {
            expr: Box::new(normalize_expr(*expr, params)?),
            negated,
            low: Box::new(normalize_expr(*low, params)?),
            high: Box::new(normalize_expr(*high, params)?),
        },
        Expr::InList {
            expr,
            list,
            negated,
        } => Expr::InList {
            expr: Box::new(normalize_expr(*expr, params)?),
            list: list
                .into_iter()
                .map(|expr| normalize_expr(expr, params))
                .collect::<Result<Vec<_>>>()?,
            negated,
        },
        Expr::IsNull(expr) => Expr::IsNull(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsNotNull(expr) => Expr::IsNotNull(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsDistinctFrom(left, right) => Expr::IsDistinctFrom(
            Box::new(normalize_expr(*left, params)?),
            Box::new(normalize_expr(*right, params)?),
        ),
        Expr::IsNotDistinctFrom(left, right) => Expr::IsNotDistinctFrom(
            Box::new(normalize_expr(*left, params)?),
            Box::new(normalize_expr(*right, params)?),
        ),
        Expr::IsTrue(expr) => Expr::IsTrue(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsNotTrue(expr) => Expr::IsNotTrue(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsFalse(expr) => Expr::IsFalse(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsNotFalse(expr) => Expr::IsNotFalse(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsUnknown(expr) => Expr::IsUnknown(Box::new(normalize_expr(*expr, params)?)),
        Expr::IsNotUnknown(expr) => Expr::IsNotUnknown(Box::new(normalize_expr(*expr, params)?)),
        Expr::Case {
            case_token,
            end_token,
            operand,
            conditions,
            else_result,
        } => Expr::Case {
            case_token,
            end_token,
            operand: operand
                .map(|expr| normalize_expr(*expr, params))
                .transpose()?
                .map(Box::new),
            conditions: conditions
                .into_iter()
                .map(|when| {
                    Ok(sqlparser::ast::CaseWhen {
                        condition: normalize_expr(when.condition, params)?,
                        result: normalize_expr(when.result, params)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            else_result: else_result
                .map(|expr| normalize_expr(*expr, params))
                .transpose()?
                .map(Box::new),
        },
        other => other,
    })
}

pub(crate) fn normalize_function_args(
    args: &mut FunctionArguments,
    params: &mut ParamLayout,
) -> Result<()> {
    if let FunctionArguments::List(list) = args {
        for arg in &mut list.args {
            match arg {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                    *expr = normalize_expr(expr.clone(), params)?;
                }
                FunctionArg::Named {
                    arg: FunctionArgExpr::Expr(expr),
                    ..
                } => {
                    *expr = normalize_expr(expr.clone(), params)?;
                }
                FunctionArg::ExprNamed {
                    arg: FunctionArgExpr::Expr(expr),
                    ..
                } => {
                    *expr = normalize_expr(expr.clone(), params)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn normalize_placeholder(name: &str, params: &mut ParamLayout) -> Result<String> {
    if name == "?" {
        let slot = params.push_anonymous();
        return Ok(format!("?{slot}"));
    }
    if let Some(rest) = name.strip_prefix('?') {
        let slot = rest
            .parse::<usize>()
            .map_err(|_| Error::Parse(format!("invalid parameter {name}")))?;
        if slot == 0 {
            return Err(Error::Parse("parameter indices are 1-based".to_owned()));
        }
        params.push_numbered(slot);
        return Ok(format!("?{slot}"));
    }
    if name.starts_with(':') || name.starts_with('@') || name.starts_with('$') {
        let slot = params.push_named(name.to_owned());
        return Ok(format!("?{slot}"));
    }
    Err(Error::Parse(format!(
        "unsupported parameter syntax: {name}"
    )))
}

pub(crate) fn scan_sql_parameters(sql: &str, params: &mut ParamLayout) {
    enum State {
        Default,
        Single,
        Double,
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut state = State::Default;
    while i < bytes.len() {
        match state {
            State::Default => match bytes[i] {
                b'\'' => {
                    state = State::Single;
                    i += 1;
                }
                b'"' => {
                    state = State::Double;
                    i += 1;
                }
                b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                    state = State::LineComment;
                    i += 2;
                }
                b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                    state = State::BlockComment;
                    i += 2;
                }
                b'?' => {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i > start
                        && let Ok(index) = sql[start..i].parse::<usize>()
                        && index > 0
                    {
                        params.push_numbered(index);
                        continue;
                    }
                    params.push_anonymous();
                }
                b':' | b'@' | b'$' => {
                    let start = i;
                    i += 1;
                    while i < bytes.len() && is_param_char(bytes[i]) {
                        i += 1;
                    }
                    if i > start + 1 {
                        params.push_named(sql[start..i].to_owned());
                    }
                }
                _ => {
                    i += 1;
                }
            },
            State::Single => {
                if bytes[i] == b'\'' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 2;
                    } else {
                        state = State::Default;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            State::Double => {
                if bytes[i] == b'"' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                        i += 2;
                    } else {
                        state = State::Default;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            State::LineComment => {
                if bytes[i] == b'\n' {
                    state = State::Default;
                }
                i += 1;
            }
            State::BlockComment => {
                if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    state = State::Default;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
}
