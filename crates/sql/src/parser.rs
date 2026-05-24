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
    if out.to_ascii_lowercase().contains(" on conflict") {
        out = wrap_insert_select_with_upsert(&out);
        out = rewrite_on_conflict_clauses(&out);
    }
    out = rewrite_glob_to_function(&out);
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    if has_jsonb_question_op(&out) {
        out = rewrite_jsonb_question_ops(&out);
    }
    if out.to_ascii_lowercase().contains("using ") {
        out = strip_create_index_using_clause(&out);
    }
    out = rewrite_strict_without_rowid_combo(&out);
    out
}

/// sqlparser-rs 0.61 chokes on `INSERT INTO t SELECT ... ON CONFLICT ...`
/// because the unwrapped SELECT body cannot be terminated by an ON
/// CONFLICT keyword. Wrap the SELECT body in parens so the parser
/// recognises it as a parenthesised SELECT source followed by the
/// ON CONFLICT trailer.
fn wrap_insert_select_with_upsert(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    // Find each top-level "insert into" occurrence
    let mut out = sql.to_owned();
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find("insert into ") {
        let insert_pos = search_from + rel;
        // Find the SELECT keyword that follows (not inside subquery)
        let after_insert = insert_pos + "insert into ".len();
        // Skip table name and optional columns list.
        let bytes_full = out.as_bytes();
        let mut j = after_insert;
        // Skip table identifier (possibly schema.table)
        while j < bytes_full.len() && bytes_full[j].is_ascii_whitespace() {
            j += 1;
        }
        while j < bytes_full.len()
            && (bytes_full[j].is_ascii_alphanumeric()
                || bytes_full[j] == b'_'
                || bytes_full[j] == b'.')
        {
            j += 1;
        }
        while j < bytes_full.len() && bytes_full[j].is_ascii_whitespace() {
            j += 1;
        }
        // Optional column list (col, col, ...)
        if j < bytes_full.len() && bytes_full[j] == b'(' {
            if let Some(close) = find_matching_paren(bytes_full, j) {
                j = close + 1;
            }
            while j < bytes_full.len() && bytes_full[j].is_ascii_whitespace() {
                j += 1;
            }
        }
        // Now expect SELECT (or VALUES / DEFAULT VALUES)
        let lower_full = out.to_ascii_lowercase();
        if j + 7 <= lower_full.len() && &lower_full[j..j + 6] == "select" {
            // Find matching ON CONFLICT after the select body (top-level)
            if let Some(on_pos) = find_top_level_on_conflict(&lower_full, bytes_full, j + 6) {
                // Wrap [j..on_pos] in parens
                // Insert ')' at on_pos
                out.insert(on_pos, ')');
                // Insert '(' at j
                out.insert(j, '(');
                // Move search_from past this rewrite
                search_from = on_pos + 2; // +2 for the inserted parens
                continue;
            }
        }
        search_from = j;
    }
    out
}

