use super::*;

pub(crate) fn bind_create_table(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_table: sqlparser::ast::CreateTable,
) -> Result<PreparedTemplate> {
    if create_table.or_replace
        || create_table.external
        || create_table.dynamic
        || create_table.global.is_some()
        || create_table.transient
        || create_table.volatile
        || create_table.iceberg
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

    if create_table.query.is_some() {
        return bind_create_table_as_select(conn, schema, schema_epoch, sql, create_table);
    }

    let session_scoped = crate::parser::bind::create_table_is_session_scoped(&create_table);
    let (schema, name) = split_name(create_table.name)?;
    // SQLite rejects `AUTOINCREMENT` on a `WITHOUT ROWID` table — the
    // sqlite_sequence machinery is rowid-based by design. Detect the
    // combination ahead of column conversion so callers see the same
    // upfront error.
    if create_table.without_rowid
        && create_table.columns.iter().any(|column| {
            column
                .options
                .iter()
                .any(|opt| matches!(&opt.option,
                    sqlparser::ast::ColumnOption::DialectSpecific(tokens)
                        if tokens.len() == 1
                            && tokens[0].to_string().eq_ignore_ascii_case("AUTOINCREMENT")
                ))
        })
    {
        return Err(Error::UnsupportedSql(
            "AUTOINCREMENT not allowed on WITHOUT ROWID tables".to_owned(),
        ));
    }
    // STRICT tables accept only the six well-defined affinities: INT,
    // INTEGER, REAL, TEXT, BLOB, ANY. Reject everything else upfront so
    // callers see the same surface as sqlite3. SQLite rejects NUMERIC
    // because its type-affinity rules collide with strict typing.
    if create_table.strict {
        for column in &create_table.columns {
            if column.data_type == sqlparser::ast::DataType::Unspecified {
                return Err(Error::UnsupportedSql(format!(
                    "STRICT table column {} requires a declared type",
                    column.name.value
                )));
            }
            let declared = column.data_type.to_string();
            if !is_strict_table_allowed_type(&declared) {
                return Err(Error::UnsupportedSql(format!(
                    "STRICT table column {} declares unsupported type {declared}",
                    column.name.value
                )));
            }
        }
    }
    let mut columns = Vec::with_capacity(create_table.columns.len());
    let mut column_lookup = std::collections::HashMap::new();
    for (ordinal, column) in create_table.columns.iter().enumerate() {
        column_lookup.insert(column.name.value.to_ascii_lowercase(), ordinal);
    }
    let mut constraints = Vec::new();

    for (ordinal, column) in create_table.columns.into_iter().enumerate() {
        columns.push(convert_column_def(
            column,
            ordinal,
            &column_lookup,
            &mut constraints,
        )?);
    }

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
        kind: if session_scoped {
            PreparedKind::CreateTempTable(CreateTableSpec {
                schema,
                name,
                if_not_exists: create_table.if_not_exists,
                columns,
                constraints,
                strict: create_table.strict,
                without_rowid: create_table.without_rowid,
                normalized_sql: Some(strip_trailing_semicolon(sql)),
            })
        } else {
            PreparedKind::CreateTable(CreateTableSpec {
                schema,
                name,
                if_not_exists: create_table.if_not_exists,
                columns,
                constraints,
                strict: create_table.strict,
                without_rowid: create_table.without_rowid,
                normalized_sql: Some(strip_trailing_semicolon(sql)),
            })
        },
    })
}

