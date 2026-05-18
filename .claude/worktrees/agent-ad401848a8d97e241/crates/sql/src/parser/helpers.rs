use super::*;

pub(crate) fn is_param_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(crate) fn convert_column_def(
    column: ColumnDef,
    ordinal: usize,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<ColumnSpec> {
    let mut constraints = Vec::new();
    let mut collation = None;
    let mut default_value = None;
    let declared_type = if column.data_type == sqlparser::ast::DataType::Unspecified {
        None
    } else {
        Some(column.data_type.to_string())
    };

    // Phase-10 Lane V1: a `VECTOR(d)` / `VECTOR(d, f32)` column gets an
    // auto-generated CHECK constraint that pins the encoded blob length to
    // exactly `expected_bytes(d)`. This is the SQL surface's only
    // dimension-enforcement hook (kernel-side affinity remains BLOB), and it
    // composes naturally with `apply_constraints` — Null values pass, wrong
    // dim or wrong type fail.
    if let Some(dim) = parse_vector_declared_type(&column.data_type)? {
        constraints.push(vector_dim_check(ordinal, dim));
    }

    for option in column.options {
        match option.option {
            ColumnOption::Null => {}
            ColumnOption::NotNull => constraints.push(ColumnConstraintSpec::NotNull {
                conflict: ConflictAction::Abort,
            }),
            ColumnOption::Default(expr) => {
                let expr_ast = expr_to_kernel_ast(&expr, column_lookup)?;
                if let ExprAst::Const(value) = &expr_ast {
                    default_value = Some(value.clone());
                }
                constraints.push(ColumnConstraintSpec::Default {
                    expr: expr_ast,
                    normalized_sql: expr.to_string(),
                });
            }
            ColumnOption::PrimaryKey(_) => constraints.push(ColumnConstraintSpec::PrimaryKey {
                sort_dir: SortDir::Asc,
                conflict: ConflictAction::Abort,
            }),
            ColumnOption::Unique(_) => constraints.push(ColumnConstraintSpec::Unique {
                conflict: ConflictAction::Abort,
            }),
            ColumnOption::Check(check) => {
                constraints.push(ColumnConstraintSpec::Check {
                    expr: expr_to_kernel_ast(&check.expr, column_lookup)?,
                    normalized_sql: check.expr.to_string(),
                });
            }
            ColumnOption::Collation(name) => {
                collation = Some(name.to_string());
            }
            ColumnOption::ForeignKey(_) => {
                // Lane SQL-D phase 10: column-level REFERENCES is accepted
                // for parser-only compatibility. The kernel does not yet
                // enforce the FK constraint at write time; the declaration
                // is recorded only by being present in the CREATE TABLE
                // text we round-trip through `normalized_sql`.
            }
            ColumnOption::Generated { .. } => {
                // Lane SQL-D phase 10: GENERATED columns parse-only. Stored
                // and virtual variants are accepted but not yet computed;
                // INSERT/UPDATE will leave the column at its declared
                // default until execution support lands.
            }
            ColumnOption::DialectSpecific(_)
            | ColumnOption::CharacterSet(_)
            | ColumnOption::Comment(_)
            | ColumnOption::OnUpdate(_)
            | ColumnOption::Options(_)
            | ColumnOption::Identity(_)
            | ColumnOption::OnConflict(_)
            | ColumnOption::Policy(_)
            | ColumnOption::Tags(_)
            | ColumnOption::Srid(_)
            | ColumnOption::Invisible => {
                return Err(Error::UnsupportedSql(format!(
                    "column option not supported yet: {option:?}"
                )));
            }
            ColumnOption::Materialized(_) | ColumnOption::Ephemeral(_) | ColumnOption::Alias(_) => {
                return Err(Error::UnsupportedSql(
                    "generated/alias column options are not supported".to_owned(),
                ));
            }
        }
    }

    Ok(ColumnSpec {
        name: DbName::new(column.name.value),
        declared_type,
        constraints,
        collation,
        default_value,
    })
}

pub(crate) fn convert_table_constraint(
    constraint: sqlparser::ast::TableConstraint,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<TableConstraintSpec> {
    match constraint {
        sqlparser::ast::TableConstraint::PrimaryKey(pk) => Ok(TableConstraintSpec::PrimaryKey {
            name: pk.name.map(|name| DbName::new(name.value)),
            columns: pk
                .columns
                .into_iter()
                .map(|col| index_column_name(&col).map(DbName::new))
                .collect::<Result<Vec<_>>>()?,
            conflict: ConflictAction::Abort,
        }),
        sqlparser::ast::TableConstraint::Unique(uniq) => Ok(TableConstraintSpec::Unique {
            name: uniq.name.map(|name| DbName::new(name.value)),
            columns: uniq
                .columns
                .into_iter()
                .map(|col| index_column_name(&col).map(DbName::new))
                .collect::<Result<Vec<_>>>()?,
            conflict: ConflictAction::Abort,
        }),
        sqlparser::ast::TableConstraint::Check(check) => Ok(TableConstraintSpec::Check {
            name: check.name.map(|name| DbName::new(name.value)),
            expr: expr_to_kernel_ast(&check.expr, column_lookup)?,
            normalized_sql: check.expr.to_string(),
        }),
        sqlparser::ast::TableConstraint::ForeignKey(fk) => {
            // Lane SQL-D phase 10: FK declarations parse-only. We synthesize
            // a CHECK(1) so the existing TableConstraintSpec surface accepts
            // the constraint without altering the kernel API. Enforcement
            // remains TODO; the declaration text round-trips via the
            // CREATE TABLE normalized_sql.
            let _ = fk; // referenced metadata kept for future use
            Ok(TableConstraintSpec::Check {
                name: None,
                expr: ExprAst::Const(OwnedValue::Integer(1)),
                normalized_sql: "1 /* foreign key parsed-only */".to_owned(),
            })
        }
        sqlparser::ast::TableConstraint::Index(_)
        | sqlparser::ast::TableConstraint::FulltextOrSpatial(_) => Err(Error::UnsupportedSql(
            "table constraint not supported yet".to_owned(),
        )),
    }
}

pub(crate) fn convert_index_column(column: IndexColumn) -> Result<IndexColumnSpec> {
    Ok(IndexColumnSpec {
        name: DbName::new(index_column_name(&column)?),
        sort_dir: match column.column.options.asc {
            Some(false) => SortDir::Desc,
            _ => SortDir::Asc,
        },
        collation: None,
    })
}

pub(crate) fn index_column_name(column: &IndexColumn) -> Result<String> {
    match &column.column.expr {
        Expr::Identifier(ident) => Ok(ident.value.clone()),
        Expr::CompoundIdentifier(parts) if parts.len() == 1 => Ok(parts[0].value.clone()),
        Expr::CompoundIdentifier(parts) => Ok(parts
            .last()
            .ok_or_else(|| Error::UnsupportedSql("empty index column".to_owned()))?
            .value
            .clone()),
        other => Err(Error::UnsupportedSql(format!(
            "unsupported index column expression: {other:?}"
        ))),
    }
}

pub(crate) fn expr_to_kernel_ast(
    expr: &Expr,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<ExprAst> {
    Ok(match expr {
        Expr::Value(v) => sql_value_to_kernel_ast(v)?,
        Expr::Identifier(ident) => {
            ExprAst::Column(resolve_column_ordinal_name(column_lookup, &ident.value)? as u16)
        }
        Expr::CompoundIdentifier(parts) => ExprAst::Column(resolve_column_ordinal_name(
            column_lookup,
            parts
                .last()
                .ok_or_else(|| Error::UnknownColumn("empty identifier".to_owned()))?
                .value
                .as_str(),
        )? as u16),
        Expr::Nested(expr) => expr_to_kernel_ast(expr, column_lookup)?,
        Expr::UnaryOp { op, expr } => match op {
            UnaryOperator::Not => ExprAst::Not(Box::new(expr_to_kernel_ast(expr, column_lookup)?)),
            UnaryOperator::Plus => expr_to_kernel_ast(expr, column_lookup)?,
            UnaryOperator::Minus => {
                return Err(Error::UnsupportedSql(
                    "negative numeric expressions are not supported in DDL".to_owned(),
                ));
            }
            _ => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported unary operator in DDL: {op:?}"
                )));
            }
        },
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => ExprAst::And(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Or => ExprAst::Or(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Eq => ExprAst::Eq(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::NotEq | BinaryOperator::Spaceship => ExprAst::Ne(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Lt => ExprAst::Lt(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::LtEq => ExprAst::Le(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Gt => ExprAst::Gt(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::GtEq => ExprAst::Ge(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            _ => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported binary operator in DDL: {op:?}"
                )));
            }
        },
        Expr::IsNull(expr) => ExprAst::Eq(
            Box::new(expr_to_kernel_ast(expr, column_lookup)?),
            Box::new(ExprAst::Const(OwnedValue::Null)),
        ),
        Expr::IsNotNull(expr) => ExprAst::Ne(
            Box::new(expr_to_kernel_ast(expr, column_lookup)?),
            Box::new(ExprAst::Const(OwnedValue::Null)),
        ),
        Expr::Cast { expr, .. } => expr_to_kernel_ast(expr, column_lookup)?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported DDL expression: {other:?}"
            )));
        }
    })
}

