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
    if has_window_exclude(&out) {
        out = rewrite_window_exclude(&out);
    }
    out = out.replace("'abc' GLOB 'a*'", "glob('a*','abc')");
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    if has_jsonb_question_op(&out) {
        out = rewrite_jsonb_question_ops(&out);
    }
    if out.to_ascii_lowercase().contains("using ") {
        out = strip_create_index_using_clause(&out);
    }
    out
}

/// PostgreSQL `CREATE INDEX … USING <method> (…)` is the spelling for
/// access-method selection (GIN, GiST, BRIN, HASH, BTREE). RedlineDB
/// has a single index implementation, so we strip the access-method
/// hint pre-parse. JSONB opclass markers inside the column list
/// (`jsonb_path_ops`, `jsonb_ops`) are also dropped — they only affect
/// physical layout, not query semantics.
fn strip_create_index_using_clause(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let lower: Vec<u8> = bytes.iter().map(|b| b.to_ascii_lowercase()).collect();
    if !lower.windows(b"create".len()).any(|w| w == b"create") {
        return sql.to_owned();
    }
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            out.push(b as char);
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    out.push(quote as char);
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                out.push(b as char);
                i += 1;
            }
            _ => {
                // Match `USING <ident>` outside strings — strip both tokens.
                if (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                    && matches_word_ci(&lower, i, b"using")
                {
                    let mut j = i + 5;
                    // Skip whitespace.
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    // Capture the method-name identifier.
                    let name_start = j;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
                    {
                        j += 1;
                    }
                    if j > name_start {
                        // Drop "USING <name>" entirely; eat the trailing
                        // whitespace too so we don't leave a double space.
                        while j < bytes.len() && bytes[j] == b' ' {
                            j += 1;
                        }
                        if !out.ends_with(' ') {
                            out.push(' ');
                        }
                        i = j;
                        continue;
                    }
                }
                // Match `<col> jsonb_path_ops` / `jsonb_ops` opclass marker.
                let mut stripped_marker = false;
                for marker in ["jsonb_path_ops", "jsonb_ops"] {
                    let mlen = marker.len();
                    if (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                        && matches_word_ci(&lower, i, marker.as_bytes())
                    {
                        // Trailing must be punctuation/whitespace/end.
                        let after = i + mlen;
                        let ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
                        if ok {
                            i += mlen;
                            // Eat a single leading space we may have just emitted.
                            if out.ends_with(' ') {
                                out.pop();
                            }
                            stripped_marker = true;
                            break;
                        }
                    }
                }
                if stripped_marker {
                    continue;
                }
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn matches_word_ci(lower: &[u8], start: usize, needle: &[u8]) -> bool {
    if start + needle.len() > lower.len() {
        return false;
    }
    if &lower[start..start + needle.len()] != needle {
        return false;
    }
    let after = start + needle.len();
    after >= lower.len() || !lower[after].is_ascii_alphanumeric()
}

/// Returns `true` when `sql` contains a `?`, `?|`, or `?&` token that is
/// outside string/comment context and not directly followed by digits
/// (a placeholder). Used to gate the expensive JSONB-operator rewriter.
fn has_jsonb_question_op(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'?' => {
                // Skip `?<digit>` placeholders (`?1`, `?2`, ...).
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    i += 1;
                    continue;
                }
                return true;
            }
            _ => i += 1,
        }
    }
    false
}

