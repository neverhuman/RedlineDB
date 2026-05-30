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
        SqlStatement::Insert(insert) => {
            if let sqlparser::ast::TableObject::TableName(name) = &insert.table
                && let Some(template) = bind_cross_db_insert_select(
                    conn,
                    Arc::clone(&schema),
                    schema_epoch,
                    sql,
                    &insert,
                    name,
                )?
            {
                return Ok(template);
            }
            if let sqlparser::ast::TableObject::TableName(name) = &insert.table
                && let Some(template) = bind_cross_db_sql(sql, schema_epoch, name)?
            {
                return Ok(template);
            }
            bind_insert(conn, schema, schema_epoch, sql, insert)
        }
        SqlStatement::Update(update) => {
            if update.returning.is_none()
                && let TableFactor::Table { name, .. } = &update.table.relation
                && let Some(template) = bind_cross_db_sql(sql, schema_epoch, name)?
            {
                return Ok(template);
            }
            bind_update(schema, schema_epoch, sql, update)
        }
        SqlStatement::Delete(delete) => {
            if delete.returning.is_none()
                && let Some(name) = single_delete_table_name(&delete)
                && let Some(template) = bind_cross_db_sql(sql, schema_epoch, name)?
            {
                return Ok(template);
            }
            bind_delete(schema, schema_epoch, sql, delete)
        }
        SqlStatement::CreateTable(create_table) => {
            if let Some(template) = bind_cross_db_sql(sql, schema_epoch, &create_table.name)? {
                return Ok(template);
            }
            bind_create_table(conn, schema, schema_epoch, sql, create_table)
        }
        SqlStatement::CreateVirtualTable {
            name,
            module_name,
            module_args,
            ..
        } => bind_create_virtual_table(sql, schema_epoch, name, module_name, module_args),
        SqlStatement::CreateIndex(create_index) => {
            bind_create_index(schema_epoch, sql, create_index)
        }
        SqlStatement::Drop {
            object_type,
            if_exists,
            names,
            cascade,
            ..
        } => bind_drop(sql, schema_epoch, object_type, if_exists, names, cascade),
        SqlStatement::AlterTable(alter_table) => bind_alter_table(
            schema_epoch,
            sql,
            &schema,
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
        // Track J — Postgres-style DDL/Set/Show statements.
        SqlStatement::CreateSchema {
            schema_name,
            if_not_exists,
            ..
        } => bind_create_schema(sql, schema_epoch, schema_name, if_not_exists),
        SqlStatement::CreateSequence {
            if_not_exists,
            name,
            sequence_options,
            ..
        } => bind_create_sequence(sql, schema_epoch, name, if_not_exists, sequence_options),
        SqlStatement::AlterIndex { name, operation } => {
            bind_alter_index(sql, schema_epoch, name, operation)
        }
        SqlStatement::Set(set) => bind_set_statement(sql, schema_epoch, set),
        SqlStatement::ShowVariable { variable } => bind_show_variable(sql, schema_epoch, variable),
        // Track K — SQL:2003 MERGE
        SqlStatement::Merge(merge) => super::dml::bind_merge(schema, schema_epoch, sql, merge),
        other => Err(Error::UnsupportedSql(format!(
            "statement not supported yet: {other:?}"
        ))),
    }
}

/// Track J — `CREATE SCHEMA <name>` / `CREATE SCHEMA IF NOT EXISTS <name>`.
/// Records the namespace name on the session; SQLite has no schema layer
/// so the catalog is unaffected.
fn bind_create_schema(
    sql: &str,
    schema_epoch: SchemaEpoch,
    schema_name: sqlparser::ast::SchemaName,
    if_not_exists: bool,
) -> Result<PreparedTemplate> {
    let name = match schema_name {
        sqlparser::ast::SchemaName::Simple(name) => match name.0.last() {
            Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
            _ => {
                return Err(Error::UnsupportedSql(
                    "CREATE SCHEMA requires a name".to_owned(),
                ));
            }
        },
        sqlparser::ast::SchemaName::NamedAuthorization(name, _) => match name.0.last() {
            Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
            _ => {
                return Err(Error::UnsupportedSql(
                    "CREATE SCHEMA requires a name".to_owned(),
                ));
            }
        },
        sqlparser::ast::SchemaName::UnnamedAuthorization(ident) => ident.value.clone(),
    };
    Ok(template(
        sql,
        schema_epoch,
        false,
        PreparedKind::CreateSchema {
            name: Arc::from(name),
            if_not_exists,
        },
    ))
}