fn bind_create_table_as_select(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_table: sqlparser::ast::CreateTable,
) -> Result<PreparedTemplate> {
    if !create_table.columns.is_empty() || !create_table.constraints.is_empty() {
        return Err(Error::UnsupportedSql(
            "CREATE TABLE AS SELECT does not accept column definitions".to_owned(),
        ));
    }

    let (schema_name, table_name) = split_name(create_table.name)?;
    if create_table.if_not_exists
        && let Some(schema_id) = match schema_name.as_ref() {
            Some(name) if name.folded().eq_ignore_ascii_case("main") => {
                schema.lookup_namespace("main")
            }
            Some(_) => {
                return Err(Error::UnsupportedSql(
                    "only the main schema is supported".to_owned(),
                ));
            }
            None => schema.lookup_namespace("main"),
        }
        && schema
            .lookup_table(schema_id, table_name.folded())
            .is_some()
    {
        return Ok(PreparedTemplate {
            sql: Arc::from(sql),
            schema_epoch,
            stats_epoch: 0,
            optimizer_hash: 0,
            param_layout: ParamLayout::default(),
            output_columns: Arc::from([]),
            readonly: false,
            kind: PreparedKind::CreateTableAsSelect(crate::statement::CreateTableAsSelectSpec {
                table: CreateTableSpec {
                    schema: schema_name,
                    name: table_name,
                    if_not_exists: true,
                    columns: Vec::new(),
                    constraints: Vec::new(),
                    strict: create_table.strict,
                    without_rowid: create_table.without_rowid,
                    normalized_sql: Some(strip_trailing_semicolon(sql)),
                },
                select: None,
            }),
        });
    }

    let query = create_table.query.ok_or_else(|| {
        Error::UnsupportedSql("CREATE TABLE AS SELECT requires a source query".to_owned())
    })?;
    let mut select_template =
        crate::parser::select::bind_query(conn, Arc::clone(&schema), schema_epoch, sql, *query)?;
    let select_plan = match select_template.kind.clone() {
        PreparedKind::Select(plan) => plan,
        _ => {
            return Err(Error::UnsupportedSql(
                "CTAS query did not bind to a SELECT plan".to_owned(),
            ));
        }
    };
    let columns = build_ctas_columns(&select_plan)?;
    select_template.kind =
        PreparedKind::CreateTableAsSelect(crate::statement::CreateTableAsSelectSpec {
            table: CreateTableSpec {
                schema: schema_name,
                name: table_name,
                if_not_exists: create_table.if_not_exists,
                columns,
                constraints: Vec::new(),
                strict: create_table.strict,
                without_rowid: create_table.without_rowid,
                normalized_sql: Some(strip_trailing_semicolon(sql)),
            },
            select: Some(select_plan),
        });
    select_template.readonly = false;
    select_template.output_columns = Arc::from([]);
    Ok(select_template)
}

fn build_ctas_columns(select: &SelectPlan) -> Result<Vec<ColumnSpec>> {
    let names = select_plan_output_names(select);
    let affinities = ctas_projection_affinities(select);
    if names.len() != affinities.len() {
        return Err(Error::UnsupportedSql(
            "CTAS column-name and affinity counts diverged".to_owned(),
        ));
    }
    let mut counts = std::collections::HashMap::<String, usize>::new();
    let mut columns = Vec::with_capacity(names.len());
    for (name, affinity) in names.into_iter().zip(affinities.into_iter()) {
        let folded = name.to_ascii_lowercase();
        let ordinal = counts.entry(folded).or_insert(0);
        let column_name = if *ordinal == 0 {
            name
        } else {
            format!("{name}:{ordinal}")
        };
        *ordinal += 1;
        columns.push(ColumnSpec {
            name: DbName::new(column_name),
            declared_type: ctas_declared_type(affinity),
            constraints: Vec::new(),
            collation: None,
            default_value: None,
            generated: None,
        });
    }
    Ok(columns)
}

fn ctas_projection_affinities(select: &SelectPlan) -> Vec<redlinedb_kernel::catalog::Affinity> {
    if select.projection.is_empty() {
        return source_output_affinities(&select.source);
    }

    let source_names = source_output_names(&select.source);
    let source_affinities = source_output_affinities(&select.source);
    let mut affinities = Vec::new();
    for item in &select.projection {
        match item {
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                affinities.extend(source_output_affinities(&select.source));
            }
            SelectItem::UnnamedExpr(expr) => {
                affinities.push(expr_ctas_affinity(expr, &source_names, &source_affinities))
            }
            SelectItem::ExprWithAlias { expr, .. } => {
                affinities.push(expr_ctas_affinity(expr, &source_names, &source_affinities))
            }
        }
    }
    affinities
}

