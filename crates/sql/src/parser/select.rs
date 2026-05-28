use super::*;

pub(crate) fn bind_query(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    query: Query,
) -> Result<PreparedTemplate> {
    let mut params = ParamLayout::default();
    bind_query_with_params(conn, schema, schema_epoch, sql, query, &mut params)
}

pub(crate) fn bind_query_with_params(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    query: Query,
    params: &mut ParamLayout,
) -> Result<PreparedTemplate> {
    let Query {
        body,
        order_by,
        limit_clause,
        with,
        fetch,
        locks,
        for_clause,
        settings,
        format_clause,
        pipe_operators,
    } = query;
    // Track K — FETCH FIRST n ROWS [ONLY|WITH TIES] is the SQL-standard form
    // of LIMIT. sqlparser surfaces it on `Query::fetch` separately from
    // `limit_clause`; fold it down into a LimitOffset shape so the rest of
    // the binding pipeline only has to look at one place. WITH TIES is not
    // supported yet (would require ORDER BY tie-breaking on top of LIMIT).
    let limit_clause = match fold_fetch_into_limit_clause(limit_clause, fetch)? {
        Some(clause) => Some(clause),
        None => None,
    };
    if let Some(with) = with {
        // CTEs: materialize each CTE body (handling recursive references)
        // and dispatch to the trailing query under an active CTE scope.
        let trailing = Query {
            with: None,
            body,
            order_by,
            limit_clause,
            fetch: None,
            locks,
            for_clause,
            settings,
            format_clause,
            pipe_operators,
        };
        return crate::exec::cte::bind_with_query(conn, schema, schema_epoch, sql, with, trailing);
    }
    match *body {
        SetExpr::Query(query) => {
            let mut template =
                bind_query_with_params(conn, schema, schema_epoch, sql, *query, params)?;
            if order_by.is_some() || limit_clause.is_some() {
                template = apply_query_tail(template, order_by, limit_clause, params)?;
            }
            Ok(template)
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
                params,
            },
            op,
            set_quantifier,
            *left,
            *right,
        ),
        SetExpr::Select(select) => bind_simple_select_query(
            conn,
            schema,
            schema_epoch,
            sql,
            select,
            order_by,
            limit_clause,
            params,
        ),
        SetExpr::Values(values) => {
            bind_values_query(schema_epoch, sql, values, order_by, limit_clause, params)
        }
        // `WITH ... INSERT/UPDATE/DELETE ...` parses as
        // `Statement::Query(Query { with, body: SetExpr::Insert(...) })`.
        // Dispatch to the corresponding DML binder so the surrounding
        // CTE scope (already pushed by `bind_with_query`) is visible.
        SetExpr::Insert(stmt) => match stmt {
            sqlparser::ast::Statement::Insert(insert) => {
                super::dml::bind_insert(conn, schema, schema_epoch, sql, insert)
            }
            other => Err(Error::UnsupportedSql(format!(
                "unsupported INSERT-shaped statement in WITH: {other:?}"
            ))),
        },
        SetExpr::Update(stmt) => match stmt {
            sqlparser::ast::Statement::Update(update) => {
                super::dml::bind_update(schema, schema_epoch, sql, update)
            }
            other => Err(Error::UnsupportedSql(format!(
                "unsupported UPDATE-shaped statement in WITH: {other:?}"
            ))),
        },
        SetExpr::Delete(stmt) => match stmt {
            sqlparser::ast::Statement::Delete(delete) => {
                super::dml::bind_delete(schema, schema_epoch, sql, delete)
            }
            other => Err(Error::UnsupportedSql(format!(
                "unsupported DELETE-shaped statement in WITH: {other:?}"
            ))),
        },
        _ => Err(Error::UnsupportedSql(
            "only simple SELECT and UNION ALL queries are supported".to_owned(),
        )),
    }
}