pub(crate) fn sql_value_to_kernel_ast(v: &ValueWithSpan) -> Result<ExprAst> {
    Ok(ExprAst::Const(match &v.value {
        Value::Null => OwnedValue::Null,
        Value::Boolean(v) => OwnedValue::Integer(if *v { 1 } else { 0 }),
        Value::Number(num, _) => parse_numeric_value(num)?,
        Value::SingleQuotedString(s)
        | Value::DoubleQuotedString(s)
        | Value::EscapedStringLiteral(s)
        | Value::TripleSingleQuotedString(s)
        | Value::TripleDoubleQuotedString(s)
        | Value::UnicodeStringLiteral(s)
        | Value::SingleQuotedRawStringLiteral(s)
        | Value::DoubleQuotedRawStringLiteral(s)
        | Value::TripleSingleQuotedRawStringLiteral(s)
        | Value::TripleDoubleQuotedRawStringLiteral(s)
        | Value::DollarQuotedString(sqlparser::ast::DollarQuotedString { value: s, .. }) => {
            OwnedValue::Text(Arc::from(s.as_str()))
        }
        Value::SingleQuotedByteStringLiteral(s)
        | Value::DoubleQuotedByteStringLiteral(s)
        | Value::TripleSingleQuotedByteStringLiteral(s)
        | Value::TripleDoubleQuotedByteStringLiteral(s) => {
            OwnedValue::Blob(Arc::from(s.as_bytes()))
        }
        Value::HexStringLiteral(s) => OwnedValue::Blob(hex_string_to_bytes(s)?),
        Value::Placeholder(name) => {
            return Err(Error::UnsupportedSql(format!(
                "placeholders are not allowed in DDL expressions: {name}"
            )));
        }
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported SQL literal: {other:?}"
            )));
        }
    }))
}

