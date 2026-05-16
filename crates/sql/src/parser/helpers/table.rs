use std::sync::Arc;

use redlinedb_kernel::catalog::{SchemaSnapshot, lookup_table};
use sqlparser::ast::{
    BinaryOperator, Expr, Ident, JoinConstraint, JoinOperator, ObjectName, ObjectNamePart,
    TableFactor, TableObject, TableWithJoins,
};

use crate::error::{Error, Result};
use crate::statement::{BoundTable, JoinKind, JoinSource, JoinStep, ParamLayout, SelectSource};

use super::expr::parse_qualified_name;

pub(crate) fn object_name_part_to_string(part: &ObjectNamePart) -> Result<String> {
    match part {
        ObjectNamePart::Identifier(ident) => Ok(ident.value.clone()),
        ObjectNamePart::Function(_) => Err(Error::UnsupportedSql(
            "function-style object names are not supported".to_owned(),
        )),
    }
}

pub(crate) fn bind_table_name(
    schema: &SchemaSnapshot,
    name: &ObjectName,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    let qualified = parse_qualified_name(name.clone())?;
    Ok(lookup_table(schema, &qualified)?)
}

pub(crate) fn bind_table_object(
    schema: &SchemaSnapshot,
    table: &TableObject,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    match table {
        TableObject::TableName(name) => bind_table_name(schema, name),
        TableObject::TableFunction(_) => Err(Error::UnsupportedSql(
            "table functions are not supported".to_owned(),
        )),
    }
}

pub(crate) fn bind_select_from(
    schema: &SchemaSnapshot,
    from: Vec<TableWithJoins>,
    params: &mut ParamLayout,
) -> Result<(SelectSource, Option<Expr>)> {
    if from.is_empty() {
        return Ok((SelectSource::Empty, None));
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

pub(crate) fn bind_table_with_joins(
    schema: &SchemaSnapshot,
    table: &TableWithJoins,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    if !table.joins.is_empty() {
        return Err(Error::UnsupportedSql(
            "joins are not supported in UPDATE/DELETE targets yet".to_owned(),
        ));
    }
    match &table.relation {
        TableFactor::Table { name, args, .. } => {
            if args.is_some() {
                return Err(Error::UnsupportedSql(
                    "table-valued functions are not supported".to_owned(),
                ));
            }
            bind_table_name(schema, name)
        }
        _ => Err(Error::UnsupportedSql(
            "only direct table scans are supported".to_owned(),
        )),
    }
}

pub(crate) fn push_projection_columns(source: &SelectSource, out: &mut Vec<String>) {
    match source {
        SelectSource::Table(table) => {
            out.extend(table.columns.iter().map(|column| column.name.to_string()));
        }
        SelectSource::Tables(tables) => {
            for table in tables {
                out.extend(table.table.columns.iter().map(|column| {
                    if let Some(alias) = &table.alias {
                        format!("{}.{}", alias, column.name)
                    } else {
                        format!("{}.{}", table.table.name, column.name)
                    }
                }));
            }
        }
        SelectSource::Joined(join) => {
            out.extend(join.base.table.columns.iter().map(|column| {
                if let Some(alias) = &join.base.alias {
                    format!("{}.{}", alias, column.name)
                } else {
                    format!("{}.{}", join.base.table.name, column.name)
                }
            }));
            for step in &join.joins {
                out.extend(step.right.table.columns.iter().map(|column| {
                    if let Some(alias) = &step.right.alias {
                        format!("{}.{}", alias, column.name)
                    } else {
                        format!("{}.{}", step.right.table.name, column.name)
                    }
                }));
            }
        }
        SelectSource::SqliteSchema => {
            out.extend(
                ["type", "name", "tbl_name", "rootpage", "sql"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        SelectSource::StaticRows { .. } => {}
        SelectSource::CompoundAll(_) => {}
        SelectSource::Empty => {}
    }
}

pub(crate) fn render_expr_name(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(ident) => ident.value.clone(),
        Expr::CompoundIdentifier(parts) => match parts.last().map(|ident| ident.value.clone()) {
            Some(name) => name,
            None => expr.to_string(),
        },
        _ => expr.to_string(),
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