fn expr_ctas_affinity(
    expr: &Expr,
    source_names: &[String],
    source_affinities: &[redlinedb_kernel::catalog::Affinity],
) -> redlinedb_kernel::catalog::Affinity {
    match expr {
        Expr::Cast { data_type, .. } => ctas_affinity_from_declared_type(&data_type.to_string())
            .unwrap_or(redlinedb_kernel::catalog::Affinity::Blob),
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => {
            expr_ctas_affinity(inner, source_names, source_affinities)
        }
        Expr::Identifier(ident) => {
            lookup_source_affinity(&ident.value, None, source_names, source_affinities)
                .unwrap_or(redlinedb_kernel::catalog::Affinity::Blob)
        }
        Expr::CompoundIdentifier(parts) => {
            let column = match parts.last() {
                Some(part) => part.value.as_str(),
                None => return redlinedb_kernel::catalog::Affinity::Blob,
            };
            let qualifier = if parts.len() > 1 {
                parts.first().map(|part| part.value.as_str())
            } else {
                None
            };
            lookup_source_affinity(column, qualifier, source_names, source_affinities)
                .unwrap_or(redlinedb_kernel::catalog::Affinity::Blob)
        }
        _ => redlinedb_kernel::catalog::Affinity::Blob,
    }
}

fn lookup_source_affinity(
    column: &str,
    qualifier: Option<&str>,
    source_names: &[String],
    source_affinities: &[redlinedb_kernel::catalog::Affinity],
) -> Option<redlinedb_kernel::catalog::Affinity> {
    if let Some(qualifier) = qualifier {
        let qualified = format!("{qualifier}.{column}");
        if let Some((idx, _)) = source_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.eq_ignore_ascii_case(&qualified))
        {
            return source_affinities.get(idx).copied();
        }
    }
    source_names
        .iter()
        .enumerate()
        .find(|(_, name)| name.eq_ignore_ascii_case(column))
        .and_then(|(idx, _)| source_affinities.get(idx).copied())
}

fn source_output_names(source: &SelectSource) -> Vec<String> {
    match source {
        SelectSource::Table(table) => table
            .columns
            .iter()
            .map(|column| column.name.to_string())
            .collect(),
        SelectSource::Tables(tables) => tables
            .iter()
            .flat_map(|table| {
                table
                    .table
                    .columns
                    .iter()
                    .map(|column| column.name.to_string())
            })
            .collect(),
        SelectSource::Joined(join) => {
            let mut names = source_output_names(&SelectSource::Table(Arc::clone(&join.base.table)));
            for step in &join.joins {
                names.extend(source_output_names(&SelectSource::Table(Arc::clone(
                    &step.right.table,
                ))));
            }
            names
        }
        SelectSource::Cte { columns, .. } => columns.iter().map(|c| c.to_string()).collect(),
        SelectSource::StaticRows { .. } => Vec::new(),
        SelectSource::CompoundAll(branches) | SelectSource::CompoundSet { branches, .. } => {
            branches
                .first()
                .map(select_plan_output_names)
                .unwrap_or_default()
        }
        SelectSource::SqliteSchema | SelectSource::SqliteTempSchema => [
            "type".to_owned(),
            "name".to_owned(),
            "tbl_name".to_owned(),
            "rootpage".to_owned(),
            "sql".to_owned(),
        ]
        .into(),
        SelectSource::Empty => Vec::new(),
    }
}