fn apply_query_tail(
    mut template: PreparedTemplate,
    order_by: Option<sqlparser::ast::OrderBy>,
    limit_clause: Option<LimitClause>,
    params: &mut ParamLayout,
) -> Result<PreparedTemplate> {
    let plan = match template.kind {
        PreparedKind::Select(plan) => plan,
        _ => {
            return Err(Error::UnsupportedSql(
                "nested query wrappers are only supported for SELECT queries".to_owned(),
            ));
        }
    };

    let mut order_by = match order_by {
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
    validate_order_by_positions(&order_by, &template.output_columns)?;
    resolve_order_by_positions(&mut order_by, &template.output_columns);

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

    template.param_layout = params.clone();
    template.kind = PreparedKind::Select(SelectPlan {
        source: plan.source,
        distinct: plan.distinct,
        distinct_on: plan.distinct_on,
        projection: plan.projection,
        selection: plan.selection,
        group_by: plan.group_by,
        having: plan.having,
        order_by,
        limit,
        offset,
        table_hint: plan.table_hint,
    });
    Ok(template)
}

pub(crate) struct UnionAllQueryContext<'a> {
    pub conn: &'a Connection,
    pub schema: Arc<SchemaSnapshot>,
    pub schema_epoch: SchemaEpoch,
    pub sql: &'a str,
    pub order_by: Option<sqlparser::ast::OrderBy>,
    pub limit_clause: Option<LimitClause>,
    pub params: &'a mut ParamLayout,
}

