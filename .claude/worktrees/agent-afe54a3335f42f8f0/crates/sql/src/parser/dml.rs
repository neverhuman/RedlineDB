use super::*;

pub(crate) fn bind_insert(
    _conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    insert: sqlparser::ast::Insert,
) -> Result<PreparedTemplate> {
    if !insert.assignments.is_empty() {
        return Err(Error::UnsupportedSql(
            "INSERT ... SET is not supported".to_owned(),
        ));
    }
    let table = bind_table_object(&schema, &insert.table)?;
    let mut params = ParamLayout::default();
    let conflict = bind_insert_conflict(&table, insert.or, insert.on, &mut params)?;
    let columns = if insert.columns.is_empty() {
        (0..table.columns.len()).collect::<Vec<_>>()
    } else {
        insert
            .columns
            .into_iter()
            .map(|column| resolve_column_ordinal_in_table(&table, &column.value))
            .collect::<Result<Vec<_>>>()?
    };

    let mut rows = Vec::new();
    let mut source_select = None;
    let mut default_values = false;
    if let Some(source) = insert.source {
        match *source.body {
            SetExpr::Values(values) => {
                for row in values.rows {
                    let mut exprs = Vec::with_capacity(row.len());
                    for expr in row {
                        exprs.push(normalize_expr(expr, &mut params)?);
                    }
                    rows.push(exprs);
                }
            }
            SetExpr::Select(select) => {
                let template = bind_simple_select_query(
                    Arc::clone(&schema),
                    schema_epoch,
                    sql,
                    select,
                    source.order_by,
                    source.limit_clause,
                    &mut params,
                )?;
                let PreparedKind::Select(plan) = template.kind else {
                    return Err(Error::UnsupportedSql(
                        "INSERT SELECT source must bind as SELECT".to_owned(),
                    ));
                };
                source_select = Some(Box::new(plan));
            }
            _ => {
                return Err(Error::UnsupportedSql(
                    "INSERT source must be VALUES or SELECT".to_owned(),
                ));
            }
        }
    } else {
        default_values = true;
    }
    let returning = match insert.returning {
        Some(items) => Some(normalize_select_projection(items, &mut params)?),
        None => None,
    };
    let output_columns = returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
        .unwrap_or_else(|| Arc::from([]));

    if params.count() == 0 {
        scan_sql_parameters(sql, &mut params);
    }
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns,
        readonly: false,
        kind: PreparedKind::Insert(InsertPlan {
            table,
            columns,
            rows,
            source_select,
            default_values,
            returning,
            conflict,
        }),
    })
}

pub(crate) fn bind_update(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    update: sqlparser::ast::Update,
) -> Result<PreparedTemplate> {
    if update.or.is_some() {
        return Err(Error::UnsupportedSql(
            "UPDATE OR ... is not supported yet".to_owned(),
        ));
    }
    if update.from.is_some() {
        return Err(Error::UnsupportedSql(
            "UPDATE ... FROM is not supported".to_owned(),
        ));
    }
    if update.limit.is_some() {
        return Err(Error::UnsupportedSql(
            "UPDATE LIMIT is not supported".to_owned(),
        ));
    }

    let table = bind_table_with_joins(&schema, &update.table)?;
    let mut params = ParamLayout::default();
    let mut assignments = Vec::new();
    for assignment in update.assignments {
        let ordinal = match assignment.target {
            sqlparser::ast::AssignmentTarget::ColumnName(name) => {
                resolve_column_ordinal_in_object_name(&table, &name)?
            }
            sqlparser::ast::AssignmentTarget::Tuple(_) => {
                return Err(Error::UnsupportedSql(
                    "tuple assignment is not supported".to_owned(),
                ));
            }
        };
        assignments.push((ordinal, normalize_expr(assignment.value, &mut params)?));
    }
    let selection = match update.selection {
        Some(expr) => Some(normalize_expr(expr, &mut params)?),
        None => None,
    };
    let returning = match update.returning {
        Some(items) => Some(normalize_select_projection(items, &mut params)?),
        None => None,
    };
    let output_columns = returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
        .unwrap_or_else(|| Arc::from([]));
    if params.count() == 0 {
        scan_sql_parameters(sql, &mut params);
    }
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns,
        readonly: false,
        kind: PreparedKind::Update(UpdatePlan {
            table,
            assignments,
            selection,
            returning,
        }),
    })
}