fn find_top_level_on_conflict(lower: &str, bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_str = Some(b);
                i += 1;
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => return None,
            _ => {}
        }
        if depth == 0
            && i + 12 <= lower.len()
            && &lower[i..i + 12] == " on conflict"
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// SQLite's `ON CONFLICT(<col> [COLLATE name]) [WHERE <pred>] DO ...`
/// is not handled by sqlparser-rs 0.61. Rewrite pre-parse:
///   * Strip `COLLATE <name>` from each column inside the conflict
///     target list (the index targets are resolved by column name and
///     by partial-index predicate inside the kernel).
///   * Strip the optional `WHERE <pred>` that follows the target and
///     precedes `DO` — this is purely an index-disambiguation hint.
///   * Collapse multiple `ON CONFLICT(...) DO ...` clauses into a
///     single clause by keeping the first `DO UPDATE` (or, if all
///     clauses are `DO NOTHING`, keeping the first).
fn rewrite_on_conflict_clauses(sql: &str) -> String {
    let mut buf = sql.to_owned();
    // Collect all `ON CONFLICT(...) [WHERE ...] DO {NOTHING|UPDATE ...}`
    // segments. For each segment we record byte range and the action
    // type so we can choose which one wins under multiple-clauses.
    let segments = collect_on_conflict_segments(&buf);
    if segments.is_empty() {
        return buf;
    }
    // For each segment, strip WHERE-between-target-and-DO and strip
    // COLLATE inside the target column list. Apply in reverse so
    // earlier offsets remain valid.
    let mut rewrites: Vec<(usize, usize, String)> = Vec::new();
    for seg in &segments {
        let original = &buf[seg.start..seg.end];
        let cleaned = strip_on_conflict_extras(original);
        if cleaned != original {
            rewrites.push((seg.start, seg.end, cleaned));
        }
    }
    for (start, end, new) in rewrites.into_iter().rev() {
        buf.replace_range(start..end, &new);
    }
    // If multiple ON CONFLICT clauses remain back-to-back, collapse them.
    let mut segs = collect_on_conflict_segments(&buf);
    if segs.len() <= 1 {
        return buf;
    }
    // Find consecutive runs where multiple segments touch (only whitespace
    // separates them) — these are SQLite's chained ON CONFLICT clauses.
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = vec![0];
    for i in 1..segs.len() {
        let prev_end = segs[i - 1].end;
        let this_start = segs[i].start;
        let gap = &buf[prev_end..this_start];
        if gap.trim().is_empty() {
            current.push(i);
        } else {
            runs.push(std::mem::take(&mut current));
            current.push(i);
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    // For each run with >= 2 segments, keep the first DO UPDATE if any,
    // otherwise the last clause. Strip the rest.
    let mut deletions: Vec<(usize, usize)> = Vec::new();
    for run in runs.iter().filter(|r| r.len() >= 2) {
        let mut keep_idx: Option<usize> = None;
        for &idx in run {
            if segs[idx].is_update {
                keep_idx = Some(idx);
                break;
            }
        }
        let keep_idx = keep_idx.unwrap_or_else(|| *run.last().unwrap());
        for &idx in run {
            if idx != keep_idx {
                deletions.push((segs[idx].start, segs[idx].end));
            }
        }
    }
    deletions.sort_by(|a, b| b.0.cmp(&a.0));
    for (s, e) in deletions {
        buf.replace_range(s..e, "");
    }
    // Recompute segs after deletions (no longer needed; just return).
    let _ = &mut segs;
    buf
}

#[derive(Debug)]
struct OnConflictSegment {
    start: usize,
    end: usize,
    is_update: bool,
}

/// Locate every ` ON CONFLICT(...) [WHERE ...] DO {NOTHING|UPDATE ...}`
/// chunk in `sql`. Whitespace before `ON` is included in `start` so
/// chained clauses can be merged cleanly.
fn collect_on_conflict_segments(sql: &str) -> Vec<OnConflictSegment> {
    let lower = sql.to_ascii_lowercase();
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let Some(idx) = find_keyword(&lower, " on conflict", i) else {
            break;
        };
        let kw_start = idx + 1; // skip the leading space
        // Skip "on conflict"
        let mut j = idx + " on conflict".len();
        // Optional target: '(' ... ')'
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'(' {
            j = match find_matching_paren(bytes, j) {
                Some(end) => end + 1,
                None => {
                    i = j + 1;
                    continue;
                }
            };
        } else if j + 13 <= lower.len() && &lower[j..j + 13] == "on constraint" {
            // ON CONFLICT ON CONSTRAINT name - skip "on constraint" and a name token.
            j += 13;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            // skip identifier (or quoted name)
            if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                let q = bytes[j];
                j += 1;
                while j < bytes.len() && bytes[j] != q {
                    j += 1;
                }
                if j < bytes.len() {
                    j += 1;
                }
            } else {
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
            }
        }
        // Optional WHERE <pred> before DO
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j + 6 <= lower.len() && &lower[j..j + 6] == "where " {
            j += 6;
            j = skip_until_keyword(&lower, bytes, j, " do ");
        }
        // Required: DO
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j + 3 > lower.len() || &lower[j..j + 3] != "do " {
            // Not a valid ON CONFLICT — advance and continue.
            i = j;
            continue;
        }
        j += 3;
        let is_update = j + 6 <= lower.len() && &lower[j..j + 6] == "update";
        // End of segment = end of the action body. For DO NOTHING it's
        // just past "nothing". For DO UPDATE SET ... [WHERE ...] we need
        // to scan to the next clause boundary (another ON CONFLICT, RETURNING, ;, or end).
        let end = if is_update {
            // Find the next clause boundary.
            j += 6; // past "update"
            scan_to_clause_boundary(&lower, bytes, j)
        } else {
            // DO NOTHING
            if j + 7 <= lower.len() && &lower[j..j + 7] == "nothing" {
                j + 7
            } else {
                j
            }
        };
        out.push(OnConflictSegment {
            start: kw_start,
            end,
            is_update,
        });
        i = end;
    }
    out
}

