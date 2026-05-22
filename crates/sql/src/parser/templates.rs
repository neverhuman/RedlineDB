use std::sync::Arc;

#[allow(unused_imports)]
use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec,
    CreateTriggerSpec, CreateViewSpec, DbName, DropIndexSpec, DropTableSpec, DropTriggerSpec,
    DropViewSpec, ExprAst, IndexColumnSpec, IndexOrigin, OwnedValue, QualifiedName, SchemaEpoch,
    SchemaSnapshot, SortDir, TableConstraintSpec, TriggerEventKind, TriggerTimeKind, lookup_index,
    lookup_table,
};
#[allow(unused_imports)]
use sqlparser::ast::{
    AlterTableOperation, Analyze as SqlAnalyze, AnalyzeFormat, AnalyzeFormatKind, BinaryOperator,
    ColumnDef, ColumnOption, ConflictTarget, Distinct, Expr, FunctionArg, FunctionArgExpr,
    FunctionArgumentClause, FunctionArguments, GroupByExpr, Ident, IndexColumn, JoinConstraint,
    JoinOperator, LimitClause, ObjectName, ObjectNamePart, OnConflictAction, OnInsert, OrderByExpr,
    OrderByKind, Query, SelectItem, SetExpr, SetOperator, SetQuantifier, SqliteOnConflict,
    Statement as SqlStatement, TableFactor, TableObject, TableWithJoins, UnaryOperator, Value,
    ValueWithSpan,
};
#[allow(unused_imports)]
use sqlparser::dialect::SQLiteDialect;
#[allow(unused_imports)]
use sqlparser::parser::Parser;

use crate::connection::Connection;
use crate::error::{Error, Result};
#[allow(unused_imports)]
use crate::statement::*;

use super::{ddl::*, dml::*, helpers::*, select::*};

pub(crate) fn bind_statement(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    statement: SqlStatement,
) -> Result<PreparedTemplate> {
    match statement {
        SqlStatement::Query(query) => bind_query(conn, schema, schema_epoch, sql, *query),
        SqlStatement::Insert(insert) => bind_insert(conn, schema, schema_epoch, sql, insert),
        SqlStatement::Update(update) => bind_update(schema, schema_epoch, sql, update),
        SqlStatement::Delete(delete) => bind_delete(schema, schema_epoch, sql, delete),
        SqlStatement::CreateTable(create_table) => {
            bind_create_table(conn, schema, schema_epoch, sql, create_table)
        }
        SqlStatement::CreateIndex(create_index) => {
            bind_create_index(schema_epoch, sql, create_index)
        }
        SqlStatement::Drop {
            object_type,
            if_exists,
            names,
            ..
        } => bind_drop(sql, schema_epoch, object_type, if_exists, names),
        SqlStatement::AlterTable(alter_table) => bind_alter_table(
            schema_epoch,
            sql,
            alter_table.name,
            alter_table.if_exists,
            alter_table.only,
            alter_table.operations,
        ),
        SqlStatement::Analyze(analyze) => bind_analyze(schema, schema_epoch, sql, analyze),
        SqlStatement::Explain {
            analyze,
            verbose: _,
            query_plan,
            estimate: _,
            statement,
            format,
            ..
        } => bind_explain(
            conn,
            schema,
            schema_epoch,
            sql,
            analyze,
            query_plan,
            format,
            *statement,
        ),
        SqlStatement::ExplainTable { .. } => Err(Error::UnsupportedSql(
            "EXPLAIN TABLE is not supported".to_owned(),
        )),
        SqlStatement::AttachDatabase {
            schema_name,
            database_file_name,
            ..
        } => bind_attach(sql, schema_epoch, schema_name, database_file_name),
        SqlStatement::Vacuum(vacuum) => bind_vacuum(sql, schema_epoch, vacuum),
        SqlStatement::CreateView(create_view) => bind_create_view(schema_epoch, sql, create_view),
        SqlStatement::CreateTrigger(create_trigger) => {
            bind_create_trigger(schema_epoch, sql, create_trigger)
        }
        SqlStatement::DropTrigger(drop_trigger) => {
            let name = parse_qualified_name(drop_trigger.trigger_name)?;
            Ok(template(
                sql,
                schema_epoch,
                false,
                PreparedKind::DropTrigger(DropTriggerSpec {
                    name,
                    if_exists: drop_trigger.if_exists,
                }),
            ))
        }
        other => Err(Error::UnsupportedSql(format!(
            "statement not supported yet: {other:?}"
        ))),
    }
}

