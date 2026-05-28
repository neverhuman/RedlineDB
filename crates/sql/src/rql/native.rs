use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use sqlparser::ast::{
    Ident, OrderByExpr, OrderByOptions, SelectItem, SelectItemQualifiedWildcardKind,
    WildcardAdditionalOptions,
};

use super::{
    RqlExpr, RqlName, RqlSelect, RqlSelectItem, RqlTableRef, normalized_expr, rql_sql, sql_name,
    u64_expr,
};
use crate::error::{Error, Result};
use crate::parser::{normalize_expr, push_projection_columns, render_expr_name};
use crate::statement::{
    BoundTable, ParamLayout, PreparedKind, PreparedTemplate, SelectPlan, SelectSource,
};

pub(super) fn lower_native_select(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    select: &RqlSelect,
) -> Result<Option<PreparedTemplate>> {
    if !native_select_shape_supported(&schema, select) {
        return Ok(None);
    }
    let source = if let Some(from) = &select.from {
        let Some(source) = native_select_source(&schema, from) else {
            return Ok(None);
        };
        source
    } else {
        SelectSource::Empty
    };
    let mut params = ParamLayout::default();
    let (projection, output_columns) = native_select_projection(
        &source,
        select.from.as_ref(),
        &select.projection,
        &mut params,
    )?;
    let selection = select
        .filter
        .as_ref()
        .map(|expr| normalized_expr(expr, &mut params))
        .transpose()?;
    let order_by = select
        .order_by
        .iter()
        .map(|order| {
            Ok(OrderByExpr {
                expr: normalized_expr(&order.expr, &mut params)?,
                options: OrderByOptions {
                    asc: Some(!order.descending),
                    nulls_first: order.nulls_first,
                },
                with_fill: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let group_by = select
        .group_by
        .iter()
        .map(|expr| normalized_expr(expr, &mut params))
        .collect::<Result<Vec<_>>>()?;
    let having = select
        .having
        .as_ref()
        .map(|expr| normalized_expr(expr, &mut params))
        .transpose()?;
    let limit = select
        .limit
        .map(|value| normalize_expr(u64_expr(value), &mut params));
    let offset = select
        .offset
        .map(|value| normalize_expr(u64_expr(value), &mut params));
    Ok(Some(PreparedTemplate {
        sql: rql_sql("select_native"),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params.clone(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source,
            distinct: select.distinct,
            distinct_on: Vec::new(),
            projection,
            selection,
            group_by,
            having,
            order_by,
            limit: limit.transpose()?,
            offset: offset.transpose()?,
            table_hint: None,
        }),
    }))
}

pub(super) fn native_select_shape_supported(schema: &SchemaSnapshot, select: &RqlSelect) -> bool {
    if !select.joins.is_empty() {
        return false;
    }
    let from = select.from.as_ref();
    if let Some(from) = from {
        if crate::exec::view::name_is_view(schema, &sql_name(&from.name)) {
            return false;
        }
    } else if select.projection.is_empty() || !select.group_by.is_empty() {
        return false;
    }
    let projection_has_aggregate = select
        .projection
        .iter()
        .any(|item| matches!(item, RqlSelectItem::Expr { expr, .. } if native_select_expr_is_bare_aggregate(expr)));
    if !select.group_by.is_empty() && !projection_has_aggregate {
        return false;
    }
    if let Some(having) = &select.having {
        if from.is_none()
            || !projection_has_aggregate
            || !native_select_aggregate_clause_expr_supported(from, &select.group_by, having)
        {
            return false;
        }
    }
    if projection_has_aggregate
        && (from.is_none()
            || !select.projection.iter().all(|item| match item {
                RqlSelectItem::Expr { expr, .. } => {
                    (native_select_expr_is_bare_aggregate(expr)
                        && native_select_projection_expr_supported(from, expr))
                        || (select.group_by.iter().any(|group| group == expr)
                            && native_select_expr_supported(from, expr))
                }
                _ => false,
            }))
    {
        return false;
    }
    select
        .projection
        .iter()
        .all(|item| native_select_item_supported(from, item))
        && select
            .filter
            .as_ref()
            .is_none_or(|expr| native_select_expr_supported(from, expr))
        && select
            .group_by
            .iter()
            .all(|expr| native_select_expr_supported(from, expr))
        && select.order_by.iter().all(|order| {
            let expr_supported = if projection_has_aggregate {
                native_select_aggregate_order_expr_supported(
                    from,
                    &select.group_by,
                    &select.projection,
                    &order.expr,
                )
            } else {
                native_select_expr_supported(from, &order.expr)
            };
            expr_supported && !matches!(order.expr, RqlExpr::Integer { value } if value > 0)
        })
}

fn native_select_aggregate_order_expr_supported(
    from: Option<&RqlTableRef>,
    group_by: &[RqlExpr],
    projection: &[RqlSelectItem],
    expr: &RqlExpr,
) -> bool {
    if let RqlExpr::Column { column } = expr
        && column.table.is_none()
        && let Some(projected_expr) = projection_alias_expr(projection, &column.name)
    {
        return native_select_aggregate_clause_expr_supported(from, group_by, projected_expr);
    }
    native_select_aggregate_clause_expr_supported(from, group_by, expr)
}

fn projection_alias_expr<'a>(projection: &'a [RqlSelectItem], target: &str) -> Option<&'a RqlExpr> {
    projection.iter().find_map(|item| {
        let RqlSelectItem::Expr {
            expr,
            alias: Some(alias),
        } = item
        else {
            return None;
        };
        alias.eq_ignore_ascii_case(target).then_some(expr)
    })
}

fn native_select_aggregate_clause_expr_supported(
    from: Option<&RqlTableRef>,
    group_by: &[RqlExpr],
    expr: &RqlExpr,
) -> bool {
    if group_by.iter().any(|group| group == expr) && native_select_expr_supported(from, expr) {
        return true;
    }
    match expr {
        RqlExpr::Null
        | RqlExpr::Bool { .. }
        | RqlExpr::Integer { .. }
        | RqlExpr::Real { .. }
        | RqlExpr::Text { .. }
        | RqlExpr::Blob { .. }
        | RqlExpr::Param { .. } => true,
        RqlExpr::Column { .. } => false,
        RqlExpr::Unary { expr, .. }
        | RqlExpr::Cast { expr, .. }
        | RqlExpr::IsNull { expr, .. }
        | RqlExpr::Nested { expr } => {
            native_select_aggregate_clause_expr_supported(from, group_by, expr)
        }
        RqlExpr::Binary { left, right, .. } => {
            native_select_aggregate_clause_expr_supported(from, group_by, left)
                && native_select_aggregate_clause_expr_supported(from, group_by, right)
        }
        RqlExpr::Between {
            expr, low, high, ..
        } => {
            native_select_aggregate_clause_expr_supported(from, group_by, expr)
                && native_select_aggregate_clause_expr_supported(from, group_by, low)
                && native_select_aggregate_clause_expr_supported(from, group_by, high)
        }
        RqlExpr::InList { expr, list, .. } => {
            native_select_aggregate_clause_expr_supported(from, group_by, expr)
                && list
                    .iter()
                    .all(|item| native_select_aggregate_clause_expr_supported(from, group_by, item))
        }
        RqlExpr::Function {
            name,
            args,
            distinct,
        } if native_select_function_is_aggregate(name, args.len()) => {
            from.is_some()
                && !*distinct
                && args
                    .iter()
                    .all(|arg| native_select_expr_supported(from, arg))
        }
        RqlExpr::Function {
            name,
            args,
            distinct,
        } => {
            !*distinct
                && !native_select_function_is_aggregate(name, args.len())
                && args
                    .iter()
                    .all(|arg| native_select_aggregate_clause_expr_supported(from, group_by, arg))
        }
        RqlExpr::CountStar => from.is_some(),
        RqlExpr::InSubquery { .. } | RqlExpr::Subquery { .. } | RqlExpr::Exists { .. } => false,
    }
}

fn native_select_item_supported(from: Option<&RqlTableRef>, item: &RqlSelectItem) -> bool {
    match item {
        RqlSelectItem::Wildcard => from.is_some(),
        RqlSelectItem::QualifiedWildcard { table } => from
            .as_ref()
            .is_some_and(|from| native_select_table_matches(from, table)),
        RqlSelectItem::Expr { expr, .. } => native_select_projection_expr_supported(from, expr),
    }
}

fn native_select_table_matches(from: &RqlTableRef, table: &str) -> bool {
    match &from.alias {
        Some(alias) => alias.eq_ignore_ascii_case(table),
        None => from.name.name.eq_ignore_ascii_case(table),
    }
}

fn native_select_expr_supported(from: Option<&RqlTableRef>, expr: &RqlExpr) -> bool {
    match expr {
        RqlExpr::Null
        | RqlExpr::Bool { .. }
        | RqlExpr::Integer { .. }
        | RqlExpr::Real { .. }
        | RqlExpr::Text { .. }
        | RqlExpr::Blob { .. }
        | RqlExpr::Param { .. } => true,
        RqlExpr::Column { column } => from.is_some_and(|from| {
            column
                .table
                .as_ref()
                .is_none_or(|table| native_select_table_matches(from, table))
        }),
        RqlExpr::Unary { expr, .. }
        | RqlExpr::Cast { expr, .. }
        | RqlExpr::IsNull { expr, .. }
        | RqlExpr::Nested { expr } => native_select_expr_supported(from, expr),
        RqlExpr::Binary { left, right, .. } => {
            native_select_expr_supported(from, left) && native_select_expr_supported(from, right)
        }
        RqlExpr::Between {
            expr, low, high, ..
        } => {
            native_select_expr_supported(from, expr)
                && native_select_expr_supported(from, low)
                && native_select_expr_supported(from, high)
        }
        RqlExpr::InList { expr, list, .. } => {
            native_select_expr_supported(from, expr)
                && list
                    .iter()
                    .all(|item| native_select_expr_supported(from, item))
        }
        RqlExpr::Function {
            name,
            args,
            distinct,
        } => {
            !*distinct
                && !native_select_function_is_aggregate(name, args.len())
                && args
                    .iter()
                    .all(|arg| native_select_expr_supported(from, arg))
        }
        RqlExpr::CountStar
        | RqlExpr::InSubquery { .. }
        | RqlExpr::Subquery { .. }
        | RqlExpr::Exists { .. } => false,
    }
}

fn native_select_projection_expr_supported(from: Option<&RqlTableRef>, expr: &RqlExpr) -> bool {
    match expr {
        RqlExpr::CountStar => from.is_some(),
        RqlExpr::Function {
            name,
            args,
            distinct,
        } if native_select_function_is_aggregate(name, args.len()) => {
            from.is_some()
                && !*distinct
                && args
                    .iter()
                    .all(|arg| native_select_expr_supported(from, arg))
        }
        _ => native_select_expr_supported(from, expr),
    }
}

fn native_select_expr_is_bare_aggregate(expr: &RqlExpr) -> bool {
    match expr {
        RqlExpr::CountStar => true,
        RqlExpr::Function { name, args, .. } => {
            native_select_function_is_aggregate(name, args.len())
        }
        _ => false,
    }
}

fn native_select_function_is_aggregate(name: &str, arity: usize) -> bool {
    let lower = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    if matches!(lower.as_str(), "min" | "max") {
        return arity == 1;
    }
    matches!(
        lower.as_str(),
        "avg"
            | "count"
            | "group_concat"
            | "json_group_array"
            | "json_group_object"
            | "median"
            | "percentile_cont"
            | "string_agg"
            | "sum"
            | "total"
    )
}

fn native_select_projection(
    source: &SelectSource,
    from: Option<&RqlTableRef>,
    projection: &[RqlSelectItem],
    params: &mut ParamLayout,
) -> Result<(Vec<SelectItem>, Vec<String>)> {
    if projection.is_empty() {
        let mut output_columns = Vec::new();
        push_projection_columns(source, &mut output_columns);
        return Ok((Vec::new(), output_columns));
    }
    let mut items = Vec::with_capacity(projection.len());
    let mut output_columns = Vec::new();
    for item in projection {
        match item {
            RqlSelectItem::Wildcard => {
                items.push(SelectItem::Wildcard(WildcardAdditionalOptions::default()));
                push_projection_columns(source, &mut output_columns);
            }
            RqlSelectItem::QualifiedWildcard { table } => {
                if !from.is_some_and(|from| native_select_table_matches(from, table)) {
                    return Err(Error::UnknownTable(table.clone()));
                }
                items.push(SelectItem::QualifiedWildcard(
                    SelectItemQualifiedWildcardKind::ObjectName(sql_name(&RqlName {
                        schema: None,
                        name: table.clone(),
                    })),
                    WildcardAdditionalOptions::default(),
                ));
                push_projection_columns(source, &mut output_columns);
            }
            RqlSelectItem::Expr { expr, alias } => {
                let expr = normalized_expr(expr, params)?;
                match alias {
                    Some(alias) => {
                        output_columns.push(alias.clone());
                        items.push(SelectItem::ExprWithAlias {
                            expr,
                            alias: Ident::new(alias),
                        });
                    }
                    None => {
                        output_columns.push(render_expr_name(&expr));
                        items.push(SelectItem::UnnamedExpr(expr));
                    }
                }
            }
        }
    }
    Ok((items, output_columns))
}

fn native_select_source(schema: &SchemaSnapshot, from: &RqlTableRef) -> Option<SelectSource> {
    if from
        .name
        .schema
        .as_ref()
        .is_some_and(|schema| !schema.eq_ignore_ascii_case("main"))
    {
        return None;
    }
    let schema_id = schema.lookup_namespace("main")?;
    let table = schema.lookup_table(schema_id, &from.name.name)?;
    if let Some(alias) = &from.alias {
        return Some(SelectSource::Tables(vec![BoundTable {
            table,
            alias: Some(Arc::from(alias.as_str())),
            index_hint: None,
        }]));
    }
    Some(SelectSource::Table(table))
}