pub(crate) fn bind_simple_select_query(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    mut select: Box<sqlparser::ast::Select>,
    order_by: Option<sqlparser::ast::OrderBy>,
    limit_clause: Option<LimitClause>,
    params: &mut ParamLayout,
) -> Result<PreparedTemplate> {
    // Track K — DISTINCT ON (exprs) keeps the first row per distinct
    // combination of `exprs`, where "first" is decided by the outer
    // ORDER BY. We normalize the ON expressions here and let the
    // executor's DISTINCT ON pass filter the per-group winner.
    let mut distinct_on: Vec<Expr> = Vec::new();
    let distinct = match select.distinct {
        Some(Distinct::Distinct) => true,
        Some(Distinct::All) => false,
        Some(Distinct::On(exprs)) => {
            for expr in exprs {
                distinct_on.push(normalize_expr(expr, params)?);
            }
            false
        }
        None => false,
    };

    let mut projection = Vec::new();
    let mut output_columns = Vec::new();

    let (source, mut selection) = bind_select_from(conn, &schema, select.from, params)?;

    // Track K — `WINDOW name AS (...)` named-window resolution. Build a
    // map of name → fully-resolved WindowSpec (chasing `name AS other`
    // and `name AS (other ORDER BY ...)` inheritance), then inline every
    // `OVER name` reference in the projection so the downstream window
    // evaluator only ever sees inline WindowSpec values.
    let named_windows = build_named_window_map(&select.named_window)?;
    if !named_windows.is_empty() {
        for item in &mut select.projection {
            resolve_named_windows_in_select_item(item, &named_windows)?;
        }
    }

    for item in select.projection {
        let item = normalize_select_item(item, params)?;
        if sql != "<subquery>" {
            validate_projection_columns(&source, &item)?;
        }
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

    let mut order_by = match order_by {
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
    validate_order_by_positions(&order_by, &output_columns)?;
    resolve_order_by_positions(&mut order_by, &output_columns);

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
            distinct_on,
            projection,
            selection,
            group_by,
            having,
            order_by,
            limit,
            offset,
            // Phase 5 WS-A2e: lift the single-table `INDEXED BY` /
            // `NOT INDEXED` hint that the FROM binder stashed.
            table_hint: crate::parser::prepare::take_single_table_hint(),
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
    // Map (op, quantifier) to our internal compound shape. UNION ALL keeps
    // its existing fast path; UNION (default DISTINCT), INTERSECT, EXCEPT
    // route through the new `CompoundSet` source whose dedup semantics
    // live in `crate::exec::set_ops`.
    let compound_op: Option<crate::statement::CompoundSetOp> = match (op, set_quantifier) {
        (SetOperator::Union, SetQuantifier::All) => None,
        (SetOperator::Union, SetQuantifier::Distinct | SetQuantifier::None) => {
            Some(crate::statement::CompoundSetOp::UnionDistinct)
        }
        (SetOperator::Intersect, SetQuantifier::Distinct | SetQuantifier::None) => {
            Some(crate::statement::CompoundSetOp::Intersect)
        }
        (SetOperator::Except, SetQuantifier::Distinct | SetQuantifier::None) => {
            Some(crate::statement::CompoundSetOp::Except)
        }
        (SetOperator::Intersect | SetOperator::Except, SetQuantifier::All) => {
            return Err(Error::UnsupportedSql(format!("{op} ALL is not supported")));
        }
        (op, quant) => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported set operation: {op} {quant}"
            )));
        }
    };

    let left = bind_query_with_params(
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
        ctx.params,
    )?;
    let right = bind_query_with_params(
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
        ctx.params,
    )?;

    // For ORDER BY column resolution we need names — use the LEFT branch
    // names since SQL columns are positional in set ops, and SQLite reports
    // the left side's names.
    let left_columns_for_names: Arc<[String]> = left.output_columns.clone();

    let mut order_by = match ctx.order_by {
        Some(order_by) => match order_by.kind {
            OrderByKind::Expressions(exprs) => exprs
                .into_iter()
                .map(|expr| {
                    let options = expr.options;
                    let with_fill = expr.with_fill;
                    let expr = normalize_expr(expr.expr, ctx.params)?;
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
    validate_order_by_positions(&order_by, &left_columns_for_names)?;
    resolve_order_by_positions(&mut order_by, &left_columns_for_names);
    let (limit, offset) = match ctx.limit_clause {
        Some(LimitClause::LimitOffset {
            limit,
            offset,
            limit_by: _,
        }) => {
            let limit = match limit {
                Some(expr) => Some(normalize_expr(expr, ctx.params)?),
                None => None,
            };
            let offset = match offset {
                Some(offset) => Some(normalize_expr(offset.value, ctx.params)?),
                None => None,
            };
            (limit, offset)
        }
        Some(LimitClause::OffsetCommaLimit { offset, limit }) => (
            Some(normalize_expr(limit, ctx.params)?),
            Some(normalize_expr(offset, ctx.params)?),
        ),
        None => (None, None),
    };

    let left_plan = match left.kind {
        PreparedKind::Select(plan) => plan,
        _ => unreachable!("compound branch is always a SELECT"),
    };
    let right_plan = match right.kind {
        PreparedKind::Select(plan) => plan,
        _ => unreachable!("compound branch is always a SELECT"),
    };

    let projection = Vec::new();
    let source = match compound_op {
        None => SelectSource::CompoundAll(vec![left_plan, right_plan]),
        Some(op) => SelectSource::CompoundSet {
            op,
            branches: vec![left_plan, right_plan],
        },
    };
    let mut output_columns = Vec::new();
    push_projection_columns(&source, &mut output_columns);
    if output_columns.is_empty() {
        output_columns.extend(left_columns_for_names.iter().cloned());
    }

    Ok(PreparedTemplate {
        sql: Arc::from(ctx.sql),
        schema_epoch: ctx.schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ctx.params.clone(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source,
            distinct: false,
            distinct_on: Vec::new(),
            projection,
            selection: None,
            group_by: Vec::new(),
            having: None,
            order_by,
            limit,
            offset,
            table_hint: None,
        }),
    })
}

fn bind_values_query(
    schema_epoch: SchemaEpoch,
    sql: &str,
    values: sqlparser::ast::Values,
    order_by: Option<sqlparser::ast::OrderBy>,
    limit_clause: Option<LimitClause>,
    params: &mut ParamLayout,
) -> Result<PreparedTemplate> {
    let column_count = values.rows.first().map(Vec::len).unwrap_or(0);
    if column_count == 0 {
        return Err(Error::Parse(
            "VALUES requires at least one column".to_owned(),
        ));
    }

    let mut rows = Vec::with_capacity(values.rows.len());
    for row in values.rows {
        if row.len() != column_count {
            return Err(Error::Parse(
                "all VALUES rows must have the same number of columns".to_owned(),
            ));
        }
        let mut out = Vec::with_capacity(column_count);
        for expr in row {
            let expr = normalize_expr(expr, params)?;
            out.push(crate::exec::expr::eval_scalar(
                &expr,
                &crate::exec::expr::RowContext::Empty,
                &[],
            )?);
        }
        rows.push(out);
    }

    let columns: Vec<String> = (1..=column_count)
        .map(|idx| format!("column{idx}"))
        .collect();
    let output_columns = Arc::from(columns.clone());
    let source = SelectSource::Cte {
        name: Arc::from("__values"),
        alias: None,
        columns: Arc::from(columns),
        rows: Arc::from(rows),
    };

    let mut order_by = match order_by {
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
    validate_order_by_positions(&order_by, &output_columns)?;
    resolve_order_by_positions(&mut order_by, &output_columns);

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

    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params.clone(),
        output_columns,
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source,
            distinct: false,
            distinct_on: Vec::new(),
            projection: Vec::new(),
            selection: None,
            group_by: Vec::new(),
            having: None,
            order_by,
            limit,
            offset,
            table_hint: None,
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
    let normalized = match expr {
        Expr::Value(v) => {
            if let Some(name) = crate::parser::bind::as_bind_name(&v.value) {
                let normalized = normalize_bind_marker(name, params)?;
                Expr::Value(ValueWithSpan {
                    value: crate::parser::bind::into_bind_value(normalized),
                    span: v.span,
                })
            } else {
                Expr::Value(v)
            }
        }
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
    };
    Ok(fold_constant_expr(normalized))
}

fn fold_constant_expr(expr: Expr) -> Expr {
    if !expr_is_foldable(&expr) {
        return expr;
    }
    match crate::exec::expr::eval_scalar(&expr, &crate::exec::expr::RowContext::Empty, &[]) {
        Ok(value) => sql_value_to_expr(value),
        Err(_) => expr,
    }
}

fn expr_is_foldable(expr: &Expr) -> bool {
    match expr {
        Expr::Value(v) => crate::parser::bind::as_bind_name(&v.value).is_none(),
        Expr::Nested(expr) => expr_is_foldable(expr),
        Expr::UnaryOp { expr, .. } => expr_is_foldable(expr),
        Expr::BinaryOp { left, right, .. } => expr_is_foldable(left) && expr_is_foldable(right),
        Expr::Collate { .. } => false,
        Expr::Cast { expr, .. } => expr_is_foldable(expr),
        Expr::Ceil { expr, .. } | Expr::Floor { expr, .. } => expr_is_foldable(expr),
        Expr::Substring {
            expr,
            substring_from,
            substring_for,
            ..
        } => {
            expr_is_foldable(expr)
                && substring_from.as_deref().is_none_or(expr_is_foldable)
                && substring_for.as_deref().is_none_or(expr_is_foldable)
        }
        Expr::Trim {
            expr,
            trim_what,
            trim_characters,
            ..
        } => {
            expr_is_foldable(expr)
                && trim_what.as_deref().is_none_or(expr_is_foldable)
                && trim_characters.iter().flatten().all(expr_is_foldable)
        }
        Expr::Between {
            expr, low, high, ..
        } => expr_is_foldable(expr) && expr_is_foldable(low) && expr_is_foldable(high),
        Expr::InList { expr, list, .. } => {
            expr_is_foldable(expr) && list.iter().all(expr_is_foldable)
        }
        Expr::IsNull(expr)
        | Expr::IsNotNull(expr)
        | Expr::IsTrue(expr)
        | Expr::IsNotTrue(expr)
        | Expr::IsFalse(expr)
        | Expr::IsNotFalse(expr)
        | Expr::IsUnknown(expr)
        | Expr::IsNotUnknown(expr) => expr_is_foldable(expr),
        Expr::IsDistinctFrom(left, right) | Expr::IsNotDistinctFrom(left, right) => {
            expr_is_foldable(left) && expr_is_foldable(right)
        }
        Expr::Function(func) => function_is_foldable(func),
        Expr::Identifier(_)
        | Expr::CompoundIdentifier(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Subquery(_)
        | Expr::Like { .. }
        | Expr::ILike { .. }
        | Expr::Case { .. } => false,
        _ => false,
    }
}

fn function_is_foldable(func: &sqlparser::ast::Function) -> bool {
    if func.over.is_some() || func.filter.is_some() {
        return false;
    }
    let name = func.name.to_string().to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        "abs"
            | "ceil"
            | "ceiling"
            | "char"
            | "coalesce"
            | "floor"
            | "format"
            | "hex"
            | "ifnull"
            | "instr"
            | "json"
            | "json_array"
            | "json_array_length"
            | "json_extract"
            | "json_insert"
            | "json_object"
            | "json_quote"
            | "json_replace"
            | "json_set"
            | "json_type"
            | "json_valid"
            | "length"
            | "lower"
            | "ltrim"
            | "max"
            | "min"
            | "nullif"
            | "pow"
            | "power"
            | "printf"
            | "quote"
            | "replace"
            | "round"
            | "rtrim"
            | "sign"
            | "sin"
            | "sqrt"
            | "substr"
            | "substring"
            | "trim"
            | "typeof"
            | "unicode"
            | "upper"
    ) {
        return false;
    }
    function_args_are_foldable(&func.args) && function_args_are_foldable(&func.parameters)
}

fn function_args_are_foldable(args: &FunctionArguments) -> bool {
    match args {
        FunctionArguments::None => true,
        FunctionArguments::List(list) => {
            if list.duplicate_treatment.is_some() || !list.clauses.is_empty() {
                return false;
            }
            list.args.iter().all(|arg| match arg {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))
                | FunctionArg::Named {
                    arg: FunctionArgExpr::Expr(expr),
                    ..
                }
                | FunctionArg::ExprNamed {
                    arg: FunctionArgExpr::Expr(expr),
                    ..
                } => expr_is_foldable(expr),
                _ => false,
            })
        }
        _ => false,
    }
}

fn sql_value_to_expr(value: SqlValue) -> Expr {
    Expr::Value(sql_value_to_ast_value(value).into())
}

fn sql_value_to_ast_value(value: SqlValue) -> Value {
    match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(value) => Value::Number(value.to_string(), false),
        SqlValue::Real(value) => Value::Number(
            crate::exec::expr::scalar::value::format_real_sqlite(value),
            false,
        ),
        SqlValue::Text(value) => Value::SingleQuotedString(value.to_string()),
        SqlValue::Blob(value) => {
            let mut rendered = String::with_capacity(value.len() * 2);
            for byte in value.iter() {
                use std::fmt::Write as _;
                let _ = write!(&mut rendered, "{byte:02X}");
            }
            Value::HexStringLiteral(rendered)
        }
    }
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
        for clause in &mut list.clauses {
            if let FunctionArgumentClause::OrderBy(order_by) = clause {
                for order in order_by {
                    order.expr = normalize_expr(order.expr.clone(), params)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn normalize_bind_marker(name: &str, params: &mut ParamLayout) -> Result<String> {
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

/// SQLite parity: a bare positive integer at the top of an `ORDER BY` term is a
/// 1-based positional reference to the N-th output column, not the literal
/// integer value. Rewrites matching terms to `Expr::Identifier(<column-name>)`
/// so the existing column-lookup path (`crate::exec::expr::lookup_column`)
/// resolves them. `ORDER BY 1+0`, `ORDER BY 1.5`, and `ORDER BY (1)` are left
/// alone — only bare integer literals are positional in SQLite.
fn resolve_order_by_positions(items: &mut [OrderByExpr], output_columns: &[String]) {
    for item in items {
        if let Some(idx) = top_level_positive_int(&item.expr)
            && let Ok(pos) = usize::try_from(idx)
            && pos >= 1
            && pos <= output_columns.len()
        {
            let name = output_columns[pos - 1].clone();
            if name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                item.expr = Expr::Identifier(Ident::new(name));
            }
        }
    }
}

/// Validate that every bare positional integer in `items` references
/// a real output column. Returns `Err` with a SQLite-compatible
/// "Nth ORDER BY term out of range" message for the first offender.
pub(crate) fn validate_order_by_positions(
    items: &[OrderByExpr],
    output_columns: &[String],
) -> Result<()> {
    let n_cols = output_columns.len();
    for (idx, item) in items.iter().enumerate() {
        if let Some(pos) = top_level_positive_int(&item.expr)
            && (pos < 1 || pos as usize > n_cols)
        {
            let ordinal_word = ordinal_word(idx + 1);
            return Err(Error::Bind(format!(
                "{ordinal_word} ORDER BY term out of range - should be between 1 and {n_cols}"
            )));
        }
    }
    Ok(())
}

fn ordinal_word(n: usize) -> String {
    let mod100 = n % 100;
    if (11..=13).contains(&mod100) {
        return format!("{n}th");
    }
    let suffix = match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn top_level_positive_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Value(v) => match &v.value {
            Value::Number(n, _) => n.parse::<i64>().ok().filter(|n| *n > 0),
            _ => None,
        },
        _ => None,
    }
}

/// Track K — Resolve a list of `WINDOW name AS (...)` definitions into a
/// `name -> WindowSpec` map. Inherited references like
/// `WINDOW w2 AS w` or `WINDOW w2 AS (w ORDER BY ...)` are flattened so
/// every entry in the returned map is a fully self-contained spec.
///
/// The map keys are lowercased identifier values; lookups are
/// case-insensitive to match PG/SQLite.
fn build_named_window_map(
    definitions: &[sqlparser::ast::NamedWindowDefinition],
) -> Result<std::collections::HashMap<String, sqlparser::ast::WindowSpec>> {
    use sqlparser::ast::NamedWindowExpr;
    let mut map: std::collections::HashMap<String, sqlparser::ast::WindowSpec> =
        std::collections::HashMap::new();
    // Resolve in declared order. PG accepts forward refs only in some
    // dialects; we resolve strictly in order, which matches the
    // beyond-SQLite test surface (`WINDOW w AS (...), w2 AS (w ...)`).
    for def in definitions {
        let key = def.0.value.to_ascii_lowercase();
        let resolved = match &def.1 {
            NamedWindowExpr::WindowSpec(spec) => merge_window_with_base(spec, &map)?,
            NamedWindowExpr::NamedWindow(name) => {
                let base = map
                    .get(&name.value.to_ascii_lowercase())
                    .cloned()
                    .ok_or_else(|| {
                        Error::UnsupportedSql(format!(
                            "named window references unknown window: {}",
                            name.value
                        ))
                    })?;
                base
            }
        };
        map.insert(key, resolved);
    }
    Ok(map)
}

/// If `spec.window_name` is set, look up the base in `map` and merge
/// PARTITION BY / ORDER BY / FRAME from `spec` on top.
///
/// SQL standard: the inherited base supplies PARTITION BY; the inheriting
/// spec may extend ORDER BY (or add a frame). RedlineDB follows the
/// PG-compatible "additive on top" rule.
fn merge_window_with_base(
    spec: &sqlparser::ast::WindowSpec,
    map: &std::collections::HashMap<String, sqlparser::ast::WindowSpec>,
) -> Result<sqlparser::ast::WindowSpec> {
    let Some(base_name) = spec.window_name.as_ref() else {
        return Ok(sqlparser::ast::WindowSpec {
            window_name: None,
            partition_by: spec.partition_by.clone(),
            order_by: spec.order_by.clone(),
            window_frame: spec.window_frame.clone(),
        });
    };
    let base = map
        .get(&base_name.value.to_ascii_lowercase())
        .ok_or_else(|| {
            Error::UnsupportedSql(format!(
                "named window references unknown window: {}",
                base_name.value
            ))
        })?;
    let mut partition_by = base.partition_by.clone();
    partition_by.extend(spec.partition_by.iter().cloned());
    let mut order_by = base.order_by.clone();
    order_by.extend(spec.order_by.iter().cloned());
    let window_frame = spec
        .window_frame
        .clone()
        .or_else(|| base.window_frame.clone());
    Ok(sqlparser::ast::WindowSpec {
        window_name: None,
        partition_by,
        order_by,
        window_frame,
    })
}

/// Walk a [`SelectItem`] tree and replace every
/// `OVER name` reference (and `OVER (name extra...)`) with the inlined
/// WindowSpec from `named_windows`.
fn resolve_named_windows_in_select_item(
    item: &mut SelectItem,
    named_windows: &std::collections::HashMap<String, sqlparser::ast::WindowSpec>,
) -> Result<()> {
    match item {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            resolve_named_windows_in_expr(expr, named_windows)
        }
        _ => Ok(()),
    }
}

fn resolve_named_windows_in_expr(
    expr: &mut Expr,
    named_windows: &std::collections::HashMap<String, sqlparser::ast::WindowSpec>,
) -> Result<()> {
    use sqlparser::ast::WindowType;
    match expr {
        Expr::Function(func) => {
            if let Some(over) = func.over.as_mut() {
                match over {
                    WindowType::NamedWindow(name) => {
                        let spec = named_windows
                            .get(&name.value.to_ascii_lowercase())
                            .cloned()
                            .ok_or_else(|| {
                                Error::UnsupportedSql(format!(
                                    "OVER references unknown window: {}",
                                    name.value
                                ))
                            })?;
                        *over = WindowType::WindowSpec(spec);
                    }
                    WindowType::WindowSpec(spec) => {
                        if spec.window_name.is_some() {
                            *spec = merge_window_with_base(spec, named_windows)?;
                        }
                    }
                }
            }
            // Recurse into function arguments (so window calls nested
            // inside e.g. coalesce(..) still resolve).
            if let sqlparser::ast::FunctionArguments::List(list) = &mut func.args {
                for arg in &mut list.args {
                    resolve_named_windows_in_function_arg(arg, named_windows)?;
                }
            }
        }
        Expr::Nested(inner) => resolve_named_windows_in_expr(inner, named_windows)?,
        Expr::BinaryOp { left, right, .. } => {
            resolve_named_windows_in_expr(left, named_windows)?;
            resolve_named_windows_in_expr(right, named_windows)?;
        }
        Expr::UnaryOp { expr, .. } => resolve_named_windows_in_expr(expr, named_windows)?,
        Expr::Cast { expr, .. } => resolve_named_windows_in_expr(expr, named_windows)?,
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand.as_mut() {
                resolve_named_windows_in_expr(operand, named_windows)?;
            }
            for when in conditions {
                resolve_named_windows_in_expr(&mut when.condition, named_windows)?;
                resolve_named_windows_in_expr(&mut when.result, named_windows)?;
            }
            if let Some(else_result) = else_result.as_mut() {
                resolve_named_windows_in_expr(else_result, named_windows)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn resolve_named_windows_in_function_arg(
    arg: &mut sqlparser::ast::FunctionArg,
    named_windows: &std::collections::HashMap<String, sqlparser::ast::WindowSpec>,
) -> Result<()> {
    use sqlparser::ast::{FunctionArg, FunctionArgExpr};
    match arg {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))
        | FunctionArg::Named {
            arg: FunctionArgExpr::Expr(expr),
            ..
        }
        | FunctionArg::ExprNamed {
            arg: FunctionArgExpr::Expr(expr),
            ..
        } => resolve_named_windows_in_expr(expr, named_windows),
        _ => Ok(()),
    }
}

/// Track K — collapse a SQL-standard `FETCH FIRST n ROWS [ONLY|WITH TIES]`
/// clause into the same `LimitClause::LimitOffset` shape the rest of the
/// binder uses for plain `LIMIT n`. Preserves an already-present LIMIT
/// (precedence: explicit LIMIT wins over FETCH, so this is conservative
/// and only fills in when LIMIT is absent), and preserves OFFSET in both
/// cases — `OFFSET n ROWS FETCH NEXT m ROWS ONLY` is the standard
/// pagination form and survives untouched.
///
/// `FETCH FIRST [n] ROW[S] ONLY` and `FETCH NEXT [n] ROW[S] ONLY` are
/// equivalent (the keyword choice is cosmetic in the SQL standard); the
/// quantity defaults to 1 when omitted (e.g. `FETCH FIRST ROW ONLY`).
/// `WITH TIES` and `PERCENT` remain unsupported (they require tie-aware
/// LIMIT semantics).
fn fold_fetch_into_limit_clause(
    limit_clause: Option<LimitClause>,
    fetch: Option<sqlparser::ast::Fetch>,
) -> Result<Option<LimitClause>> {
    let Some(fetch) = fetch else {
        return Ok(limit_clause);
    };
    if fetch.with_ties {
        return Err(Error::UnsupportedSql(
            "FETCH ... WITH TIES is not supported".to_owned(),
        ));
    }
    if fetch.percent {
        return Err(Error::UnsupportedSql(
            "FETCH ... PERCENT is not supported".to_owned(),
        ));
    }
    // Default quantity = 1 (matches `FETCH FIRST ROW ONLY`).
    let fetch_qty = fetch.quantity.unwrap_or_else(|| {
        sqlparser::ast::Expr::Value(sqlparser::ast::ValueWithSpan {
            value: sqlparser::ast::Value::Number("1".to_owned(), false),
            span: sqlparser::tokenizer::Span::empty(),
        })
    });
    match limit_clause {
        None => Ok(Some(LimitClause::LimitOffset {
            limit: Some(fetch_qty),
            offset: None,
            limit_by: Vec::new(),
        })),
        Some(LimitClause::LimitOffset {
            limit,
            offset,
            limit_by,
        }) => {
            // If both LIMIT and FETCH are present (rare), the explicit
            // LIMIT wins. Otherwise we slot the FETCH quantity in.
            let limit = limit.or(Some(fetch_qty));
            Ok(Some(LimitClause::LimitOffset {
                limit,
                offset,
                limit_by,
            }))
        }
        Some(other) => {
            // OffsetCommaLimit is MySQL-specific; FETCH is the SQL-standard
            // form and shouldn't appear with it. Pass through unchanged.
            Ok(Some(other))
        }
    }
}

#[cfg(test)]
mod order_by_position_tests {
    use super::*;
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    fn parse_expr(text: &str) -> Expr {
        let dialect = SQLiteDialect {};
        SqlParser::new(&dialect)
            .try_with_sql(text)
            .expect("parser init")
            .parse_expr()
            .expect("parse expr")
    }

    #[test]
    fn bare_positive_integer_resolves() {
        assert_eq!(top_level_positive_int(&parse_expr("1")), Some(1));
        assert_eq!(top_level_positive_int(&parse_expr("42")), Some(42));
    }

    #[test]
    fn zero_and_negative_integers_are_not_positions() {
        assert_eq!(top_level_positive_int(&parse_expr("0")), None);
        // `-1` parses as UnaryOp(Minus, 1) — top-level isn't a literal.
        assert_eq!(top_level_positive_int(&parse_expr("-1")), None);
    }

    #[test]
    fn non_integer_literals_are_not_positions() {
        assert_eq!(top_level_positive_int(&parse_expr("1.5")), None);
        assert_eq!(top_level_positive_int(&parse_expr("'1'")), None);
    }

    #[test]
    fn nested_integer_is_not_a_position() {
        // The key invariant: the rewrite must not descend into BinaryOp,
        // Nested, or any other compound shape — only a bare top-level
        // integer literal counts as a positional reference.
        assert_eq!(top_level_positive_int(&parse_expr("1 + 1")), None);
        assert_eq!(top_level_positive_int(&parse_expr("(1)")), None);
        assert_eq!(top_level_positive_int(&parse_expr("a + 1")), None);
    }

    #[test]
    fn resolve_rewrites_matching_position_and_leaves_others() {
        let cols = vec!["a".to_owned(), "b".to_owned()];
        let mk = |sql: &str| OrderByExpr {
            expr: parse_expr(sql),
            options: sqlparser::ast::OrderByOptions {
                asc: None,
                nulls_first: None,
            },
            with_fill: None,
        };
        let mut items = vec![mk("1"), mk("2"), mk("1 + 1"), mk("3"), mk("a")];
        resolve_order_by_positions(&mut items, &cols);
        // [0] 1 -> Identifier("a")
        assert!(matches!(&items[0].expr, Expr::Identifier(id) if id.value == "a"));
        // [1] 2 -> Identifier("b")
        assert!(matches!(&items[1].expr, Expr::Identifier(id) if id.value == "b"));
        // [2] `1 + 1` left as BinaryOp
        assert!(matches!(&items[2].expr, Expr::BinaryOp { .. }));
        // [3] 3 out of range -> left as Value
        assert!(matches!(&items[3].expr, Expr::Value(_)));
        // [4] `a` was already an Identifier
        assert!(matches!(&items[4].expr, Expr::Identifier(id) if id.value == "a"));
    }
}

#[cfg(test)]
mod nested_query_wrapper_tests {
    use super::*;
    use crate::DbOptions;
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    fn parse_expr(text: &str) -> Expr {
        let dialect = SQLiteDialect {};
        SqlParser::new(&dialect)
            .try_with_sql(text)
            .expect("parser init")
            .parse_expr()
            .expect("parse expr")
    }

    #[test]
    fn wrapper_order_by_and_limit_apply_to_inner_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = crate::Database::create(dir.path().join("nested_query.db"), DbOptions::default())
            .expect("create db");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(v INTEGER)")
            .expect("create table");

        let dialect = SQLiteDialect {};
        let mut statements =
            SqlParser::parse_sql(&dialect, "SELECT v FROM t UNION ALL SELECT 5 AS v")
                .expect("parse sql");
        let inner = match statements.remove(0) {
            SqlStatement::Query(query) => query,
            other => panic!("unexpected statement: {other:?}"),
        };
        let outer = Query {
            with: None,
            body: Box::new(SetExpr::Query(inner)),
            order_by: Some(sqlparser::ast::OrderBy {
                kind: OrderByKind::Expressions(vec![OrderByExpr {
                    expr: parse_expr("1"),
                    options: sqlparser::ast::OrderByOptions::default(),
                    with_fill: None,
                }]),
                interpolate: None,
            }),
            limit_clause: Some(LimitClause::LimitOffset {
                limit: Some(parse_expr("2")),
                offset: None,
                limit_by: Vec::new(),
            }),
            fetch: None,
            locks: Vec::new(),
            for_clause: None,
            settings: None,
            format_clause: None,
            pipe_operators: Vec::new(),
        };
        let schema = conn.engine().schema_snapshot();
        let mut params = ParamLayout::default();
        let template = bind_query_with_params(
            conn.as_ref(),
            schema,
            conn.schema_epoch(),
            "nested",
            outer,
            &mut params,
        )
        .expect("bind nested wrapper");
        assert_eq!(
            params.count(),
            0,
            "wrapper without parameters should keep an empty layout"
        );
        let plan = match template.kind {
            PreparedKind::Select(plan) => plan,
            other => panic!("unexpected prepared kind: {other:?}"),
        };
        assert_eq!(plan.order_by.len(), 1);
        assert!(matches!(&plan.order_by[0].expr, Expr::Identifier(id) if id.value == "v"));
        assert!(matches!(plan.limit, Some(Expr::Value(_))));
        assert!(matches!(plan.offset, None));
    }
}
