use super::*;

pub(crate) fn bind_insert(
    conn: &Connection,
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
    if let sqlparser::ast::TableObject::TableName(ref name) = insert.table
        && crate::exec::view::name_is_view(&schema, name)
    {
        let view_name = name.clone();
        return bind_insert_view(conn, schema_epoch, sql, insert, &view_name);
    }
    let table = bind_table_object(&schema, &insert.table)?;
    let mut params = ParamLayout::default();
    let conflict = bind_insert_conflict(&table, insert.or, insert.on, &mut params)?;
    let columns = if insert.columns.is_empty() {
        // Phase-11 SQL-D A6: implicit INSERT (no column list) only
        // binds to non-generated columns; user-provided VALUES must
        // never feed a generated column.
        (0..table.columns.len())
            .filter(|idx| {
                table
                    .columns
                    .get(*idx)
                    .map(|c| c.generated.is_none())
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>()
    } else {
        let ordinals: Vec<usize> = insert
            .columns
            .into_iter()
            .map(|column| resolve_column_ordinal_in_table(&table, &column.value))
            .collect::<Result<Vec<_>>>()?;
        // Phase-11 SQL-D A6: SQLite rejects explicit INSERT into a
        // generated column with "cannot INSERT into generated column X".
        for ord in &ordinals {
            if let Some(col) = table.columns.get(*ord)
                && col.generated.is_some()
            {
                return Err(Error::UnsupportedSql(format!(
                    "cannot INSERT into generated column \"{}\"",
                    col.name
                )));
            }
        }
        ordinals
    };

    let mut rows = Vec::new();
    let mut source_select = None;
    let mut default_values = false;
    if let Some(source) = insert.source {
        let source = *source;
        match *source.body {
            SetExpr::Values(values) => {
                for row in values.rows {
                    let mut exprs = Vec::with_capacity(row.len());
                    for expr in row {
                        exprs.push(normalize_dml_value(expr, &mut params)?);
                    }
                    rows.push(exprs);
                }
            }
            body => {
                let template = bind_query_with_params(
                    conn,
                    Arc::clone(&schema),
                    schema_epoch,
                    sql,
                    Query {
                        body: Box::new(body),
                        order_by: source.order_by,
                        limit_clause: source.limit_clause,
                        with: source.with,
                        fetch: source.fetch,
                        locks: source.locks,
                        for_clause: source.for_clause,
                        settings: source.settings,
                        format_clause: source.format_clause,
                        pipe_operators: source.pipe_operators,
                    },
                    &mut params,
                )?;
                let PreparedKind::Select(plan) = template.kind else {
                    return Err(Error::UnsupportedSql(
                        "INSERT SELECT source must bind as SELECT".to_owned(),
                    ));
                };
                source_select = Some(Box::new(plan));
            }
        }
    } else {
        default_values = true;
    }
    let returning = match insert.returning {
        Some(items) => Some(normalize_select_projection(items, &mut params)?),
        None => None,
    };
    let output_columns = match returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
    {
        Some(cols) => cols,
        None => Arc::from([]),
    };

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

fn bind_insert_view(
    conn: &Connection,
    schema_epoch: SchemaEpoch,
    sql: &str,
    insert: sqlparser::ast::Insert,
    name: &sqlparser::ast::ObjectName,
) -> Result<PreparedTemplate> {
    let columns = crate::exec::view::view_column_names(conn, name)?;
    let mut params = ParamLayout::default();
    let mut rows = Vec::new();
    let Some(source) = insert.source else {
        return Err(Error::UnsupportedSql(
            "INSERT INTO view requires VALUES".to_owned(),
        ));
    };
    match *source.body {
        SetExpr::Values(values) => {
            for row in values.rows {
                let mut exprs = Vec::with_capacity(row.len());
                for expr in row {
                    exprs.push(normalize_dml_value(expr, &mut params)?);
                }
                rows.push(exprs);
            }
        }
        _ => {
            return Err(Error::UnsupportedSql(
                "INSERT INTO view supports VALUES only".to_owned(),
            ));
        }
    }
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::InsertView(crate::statement::InsertViewPlan {
            view_name: Arc::from(name.to_string()),
            columns: Arc::from(columns),
            rows,
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
    // Phase 5 WS-A2f rolled back at parser layer: SQLite's autoconf
    // amalgamation parser rejects `UPDATE ... LIMIT n` regardless of the
    // `-DSQLITE_ENABLE_UPDATE_DELETE_LIMIT` compile flag, so accepting it
    // in RedlineDB makes the official parity gate diverge from reference.
    // Users who want this can do `UPDATE t SET ... WHERE rowid IN
    // (SELECT rowid FROM t ORDER BY x LIMIT n)` instead.
    if update.limit.is_some() {
        return Err(Error::UnsupportedSql(
            "UPDATE LIMIT is not supported".to_owned(),
        ));
    }

    if let sqlparser::ast::TableFactor::Table { ref name, .. } = update.table.relation
        && crate::exec::view::name_is_view(&schema, name)
    {
        return Err(crate::exec::view::cannot_modify_view_error(
            &name.to_string(),
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
        // Phase-11 SQL-D A6: SQLite rejects assigning a value directly
        // to a generated column with "cannot UPDATE generated column X".
        if let Some(col) = table.columns.get(ordinal)
            && col.generated.is_some()
        {
            return Err(Error::UnsupportedSql(format!(
                "cannot UPDATE generated column \"{}\"",
                col.name
            )));
        }
        assignments.push((ordinal, normalize_dml_value(assignment.value, &mut params)?));
    }
    let selection = match update.selection {
        Some(expr) => Some(normalize_expr(expr, &mut params)?),
        None => None,
    };
    let returning = match update.returning {
        Some(items) => Some(normalize_select_projection(items, &mut params)?),
        None => None,
    };
    // Phase 5 WS-A2f: sqlparser 0.61's SQLite dialect parses `UPDATE ...
    // LIMIT n` but does NOT accept `UPDATE ... ORDER BY ...`, so the
    // `order_by` field on `UpdatePlan` always starts empty here. A
    // pre-parse rewrite in `parser.rs` is needed to teach UPDATE ORDER BY.
    let limit = match update.limit {
        Some(expr) => Some(normalize_expr(expr, &mut params)?),
        None => None,
    };
    let output_columns = match returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
    {
        Some(cols) => cols,
        None => Arc::from([]),
    };
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
            order_by: Vec::new(),
            limit,
            offset: None,
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
    // Phase 5 WS-A2f rolled back at parser layer: SQLite's autoconf
    // amalgamation parser rejects `DELETE ... ORDER BY ... LIMIT n`
    // regardless of the `-DSQLITE_ENABLE_UPDATE_DELETE_LIMIT` compile
    // flag, so accepting it makes the parity gate diverge. Users can do
    // `DELETE FROM t WHERE rowid IN (SELECT rowid FROM t ORDER BY x
    // LIMIT n)` for equivalent semantics.
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
    if let sqlparser::ast::TableFactor::Table { ref name, .. } = from[0].relation
        && crate::exec::view::name_is_view(&schema, name)
    {
        return Err(crate::exec::view::cannot_modify_view_error(
            &name.to_string(),
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
    // Phase 5 WS-A2f: normalize ORDER BY exprs / LIMIT bind values so
    // `execute_delete` can evaluate them per row. sqlparser 0.61 does
    // accept `DELETE FROM t [WHERE ...] [ORDER BY ...] [LIMIT n]` in
    // SQLite dialect; OFFSET on DELETE is not parsed and stays `None`.
    let order_by = delete
        .order_by
        .into_iter()
        .map(|expr| {
            let options = expr.options;
            let with_fill = expr.with_fill;
            let expr = normalize_expr(expr.expr, &mut params)?;
            Ok(sqlparser::ast::OrderByExpr {
                expr,
                options,
                with_fill,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let limit = match delete.limit {
        Some(expr) => Some(normalize_expr(expr, &mut params)?),
        None => None,
    };
    let output_columns = match returning
        .as_ref()
        .map(|items| returning_output_columns(&table, items))
    {
        Some(cols) => cols,
        None => Arc::from([]),
    };
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
            order_by,
            limit,
            offset: None,
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
        Some(ConflictTarget::Columns(columns)) => {
            let ordinals: Vec<usize> = columns
                .into_iter()
                .map(|column| resolve_column_ordinal_in_table(table, &column.value))
                .collect::<Result<Vec<_>>>()?;
            // SQLite parity: the conflict-target column set must match
            // some UNIQUE / PRIMARY KEY constraint (including rowid-alias
            // INTEGER PRIMARY KEY). Reject early with SQLite's wording.
            if !upsert_target_columns_have_unique(table, &ordinals) {
                return Err(Error::Bind(
                    "ON CONFLICT clause does not match any PRIMARY KEY or UNIQUE constraint"
                        .to_owned(),
                ));
            }
            Some(UpsertTarget::Columns(ordinals))
        }
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
                        // Phase-11 SQL-D A6: ON CONFLICT DO UPDATE is
                        // still an UPDATE — assigning a generated column
                        // is forbidden in SQLite.
                        if let Some(col) = table.columns.get(ordinal)
                            && col.generated.is_some()
                        {
                            return Err(Error::UnsupportedSql(format!(
                                "cannot UPDATE generated column \"{}\"",
                                col.name
                            )));
                        }
                        Ok((ordinal, normalize_dml_value(assignment.value, params)?))
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

fn normalize_dml_value(expr: Expr, params: &mut ParamLayout) -> Result<DmlValue> {
    if is_default_dml_expr(&expr) {
        return Ok(DmlValue::Default);
    }
    Ok(DmlValue::Expr(normalize_expr(expr, params)?))
}

fn is_default_dml_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Identifier(ident) if ident.value.eq_ignore_ascii_case("default")
    ) || matches!(
        expr,
        Expr::Value(v) if crate::parser::bind::as_bind_name(&v.value)
            .is_some_and(|name| name.eq_ignore_ascii_case("default"))
    )
}

/// Track K — Bind a SQL:2003 `MERGE INTO target USING source ON ... WHEN ...`
/// statement into a `MergePlan`. The target and source must be concrete
/// tables (subqueries on the source side are not supported in this lane —
/// PG accepts them but our test surface only exercises bare tables).
pub(crate) fn bind_merge(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    merge: sqlparser::ast::Merge,
) -> Result<PreparedTemplate> {
    let sqlparser::ast::Merge {
        table,
        source,
        on,
        clauses,
        output,
        into: _,
        merge_token: _,
        optimizer_hint: _,
    } = merge;
    if output.is_some() {
        return Err(Error::UnsupportedSql(
            "MERGE ... OUTPUT (MSSQL) is not supported".to_owned(),
        ));
    }
    let (target_def, target_alias) = bind_merge_table_factor(&schema, &table, "MERGE target")?;
    let (source_def, source_alias) = bind_merge_table_factor(&schema, &source, "MERGE source")?;

    let mut params = ParamLayout::default();
    let on_expr = normalize_expr(*on, &mut params)?;

    let mut bound_clauses: Vec<crate::statement::MergeClausePlan> =
        Vec::with_capacity(clauses.len());
    for clause in clauses {
        let sqlparser::ast::MergeClause {
            clause_kind,
            predicate,
            action,
            when_token: _,
        } = clause;
        let predicate = match predicate {
            Some(expr) => Some(normalize_expr(expr, &mut params)?),
            None => None,
        };
        match clause_kind {
            sqlparser::ast::MergeClauseKind::Matched => match action {
                sqlparser::ast::MergeAction::Update(update) => {
                    let assignments =
                        bind_merge_assignments(&target_def, update.assignments, &mut params)?;
                    if update.update_predicate.is_some() || update.delete_predicate.is_some() {
                        return Err(Error::UnsupportedSql(
                            "MERGE WHEN MATCHED THEN UPDATE WHERE/DELETE WHERE (Oracle) is not supported".to_owned(),
                        ));
                    }
                    bound_clauses.push(crate::statement::MergeClausePlan::MatchedUpdate {
                        predicate,
                        assignments,
                    });
                }
                sqlparser::ast::MergeAction::Delete { .. } => {
                    bound_clauses
                        .push(crate::statement::MergeClausePlan::MatchedDelete { predicate });
                }
                sqlparser::ast::MergeAction::Insert(_) => {
                    return Err(Error::UnsupportedSql(
                        "MERGE WHEN MATCHED THEN INSERT is not allowed".to_owned(),
                    ));
                }
            },
            sqlparser::ast::MergeClauseKind::NotMatched
            | sqlparser::ast::MergeClauseKind::NotMatchedByTarget => match action {
                sqlparser::ast::MergeAction::Insert(insert) => {
                    let columns = if insert.columns.is_empty() {
                        (0..target_def.columns.len())
                            .filter(|idx| {
                                target_def
                                    .columns
                                    .get(*idx)
                                    .map(|c| c.generated.is_none())
                                    .unwrap_or(true)
                            })
                            .collect::<Vec<_>>()
                    } else {
                        let mut ordinals = Vec::with_capacity(insert.columns.len());
                        for col in insert.columns {
                            let name = match col.0.last() {
                                Some(p) => super::helpers::object_name_part_to_string(p)?,
                                None => {
                                    return Err(Error::UnsupportedSql(
                                        "MERGE INSERT column name is empty".to_owned(),
                                    ));
                                }
                            };
                            ordinals.push(resolve_column_ordinal_in_table(&target_def, &name)?);
                        }
                        ordinals
                    };
                    if insert.insert_predicate.is_some() {
                        return Err(Error::UnsupportedSql(
                            "MERGE WHEN NOT MATCHED THEN INSERT WHERE (Oracle) is not supported"
                                .to_owned(),
                        ));
                    }
                    let values = match insert.kind {
                        sqlparser::ast::MergeInsertKind::Values(vals) => {
                            if vals.rows.len() != 1 {
                                return Err(Error::UnsupportedSql(
                                    "MERGE INSERT VALUES expects a single row".to_owned(),
                                ));
                            }
                            let row = vals.rows.into_iter().next().expect("checked len");
                            if row.len() != columns.len() {
                                return Err(Error::Bind(format!(
                                    "MERGE INSERT column count ({}) does not match value count ({})",
                                    columns.len(),
                                    row.len(),
                                )));
                            }
                            row.into_iter()
                                .map(|expr| normalize_dml_value(expr, &mut params))
                                .collect::<Result<Vec<_>>>()?
                        }
                        sqlparser::ast::MergeInsertKind::Row => {
                            return Err(Error::UnsupportedSql(
                                "MERGE INSERT ROW is not supported".to_owned(),
                            ));
                        }
                    };
                    bound_clauses.push(crate::statement::MergeClausePlan::NotMatchedInsert {
                        predicate,
                        columns,
                        values,
                    });
                }
                _ => {
                    return Err(Error::UnsupportedSql(
                        "MERGE WHEN NOT MATCHED action must be INSERT".to_owned(),
                    ));
                }
            },
            sqlparser::ast::MergeClauseKind::NotMatchedBySource => {
                return Err(Error::UnsupportedSql(
                    "MERGE WHEN NOT MATCHED BY SOURCE (PG17+) is not supported".to_owned(),
                ));
            }
        }
    }

    if params.count() == 0 {
        scan_sql_parameters(sql, &mut params);
    }
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::Merge(crate::statement::MergePlan {
            target: target_def,
            target_alias,
            source: source_def,
            source_alias,
            on: on_expr,
            clauses: bound_clauses,
        }),
    })
}

fn bind_merge_table_factor(
    schema: &SchemaSnapshot,
    factor: &sqlparser::ast::TableFactor,
    label: &str,
) -> Result<(Arc<redlinedb_kernel::catalog::TableDef>, Option<Arc<str>>)> {
    match factor {
        sqlparser::ast::TableFactor::Table {
            name, alias, args, ..
        } => {
            if args.is_some() {
                return Err(Error::UnsupportedSql(format!(
                    "{label} cannot be a table-valued function"
                )));
            }
            let def = super::helpers::bind_table_name(schema, name)?;
            let alias_arc: Option<Arc<str>> =
                alias.as_ref().map(|a| Arc::from(a.name.value.as_str()));
            Ok((def, alias_arc))
        }
        _ => Err(Error::UnsupportedSql(format!(
            "{label} must be a direct table reference"
        ))),
    }
}

fn bind_merge_assignments(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    assignments: Vec<sqlparser::ast::Assignment>,
    params: &mut ParamLayout,
) -> Result<Vec<(usize, DmlValue)>> {
    let mut out = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        let ordinal = match assignment.target {
            sqlparser::ast::AssignmentTarget::ColumnName(name) => {
                resolve_column_ordinal_in_object_name(table, &name)?
            }
            sqlparser::ast::AssignmentTarget::Tuple(_) => {
                return Err(Error::UnsupportedSql(
                    "MERGE tuple assignment is not supported".to_owned(),
                ));
            }
        };
        if let Some(col) = table.columns.get(ordinal)
            && col.generated.is_some()
        {
            return Err(Error::UnsupportedSql(format!(
                "cannot UPDATE generated column \"{}\"",
                col.name
            )));
        }
        out.push((ordinal, normalize_dml_value(assignment.value, params)?));
    }
    Ok(out)
}

/// True if `target_ordinals` exactly matches the leading column set
/// of some UNIQUE / PRIMARY KEY constraint on `table` (rowid-alias
/// INTEGER PRIMARY KEY also counts when the single target column is
/// the alias). Order of `target_ordinals` is significant — SQLite
/// requires exact match.
fn upsert_target_columns_have_unique(
    table: &redlinedb_kernel::catalog::TableDef,
    target_ordinals: &[usize],
) -> bool {
    if target_ordinals.is_empty() {
        return false;
    }
    // Rowid-alias INTEGER PRIMARY KEY: a single-column target whose
    // ordinal equals the alias column is a valid PK target.
    if target_ordinals.len() == 1 && table.rowid_alias_column == Some(target_ordinals[0] as u16) {
        return true;
    }
    for index in &table.indexes {
        if !index.unique {
            continue;
        }
        if index.keys.len() != target_ordinals.len() {
            continue;
        }
        let mut matches = true;
        for (ord_idx, key) in index.keys.iter().enumerate() {
            let column_ord = match &key.source {
                redlinedb_kernel::catalog::IndexKeySource::Column { attnum } => *attnum as usize,
                _ => {
                    matches = false;
                    break;
                }
            };
            if column_ord != target_ordinals[ord_idx] {
                matches = false;
                break;
            }
        }
        if matches {
            return true;
        }
    }
    false
}
