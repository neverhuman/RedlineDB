use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use sqlparser::ast::{
    BinaryOperator, Expr, Ident, JoinConstraint, JoinOperator, ObjectName, TableFactor,
    TableWithJoins,
};

use crate::error::{Error, Result};
use crate::statement::{BoundTable, JoinKind, JoinSource, JoinStep, ParamLayout, SelectSource};

use super::bind::{bind_table_name, object_name_part_to_string};

pub(crate) fn bind_select_from(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    from: Vec<TableWithJoins>,
    params: &mut ParamLayout,
) -> Result<(SelectSource, Option<Expr>)> {
    if from.is_empty() {
        return Ok((SelectSource::Empty, None));
    }

    // CTE-aware single-source fast path. When a single FROM entry without
    // joins names an active CTE, route it through `SelectSource::Cte` so
    // the executor reads the pre-materialized rows instead of looking the
    // name up in the catalog.
    if from.len() == 1 && from[0].joins.is_empty()
        && let TableFactor::Table { name, alias, .. } = &from[0].relation
    {
        let alias_arc: Option<Arc<str>> = alias.as_ref().map(|a| Arc::from(a.name.value.as_str()));
        if let Some(source) =
            crate::exec::cte::try_resolve_cte_source(name, alias_arc.as_ref(), params)
        {
            return Ok((source, None));
        }
    }

    // Table-valued function fast path. `pragma_table_info(t)` and friends
    // come through as `TableFactor::Table { args: Some(...), .. }`; if the
    // function name resolves in the TV registry we materialise the rows
    // here and route them through `SelectSource::Cte` so the rest of the
    // pipeline (`SELECT *`, `WHERE`, `ORDER BY`) keeps the column names.
    if from.len() == 1 && from[0].joins.is_empty()
        && let TableFactor::Table { name, alias, args: Some(args), .. } = &from[0].relation
        && let Some(source) = try_table_valued_source(conn, schema, name, alias.as_ref(), args)?
    {
        return Ok((source, None));
    }

    if from.len() == 1 && !from[0].joins.is_empty() {
        if let TableFactor::Table { name, .. } = &from[0].relation
            && is_sqlite_schema_name(name)
        {
            return Err(Error::UnsupportedSql(
                "sqlite_schema cannot participate in joins".to_owned(),
            ));
        }
        let join = bind_select_join_source(schema, from.into_iter().next().expect("one"), params)?;
        return Ok((SelectSource::Joined(join), None));
    }

    let mut tables: Vec<BoundTable> = Vec::new();
    let mut selection = None;
    let mut saw_sqlite_schema = false;

    for table in from {
        match &table.relation {
            TableFactor::Table { name, .. } if is_sqlite_schema_name(name) => {
                if !table.joins.is_empty() {
                    return Err(Error::UnsupportedSql(
                        "sqlite_schema cannot participate in joins".to_owned(),
                    ));
                }
                saw_sqlite_schema = true;
                continue;
            }
            _ => {}
        }
        if table.joins.is_empty() {
            let bound = bind_select_table_factor(schema, table.relation)?;
            tables.push(bound);
            continue;
        }
        let (mut more, join_selection) = bind_select_table_with_joins(schema, table, params)?;
        tables.append(&mut more);
        if let Some(expr) = join_selection {
            selection = Some(match selection {
                Some(prev) => and_expr(prev, expr),
                None => expr,
            });
        }
    }

    let source = if saw_sqlite_schema && tables.is_empty() {
        SelectSource::SqliteSchema
    } else if tables.len() == 1 && tables[0].alias.is_none() && selection.is_none() {
        SelectSource::Table(Arc::clone(&tables[0].table))
    } else {
        SelectSource::Tables(tables)
    };

    Ok((source, selection))
}