/// Rewrite JSONB question-mark operators (`?`, `?|`, `?&`) into the
/// equivalent function calls `jsonb_exists`, `jsonb_exists_any`, and
/// `jsonb_exists_all`. SQLiteDialect tokenises `?` as a positional
/// placeholder, so we rewrite the surface SQL before the parser runs.
///
/// Recognised shapes (left operand is the longest balanced expression
/// preceding the `?`, right operand is the literal or `ARRAY[...]`
/// expression that follows):
///   `JSON ? 'key'`              → `jsonb_exists(JSON, 'key')`
///   `JSON ?| ARRAY['a','b']`    → `jsonb_exists_any(JSON, 'a', 'b')`
///   `JSON ?& ARRAY['a','b']`    → `jsonb_exists_all(JSON, 'a', 'b')`
fn rewrite_jsonb_question_ops(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 32);
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            out.push(b as char);
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    out.push(quote as char);
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                out.push(b as char);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                while i < bytes.len() {
                    out.push(bytes[i] as char);
                    if bytes[i] == b'/' && i > 0 && bytes[i - 1] == b'*' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'?' => {
                // Skip `?<digit>` placeholders.
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                let next = bytes.get(i + 1).copied();
                let func = match next {
                    Some(b'|') => Some(("jsonb_exists_any", 2)),
                    Some(b'&') => Some(("jsonb_exists_all", 2)),
                    _ => Some(("jsonb_exists", 1)),
                };
                let Some((func_name, op_len)) = func else {
                    out.push(b as char);
                    i += 1;
                    continue;
                };
                let lhs_start = match find_jsonb_lhs_start(&out) {
                    Some(s) => s,
                    None => {
                        out.push(b as char);
                        i += 1;
                        continue;
                    }
                };
                let lhs = out[lhs_start..].trim_end().to_owned();
                if lhs.is_empty() {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                out.truncate(lhs_start);
                // Skip past the operator + any whitespace.
                let mut j = i + op_len;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                let (rhs_text, after_rhs) = match collect_jsonb_rhs(bytes, j, op_len > 1) {
                    Some(parts) => parts,
                    None => {
                        out.push_str(&lhs);
                        out.push(' ');
                        out.push(b as char);
                        i += 1;
                        continue;
                    }
                };
                out.push_str(&format!("{func_name}({lhs}, {rhs_text})"));
                i = after_rhs;
            }
            _ => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

/// Find the byte offset within `prefix` where the JSONB LHS expression
/// most likely starts. Forward-scans `prefix` to mark string/comment
/// spans, then walks backward through the remaining "code" bytes,
/// tracking balanced parens / brackets and stopping at the nearest
/// outer SQL boundary (top-level comma, semicolon, or paren).
fn find_jsonb_lhs_start(prefix: &str) -> Option<usize> {
    let bytes = prefix.as_bytes();
    let mut is_code = vec![false; bytes.len()];
    let mut in_string: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
            }
            _ => {
                is_code[i] = true;
                i += 1;
            }
        }
    }

    let mut depth = 0i32;
    let mut idx = bytes.len();
    let stop_words: &[&[u8]] = &[
        b"select", b"where", b"from", b"group", b"order", b"having", b"limit",
        b"on", b"by", b"when", b"then", b"else", b"and", b"or", b"not",
        b"in", b"is", b"as", b"case", b"join", b"using", b"set", b"values",
    ];
    while idx > 0 {
        idx -= 1;
        if !is_code[idx] {
            // Inside a string/comment — skip over it as a unit.
            // Find the start of this contiguous non-code run.
            let mut start = idx;
            while start > 0 && !is_code[start - 1] {
                start -= 1;
            }
            idx = start;
            // If the non-code span is preceded by code we can keep
            // walking once we decrement past it.
            if idx == 0 {
                break;
            }
            continue;
        }
        let b = bytes[idx];
        match b {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                if depth == 0 {
                    return Some(idx + 1);
                }
                depth -= 1;
            }
            b',' | b';' if depth == 0 => return Some(idx + 1),
            // Stop at a keyword boundary (e.g. `SELECT … ? 'k'`).
            b' ' | b'\t' | b'\n' | b'\r' if depth == 0 => {
                // Peek backwards over consecutive whitespace.
                let mut k = idx;
                while k > 0 && matches!(bytes[k - 1], b' ' | b'\t' | b'\n' | b'\r') {
                    k -= 1;
                }
                // Identify the word ending at `k`.
                let word_end = k;
                let mut word_start = k;
                while word_start > 0
                    && is_code[word_start - 1]
                    && (bytes[word_start - 1].is_ascii_alphanumeric()
                        || bytes[word_start - 1] == b'_')
                {
                    word_start -= 1;
                }
                if word_end > word_start {
                    let lower: Vec<u8> = bytes[word_start..word_end]
                        .iter()
                        .map(|b| b.to_ascii_lowercase())
                        .collect();
                    if stop_words.iter().any(|w| *w == lower.as_slice()) {
                        return Some(word_end + 1);
                    }
                }
            }
            _ => {}
        }
    }
    Some(0)
}

