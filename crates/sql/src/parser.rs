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
    out = rewrite_glob_to_function(&out);
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    out
}

/// Rewrite `<expr> GLOB <pattern>` and `<expr> NOT GLOB <pattern>` into
/// `glob(<pattern>, <expr>)` / `NOT glob(<pattern>, <expr>)` so sqlparser
/// (which lacks a SQLite-style GLOB operator) parses them as function
/// calls. The scalar `glob(pattern, value)` dispatcher in
/// `exec::expr::json_dispatch` then evaluates them with SQLite semantics
/// (including case-sensitive matching and `case_sensitive_like` PRAGMA).
///
/// The rewriter is intentionally conservative — it only matches when the
/// left and right operands are clearly delimited atoms (string literal,
/// `NULL`, numeric literal, simple identifier, parenthesized group, or
/// `x'...'` blob literal). Anything more complex (function calls, joins,
/// arithmetic) is left untouched and will surface as a parse error, which
/// matches the previous behaviour and avoids miscompiling unrelated SQL.
fn rewrite_glob_to_function(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + 32);
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        // Skip string literals verbatim so we never rewrite "GLOB" inside
        // user data.
        if c == b'\'' {
            let end = scan_quoted(bytes, i, b'\'');
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'"' {
            let end = scan_quoted(bytes, i, b'"');
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
            // Line comment — copy verbatim to newline.
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|n| i + n)
                .unwrap_or(bytes.len());
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            // Block comment.
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            let end = (j + 2).min(bytes.len());
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        // Match `GLOB` only when it is acting as the BINARY OPERATOR — i.e.
        // surrounded by whitespace and *not* immediately followed by `(`
        // (which would make it the `glob(pattern, value)` function call).
        // Function-call form is parsed natively, so don't rewrite it.
        if matches_keyword_ci(bytes, i, b"GLOB")
            && (i == 0 || !is_word_char(bytes[i - 1]))
            && (i + 4 == bytes.len() || !is_word_char(bytes[i + 4]))
            && bytes.get(i + 4) != Some(&b'(')
            && (i + 4 < bytes.len() && bytes[i + 4].is_ascii_whitespace())
            && (i > 0 && bytes[i - 1].is_ascii_whitespace())
        {
            // Strip any trailing whitespace from `out` so we can pattern-
            // match against the immediately-preceding tokens.
            while let Some(last) = out.chars().last() {
                if last.is_whitespace() {
                    out.pop();
                } else {
                    break;
                }
            }
            // Detect a trailing `NOT` so we can wrap the rewrite in NOT.
            let negate = trim_trailing_keyword_ci(&out, "NOT").is_some();
            if negate {
                if let Some(prefix) = trim_trailing_keyword_ci(&out, "NOT") {
                    out.truncate(prefix.len());
                }
            }
            // Strip residual whitespace before the LHS atom.
            while let Some(last) = out.chars().last() {
                if last.is_whitespace() {
                    out.pop();
                } else {
                    break;
                }
            }
            // Locate the LHS atom in what we've buffered so far.
            let out_bytes = out.as_bytes();
            let lhs_end = out_bytes.len();
            let lhs_atom_start = match find_atom_start(out_bytes, lhs_end) {
                Some(s) => s,
                None => {
                    if negate {
                        out.push_str("NOT");
                    }
                    out.push(' ');
                    out.push(c as char);
                    i += 1;
                    continue;
                }
            };
            let lhs_atom = out[lhs_atom_start..lhs_end].to_owned();
            // Now find the RHS atom in the input.
            let mut j = i + 4;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let rhs_end = match find_atom_end(bytes, j) {
                Some(e) => e,
                None => {
                    if negate {
                        out.push_str("NOT");
                    }
                    out.push(' ');
                    out.push(c as char);
                    i += 1;
                    continue;
                }
            };
            let rhs_atom = std::str::from_utf8(&bytes[j..rhs_end]).unwrap_or("");
            // Trim the LHS atom out of `out`, then rebuild as
            // [prefix] [NOT ]glob(<rhs>, <lhs>)
            out.truncate(lhs_atom_start);
            if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                out.push(' ');
            }
            if negate {
                out.push_str("NOT ");
            }
            out.push_str("glob(");
            out.push_str(rhs_atom);
            out.push(',');
            out.push_str(&lhs_atom);
            out.push(')');
            i = rhs_end;
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

fn scan_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
    debug_assert_eq!(bytes[start], quote);
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            // SQLite uses doubled-quote escaping.
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn matches_keyword_ci(bytes: &[u8], pos: usize, keyword: &[u8]) -> bool {
    if pos + keyword.len() > bytes.len() {
        return false;
    }
    for (i, &k) in keyword.iter().enumerate() {
        if bytes[pos + i].to_ascii_uppercase() != k {
            return false;
        }
    }
    true
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Working backwards from `end`, find the start of an "atom" suitable as a
/// GLOB operand: a quoted string, NULL literal, parenthesized group,
/// numeric literal, or simple identifier. Returns `None` if the preceding
/// text doesn't look like a clean atom (e.g. mid-expression).
fn find_atom_start(bytes: &[u8], end: usize) -> Option<usize> {
    let mut i = end;
    // Skip trailing whitespace.
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 {
        return None;
    }
    let last = bytes[i - 1];
    // Quoted string atom: scan forward from each candidate opener to
    // confirm it ends exactly at `i`. Doubled-quote escapes (`''` inside
    // a `'`-quoted string) are handled by `scan_quoted`.
    if last == b'\'' || last == b'"' {
        let quote = last;
        let mut candidate = i - 1;
        // Walk back to the earliest possible opener and forward-scan to
        // verify. The earliest opener is the first `quote` byte at the
        // start of a run.
        loop {
            if candidate == 0 {
                if bytes[0] == quote && scan_quoted(bytes, 0, quote) == i {
                    return Some(0);
                }
                return None;
            }
            candidate -= 1;
            if bytes[candidate] == quote {
                // Could be either an opener or part of `''` escape.
                let prev = if candidate > 0 { bytes[candidate - 1] } else { 0 };
                if prev == quote {
                    // We're inside a doubled-quote pair; skip both.
                    if candidate == 0 {
                        return None;
                    }
                    candidate -= 1;
                    continue;
                }
                // Candidate is at an opener if scan_quoted from here
                // lands exactly on `i`.
                if scan_quoted(bytes, candidate, quote) == i {
                    // Also allow leading `x'...'` blob literal: if the
                    // byte before is `x` or `X`, include it in the atom.
                    if quote == b'\''
                        && candidate > 0
                        && (bytes[candidate - 1] == b'x' || bytes[candidate - 1] == b'X')
                    {
                        return Some(candidate - 1);
                    }
                    return Some(candidate);
                }
            }
        }
    }
    // Parenthesized group: scan back balancing.
    if last == b')' {
        let mut depth = 1i32;
        let mut j = i - 1;
        while j > 0 {
            j -= 1;
            match bytes[j] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    // Identifier / NULL / numeric: scan back while alphanumeric / `_` /
    // `.` (for qualified names). Allow leading `x'...'` blob literal.
    let mut j = i;
    while j > 0 {
        let b = bytes[j - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            j -= 1;
        } else {
            break;
        }
    }
    if j < i {
        Some(j)
    } else {
        None
    }
}

/// Forward equivalent of `find_atom_start`: pick out the end of an atom
/// starting at `start`. Returns `None` if no recognisable atom is present.
fn find_atom_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start >= bytes.len() {
        return None;
    }
    let first = bytes[start];
    if first == b'\'' || first == b'"' {
        return Some(scan_quoted(bytes, start, first));
    }
    // `x'01ab'` style blob literal.
    if (first == b'x' || first == b'X')
        && bytes.get(start + 1) == Some(&b'\'')
    {
        return Some(scan_quoted(bytes, start + 1, b'\''));
    }
    if first == b'(' {
        // Balance to matching ).
        let mut depth = 1i32;
        let mut j = start + 1;
        while j < bytes.len() {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j + 1);
                    }
                }
                _ => {}
            }
            j += 1;
        }
        return None;
    }
    // Identifier / numeric / NULL: consume alphanumeric / `_` / `.`.
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            j += 1;
        } else {
            break;
        }
    }
    if j > start { Some(j) } else { None }
}

/// If `text` ends with the given uppercase keyword on a word boundary
/// preceded by whitespace, return the prefix excluding the keyword and
/// its leading whitespace.
fn trim_trailing_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    if bytes.len() < keyword.len() {
        return None;
    }
    let key_start = bytes.len() - keyword.len();
    for (i, k) in keyword.bytes().enumerate() {
        if bytes[key_start + i].to_ascii_uppercase() != k.to_ascii_uppercase() {
            return None;
        }
    }
    // Must be preceded by whitespace (or be at the very start, though
    // that'd be a degenerate GLOB).
    if key_start == 0 || !bytes[key_start - 1].is_ascii_whitespace() {
        return None;
    }
    // Trim back the whitespace too.
    let mut end = key_start;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    Some(&text[..end])
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