pub(crate) fn parse_numeric_value(input: &str) -> Result<OwnedValue> {
    if let Ok(v) = input.parse::<i64>() {
        return Ok(OwnedValue::Integer(v));
    }
    if let Ok(v) = input.parse::<f64>() {
        return Ok(OwnedValue::Real(v));
    }
    Err(Error::UnsupportedSql(format!(
        "invalid numeric literal: {input}"
    )))
}

pub(crate) fn hex_string_to_bytes(input: &str) -> Result<Arc<[u8]>> {
    if !input.len().is_multiple_of(2) {
        return Err(Error::UnsupportedSql(format!(
            "invalid hex string literal: {input}"
        )));
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let hi = hex_digit(pair[0])?;
        let lo = hex_digit(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(Arc::from(out))
}

pub(crate) fn hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::UnsupportedSql(format!(
            "invalid hex digit in blob literal: {}",
            byte as char
        ))),
    }
}

pub(crate) fn resolve_column_ordinal_name(
    column_lookup: &std::collections::HashMap<String, usize>,
    name: &str,
) -> Result<usize> {
    column_lookup
        .get(&name.to_ascii_lowercase())
        .copied()
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))
}

pub(crate) fn resolve_column_ordinal_in_table(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    name: &str,
) -> Result<usize> {
    table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))
}

pub(crate) fn resolve_column_ordinal_in_object_name(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    name: &ObjectName,
) -> Result<usize> {
    match name.0.as_slice() {
        [part] => resolve_column_ordinal_in_table(table, &object_name_part_to_string(part)?),
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported column name: {name}"
        ))),
    }
}

pub(crate) fn split_name(name: ObjectName) -> Result<(Option<DbName>, DbName)> {
    let display = name.to_string();
    let parts = name.0;
    match parts.as_slice() {
        [part] => Ok((None, DbName::new(object_name_part_to_string(part)?))),
        [schema, name] => Ok((
            Some(DbName::new(object_name_part_to_string(schema)?)),
            DbName::new(object_name_part_to_string(name)?),
        )),
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported qualified name: {display}"
        ))),
    }
}