pub(crate) fn bind_delete(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    delete: sqlparser::ast::Delete,
) -> Result<PreparedTemplate> {
    if delete.using.is_some() {
        return Err(Error::UnsupportedSql(
            "DELETE ... USING is not supported".to_owned(),
        ));
    }
    if !delete.order_by.is_empty() {
        return Err(Error::UnsupportedSql(
            "DELETE ORDER BY is not supported".to_owned(),
        ));
    }
    if delete.limit.is_some() {
        return Err(Error::UnsupportedSql(
            "DELETE LIMIT is not supported".to_owned(),
        ));
    }

    let from = match delete.from {
        sqlparser::ast::FromTable::WithFromKeyword(from)
        | sqlparser::ast::FromTable::WithoutKeyword(from) => from,
    };
    if from.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-table DELETE is supported".to_owned(),
        ));
    }
    let table = bind_table_with_joins(&schema, &from[0])?;
    let mut params = ParamLayout::default();
    let selection = match delete.selection {
        Some(expr) => Some(normalize_expr(expr, &mut params)?),
        None => None,
    };
    let returning = match delete.returning {
        Some(items) => Some(normalize_select_projection(items, &mut params)?),
        None => None,
    };
    let output_columns = returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
        .unwrap_or_else(|| Arc::from([]));
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns,
        readonly: false,
        kind: PreparedKind::Delete(DeletePlan {
            table,
            selection,
            returning,
        }),
    })
}

pub(crate) fn bind_insert_conflict(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    or: Option<SqliteOnConflict>,
    on: Option<OnInsert>,
    params: &mut ParamLayout,
) -> Result<Option<InsertConflict>> {
    if or.is_some() && on.is_some() {
        return Err(Error::UnsupportedSql(
            "INSERT cannot use both OR and ON CONFLICT".to_owned(),
        ));
    }
    if let Some(or) = or {
        let algorithm = match or {
            SqliteOnConflict::Rollback => ConflictAlgorithm::Rollback,
            SqliteOnConflict::Abort => ConflictAlgorithm::Abort,
            SqliteOnConflict::Fail => ConflictAlgorithm::Fail,
            SqliteOnConflict::Ignore => ConflictAlgorithm::Ignore,
            SqliteOnConflict::Replace => ConflictAlgorithm::Replace,
        };
        return Ok(Some(InsertConflict::Sqlite(algorithm)));
    }

    let Some(on) = on else {
        return Ok(None);
    };

    let OnInsert::OnConflict(on_conflict) = on else {
        return Err(Error::UnsupportedSql(
            "INSERT ON DUPLICATE KEY UPDATE is not supported".to_owned(),
        ));
    };

    let target = match on_conflict.conflict_target {
        Some(ConflictTarget::Columns(columns)) => Some(UpsertTarget::Columns(
            columns
                .into_iter()
                .map(|column| resolve_column_ordinal_in_table(table, &column.value))
                .collect::<Result<Vec<_>>>()?,
        )),
        Some(ConflictTarget::OnConstraint(name)) => {
            let (schema, constraint) = split_name(name)?;
            if schema.is_some() {
                return Err(Error::UnsupportedSql(
                    "ON CONFLICT ON CONSTRAINT does not accept a schema".to_owned(),
                ));
            }
            Some(UpsertTarget::Constraint(constraint.folded().into()))
        }
        None => None,
    };

    let action = match on_conflict.action {
        OnConflictAction::DoNothing => UpsertAction::DoNothing,
        OnConflictAction::DoUpdate(do_update) => {
            UpsertAction::DoUpdate(Box::new(UpsertUpdatePlan {
                assignments: do_update
                    .assignments
                    .into_iter()
                    .map(|assignment| {
                        let ordinal = match assignment.target {
                            sqlparser::ast::AssignmentTarget::ColumnName(name) => {
                                resolve_column_ordinal_in_object_name(table, &name)?
                            }
                            sqlparser::ast::AssignmentTarget::Tuple(_) => {
                                return Err(Error::UnsupportedSql(
                                    "tuple assignment is not supported".to_owned(),
                                ));
                            }
                        };
                        Ok((ordinal, normalize_expr(assignment.value, params)?))
                    })
                    .collect::<Result<Vec<_>>>()?,
                selection: match do_update.selection {
                    Some(expr) => Some(normalize_expr(expr, params)?),
                    None => None,
                },
            }))
        }
    };

    Ok(Some(InsertConflict::Upsert(Box::new(UpsertPlan {
        target,
        action,
    }))))
}
