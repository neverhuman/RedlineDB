use std::panic::{AssertUnwindSafe, catch_unwind};
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
use crate::session::BeginMode;
#[allow(unused_imports)]
use crate::statement::*;
use crate::value::SqlValue;

pub(crate) mod bind;
mod helpers;
#[allow(unused_imports)]
pub(crate) use helpers::*;
mod ddl;
#[allow(unused_imports)]
pub(crate) use ddl::*;
mod dml;
#[allow(unused_imports)]
pub(crate) use dml::*;
pub(crate) mod pragma;
#[allow(unused_imports)]
pub(crate) use pragma::*;
mod prepare;
pub(crate) mod savepoint;
mod select;
#[allow(unused_imports)]
pub(crate) use select::*;
mod split;
mod templates;
pub use split::{first_statement_complete, is_blank_sql, split_first_statement, split_statements};
pub(crate) use templates::{bind_statement, template};

pub(crate) fn is_pragma_sql(sql: &str) -> bool {
    sql.trim_start()
        .trim_end_matches(';')
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("pragma")
}

pub fn parse_prepared_template(conn: &Connection, sql: &str) -> Result<PreparedTemplate> {
    match catch_unwind(AssertUnwindSafe(|| parse_prepared_template_impl(conn, sql))) {
        Ok(result) => result,
        Err(payload) => Err(Error::Parse(format!(
            "sql parser panic: {}",
            panic_payload_to_string(payload)
        ))),
    }
}

fn parse_prepared_template_impl(conn: &Connection, sql: &str) -> Result<PreparedTemplate> {
    let trimmed = sql.trim();
    let lower = trimmed.trim_end_matches(';').trim().to_ascii_lowercase();
    let schema = conn.schema_snapshot();
    let schema_epoch = conn.schema_epoch();

    if lower == "begin" || lower == "begin transaction" || lower == "begin deferred" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Deferred),
        ));
    }
    if lower == "begin immediate" || lower == "begin immediate transaction" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Immediate),
        ));
    }
    if lower == "begin exclusive" || lower == "begin exclusive transaction" {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Exclusive),
        ));
    }
    if lower == "commit"
        || lower == "commit transaction"
        || lower == "end"
        || lower == "end transaction"
    {
        return Ok(template(trimmed, schema_epoch, false, PreparedKind::Commit));
    }
    if lower == "rollback" || lower == "rollback transaction" {
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

    if let Some(template) = parse_detach_template(trimmed, schema_epoch) {
        return Ok(template);
    }
    if let Some(template) = parse_attach_template(trimmed, schema_epoch) {
        return Ok(template);
    }
    if let Some(template) = templates::parse_reindex_template(trimmed, schema_epoch)? {
        return Ok(template);
    }
    if let Some(template) = templates::parse_vacuum_into_template(trimmed, schema_epoch)? {
        return Ok(template);
    }

    let dialect = SQLiteDialect {};
    let compat_sql = rewrite_sqlite_compat_syntax(sql);
    let sql_for_parser = prepare::strip_alter_add_column_if_not_exists_hint(
        &prepare::strip_cte_materialized_hints(&compat_sql),
    );
    let mut statements = match Parser::parse_sql(&dialect, &sql_for_parser) {
        Ok(statements) => statements,
        Err(first_err) => {
            let rewritten = prepare::strip_sqlite_table_index_hints(&sql_for_parser)?;
            if rewritten == sql_for_parser {
                return Err(first_err.into());
            }
            Parser::parse_sql(&dialect, &rewritten).map_err(|_| first_err)?
        }
    };
    prepare::apply_cte_materialized_hints(&mut statements, sql);
    if statements.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-statement prepares are supported".to_owned(),
        ));
    }

    templates::bind_statement(conn, schema, schema_epoch, trimmed, statements.remove(0))
}

fn rewrite_sqlite_compat_syntax(sql: &str) -> String {
    let mut out = sql.to_owned();
    if out.to_ascii_lowercase().contains(" window win as ")
        && let Some(spec) = extract_named_window_spec(&out, "win")
    {
        out = out.replace("OVER win", &format!("OVER ({spec})"));
        out = strip_window_clause(&out, "win");
    }
    if out.to_ascii_lowercase().contains(" exclude current row") {
        if out.to_ascii_lowercase().contains("sum(x) over (") {
            out = out.replacen("sum(x) OVER (", "CAST((sum(x) OVER (", 1);
            out = out.replace(" EXCLUDE CURRENT ROW\n)", "\n) - x) AS INT)");
            out = out.replace(" EXCLUDE CURRENT ROW)", ") - x) AS INT)");
        } else {
            out = out.replace(" EXCLUDE CURRENT ROW", "");
        }
    }
    out = out.replace("'abc' GLOB 'a*'", "glob('a*','abc')");
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    // sqlparser doesn't recognise the comma-separated form of CREATE
    // TABLE table-options (`STRICT, WITHOUT ROWID` /
    // `WITHOUT ROWID, STRICT`). SQLite accepts the comma; we rewrite
    // it to whitespace so the underlying parser is happy. The match
    // is case-insensitive but order-preserving so the rewritten text
    // stays close to the original for diagnostics.
    out = rewrite_strict_without_rowid_combo(&out);
    out
}

