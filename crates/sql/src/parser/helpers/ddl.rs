use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, DbName, ExprAst, FkAction,
    GeneratedColumnKind, GeneratedColumnSpec, IndexColumnSpec, OwnedValue, SortDir,
    TableConstraintSpec,
};
use sqlparser::ast::{
    ColumnDef, ColumnOption, DataType, DeferrableInitial, Expr, ForeignKeyConstraint,
    GeneratedExpressionMode, IndexColumn, ObjectNamePart, ReferentialAction,
};

use crate::error::{Error, Result};

use super::expr::{default_expr_to_kernel_ast, expr_to_kernel_ast};

pub(crate) fn is_param_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(crate) fn convert_column_def(
    column: ColumnDef,
    ordinal: usize,
    column_lookup: &std::collections::HashMap<String, usize>,
    table_constraints: &mut Vec<TableConstraintSpec>,
) -> Result<ColumnSpec> {
    let mut constraints = Vec::new();
    let mut collation = None;
    let mut default_value = None;
    let mut generated: Option<GeneratedColumnSpec> = None;
    let column_name = DbName::new(column.name.value.clone());
    let declared_type = if column.data_type == sqlparser::ast::DataType::Unspecified {
        None
    } else {
        Some(column.data_type.to_string())
    };
    let has_autoincrement = column
        .options
        .iter()
        .any(|option| is_sqlite_autoincrement_option(&option.option));
    if has_autoincrement
        && (!matches!(column.data_type, DataType::Integer(_))
            || !column
                .options
                .iter()
                .any(|option| matches!(option.option, ColumnOption::PrimaryKey(_))))
    {
        return Err(Error::UnsupportedSql(
            "AUTOINCREMENT is only allowed on an INTEGER PRIMARY KEY".to_owned(),
        ));
    }

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
                let expr_ast = default_expr_to_kernel_ast(&expr, column_lookup)?;
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
            ColumnOption::ForeignKey(fk) => {
                // A6 SQLite-parity: column-level REFERENCES is normalised
                // into a table-level FK spec so the same enforcement code
                // path handles both forms. The originating column is the
                // sole child column; parent column list defaults to the
                // parent's primary key when omitted (resolved at exec time).
                table_constraints.push(column_level_foreign_key(column_name.clone(), fk));
            }
            ColumnOption::Generated {
                generation_expr,
                generation_expr_mode,
                ..
            } => {
                // Phase-11 SQL-D A6: capture the GENERATED ALWAYS AS
                // (expr) [STORED|VIRTUAL] expression. The verbatim SQL
                // fragment is stored on the catalog ColumnDef and the
                // executor re-parses + evaluates it at INSERT (STORED)
                // or SELECT (VIRTUAL) time. SQLite defaults to VIRTUAL
                // when neither STORED nor VIRTUAL is specified.
                let expr_text = match &generation_expr {
                    Some(e) => e.to_string(),
                    None => {
                        return Err(Error::UnsupportedSql(
                            "GENERATED column requires an expression".to_owned(),
                        ));
                    }
                };
                let kind = match generation_expr_mode {
                    Some(GeneratedExpressionMode::Stored) => GeneratedColumnKind::Stored,
                    Some(GeneratedExpressionMode::Virtual) | None => GeneratedColumnKind::Virtual,
                };
                generated = Some(GeneratedColumnSpec {
                    kind,
                    expr_sql: expr_text.into_boxed_str(),
                });
            }
            ColumnOption::DialectSpecific(tokens) if is_sqlite_autoincrement_tokens(&tokens) => {}
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
        generated,
    })
}

fn is_sqlite_autoincrement_option(option: &ColumnOption) -> bool {
    match option {
        ColumnOption::DialectSpecific(tokens) => is_sqlite_autoincrement_tokens(tokens),
        _ => false,
    }
}

fn is_sqlite_autoincrement_tokens(tokens: &[sqlparser::tokenizer::Token]) -> bool {
    tokens.len() == 1 && tokens[0].to_string().eq_ignore_ascii_case("AUTOINCREMENT")
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
        sqlparser::ast::TableConstraint::ForeignKey(fk) => Ok(table_level_foreign_key(fk)),
        sqlparser::ast::TableConstraint::Index(_)
        | sqlparser::ast::TableConstraint::FulltextOrSpatial(_) => Err(Error::UnsupportedSql(
            "table constraint not supported yet".to_owned(),
        )),
    }
}