fn source_output_affinities(source: &SelectSource) -> Vec<redlinedb_kernel::catalog::Affinity> {
    match source {
        SelectSource::Table(table) => table.columns.iter().map(|column| column.affinity).collect(),
        SelectSource::Tables(tables) => tables
            .iter()
            .flat_map(|table| table.table.columns.iter().map(|column| column.affinity))
            .collect(),
        SelectSource::Joined(join) => {
            let mut affs =
                source_output_affinities(&SelectSource::Table(Arc::clone(&join.base.table)));
            for step in &join.joins {
                affs.extend(source_output_affinities(&SelectSource::Table(Arc::clone(
                    &step.right.table,
                ))));
            }
            affs
        }
        SelectSource::Cte { rows, columns, .. } => columns
            .iter()
            .enumerate()
            .map(|(idx, _)| infer_affinity(rows, idx))
            .collect(),
        SelectSource::StaticRows { rows, .. } => rows
            .first()
            .map(|first| {
                first
                    .iter()
                    .enumerate()
                    .map(|(idx, _)| infer_affinity(rows.as_ref(), idx))
                    .collect()
            })
            .unwrap_or_default(),
        SelectSource::CompoundAll(branches) | SelectSource::CompoundSet { branches, .. } => {
            branches
                .first()
                .map(|plan| ctas_projection_affinities(plan))
                .unwrap_or_default()
        }
        SelectSource::SqliteSchema | SelectSource::SqliteTempSchema => vec![
            redlinedb_kernel::catalog::Affinity::Text,
            redlinedb_kernel::catalog::Affinity::Text,
            redlinedb_kernel::catalog::Affinity::Text,
            redlinedb_kernel::catalog::Affinity::Integer,
            redlinedb_kernel::catalog::Affinity::Text,
        ],
        SelectSource::Empty => Vec::new(),
    }
}

fn infer_affinity(rows: &[Vec<SqlValue>], col: usize) -> redlinedb_kernel::catalog::Affinity {
    for row in rows {
        if let Some(value) = row.get(col) {
            match value {
                SqlValue::Integer(_) => return redlinedb_kernel::catalog::Affinity::Integer,
                SqlValue::Real(_) => return redlinedb_kernel::catalog::Affinity::Real,
                SqlValue::Text(_) => return redlinedb_kernel::catalog::Affinity::Text,
                SqlValue::Blob(_) => return redlinedb_kernel::catalog::Affinity::Blob,
                SqlValue::Null => continue,
            }
        }
    }
    redlinedb_kernel::catalog::Affinity::Blob
}

fn ctas_affinity_from_declared_type(
    declared_type: &str,
) -> Option<redlinedb_kernel::catalog::Affinity> {
    let affinity = redlinedb_kernel::catalog::derive_affinity(Some(declared_type));
    match affinity {
        redlinedb_kernel::catalog::Affinity::Blob => None,
        redlinedb_kernel::catalog::Affinity::Text => {
            Some(redlinedb_kernel::catalog::Affinity::Text)
        }
        redlinedb_kernel::catalog::Affinity::Numeric => {
            Some(redlinedb_kernel::catalog::Affinity::Numeric)
        }
        redlinedb_kernel::catalog::Affinity::Integer => {
            Some(redlinedb_kernel::catalog::Affinity::Integer)
        }
        redlinedb_kernel::catalog::Affinity::Real => {
            Some(redlinedb_kernel::catalog::Affinity::Real)
        }
    }
}

fn ctas_declared_type(affinity: redlinedb_kernel::catalog::Affinity) -> Option<String> {
    match affinity {
        redlinedb_kernel::catalog::Affinity::Blob => None,
        redlinedb_kernel::catalog::Affinity::Text => Some("TEXT".to_owned()),
        redlinedb_kernel::catalog::Affinity::Numeric => Some("NUM".to_owned()),
        redlinedb_kernel::catalog::Affinity::Integer => Some("INT".to_owned()),
        redlinedb_kernel::catalog::Affinity::Real => Some("REAL".to_owned()),
    }
}

