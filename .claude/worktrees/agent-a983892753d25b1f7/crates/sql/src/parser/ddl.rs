use super::*;

pub(crate) fn bind_create_table(
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_table: sqlparser::ast::CreateTable,
) -> Result<PreparedTemplate> {
    if create_table.query.is_some() {
        return Err(Error::UnsupportedSql(
            "CREATE TABLE AS SELECT is not supported".to_owned(),
        ));
    }
    if create_table.or_replace
        || create_table.temporary
        || create_table.external
        || create_table.dynamic
        || create_table.global.is_some()
        || create_table.transient
        || create_table.volatile
        || create_table.iceberg
        || create_table.query.is_some()
        || create_table.like.is_some()
        || create_table.clone.is_some()
        || create_table.version.is_some()
        || create_table.comment.is_some()
        || create_table.on_commit.is_some()
        || create_table.on_cluster.is_some()
        || create_table.primary_key.is_some()
        || create_table.order_by.is_some()
        || create_table.partition_by.is_some()
        || create_table.cluster_by.is_some()
        || create_table.clustered_by.is_some()
        || create_table.inherits.is_some()
        || create_table.partition_of.is_some()
        || create_table.for_values.is_some()
        || create_table.copy_grants
        || create_table.enable_schema_evolution.is_some()
        || create_table.change_tracking.is_some()
    {
        return Err(Error::UnsupportedSql(
            "CREATE TABLE modifiers are not supported".to_owned(),
        ));
    }

    let (schema, name) = split_name(create_table.name)?;
    let mut columns = Vec::with_capacity(create_table.columns.len());
    let mut column_lookup = std::collections::HashMap::new();
    for (ordinal, column) in create_table.columns.iter().enumerate() {
        column_lookup.insert(column.name.value.to_ascii_lowercase(), ordinal);
    }

    for (ordinal, column) in create_table.columns.into_iter().enumerate() {
        columns.push(convert_column_def(column, ordinal, &column_lookup)?);
    }

    let mut constraints = Vec::new();
    for constraint in create_table.constraints {
        constraints.push(convert_table_constraint(constraint, &column_lookup)?);
    }

    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::CreateTable(CreateTableSpec {
            schema,
            name,
            if_not_exists: create_table.if_not_exists,
            columns,
            constraints,
            strict: create_table.strict,
            without_rowid: create_table.without_rowid,
            normalized_sql: Some(sql.to_owned()),
        }),
    })
}

pub(crate) fn bind_create_index(
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_index: sqlparser::ast::CreateIndex,
) -> Result<PreparedTemplate> {
    if create_index.concurrently
        || create_index.using.is_some()
        || !create_index.include.is_empty()
        || create_index.nulls_distinct.is_some()
        || !create_index.with.is_empty()
        || !create_index.index_options.is_empty()
        || !create_index.alter_options.is_empty()
    {
        return Err(Error::UnsupportedSql(
            "CREATE INDEX modifiers are not supported".to_owned(),
        ));
    }
    let name = create_index
        .name
        .ok_or_else(|| Error::UnsupportedSql("CREATE INDEX requires a name".to_owned()))?;
    let (schema, name) = split_name(name)?;
    let table = parse_qualified_name(create_index.table_name)?;
    let mut columns = Vec::with_capacity(create_index.columns.len());
    let mut has_expression_column = false;
    for column in create_index.columns {
        match convert_index_column(column.clone()) {
            Ok(c) => columns.push(c),
            Err(_) => {
                // Lane SQL-D phase 10: parse-only acceptance of expression
                // indexes. We record a placeholder column so the catalog
                // operation can be rejected at execute time with a clear
                // diagnostic, while CREATE INDEX still parses for tools that
                // round-trip schema text.
                has_expression_column = true;
                columns.push(IndexColumnSpec {
                    name: DbName::new(format!("__expr_{}", columns.len())),
                    sort_dir: SortDir::Asc,
                    collation: None,
                });
            }
        }
    }

    let has_predicate = create_index.predicate.is_some();
    if has_predicate || has_expression_column {
        // Both partial and expression indexes are parser-only in this lane:
        // the kernel does not yet thread the predicate / expression through
        // index DML. Surface a clear unsupported error so callers know the
        // syntax is recognised but not yet enforced.
        return Err(Error::UnsupportedSql(
            "partial and expression indexes are parsed-only; execution not yet implemented"
                .to_owned(),
        ));
    }

    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::CreateIndex(CreateIndexSpec {
            schema,
            name,
            table,
            unique: create_index.unique,
            columns,
            origin: IndexOrigin::User,
            normalized_sql: Some(sql.to_owned()),
        }),
    })
}