pub(crate) fn bind_select_join_source(
    schema: &SchemaSnapshot,
    table: TableWithJoins,
    params: &mut ParamLayout,
) -> Result<JoinSource> {
    let base = bind_select_table_factor(schema, table.relation)?;
    let mut left_tables = vec![base.clone()];
    let mut joins = Vec::new();
    for join in table.joins {
        let right = bind_select_join_relation(schema, join.relation)?;
        let (kind, join_selection) = match join.join_operator {
            JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => (
                JoinKind::Inner,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::CrossJoin(constraint) => match constraint {
                JoinConstraint::None => (JoinKind::Inner, None),
                _ => {
                    return Err(Error::UnsupportedSql(
                        "CROSS JOIN cannot have a constraint".to_owned(),
                    ));
                }
            },
            JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => (
                JoinKind::Left,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::Right(_)
            | JoinOperator::RightOuter(_)
            | JoinOperator::FullOuter(_)
            | JoinOperator::Semi(_)
            | JoinOperator::LeftSemi(_)
            | JoinOperator::RightSemi(_)
            | JoinOperator::Anti(_)
            | JoinOperator::LeftAnti(_)
            | JoinOperator::RightAnti(_)
            | JoinOperator::CrossApply
            | JoinOperator::OuterApply
            | JoinOperator::AsOf { .. }
            | JoinOperator::StraightJoin(_) => {
                return Err(Error::UnsupportedSql(
                    "only INNER, CROSS, and LEFT joins are supported".to_owned(),
                ));
            }
        };
        joins.push(JoinStep {
            right,
            kind,
            selection: join_selection,
        });
        left_tables.push(joins.last().expect("join just pushed").right.clone());
    }

    Ok(JoinSource { base, joins })
}

pub(crate) fn bind_select_table_with_joins(
    schema: &SchemaSnapshot,
    table: TableWithJoins,
    params: &mut ParamLayout,
) -> Result<(Vec<BoundTable>, Option<Expr>)> {
    let join_source = bind_select_join_source(schema, table, params)?;
    if join_source
        .joins
        .iter()
        .any(|join| matches!(join.kind, JoinKind::Left))
    {
        return Err(Error::UnsupportedSql(
            "LEFT joins require a single-table FROM source".to_owned(),
        ));
    }

    let mut tables = vec![join_source.base];
    let mut selection = None;
    for join in join_source.joins {
        if let Some(expr) = join.selection {
            selection = Some(match selection {
                Some(prev) => and_expr(prev, expr),
                None => expr,
            });
        }
        tables.push(join.right);
    }
    Ok((tables, selection))
}

pub(crate) fn bind_select_table_factor(
    schema: &SchemaSnapshot,
    relation: TableFactor,
) -> Result<BoundTable> {
    match relation {
        TableFactor::Table {
            name, alias, args, ..
        } => {
            if args.is_some() {
                return Err(Error::UnsupportedSql(
                    "table-valued functions are not supported".to_owned(),
                ));
            }
            let alias_arc: Option<Arc<str>> =
                alias.as_ref().map(|a| Arc::from(a.name.value.as_str()));
            // CTE-name resolution: if the name matches an active CTE
            // in scope, return a synthetic BoundTable whose TableDef is
            // backed by pre-materialized rows.
            if let Some(bound) =
                crate::exec::cte::try_resolve_cte_bound_table(&name, alias_arc.as_ref())
            {
                return Ok(bound);
            }
            Ok(BoundTable {
                table: bind_table_name(schema, &name)?,
                alias: alias.map(|alias| Arc::from(alias.name.value)),
            })
        }
        _ => Err(Error::UnsupportedSql(
            "only direct table scans are supported".to_owned(),
        )),
    }
}

pub(crate) fn bind_select_join_relation(
    schema: &SchemaSnapshot,
    relation: TableFactor,
) -> Result<BoundTable> {
    bind_select_table_factor(schema, relation)
}

pub(crate) fn bind_join_constraint(
    left: &[BoundTable],
    right: &BoundTable,
    constraint: JoinConstraint,
    params: &mut ParamLayout,
) -> Result<Option<Expr>> {
    match constraint {
        JoinConstraint::None => Ok(None),
        JoinConstraint::On(expr) => Ok(Some(crate::parser::select::normalize_expr(expr, params)?)),
        JoinConstraint::Using(columns) => {
            let right_name = match right.alias.as_ref().map(|alias| alias.to_string()) {
                Some(n) => n,
                None => right.table.name.to_string(),
            };
            let left_name = match left.last().map(|table| {
                match table.alias.as_ref().map(|alias| alias.to_string()) {
                    Some(n) => n,
                    None => table.table.name.to_string(),
                }
            }) {
                Some(n) => n,
                None => {
                    return Err(Error::UnsupportedSql(
                        "USING requires a left table".to_owned(),
                    ));
                }
            };
            let mut expr = None;
            for column in columns {
                let column_part = match column.0.last() {
                    Some(p) => p,
                    None => {
                        return Err(Error::UnsupportedSql("empty USING column".to_owned()));
                    }
                };
                let column_name = object_name_part_to_string(column_part)?;
                let left_col = Expr::CompoundIdentifier(vec![
                    Ident::new(left_name.clone()),
                    Ident::new(column_name.clone()),
                ]);
                let right_col = Expr::CompoundIdentifier(vec![
                    Ident::new(right_name.clone()),
                    Ident::new(column_name),
                ]);
                let eq = Expr::BinaryOp {
                    left: Box::new(left_col),
                    op: BinaryOperator::Eq,
                    right: Box::new(right_col),
                };
                expr = Some(match expr {
                    Some(prev) => and_expr(prev, eq),
                    None => eq,
                });
            }
            Ok(expr)
        }
        JoinConstraint::Natural => Err(Error::UnsupportedSql(
            "NATURAL joins are not supported".to_owned(),
        )),
    }
}

pub(crate) fn and_expr(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: BinaryOperator::And,
        right: Box::new(right),
    }
}

pub(crate) fn is_sqlite_schema_name(name: &ObjectName) -> bool {
    match name.0.as_slice() {
        [part] => object_name_part_to_string(part)
            .map(|s| {
                s.eq_ignore_ascii_case("sqlite_schema") || s.eq_ignore_ascii_case("sqlite_master")
            })
            .unwrap_or(false),
        [schema, table] => {
            let schema = object_name_part_to_string(schema).ok();
            let table = object_name_part_to_string(table).ok();
            matches!(
                (schema.as_deref(), table.as_deref()),
                (Some("main"), Some("sqlite_schema")) | (Some("main"), Some("sqlite_master"))
            )
        }
        _ => false,
    }
}

/// If `name(args)` resolves to a registered table-valued function, evaluate
/// it now and produce a `SelectSource::Cte` carrying the result. Returns
/// `Ok(None)` when the name isn't a TVF — that lets the regular FROM
/// resolver continue with whatever the table reference actually is.
fn try_table_valued_source(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    name: &ObjectName,
    alias: Option<&sqlparser::ast::TableAlias>,
    args: &sqlparser::ast::TableFunctionArgs,
) -> Result<Option<SelectSource>> {
    let func_name = match name.0.as_slice() {
        [part] => object_name_part_to_string(part)?,
        _ => return Ok(None),
    };
    let Some(func) = crate::exec::table_valued::lookup(&func_name) else {
        return Ok(None);
    };
    let lowered = crate::exec::table_valued::lower_args(args)?;
    let result = func.eval(conn, schema, &lowered)?;
    let alias_arc: Option<Arc<str>> = alias.map(|a| Arc::from(a.name.value.as_str()));
    Ok(Some(SelectSource::Cte {
        name: Arc::from(func.name()),
        alias: alias_arc,
        columns: Arc::from(result.columns),
        rows: Arc::from(result.rows),
    }))
}