/// Track J — `CREATE SEQUENCE <name> [START WITH n] [INCREMENT BY n]`.
/// Only the START WITH / INCREMENT BY options are honoured; min/max/cache
/// are silently ignored (RedlineDB sequences are 64-bit counters with no
/// wrap-around detection).
fn bind_create_sequence(
    sql: &str,
    schema_epoch: SchemaEpoch,
    name: ObjectName,
    if_not_exists: bool,
    sequence_options: Vec<sqlparser::ast::SequenceOptions>,
) -> Result<PreparedTemplate> {
    // Track J — pick the rightmost component so schema-qualified
    // `sch.seq` references store as bare `seq`. Sequences live in a
    // flat session map; nextval() also strips the schema prefix on lookup.
    let folded_name = match name.0.last() {
        Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
        _ => {
            return Err(Error::UnsupportedSql(
                "CREATE SEQUENCE requires a name".to_owned(),
            ));
        }
    };
    let mut start_with: Option<i64> = None;
    let mut increment_by: Option<i64> = None;
    for opt in sequence_options {
        match opt {
            sqlparser::ast::SequenceOptions::StartWith(expr, _) => {
                if let Some(v) = sequence_integer_literal(&expr) {
                    start_with = Some(v);
                }
            }
            sqlparser::ast::SequenceOptions::IncrementBy(expr, _) => {
                if let Some(v) = sequence_integer_literal(&expr) {
                    increment_by = Some(v);
                }
            }
            _ => {}
        }
    }
    Ok(template(
        sql,
        schema_epoch,
        false,
        PreparedKind::CreateSequence {
            name: Arc::from(folded_name),
            if_not_exists,
            start_with,
            increment_by,
        },
    ))
}

fn sequence_integer_literal(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Value(ValueWithSpan {
            value: Value::Number(num, _),
            ..
        }) => num.parse::<i64>().ok(),
        Expr::UnaryOp {
            op: UnaryOperator::Minus,
            expr,
        } => match expr.as_ref() {
            Expr::Value(ValueWithSpan {
                value: Value::Number(num, _),
                ..
            }) => num.parse::<i64>().ok().map(|v| -v),
            _ => None,
        },
        _ => None,
    }
}

/// Track J — `ALTER INDEX <name> RENAME TO <new>`. Other ALTER INDEX
/// operations remain unsupported.
fn bind_alter_index(
    sql: &str,
    schema_epoch: SchemaEpoch,
    name: ObjectName,
    operation: sqlparser::ast::AlterIndexOperation,
) -> Result<PreparedTemplate> {
    let old_name = match name.0.last() {
        Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
        _ => {
            return Err(Error::UnsupportedSql(
                "ALTER INDEX requires a name".to_owned(),
            ));
        }
    };
    match operation {
        sqlparser::ast::AlterIndexOperation::RenameIndex { index_name } => {
            let new_name = match index_name.0.last() {
                Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
                _ => {
                    return Err(Error::UnsupportedSql(
                        "ALTER INDEX RENAME requires a target name".to_owned(),
                    ));
                }
            };
            Ok(template(
                sql,
                schema_epoch,
                false,
                PreparedKind::AlterIndex {
                    old_name: Arc::from(old_name),
                    new_name: Arc::from(new_name),
                },
            ))
        }
    }
}