pub(crate) fn bind_drop(
    sql: &str,
    schema_epoch: SchemaEpoch,
    object_type: sqlparser::ast::ObjectType,
    if_exists: bool,
    names: Vec<ObjectName>,
) -> Result<PreparedTemplate> {
    if names.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-object DROP is supported".to_owned(),
        ));
    }
    let name = parse_qualified_name(names.into_iter().next().unwrap())?;
    let kind = match object_type {
        sqlparser::ast::ObjectType::Table => {
            PreparedKind::DropTable(DropTableSpec { name, if_exists })
        }
        sqlparser::ast::ObjectType::Index => {
            PreparedKind::DropIndex(DropIndexSpec { name, if_exists })
        }
        _ => {
            return Err(Error::UnsupportedSql(
                "only DROP TABLE and DROP INDEX are supported".to_owned(),
            ));
        }
    };
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind,
    })
}

pub(crate) fn bind_alter_table(
    schema_epoch: SchemaEpoch,
    sql: &str,
    name: ObjectName,
    if_exists: bool,
    only: bool,
    operations: Vec<AlterTableOperation>,
) -> Result<PreparedTemplate> {
    if only {
        return Err(Error::UnsupportedSql(
            "ALTER TABLE ONLY is not supported".to_owned(),
        ));
    }
    if operations.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-operation ALTER TABLE is supported".to_owned(),
        ));
    }
    let operation = match operations.into_iter().next().expect("len checked") {
        AlterTableOperation::RenameTable { table_name } => {
            let table_name = match table_name {
                sqlparser::ast::RenameTableNameKind::As(name)
                | sqlparser::ast::RenameTableNameKind::To(name) => name,
            };
            redlinedb_kernel::catalog::AlterTableOperationSpec::RenameTable {
                table_name: parse_qualified_name(table_name)?,
            }
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => redlinedb_kernel::catalog::AlterTableOperationSpec::RenameColumn {
            old_name: DbName::new(old_column_name.value),
            new_name: DbName::new(new_column_name.value),
        },
        AlterTableOperation::AddColumn {
            column_keyword: _,
            if_not_exists,
            column_def,
            column_position,
            ..
        } => {
            if column_position.is_some() {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE ADD COLUMN position is not supported".to_owned(),
                ));
            }
            let column = convert_column_def(column_def, 0, &std::collections::HashMap::new())?;
            if column.constraints.iter().any(|constraint| {
                !matches!(
                    constraint,
                    ColumnConstraintSpec::NotNull { .. } | ColumnConstraintSpec::Default { .. }
                )
            }) {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE ADD COLUMN supports NOT NULL and DEFAULT only".to_owned(),
                ));
            }
            if column
                .constraints
                .iter()
                .any(|constraint| matches!(constraint, ColumnConstraintSpec::Default { .. }))
                && column.default_value.is_none()
            {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE ADD COLUMN default must be constant".to_owned(),
                ));
            }
            redlinedb_kernel::catalog::AlterTableOperationSpec::AddColumn {
                column,
                if_not_exists,
            }
        }
        AlterTableOperation::DropColumn {
            has_column_keyword: _,
            column_names,
            if_exists,
            drop_behavior,
        } => {
            if drop_behavior.is_some() {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE DROP COLUMN CASCADE/RESTRICT is not supported".to_owned(),
                ));
            }
            if column_names.len() != 1 {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE DROP COLUMN supports a single column at a time".to_owned(),
                ));
            }
            // Parser-only Tier-1 stub: catalog mutation is rejected. We still
            // accept and validate the syntax so callers can build prepared
            // templates and schema migration tools surface the correct
            // unsupported-execution error instead of a parse error.
            redlinedb_kernel::catalog::AlterTableOperationSpec::DropColumn {
                column_name: DbName::new(column_names.into_iter().next().unwrap().value),
                if_exists,
            }
        }
        other => {
            return Err(Error::UnsupportedSql(format!(
                "ALTER TABLE operation not supported yet: {other:?}"
            )));
        }
    };

    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::AlterTable(redlinedb_kernel::catalog::AlterTableSpec {
            name: parse_qualified_name(name)?,
            if_exists,
            operation,
        }),
    })
}

pub(crate) fn bind_analyze(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    analyze: SqlAnalyze,
) -> Result<PreparedTemplate> {
    let table = match analyze.table_name {
        Some(name) => Some(bind_table_name(&schema, &name)?),
        None => None,
    };
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::Analyze(crate::statement::AnalyzePlan { table }),
    })
}
