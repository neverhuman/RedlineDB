use std::sync::Arc;

#[allow(unused_imports)]
use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
    DropIndexSpec, DropTableSpec, ExprAst, IndexColumnSpec, IndexOrigin, OwnedValue, QualifiedName,
    SchemaEpoch, SchemaSnapshot, SortDir, TableConstraintSpec, lookup_index, lookup_table,
};
#[allow(unused_imports)]
use sqlparser::ast::{
    AlterTableOperation, Analyze as SqlAnalyze, AnalyzeFormat, AnalyzeFormatKind, BinaryOperator,
    ColumnDef, ColumnOption, ConflictTarget, Distinct, Expr, FunctionArg, FunctionArgExpr,
    FunctionArguments, GroupByExpr, Ident, IndexColumn, JoinConstraint, JoinOperator, LimitClause,
    ObjectName, ObjectNamePart, OnConflictAction, OnInsert, OrderByExpr, OrderByKind, Query,
    SelectItem, SetExpr, SetOperator, SetQuantifier, SqliteOnConflict, Statement as SqlStatement,
    TableFactor, TableObject, TableWithJoins, UnaryOperator, Value, ValueWithSpan,
};
#[allow(unused_imports)]
use sqlparser::dialect::SQLiteDialect;
#[allow(unused_imports)]
use sqlparser::parser::Parser;

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::session::BeginMode;
#[allow(unused_imports)]
use crate::statement::*;
use crate::value::SqlValue;

mod helpers;
#[allow(unused_imports)]
pub(crate) use helpers::*;
mod ddl;
#[allow(unused_imports)]
pub(crate) use ddl::*;
mod dml;
#[allow(unused_imports)]
pub(crate) use dml::*;
mod pragma;
#[allow(unused_imports)]
pub(crate) use pragma::*;
pub(crate) mod savepoint;
mod select;
#[allow(unused_imports)]
pub(crate) use select::*;

pub(crate) fn is_pragma_sql(sql: &str) -> bool {
    sql.trim_start()
        .trim_end_matches(';')
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("pragma")
}

/// Split `sql` into `(head, tail)` where `head` is the first statement
/// (including its trailing `;` if present) and `tail` is the remainder of the
/// input. `tail` is a byte slice of the original `sql`, with no leading
/// whitespace stripped — this matches SQLite's `pzTail` contract where the
/// tail pointer must reference into the caller's original buffer.
///
/// The split is "string-aware": semicolons inside `'...'`, `"..."`, or
/// `[...]` (SQLite legacy bracket quoting) and inside `--` line comments or
/// `/* ... */` block comments are not considered terminators. Doubled quote
/// characters inside a string are treated as escapes.
///
/// If `sql` contains no terminating semicolon, the entire string is the
/// `head` and `tail` is empty. If `sql` is purely whitespace/comments, both
/// `head` and `tail` are returned trimmed appropriately.
pub fn split_first_statement(sql: &str) -> (&str, &str) {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let len = bytes.len();
    let mut in_string: Option<u8> = None;
    while i < len {
        let b = bytes[i];
        if let Some(quote) = in_string {
            // Inside a string literal: handle escaped quote (doubled quote).
            if b == quote {
                if i + 1 < len && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
                i += 1;
                continue;
            }
            // SQLite-style `[...]` legacy bracket quoting closes on `]`.
            if quote == b'[' && b == b']' {
                in_string = None;
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_string = Some(b);
                i += 1;
            }
            b'[' => {
                in_string = Some(b'[');
                i += 1;
            }
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                // Line comment until \n or EOF.
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                // Block comment until */
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
            }
            b';' => {
                let head_end = i + 1;
                return (&sql[..head_end], &sql[head_end..]);
            }
            _ => {
                i += 1;
            }
        }
    }
    (sql, "")
}

/// True if `sql` (after trimming whitespace and stripping comments) is empty.
/// Used to detect SQL that is entirely a comment block — `sqlite3_prepare_v2`
/// treats such input as a successful no-op (`out_stmt` becomes NULL).
pub fn is_blank_sql(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let len = bytes.len();
    while i < len {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b';' {
            i += 1;
            continue;
        }
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            continue;
        }
        return false;
    }
    true
}

/// Iterate over every top-level statement in `sql`, yielding `(head, tail)`
/// pairs analogous to repeatedly calling `split_first_statement`. Skips runs
/// of pure-whitespace/comment chunks. The yielded `head` slice is the SQL of
/// one statement (including its trailing `;` if any) and `tail` is the
/// remainder of the input after that statement.
///
/// Used by tests and by callers that want the full split up-front; runtime
/// `Connection::execute` walks the splitter incrementally so it can stop at
/// the first failing statement.
#[allow(dead_code)]
pub fn split_statements(sql: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = sql;
    while !rest.is_empty() {
        if is_blank_sql(rest) {
            break;
        }
        let (head, tail) = split_first_statement(rest);
        if head.is_empty() {
            break;
        }
        if !is_blank_sql(head) {
            out.push(head);
        }
        rest = tail;
    }
    out
}

pub fn parse_prepared_template(conn: &Connection, sql: &str) -> Result<PreparedTemplate> {
    let trimmed = sql.trim();
    let lower = trimmed.trim_end_matches(';').trim().to_ascii_lowercase();
    let engine = conn.engine();
    let schema = engine.schema_snapshot();
    let schema_epoch = engine.schema_epoch();

    if lower == "begin" || lower == "begin deferred" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Deferred),
        ));
    }
    if lower == "begin immediate" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Immediate),
        ));
    }
    if lower == "begin exclusive" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Exclusive),
        ));
    }
    if lower == "commit" {
        return Ok(template(trimmed, schema_epoch, false, PreparedKind::Commit));
    }
    if lower == "rollback" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Rollback,
        ));
    }

    if let Some(template) = parse_pragma_template(conn, trimmed, &lower, schema_epoch, &schema)? {
        return Ok(template);
    }

    let dialect = SQLiteDialect {};
    let mut statements = Parser::parse_sql(&dialect, sql)?;
    if statements.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-statement prepares are supported".to_owned(),
        ));
    }

    bind_statement(conn, schema, schema_epoch, trimmed, statements.remove(0))
}

fn bind_statement(
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
            bind_create_table(schema_epoch, sql, create_table)
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
        SqlStatement::CreateView(_) => Err(Error::UnsupportedSql(
            "CREATE VIEW is parsed-only; execution not yet implemented".to_owned(),
        )),
        SqlStatement::CreateTrigger(_) => Err(Error::UnsupportedSql(
            "CREATE TRIGGER is parsed-only; execution not yet implemented".to_owned(),
        )),
        other => Err(Error::UnsupportedSql(format!(
            "statement not supported yet: {other:?}"
        ))),
    }
}

fn template(
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