pub(crate) fn parse_reindex_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Result<Option<PreparedTemplate>> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.eq_ignore_ascii_case("reindex") {
        return Ok(Some(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Reindex,
        )));
    }
    Ok(None)
}

pub(crate) fn parse_vacuum_into_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Result<Option<PreparedTemplate>> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("vacuum into ") else {
        return Ok(None);
    };
    let original_rest = &trimmed[trimmed.len() - rest.len()..];
    let path = match original_rest.trim() {
        value if value.starts_with('\'') || value.starts_with('"') => {
            let bytes = value.as_bytes();
            if bytes.len() < 2 {
                return Err(Error::UnsupportedSql(
                    "VACUUM INTO expects a database path".to_owned(),
                ));
            }
            let quote = bytes[0];
            let mut i = 1usize;
            let mut out = String::new();
            while i < bytes.len() {
                if bytes[i] == quote {
                    if i + 1 < bytes.len() && bytes[i + 1] == quote {
                        out.push(quote as char);
                        i += 2;
                        continue;
                    }
                    break;
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            out
        }
        other => other.to_owned(),
    };
    if path.is_empty() {
        return Err(Error::UnsupportedSql(
            "VACUUM INTO expects a database path".to_owned(),
        ));
    }
    Ok(Some(template(
        trimmed,
        schema_epoch,
        false,
        PreparedKind::VacuumInto {
            path: Arc::from(path),
        },
    )))
}

pub(crate) fn template(
    sql: &str,
    schema_epoch: SchemaEpoch,
    readonly: bool,
    kind: PreparedKind,
) -> PreparedTemplate {
    PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: crate::statement::ParamLayout::default(),
        output_columns: Arc::from([]),
        readonly,
        kind,
    }
}

/// Build a `PreparedTemplate` for `ATTACH DATABASE 'path' AS alias`.
/// Only literal string paths are supported (no expression evaluation at
/// prepare time).
fn bind_attach(
    sql: &str,
    schema_epoch: SchemaEpoch,
    schema_name: Ident,
    file_name: Expr,
) -> Result<PreparedTemplate> {
    let path = match file_name {
        Expr::Value(ValueWithSpan { value, .. }) => match value {
            Value::SingleQuotedString(s)
            | Value::DoubleQuotedString(s)
            | Value::EscapedStringLiteral(s) => s,
            other => {
                return Err(Error::UnsupportedSql(format!(
                    "ATTACH expects a string literal path, got {other:?}"
                )));
            }
        },
        other => {
            return Err(Error::UnsupportedSql(format!(
                "ATTACH expects a string literal path, got {other:?}"
            )));
        }
    };
    Ok(template(
        sql,
        schema_epoch,
        false,
        PreparedKind::Attach(crate::exec::attach::AttachPlan::Attach {
            path: std::path::PathBuf::from(path),
            alias: Arc::from(schema_name.value),
        }),
    ))
}

fn bind_vacuum(
    sql: &str,
    schema_epoch: SchemaEpoch,
    vacuum: sqlparser::ast::VacuumStatement,
) -> Result<PreparedTemplate> {
    if vacuum.full || vacuum.sort_only || vacuum.delete_only || vacuum.recluster || vacuum.boost {
        return Err(Error::UnsupportedSql(
            "VACUUM modifiers are not supported".to_owned(),
        ));
    }
    if vacuum.reindex {
        return Ok(template(sql, schema_epoch, false, PreparedKind::Reindex));
    }
    if let Some(table_name) = vacuum.table_name {
        return Err(Error::UnsupportedSql(format!(
            "VACUUM table target is not supported: {table_name}"
        )));
    }
    if vacuum.threshold.is_some() {
        return Err(Error::UnsupportedSql(
            "VACUUM threshold is not supported".to_owned(),
        ));
    }
    Ok(template(sql, schema_epoch, false, PreparedKind::Vacuum))
}