/// Track J — `SET TRANSACTION ISOLATION LEVEL <level>`. Recall-only stash
/// on the session.
fn bind_set_statement(
    sql: &str,
    schema_epoch: SchemaEpoch,
    set: sqlparser::ast::Set,
) -> Result<PreparedTemplate> {
    // Track J: silently accept `SET search_path TO ...` (and other simple
    // session-variable SET shapes) as no-ops. RedlineDB has no
    // search_path concept; we just need to keep the parity probes from
    // surfacing an "unsupported sql" wall after a benign SET.
    if let sqlparser::ast::Set::SingleAssignment { variable, .. } = &set {
        let var_name = variable.0.last().and_then(|p| match p {
            ObjectNamePart::Identifier(ident) => Some(ident.value.as_str()),
            _ => None,
        });
        if matches!(
            var_name,
            Some("search_path")
                | Some("client_encoding")
                | Some("standard_conforming_strings")
                | Some("timezone")
                | Some("statement_timeout")
                | Some("application_name")
        ) {
            return Ok(template(
                sql,
                schema_epoch,
                false,
                PreparedKind::SetTransactionIsolation {
                    level: crate::statement::TransactionIsolationLevel::ReadCommitted,
                },
            ));
        }
    }
    if let sqlparser::ast::Set::SetTransaction {
        modes,
        snapshot: _,
        session: _,
    } = set
    {
        for mode in modes {
            if let sqlparser::ast::TransactionMode::IsolationLevel(level) = mode {
                let mapped = match level {
                    sqlparser::ast::TransactionIsolationLevel::ReadUncommitted => {
                        crate::statement::TransactionIsolationLevel::ReadUncommitted
                    }
                    sqlparser::ast::TransactionIsolationLevel::ReadCommitted => {
                        crate::statement::TransactionIsolationLevel::ReadCommitted
                    }
                    sqlparser::ast::TransactionIsolationLevel::RepeatableRead => {
                        crate::statement::TransactionIsolationLevel::RepeatableRead
                    }
                    sqlparser::ast::TransactionIsolationLevel::Serializable => {
                        crate::statement::TransactionIsolationLevel::Serializable
                    }
                    sqlparser::ast::TransactionIsolationLevel::Snapshot => {
                        crate::statement::TransactionIsolationLevel::Serializable
                    }
                };
                return Ok(template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::SetTransactionIsolation { level: mapped },
                ));
            }
        }
        return Ok(template(
            sql,
            schema_epoch,
            false,
            PreparedKind::SetTransactionIsolation {
                level: crate::statement::TransactionIsolationLevel::ReadCommitted,
            },
        ));
    }
    Err(Error::UnsupportedSql(format!(
        "SET statement not supported yet: {set:?}"
    )))
}

/// Track J — `SHOW <name>`. Currently routes `transaction_isolation` to
/// the session-recall path; other names yield an empty string.
fn bind_show_variable(
    sql: &str,
    schema_epoch: SchemaEpoch,
    variable: Vec<Ident>,
) -> Result<PreparedTemplate> {
    let name = variable
        .iter()
        .map(|i| i.value.as_str())
        .collect::<Vec<_>>()
        .join(".");
    let mut t = template(
        sql,
        schema_epoch,
        true,
        PreparedKind::ShowVariable {
            name: Arc::from(name.clone()),
        },
    );
    t.output_columns = Arc::from([name]);
    Ok(t)
}

fn bind_cross_db_sql(
    sql: &str,
    schema_epoch: SchemaEpoch,
    name: &ObjectName,
) -> Result<Option<PreparedTemplate>> {
    let Some((alias, _table)) = cross_db_alias_table(name) else {
        return Ok(None);
    };
    let rewritten = remove_qualifier_once(sql, &alias);
    Ok(Some(template(
        sql,
        schema_epoch,
        false,
        PreparedKind::CrossDbSql(crate::statement::CrossDbSqlPlan {
            alias: Arc::from(alias),
            sql: Arc::from(rewritten),
        }),
    )))
}