fn find_keyword(lower: &str, kw: &str, from: usize) -> Option<usize> {
    if from >= lower.len() {
        return None;
    }
    lower[from..].find(kw).map(|p| from + p)
}

fn skip_until_keyword(lower: &str, bytes: &[u8], from: usize, kw: &str) -> usize {
    let mut j = from;
    while j < bytes.len() {
        if j + kw.len() <= lower.len() && &lower[j..j + kw.len()] == kw {
            return j;
        }
        j += 1;
    }
    j
}

fn scan_to_clause_boundary(lower: &str, bytes: &[u8], from: usize) -> usize {
    let mut j = from;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while j < bytes.len() {
        let b = bytes[j];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            j += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_str = Some(b);
                j += 1;
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => return j,
            _ => {}
        }
        if depth == 0 {
            if j + 12 <= lower.len() && &lower[j..j + 12] == " on conflict" {
                return j;
            }
            if j + 11 <= lower.len() && &lower[j..j + 11] == " returning " {
                return j;
            }
        }
        j += 1;
    }
    j
}

fn strip_on_conflict_extras(segment: &str) -> String {
    // Strip COLLATE <name> inside the target column list.
    let mut out = segment.to_owned();
    let bytes = out.as_bytes();
    if let Some(open) = bytes.iter().position(|&b| b == b'(')
        && let Some(close) = find_matching_paren(bytes, open)
    {
        let inner = &out[open + 1..close];
        let cleaned = strip_collate_clauses(inner);
        if cleaned != inner {
            out.replace_range(open + 1..close, &cleaned);
        }
    }
    // Strip ' WHERE <pred>' that sits between the target and ' DO '.
    let lower = out.to_ascii_lowercase();
    if let Some(target_close) = out.find(')') {
        let after = &lower[target_close + 1..];
        if let Some(rel_where) = after.find(" where ") {
            let abs_where_start = target_close + 1 + rel_where;
            // Find " do " after that
            if let Some(rel_do) = lower[abs_where_start..].find(" do ") {
                let abs_do = abs_where_start + rel_do;
                out.replace_range(abs_where_start..abs_do, "");
            }
        }
    }
    out
}

fn strip_collate_clauses(inner: &str) -> String {
    // Strip " COLLATE <ident>" matches (case-insensitive).
    let lower = inner.to_ascii_lowercase();
    let bytes = inner.as_bytes();
    let mut out = String::with_capacity(inner.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 9 <= lower.len() && &lower[i..i + 9] == " collate " {
            // Skip " collate "
            let mut j = i + 9;
            // Skip the collation name
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
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



fn rewrite_strict_without_rowid_combo(sql: &str) -> String {
    let mut s = sql.to_owned();
    for pat in [
        (", STRICT", " STRICT"),
        (",STRICT", " STRICT"),
        (", strict", " strict"),
        (",strict", " strict"),
        (", WITHOUT ROWID", " WITHOUT ROWID"),
        (",WITHOUT ROWID", " WITHOUT ROWID"),
        (", without rowid", " without rowid"),
        (",without rowid", " without rowid"),
    ] {
        s = s.replace(pat.0, pat.1);
    }
    s
}