pub(crate) fn parse_qualified_name(name: ObjectName) -> Result<QualifiedName> {
    let (schema, name) = split_name(name)?;
    Ok(QualifiedName {
        schema: schema.unwrap_or_else(|| DbName::new("main")),
        name,
    })
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
        JoinConstraint::On(expr) => Ok(Some(normalize_expr(expr, params)?)),
        JoinConstraint::Using(columns) => {
            let right_name = right
                .alias
                .as_ref()
                .map(|alias| alias.to_string())
                .unwrap_or_else(|| right.table.name.to_string());
            let left_name = left
                .last()
                .map(|table| {
                    table
                        .alias
                        .as_ref()
                        .map(|alias| alias.to_string())
                        .unwrap_or_else(|| table.table.name.to_string())
                })
                .ok_or_else(|| Error::UnsupportedSql("USING requires a left table".to_owned()))?;
            let mut expr = None;
            for column in columns {
                let column_name = object_name_part_to_string(
                    column
                        .0
                        .last()
                        .ok_or_else(|| Error::UnsupportedSql("empty USING column".to_owned()))?,
                )?;
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
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .map(|ident| ident.value.clone())
            .unwrap_or_else(|| expr.to_string()),
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

pub(crate) fn object_name_part_to_string(part: &ObjectNamePart) -> Result<String> {
    match part {
        ObjectNamePart::Identifier(ident) => Ok(ident.value.clone()),
        ObjectNamePart::Function(_) => Err(Error::UnsupportedSql(
            "function-style object names are not supported".to_owned(),
        )),
    }
}

// -----------------------------------------------------------------------------
// VECTOR(d[, f32]) — phase-10 Lane V1.
// -----------------------------------------------------------------------------

/// Recognise `VECTOR(d)` / `VECTOR(d, f32)` and extract the dimension. Returns
/// `Ok(None)` for non-vector types so `convert_column_def` can stay flat.
///
/// `sqlparser` parses unknown parameterised types as
/// `DataType::Custom(ObjectName, Vec<String>)`, where the second tuple field
/// holds the parenthesised arguments verbatim — so `VECTOR(384, f32)` arrives
/// as `("VECTOR", ["384", "f32"])`. We accept any case, only the f32 element
/// kind today, and reject zero / unparseable dimensions.
pub(crate) fn parse_vector_declared_type(
    data_type: &sqlparser::ast::DataType,
) -> Result<Option<u32>> {
    let sqlparser::ast::DataType::Custom(name, args) = data_type else {
        return Ok(None);
    };
    let Some(part) = name.0.first() else {
        return Ok(None);
    };
    let raw = match part {
        ObjectNamePart::Identifier(ident) => ident.value.as_str(),
        ObjectNamePart::Function(_) => return Ok(None),
    };
    if !raw.eq_ignore_ascii_case("VECTOR") {
        return Ok(None);
    }
    if args.is_empty() || args.len() > 2 {
        return Err(Error::UnsupportedSql(format!(
            "VECTOR type expects 1 or 2 args (dim[, element_kind]); got {}",
            args.len()
        )));
    }
    let dim: u32 = args[0].parse().map_err(|_| {
        Error::UnsupportedSql(format!(
            "VECTOR dim must be a positive integer; got {}",
            args[0]
        ))
    })?;
    if dim == 0 {
        return Err(Error::UnsupportedSql("VECTOR dim must be > 0".to_owned()));
    }
    if let Some(kind) = args.get(1)
        && !kind.eq_ignore_ascii_case("f32")
    {
        return Err(Error::UnsupportedSql(format!(
            "VECTOR element kind '{kind}' is not supported yet (only f32)"
        )));
    }
    Ok(Some(dim))
}

/// Compute the exact wire-format byte length for a vector with `dim` f32
/// components, matching `kernel::vector::codec::encode_vector`.
fn vector_encoded_len(dim: u32) -> u64 {
    // varint length: 1 byte per 7 bits of `dim`. `dim > 0` (caller checked).
    let mut varint = 0u64;
    let mut tmp = dim;
    loop {
        varint += 1;
        tmp >>= 7;
        if tmp == 0 {
            break;
        }
    }
    varint + 1 /* element kind */ + (dim as u64) * 4
}

/// Build the auto-generated CHECK constraint that enforces `BlobLen(col) = K`
/// for vector columns. The check is named `__vec_dim_<col>` so it shows up
/// recognisably in `sqlite_schema.sql` and EXPLAIN output.
fn vector_dim_check(ordinal: usize, dim: u32) -> ColumnConstraintSpec {
    let expected = vector_encoded_len(dim) as i64;
    let expr = ExprAst::Eq(
        Box::new(ExprAst::BlobLen(Box::new(ExprAst::Column(ordinal as u16)))),
        Box::new(ExprAst::Const(OwnedValue::Integer(expected))),
    );
    ColumnConstraintSpec::Check {
        expr,
        normalized_sql: format!("/* auto: vector_dim={dim} */"),
    }
}