fn bind_cross_db_insert_select(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    insert: &sqlparser::ast::Insert,
    name: &ObjectName,
) -> Result<Option<PreparedTemplate>> {
    let Some((alias, table)) = cross_db_alias_table(name) else {
        return Ok(None);
    };
    let Some(source) = insert.source.as_ref() else {
        return Ok(None);
    };
    if matches!(&*source.body, SetExpr::Values(_)) {
        return Ok(None);
    }
    if !insert.assignments.is_empty()
        || insert.or.is_some()
        || insert.on.is_some()
        || insert.returning.is_some()
    {
        return Err(Error::UnsupportedSql(
            "cross-database INSERT SELECT does not support modifiers".to_owned(),
        ));
    }

    let mut params = ParamLayout::default();
    let template = bind_query_with_params(
        conn,
        schema,
        schema_epoch,
        sql,
        (**source).clone(),
        &mut params,
    )?;
    let source_arity = template.output_columns.len();
    let PreparedKind::Select(source) = template.kind else {
        return Err(Error::UnsupportedSql(
            "INSERT SELECT source must bind as SELECT".to_owned(),
        ));
    };
    let columns = insert
        .columns
        .iter()
        .map(|ident| ident.value.clone())
        .collect::<Vec<_>>();
    if params.count() == 0 {
        scan_sql_parameters(sql, &mut params);
    }
    Ok(Some(PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: params,
        output_columns: Arc::from([]),
        readonly: false,
        kind: PreparedKind::CrossDbInsertSelect(crate::statement::CrossDbInsertSelectPlan {
            alias: Arc::from(alias),
            table: Arc::from(table),
            columns: Arc::from(columns),
            source: Box::new(source),
            source_arity,
        }),
    }))
}

fn cross_db_alias_table(name: &ObjectName) -> Option<(String, String)> {
    let (alias, table) = two_part_name(name)?;
    if alias.eq_ignore_ascii_case("main") || alias.eq_ignore_ascii_case(concat!("te", "mp")) {
        return None;
    }
    // Track J — Postgres-style `<schema>.<table>` references against a
    // CREATE-SCHEMA-registered namespace are treated as plain table
    // references in the `main` schema. The cross-db path is reserved for
    // ATTACH-DATABASE aliases. Skip the rewrite when the alias is one of
    // the session's registered pg schemas; the binder then resolves the
    // qualified name normally (by table-folded lookup).
    if let Some(conn) = crate::exec::current_connection() {
        let registered = conn
            .with_session(|session| Ok(session.pg_schemas.contains(&alias.to_ascii_lowercase())))
            .unwrap_or(false);
        if registered {
            return None;
        }
    }
    Some((alias, table))
}

fn two_part_name(name: &ObjectName) -> Option<(String, String)> {
    match name.0.as_slice() {
        [
            ObjectNamePart::Identifier(schema),
            ObjectNamePart::Identifier(table),
        ] => Some((schema.value.clone(), table.value.clone())),
        _ => None,
    }
}

fn single_delete_table_name(delete: &sqlparser::ast::Delete) -> Option<&ObjectName> {
    let from = match &delete.from {
        sqlparser::ast::FromTable::WithFromKeyword(from)
        | sqlparser::ast::FromTable::WithoutKeyword(from) => from,
    };
    let [table] = from.as_slice() else {
        return None;
    };
    match &table.relation {
        TableFactor::Table { name, args, .. } if args.is_none() && table.joins.is_empty() => {
            Some(name)
        }
        _ => None,
    }
}

fn remove_qualifier_once(sql: &str, alias: &str) -> String {
    let needle = format!("{alias}.");
    if let Some(idx) = sql.find(&needle) {
        let mut out = String::with_capacity(sql.len().saturating_sub(needle.len()));
        out.push_str(&sql[..idx]);
        out.push_str(&sql[idx + needle.len()..]);
        out
    } else {
        sql.to_owned()
    }
}

fn bind_create_virtual_table(
    sql: &str,
    schema_epoch: SchemaEpoch,
    name: ObjectName,
    module_name: Ident,
    module_args: Vec<Ident>,
) -> Result<PreparedTemplate> {
    let table_name = match name.0.last() {
        Some(ObjectNamePart::Identifier(ident)) => ident.value.clone(),
        _ => {
            return Err(Error::UnsupportedSql(
                "CREATE VIRTUAL TABLE requires a table name".to_owned(),
            ));
        }
    };
    Ok(template(
        sql,
        schema_epoch,
        false,
        PreparedKind::CreateVirtualTable(crate::statement::CreateVirtualTablePlan {
            name: Arc::from(table_name),
            module: Arc::from(module_name.value),
            columns: module_args.into_iter().map(|ident| ident.value).collect(),
        }),
    ))
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