pub(crate) fn bind_create_index(
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_index: sqlparser::ast::CreateIndex,
) -> Result<PreparedTemplate> {
    // Track J — relaxed acceptance of common Postgres CREATE INDEX
    // modifiers:
    //   - `INCLUDE (...)` covering-index columns: parsed and ignored (the
    //     covering set is a planner hint; query results match an ordinary
    //     index).
    //   - `NULLS [NOT] DISTINCT`: ignored (RedlineDB treats every NULL as
    //     distinct in indexed columns, matching the default).
    //   - `USING <method>` is stripped pre-parse by
    //     `strip_create_index_using_clause`.
    if create_index.concurrently
        || !create_index.with.is_empty()
        || !create_index.index_options.is_empty()
        || !create_index.alter_options.is_empty()
    {
        return Err(Error::UnsupportedSql(
            "CREATE INDEX modifiers are not supported".to_owned(),
        ));
    }
    let name = match create_index.name {
        Some(n) => n,
        None => {
            return Err(Error::UnsupportedSql(
                "CREATE INDEX requires a name".to_owned(),
            ));
        }
    };
    let (schema, name) = split_name(name)?;
    let table = parse_qualified_name(create_index.table_name)?;
    let mut columns = Vec::with_capacity(create_index.columns.len());
    for column in create_index.columns {
        match convert_index_column(column.clone()) {
            Ok(c) => columns.push(c),
            Err(_) => {
                // A6 SQL-D: expression-source index column. Stash the
                // verbatim SQL fragment so the executor can re-parse
                // and evaluate it per row, plus the set of referenced
                // column ordinals for UPDATE re-emit decisions.
                let expr_sql = column.column.expr.to_string();
                let referenced =
                    super::helpers::ddl::index_expr_referenced_cols(&column.column.expr);
                columns.push(IndexColumnSpec {
                    name: DbName::new(format!("__expr_{}", columns.len())),
                    sort_dir: match column.column.options.asc {
                        Some(false) => SortDir::Desc,
                        _ => SortDir::Asc,
                    },
                    collation: None,
                    expr_sql: Some(expr_sql),
                    expr_referenced_cols: referenced,
                });
            }
        }
    }

    // A6 SQL-D: thread the partial-index WHERE predicate through to
    // the kernel as its verbatim SQL fragment; the executor re-parses
    // it per DML so the predicate semantics match the catalog text.
    let predicate_sql = create_index.predicate.as_ref().map(|p| p.to_string());

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
            if_not_exists: create_index.if_not_exists,
            table,
            unique: create_index.unique,
            columns,
            origin: IndexOrigin::User,
            normalized_sql: Some(sql.to_owned()),
            predicate_sql,
        }),
    })
}