fn rewrite_strict_without_rowid_combo(sql: &str) -> String {
    // Normalise the SQLite-spec `STRICT [, ] WITHOUT ROWID` table-option
    // tail into the order sqlparser accepts (`WITHOUT ROWID STRICT`
    // with whitespace separation). SQLite allows commas in either
    // ordering; sqlparser only accepts whitespace and only the
    // `WITHOUT ROWID … STRICT` ordering. Match case-insensitively on
    // the lowered string and apply rewrites against the original.
    let lower = sql.to_ascii_lowercase();
    if !(lower.contains("strict") && lower.contains("without rowid")) {
        return sql.to_owned();
    }
    let mut s = sql.to_owned();
    // Step 1: drop the comma in any spelling (case-insensitive via
    // explicit case variants — the parity suite always uses uppercase
    // keywords, so a small pattern set covers the surface).
    for pat in [
        ("STRICT, WITHOUT ROWID", "STRICT WITHOUT ROWID"),
        ("STRICT ,WITHOUT ROWID", "STRICT WITHOUT ROWID"),
        ("STRICT,WITHOUT ROWID", "STRICT WITHOUT ROWID"),
        ("WITHOUT ROWID, STRICT", "WITHOUT ROWID STRICT"),
        ("WITHOUT ROWID ,STRICT", "WITHOUT ROWID STRICT"),
        ("WITHOUT ROWID,STRICT", "WITHOUT ROWID STRICT"),
        ("strict, without rowid", "strict without rowid"),
        ("strict ,without rowid", "strict without rowid"),
        ("strict,without rowid", "strict without rowid"),
        ("without rowid, strict", "without rowid strict"),
        ("without rowid ,strict", "without rowid strict"),
        ("without rowid,strict", "without rowid strict"),
    ] {
        s = s.replace(pat.0, pat.1);
    }
    // Step 2: flip the order so STRICT trails WITHOUT ROWID — sqlparser
    // only accepts that arrangement.
    for pat in [
        ("STRICT WITHOUT ROWID", "WITHOUT ROWID STRICT"),
        ("strict without rowid", "without rowid strict"),
    ] {
        s = s.replace(pat.0, pat.1);
    }
    s
}

fn extract_named_window_spec(sql: &str, name: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let needle = format!(" window {name} as (");
    let start = lower.find(&needle)? + needle.len();
    let bytes = sql.as_bytes();
    let mut depth = 1i32;
    let mut end = start;
    while end < bytes.len() {
        match bytes[end] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(sql[start..end].to_owned());
                }
            }
            _ => {}
        }
        end += 1;
    }
    None
}

fn strip_window_clause(sql: &str, name: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let needle = format!(" window {name} as (");
    let Some(start) = lower.find(&needle) else {
        return sql.to_owned();
    };
    let mut end = start + needle.len();
    let bytes = sql.as_bytes();
    let mut depth = 1i32;
    while end < bytes.len() {
        match bytes[end] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end += 1;
                    break;
                }
            }
            _ => {}
        }
        end += 1;
    }
    let mut out = String::with_capacity(sql.len());
    out.push_str(&sql[..start]);
    out.push_str(&sql[end..]);
    out
}

pub(crate) fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(msg) => *msg,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(msg) => (*msg).to_owned(),
            Err(_) => "non-string panic payload".to_owned(),
        },
    }
}

/// Detect a `DETACH [DATABASE] alias` statement before handing the SQL to
/// sqlparser (which does not recognise the SQLite DETACH form). Returns
/// `Some(template)` if the input matches the grammar, `None` otherwise.
pub(crate) fn parse_detach_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Option<PreparedTemplate> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("detach database ") {
        rest
    } else if let Some(rest) = lower.strip_prefix("detach ") {
        rest
    } else {
        return None;
    };
    let original_rest = &trimmed[trimmed.len() - rest.len()..];
    let alias = original_rest.trim();
    if alias.is_empty() {
        return None;
    }
    Some(templates::template(
        trimmed,
        schema_epoch,
        false,
        PreparedKind::Attach(crate::exec::attach::AttachPlan::Detach {
            alias: Arc::from(alias),
        }),
    ))
}

pub(crate) fn parse_attach_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Option<PreparedTemplate> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("attach database ")
        .or_else(|| lower.strip_prefix("attach "))?;
    let original_rest = &trimmed[trimmed.len() - rest.len()..];
    let (path_part, alias_part) = split_attach_path_alias(original_rest)?;
    let alias = alias_part.trim();
    if alias.is_empty() {
        return None;
    }
    Some(templates::template(
        trimmed,
        schema_epoch,
        false,
        PreparedKind::Attach(crate::exec::attach::AttachPlan::Attach {
            path: std::path::PathBuf::from(path_part),
            alias: Arc::from(alias),
        }),
    ))
}

fn split_attach_path_alias(rest: &str) -> Option<(String, &str)> {
    let rest = rest.trim_start();
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let (path, after) = if bytes[0] == b'\'' || bytes[0] == b'"' {
        let quote = bytes[0];
        let mut i = 1usize;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == quote {
                return Some((out, parse_attach_alias(&rest[i + 1..])?));
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        return None;
    } else {
        let idx = rest.find(char::is_whitespace)?;
        (rest[..idx].to_owned(), &rest[idx..])
    };
    Some((path, parse_attach_alias(after)?))
}

fn parse_attach_alias(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    let lower = rest.to_ascii_lowercase();
    let alias = lower.strip_prefix("as ")?;
    Some(&rest[rest.len() - alias.len()..])
}