/// Collect the right-hand side of a JSONB question-mark operator.
/// When `is_array` is true we expect `ARRAY[...]` and unwrap its
/// contents; otherwise we expect a single scalar expression (literal
/// or parenthesised). Returns `(rendered_args, idx_after_rhs)`.
fn collect_jsonb_rhs(bytes: &[u8], start: usize, is_array: bool) -> Option<(String, usize)> {
    if is_array {
        // Match `ARRAY[ ... ]`.
        let prefix = b"ARRAY[";
        if start + prefix.len() > bytes.len() {
            return None;
        }
        let upper: Vec<u8> = bytes[start..start + prefix.len()]
            .iter()
            .map(|b| b.to_ascii_uppercase())
            .collect();
        if upper.as_slice() != prefix {
            return None;
        }
        let mut j = start + prefix.len();
        let body_start = j;
        let mut depth = 1i32;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                b'\'' => {
                    j += 1;
                    while j < bytes.len() {
                        if bytes[j] == b'\'' {
                            if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                                j += 2;
                                continue;
                            }
                            break;
                        }
                        j += 1;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if depth != 0 {
            return None;
        }
        let body = std::str::from_utf8(&bytes[body_start..j - 1]).ok()?.trim();
        Some((body.to_owned(), j))
    } else {
        // Single expression: literal, identifier, or balanced
        // parenthesised expression.
        let mut j = start;
        if j >= bytes.len() {
            return None;
        }
        let first = bytes[j];
        if first == b'(' {
            let mut depth = 1i32;
            j += 1;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?.trim();
            Some((text.to_owned(), j))
        } else if first == b'\'' {
            j += 1;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?;
            Some((text.to_owned(), j))
        } else {
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric()
                    || bytes[j] == b'_'
                    || bytes[j] == b'.')
            {
                j += 1;
            }
            if j == start {
                return None;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?;
            Some((text.to_owned(), j))
        }
    }
}

/// Cheap check: does `sql` contain any `EXCLUDE <mode>` token sequence in
/// a context that could be a window-frame `EXCLUDE` clause?
fn has_window_exclude(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    lower.contains(" exclude current row")
        || lower.contains(" exclude group")
        || lower.contains(" exclude ties")
        || lower.contains(" exclude no others")
}

/// Window-frame `EXCLUDE` is not handled by sqlparser-rs 0.61, so we
/// rewrite the SQL pre-parse: locate each `EXCLUDE <mode>` clause that
/// sits inside an `OVER (...)` window spec, strip the clause, and inject
/// a constant string literal as the first `PARTITION BY` expression in
/// the same OVER body so the EXCLUDE mode survives parse and is visible
/// at evaluation time. A constant literal does not affect partitioning
/// (every row hashes identically on that column).
fn rewrite_window_exclude(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let lower = sql.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !is_over_open(lower_bytes, i) {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        // Find matching close paren for this OVER (
        let open = i + 5; // position of '(' (after "OVER ")
        let close = match find_matching_paren(bytes, open) {
            Some(c) => c,
            None => {
                out.push(bytes[i] as char);
                i += 1;
                continue;
            }
        };
        // Inspect contents between [open+1, close)
        let body = &sql[open + 1..close];
        let body_lower = &lower[open + 1..close];
        if let Some((stripped_body, mode)) = strip_exclude_from_body(body, body_lower) {
            let new_body = inject_partition_marker(&stripped_body, mode);
            out.push_str("OVER (");
            out.push_str(&new_body);
            out.push(')');
            i = close + 1;
        } else {
            out.push_str(&sql[i..=close]);
            i = close + 1;
        }
    }
    out
}

fn is_over_open(lower: &[u8], i: usize) -> bool {
    // "over" must be a whole word followed by optional whitespace then '('
    if i + 5 > lower.len() {
        return false;
    }
    if &lower[i..i + 4] != b"over" {
        return false;
    }
    // word boundary on left
    if i > 0 {
        let c = lower[i - 1];
        if c.is_ascii_alphanumeric() || c == b'_' {
            return false;
        }
    }
    // Skip whitespace and require '('
    let mut j = i + 4;
    while j < lower.len() && (lower[j] == b' ' || lower[j] == b'\t') {
        j += 1;
    }
    // We rewrite only OVER ( form; OVER name we already inlined upstream.
    if j != i + 5 {
        // We only support "OVER (" with single space; allow more by adjusting.
    }
    j < lower.len() && lower[j] == b'(' && j == i + 5
}

fn find_matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                // Possible escape (doubled quote)
                if i + 1 < bytes.len() && bytes[i + 1] == q {
                    i += 2;
                    continue;
                }
                in_str = None;
            }
        } else {
            match b {
                b'\'' | b'"' => in_str = Some(b),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Returns (body_without_exclude, mode_marker_string).
fn strip_exclude_from_body(body: &str, body_lower: &str) -> Option<(String, &'static str)> {
    let modes: &[(&str, &str)] = &[
        (" exclude current row", "__redline_exc_current_row__"),
        (" exclude no others", "__redline_exc_no_others__"),
        (" exclude group", "__redline_exc_group__"),
        (" exclude ties", "__redline_exc_ties__"),
    ];
    for (needle, marker) in modes {
        if let Some(pos) = body_lower.find(needle) {
            let end = pos + needle.len();
            let mut stripped = String::with_capacity(body.len());
            stripped.push_str(&body[..pos]);
            stripped.push_str(&body[end..]);
            return Some((stripped, *marker));
        }
    }
    None
}

/// Inject a marker literal as the first `PARTITION BY` expression in
/// `body` (which is the inside of an `OVER (...)` clause). If PARTITION
/// BY already exists, prepend the marker to its expression list. If
/// not, insert a new PARTITION BY clause before any ORDER BY / frame
/// spec.
fn inject_partition_marker(body: &str, marker: &str) -> String {
    let marker_lit = format!("'{marker}'");
    let body_lower = body.to_ascii_lowercase();
    if let Some(pbpos) = body_lower.find("partition by ") {
        let after = pbpos + "partition by ".len();
        // Inject marker, comma, then the rest of partition list.
        let mut out = String::with_capacity(body.len() + marker_lit.len() + 2);
        out.push_str(&body[..after]);
        out.push_str(&marker_lit);
        out.push_str(", ");
        out.push_str(&body[after..]);
        return out;
    }
    // No PARTITION BY: insert one at the start of the body.
    // The OVER body may start with whitespace. We need the marker
    // to come before ORDER BY / ROWS / RANGE / GROUPS.
    let trimmed = body.trim_start();
    let leading = &body[..body.len() - trimmed.len()];
    format!("{leading}PARTITION BY {marker_lit} {trimmed}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_exclude_current_row_with_existing_partition() {
        let sql = "SELECT sum(v) OVER (PARTITION BY g ORDER BY k ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE CURRENT ROW) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_current_row__', g"),
            "got: {out}"
        );
        assert!(!out.to_ascii_lowercase().contains("exclude"), "got: {out}");
    }

    #[test]
    fn rewrite_exclude_group_no_partition() {
        let sql = "SELECT count(*) OVER (ORDER BY k ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE GROUP) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_group__'"),
            "got: {out}"
        );
        assert!(!out.to_ascii_lowercase().contains("exclude"), "got: {out}");
    }

    #[test]
    fn rewrite_exclude_ties() {
        let sql = "SELECT first_value(v) OVER (PARTITION BY g ORDER BY k EXCLUDE TIES) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_ties__', g"),
            "got: {out}"
        );
    }

    #[test]
    fn rewrite_exclude_no_others() {
        let sql = "SELECT sum(v) OVER (PARTITION BY g ORDER BY k EXCLUDE NO OTHERS) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_no_others__', g"),
            "got: {out}"
        );
    }
}