pub(crate) fn bind_drop(
    sql: &str,
    schema_epoch: SchemaEpoch,
    object_type: sqlparser::ast::ObjectType,
    if_exists: bool,
    names: Vec<ObjectName>,
    cascade: bool,
) -> Result<PreparedTemplate> {
    if names.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-object DROP is supported".to_owned(),
        ));
    }
    let raw_name = names.into_iter().next().unwrap();
    let kind = match object_type {
        sqlparser::ast::ObjectType::Schema => {
            // Track J: DROP SCHEMA <name> [CASCADE].
            let schema_name = match raw_name.0.last() {
                Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
                _ => {
                    return Err(Error::UnsupportedSql(
                        "DROP SCHEMA requires a name".to_owned(),
                    ));
                }
            };
            PreparedKind::DropSchema {
                name: Arc::from(schema_name),
                if_exists,
                cascade,
            }
        }
        sqlparser::ast::ObjectType::Sequence => {
            // Track J: DROP SEQUENCE <name>.
            let seq_name = match raw_name.0.last() {
                Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
                _ => {
                    return Err(Error::UnsupportedSql(
                        "DROP SEQUENCE requires a name".to_owned(),
                    ));
                }
            };
            PreparedKind::DropSequence {
                name: Arc::from(seq_name),
                if_exists,
            }
        }
        sqlparser::ast::ObjectType::Table => {
            let name = parse_qualified_name(raw_name)?;
            PreparedKind::DropTable(DropTableSpec { name, if_exists })
        }
        sqlparser::ast::ObjectType::Index => {
            let name = parse_qualified_name(raw_name)?;
            PreparedKind::DropIndex(DropIndexSpec { name, if_exists })
        }
        sqlparser::ast::ObjectType::View => {
            let name = parse_qualified_name(raw_name)?;
            PreparedKind::DropView(DropViewSpec { name, if_exists })
        }
        _ => {
            return Err(Error::UnsupportedSql(
                "only DROP TABLE, DROP INDEX, DROP VIEW, DROP SCHEMA, and DROP SEQUENCE are supported".to_owned(),
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
    schema: &SchemaSnapshot,
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
    // Track J — ADD CONSTRAINT needs a column lookup so the check-expression
    // identifiers resolve against the existing table at parse time.
    let alter_target_lookup: std::collections::HashMap<String, usize> = (|| {
        let qname = parse_qualified_name(name.clone()).ok()?;
        let schema_id = schema.lookup_namespace("main")?;
        let table = schema.lookup_table(schema_id, qname.name.folded())?;
        Some(
            table
                .columns
                .iter()
                .enumerate()
                .map(|(idx, col)| (col.folded.as_ref().to_owned(), idx))
                .collect(),
        )
    })()
    .unwrap_or_default();
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
            let mut alter_constraints = Vec::new();
            let column = convert_column_def(
                column_def,
                0,
                &std::collections::HashMap::new(),
                &mut alter_constraints,
            )?;
            if !alter_constraints.is_empty() {
                return Err(Error::UnsupportedSql(
                    "ALTER TABLE ADD COLUMN with FOREIGN KEY is not supported".to_owned(),
                ));
            }
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
            let if_not_exists = if_not_exists
                || sql
                    .to_ascii_lowercase()
                    .contains("add column if not exists");
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
            // Parser-only Tier-1 acceptance: catalog mutation is rejected.
            // We still accept and validate the syntax so callers can build
            // prepared templates and schema migration tools surface the
            // correct unsupported-execution error instead of a parse error.
            redlinedb_kernel::catalog::AlterTableOperationSpec::DropColumn {
                column_name: DbName::new(column_names.into_iter().next().unwrap().value),
                if_exists,
            }
        }
        // Track J — ALTER COLUMN SET/DROP DEFAULT, SET/DROP NOT NULL,
        // SET DATA TYPE, ADD GENERATED IDENTITY.
        AlterTableOperation::AlterColumn { column_name, op } => match op {
            sqlparser::ast::AlterColumnOperation::SetDefault { value } => {
                let lookup = std::collections::HashMap::new();
                let expr_ast =
                    super::helpers::expr::default_expr_to_kernel_ast(&value, &lookup)?;
                let default_value = match expr_ast {
                    ExprAst::Const(v) => Some(v),
                    _ => {
                        return Err(Error::UnsupportedSql(
                            "ALTER COLUMN SET DEFAULT requires a constant value".to_owned(),
                        ));
                    }
                };
                redlinedb_kernel::catalog::AlterTableOperationSpec::SetColumnDefault {
                    column_name: DbName::new(column_name.value),
                    default_value,
                }
            }
            sqlparser::ast::AlterColumnOperation::DropDefault => {
                redlinedb_kernel::catalog::AlterTableOperationSpec::DropColumnDefault {
                    column_name: DbName::new(column_name.value),
                }
            }
            sqlparser::ast::AlterColumnOperation::DropNotNull => {
                redlinedb_kernel::catalog::AlterTableOperationSpec::DropColumnNotNull {
                    column_name: DbName::new(column_name.value),
                }
            }
            sqlparser::ast::AlterColumnOperation::SetNotNull => {
                redlinedb_kernel::catalog::AlterTableOperationSpec::SetColumnNotNull {
                    column_name: DbName::new(column_name.value),
                }
            }
            sqlparser::ast::AlterColumnOperation::SetDataType {
                data_type,
                using: _,
                had_set: _,
            } => redlinedb_kernel::catalog::AlterTableOperationSpec::SetColumnType {
                column_name: DbName::new(column_name.value),
                declared_type: data_type.to_string(),
            },
            sqlparser::ast::AlterColumnOperation::AddGenerated {
                generated_as,
                sequence_options: _,
            } => {
                let always = matches!(
                    generated_as,
                    Some(sqlparser::ast::GeneratedAs::Always)
                );
                redlinedb_kernel::catalog::AlterTableOperationSpec::AddColumnIdentity {
                    column_name: DbName::new(column_name.value),
                    always,
                }
            }
        },
        AlterTableOperation::AddConstraint {
            constraint,
            not_valid: _,
        } => {
            let constraint_spec = convert_table_constraint(constraint, &alter_target_lookup)?;
            redlinedb_kernel::catalog::AlterTableOperationSpec::AddNamedConstraint {
                constraint: constraint_spec,
                if_not_exists: false,
            }
        }
        AlterTableOperation::DropConstraint {
            if_exists,
            name,
            drop_behavior: _,
        } => redlinedb_kernel::catalog::AlterTableOperationSpec::DropConstraint {
            name: DbName::new(name.value),
            if_exists,
        },
        AlterTableOperation::RenameConstraint { old_name, new_name } => {
            redlinedb_kernel::catalog::AlterTableOperationSpec::RenameConstraint {
                old_name: DbName::new(old_name.value),
                new_name: DbName::new(new_name.value),
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

/// Bind `CREATE [TEMP] VIEW [IF NOT EXISTS] name [(col, col)] AS SELECT ...`.
///
/// The body SELECT is re-emitted to canonical SQL via the sqlparser
/// `Display` impl and stored verbatim on the [`CreateViewSpec`]. The
/// kernel persists it as-is; at expansion time the SQL crate re-parses
/// the body and binds it as a derived row source.
///
/// Rejects MySQL/Snowflake/Clickhouse modifiers that fresh SQLite-style
/// applications do not need (materialized, secure, OR REPLACE,
/// WITH NO SCHEMA BINDING, TO clause, CLUSTER BY, etc.).
pub(crate) fn bind_create_view(
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_view: sqlparser::ast::CreateView,
) -> Result<PreparedTemplate> {
    if create_view.or_alter
        || create_view.or_replace
        || create_view.materialized
        || create_view.secure
        || create_view.with_no_schema_binding
        || create_view.to.is_some()
        || create_view.params.is_some()
        || !create_view.cluster_by.is_empty()
        || create_view.comment.is_some()
    {
        return Err(Error::UnsupportedSql(
            "CREATE VIEW modifiers are not supported".to_owned(),
        ));
    }
    let (schema, name) = split_name(create_view.name)?;
    let columns = create_view
        .columns
        .into_iter()
        .map(|col| DbName::new(col.name.value))
        .collect();
    // Render the body SELECT back to canonical SQL; the kernel persists
    // it verbatim and the binder re-parses it on each view expansion.
    let body_sql = create_view.query.to_string();
    Ok(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::CreateView(CreateViewSpec {
            schema,
            name,
            if_not_exists: create_view.if_not_exists,
            // SQLite `TEMP VIEW` modifier flag.
            session_scoped: create_view.temporary,
            columns,
            body_sql,
            normalized_sql: Some(sql.to_owned()),
        }),
    })
}

/// Bind `CREATE TRIGGER name {BEFORE|AFTER} {INSERT|UPDATE [OF col,...]|DELETE}
/// ON table [FOR EACH ROW] [WHEN expr] BEGIN body END`.
///
/// The body's statement list is re-emitted to canonical SQL and stored
/// verbatim on the [`CreateTriggerSpec`]; the fire-hook re-parses the
/// body and runs it against an `OLD`/`NEW` row context at runtime.
///
/// Rejects modifiers that fresh SQLite-style applications do not need
/// (`OR REPLACE`, `OR ALTER`, MSSQL/Postgres trigger function bodies,
/// constraint triggers, `REFERENCING NEW TABLE AS ...`, etc.) plus
/// `INSTEAD OF` (deferred to a followup task).
pub(crate) fn bind_create_trigger(
    schema_epoch: SchemaEpoch,
    sql: &str,
    create_trigger: sqlparser::ast::CreateTrigger,
) -> Result<PreparedTemplate> {
    if create_trigger.or_alter
        || create_trigger.or_replace
        || create_trigger.is_constraint
        || create_trigger.referenced_table_name.is_some()
        || !create_trigger.referencing.is_empty()
        || create_trigger.exec_body.is_some()
        || create_trigger.statements_as
        || create_trigger.characteristics.is_some()
    {
        return Err(Error::UnsupportedSql(
            "CREATE TRIGGER modifiers are not supported".to_owned(),
        ));
    }
    let period = create_trigger.period.ok_or_else(|| {
        Error::UnsupportedSql("CREATE TRIGGER requires BEFORE or AFTER".to_owned())
    })?;
    let when_time = match period {
        sqlparser::ast::TriggerPeriod::Before => TriggerTimeKind::Before,
        sqlparser::ast::TriggerPeriod::After => TriggerTimeKind::After,
        sqlparser::ast::TriggerPeriod::InsteadOf => TriggerTimeKind::InsteadOf,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "CREATE TRIGGER period not supported: {other:?}"
            )));
        }
    };
    if create_trigger.events.len() != 1 {
        return Err(Error::UnsupportedSql(
            "CREATE TRIGGER requires exactly one event".to_owned(),
        ));
    }
    let (when_event, when_cols) = match create_trigger
        .events
        .into_iter()
        .next()
        .expect("checked len")
    {
        sqlparser::ast::TriggerEvent::Insert => (TriggerEventKind::Insert, Vec::new()),
        sqlparser::ast::TriggerEvent::Delete => (TriggerEventKind::Delete, Vec::new()),
        sqlparser::ast::TriggerEvent::Update(cols) => (
            TriggerEventKind::Update,
            cols.into_iter().map(|c| DbName::new(c.value)).collect(),
        ),
        sqlparser::ast::TriggerEvent::Truncate => {
            return Err(Error::UnsupportedSql(
                "TRUNCATE triggers are not supported".to_owned(),
            ));
        }
    };
    let (schema, name) = split_name(create_trigger.name)?;
    // We only persist the table name (not its schema). Multi-schema
    // resolution is deferred to ATTACH/DETACH (A2).
    let table = parse_qualified_name(create_trigger.table_name)?.name;
    let when_predicate_sql = create_trigger
        .condition
        .as_ref()
        .map(|expr| expr.to_string());
    let body_sql = match create_trigger.statements {
        Some(stmts) => stmts.to_string(),
        None => {
            return Err(Error::UnsupportedSql(
                "CREATE TRIGGER requires a BEGIN ... END body".to_owned(),
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
        kind: PreparedKind::CreateTrigger(CreateTriggerSpec {
            schema,
            name,
            if_not_exists: false, // sqlparser does not surface this on CreateTrigger
            table,
            when_time,
            when_event,
            when_cols,
            when_predicate_sql,
            body_sql,
            normalized_sql: Some(sql.to_owned()),
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

/// True when `declared` is one of the six declared types SQLite allows
/// inside a `CREATE TABLE … STRICT` definition: INT, INTEGER, REAL,
/// TEXT, BLOB, ANY. Everything else (notably VARCHAR / NUMERIC) is
/// rejected at CREATE-time. Matches SQLite case-insensitively.
fn is_strict_table_allowed_type(declared: &str) -> bool {
    const ALLOWED: &[&str] = &["INT", "INTEGER", "REAL", "TEXT", "BLOB", "ANY"];
    let normalized = declared.trim().to_ascii_uppercase();
    ALLOWED.iter().any(|t| *t == normalized)
}

/// Trim a single trailing `;` from a SQL statement before stashing it on
/// `sqlite_master.sql`. SQLite never includes the terminator in the
/// reflected text, so our `normalized_sql` field mirrors that surface.
pub(crate) fn strip_trailing_semicolon(sql: &str) -> String {
    let trimmed = sql.trim_end();
    if let Some(stripped) = trimmed.strip_suffix(';') {
        stripped.trim_end().to_owned()
    } else {
        trimmed.to_owned()
    }
}