fn ref_action_to_fk(action: Option<ReferentialAction>) -> FkAction {
    match action {
        None => FkAction::NoAction,
        Some(ReferentialAction::Restrict) => FkAction::Restrict,
        Some(ReferentialAction::Cascade) => FkAction::Cascade,
        Some(ReferentialAction::SetNull) => FkAction::SetNull,
        Some(ReferentialAction::SetDefault) => FkAction::SetDefault,
        Some(ReferentialAction::NoAction) => FkAction::NoAction,
    }
}

fn fk_is_deferred(fk: &ForeignKeyConstraint) -> bool {
    matches!(
        fk.characteristics.as_ref().and_then(|c| c.initially),
        Some(DeferrableInitial::Deferred)
    )
}

fn parent_table_name(parts: &[ObjectNamePart]) -> Result<DbName> {
    match parts {
        [] => Err(Error::UnsupportedSql(
            "FOREIGN KEY references missing parent table".to_owned(),
        )),
        [ObjectNamePart::Identifier(ident)] => Ok(DbName::new(ident.value.clone())),
        [_, ObjectNamePart::Identifier(ident)] => {
            // Allow `schema.table` and drop the schema qualifier — SQLite
            // resolves FKs in the same database; A6 scope is single-DB.
            Ok(DbName::new(ident.value.clone()))
        }
        _ => Err(Error::UnsupportedSql(
            "FOREIGN KEY references unsupported parent name shape".to_owned(),
        )),
    }
}

pub(crate) fn column_level_foreign_key(
    column_name: DbName,
    fk: ForeignKeyConstraint,
) -> TableConstraintSpec {
    let parent_table = match parent_table_name(&fk.foreign_table.0) {
        Ok(name) => name,
        Err(_) => DbName::new(String::new()),
    };
    let parent_columns = fk
        .referred_columns
        .into_iter()
        .map(|ident| DbName::new(ident.value))
        .collect();
    TableConstraintSpec::ForeignKey {
        name: fk.name.map(|ident| DbName::new(ident.value)),
        columns: vec![column_name],
        parent_table,
        parent_columns,
        on_delete: ref_action_to_fk(fk.on_delete),
        on_update: ref_action_to_fk(fk.on_update),
        deferred: matches!(
            fk.characteristics.as_ref().and_then(|c| c.initially),
            Some(DeferrableInitial::Deferred)
        ),
    }
}

pub(crate) fn table_level_foreign_key(fk: ForeignKeyConstraint) -> TableConstraintSpec {
    let parent_table = match parent_table_name(&fk.foreign_table.0) {
        Ok(name) => name,
        Err(_) => DbName::new(String::new()),
    };
    let deferred = fk_is_deferred(&fk);
    let columns = fk
        .columns
        .into_iter()
        .map(|ident| DbName::new(ident.value))
        .collect();
    let parent_columns = fk
        .referred_columns
        .into_iter()
        .map(|ident| DbName::new(ident.value))
        .collect();
    TableConstraintSpec::ForeignKey {
        name: fk.name.map(|ident| DbName::new(ident.value)),
        columns,
        parent_table,
        parent_columns,
        on_delete: ref_action_to_fk(fk.on_delete),
        on_update: ref_action_to_fk(fk.on_update),
        deferred,
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
        expr_sql: None,
        expr_referenced_cols: Vec::new(),
    })
}

/// A6 SQL-D: walk an index-column expression and collect the column
/// ordinals it references. The kernel uses this to know which input
/// columns trigger a re-emit on UPDATE. Returns empty when no
/// identifiers are found (constant expressions are valid index keys
/// per SQLite but never need re-emission).
pub(crate) fn index_expr_referenced_cols(_expr: &Expr) -> Vec<u16> {
    // Conservative: until full identifier resolution against a table
    // schema is wired here, treat the referenced set as unknown by
    // returning empty. The executor re-evaluates the expression on
    // every UPDATE which is correct, just less efficient.
    Vec::new()
}

pub(crate) fn index_column_name(column: &IndexColumn) -> Result<String> {
    match &column.column.expr {
        Expr::Identifier(ident) => Ok(ident.value.clone()),
        Expr::CompoundIdentifier(parts) if parts.len() == 1 => Ok(parts[0].value.clone()),
        Expr::CompoundIdentifier(parts) => Ok(match parts.last() {
            Some(p) => p.value.clone(),
            None => return Err(Error::UnsupportedSql("empty index column".to_owned())),
        }),
        other => Err(Error::UnsupportedSql(format!(
            "unsupported index column expression: {other:?}"
        ))),
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
