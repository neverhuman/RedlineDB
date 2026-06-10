use super::super::{dml_order_limit_rewrite_enabled, templates};
use super::shared::*;
use super::sqlite_tail::{
    has_jsonb_question_op, rewrite_glob_to_function, rewrite_jsonb_question_ops,
    rewrite_strict_without_rowid_combo, strip_create_index_using_clause,
};
use super::{
    has_pg_array_literal, has_pg_bytea_literal, has_postfix_index, is_identifier_char,
    rewrite_alter_column_drop_identity, rewrite_array_agg_function, rewrite_array_length_function,
    rewrite_at_time_zone, rewrite_create_sequence_options_order, rewrite_date_arith_with_modifier,
    rewrite_dml_order_limit_to_subquery, rewrite_grouping_sets_to_union_all,
    rewrite_join_lateral_to_subquery, rewrite_overriding_system_value, rewrite_pg_array_literal,
    rewrite_pg_array_overlap, rewrite_pg_bytea_literal, rewrite_pg_interval_literal,
    rewrite_postfix_index, rewrite_rollup_cube_to_grouping_sets,
};
use crate::statement::{PreparedKind, PreparedTemplate};
use redlinedb_kernel::catalog::SchemaEpoch;
use std::sync::Arc;

pub(crate) fn rewrite_sqlite_compat_syntax(sql: &str) -> String {
    let mut out = sql.to_owned();
    if contains_ignore_ascii_case(&out, b" window win as ")
        && let Some(spec) = extract_named_window_spec(&out, "win")
    {
        out = out.replace("OVER win", &format!("OVER ({spec})"));
        out = strip_window_clause(&out, "win");
    }
    if has_window_exclude(&out) {
        out = rewrite_window_exclude(&out);
    }
    if contains_on_conflict_clause(&out) {
        out = wrap_insert_select_with_upsert(&out);
        out = rewrite_on_conflict_clauses(&out);
    }
    out = rewrite_glob_to_function(&out);
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    if has_jsonb_question_op(&out) {
        out = rewrite_jsonb_question_ops(&out);
    }
    if contains_ignore_ascii_case(&out, b"using ") {
        out = strip_create_index_using_clause(&out);
    }
    out = rewrite_strict_without_rowid_combo(&out);
    // Track J — beyond-Postgres parity pre-parse rewrites: sequence option
    // order + DROP IDENTITY + OVERRIDING SYSTEM VALUE shapes that
    // sqlparser 0.61 rejects.
    out = rewrite_create_sequence_options_order(&out);
    out = rewrite_alter_column_drop_identity(&out);
    out = rewrite_overriding_system_value(&out);
    // Track H — beyond-SQLite (Postgres parity) pre-parse rewrites. Each
    // helper is a no-op unless the surface SQL contains the corresponding
    // PG token; the SELECT/DDL flow is otherwise unaffected for ordinary
    // SQLite-style inputs.
    //
    // Note: `@>` and `<@` are already wired into the BinaryOperator layer
    // by Track F (jsonb containment), so JSON-array operands work without
    // a parser rewrite. We only need to rewrite the parts SQLite/sqlparser
    // can't accept at all: ARRAY[...] literals, &&, and the bytea hex
    // literal syntax.
    if has_pg_array_literal(&out) {
        out = rewrite_pg_array_literal(&out);
    }
    if has_pg_bytea_literal(&out) {
        out = rewrite_pg_bytea_literal(&out);
    }
    if contains_ignore_ascii_case(&out, b"array_length(") {
        out = rewrite_array_length_function(&out);
    }
    if contains_ignore_ascii_case(&out, b"array_agg(") {
        out = rewrite_array_agg_function(&out);
    }
    if out.contains("&&") {
        out = rewrite_pg_array_overlap(&out);
    }
    // PG's 1-based array indexing `(ARRAY['a','b'])[1]` — we run this AFTER
    // the ARRAY-literal rewrite turned the bracket pair into `json_array(...)`.
    // The bracketed-postfix form `EXPR[N]` becomes `json_extract(EXPR, '$[N-1]')`.
    if has_postfix_index(&out) {
        out = rewrite_postfix_index(&out);
    }
    // `EXPR AT TIME ZONE 'TZ'` — RedlineDB stores all timestamps as tz-naive
    // UTC, so we drop the trailing `AT TIME ZONE 'TZ'` clause. The downstream
    // parse_timestring helper now strips a trailing `+HH[:MM]` offset from
    // the literal itself, so the round-trip is correct for UTC inputs.
    if contains_ignore_ascii_case(&out, b"at time zone") {
        out = rewrite_at_time_zone(&out);
    }
    // `INTERVAL 'N units'` literal → SQLite-style `'+N units'` text. This
    // turns the `date + INTERVAL '5 days'` shape into `date + '+5 days'`
    // which we then rewrite below into `datetime(date, '+5 days')`.
    if contains_ignore_ascii_case(&out, b"interval ") {
        out = rewrite_pg_interval_literal(&out);
    }
    // `date + 'modifier'` / `date - 'modifier'` (where 'modifier' is a
    // `[+-]N (days|months|years|hours|minutes|seconds)` SQLite-style
    // string) becomes `datetime(date, 'modifier')`. We run this after the
    // INTERVAL rewrite so PG intervals flow into the SQLite datetime path.
    if out.contains("'+") || out.contains("'-") {
        out = rewrite_date_arith_with_modifier(&out);
    }
    // Track K — `SELECT ... INTO table_name [FROM ...]` is the PG-standard
    // form of `CREATE TABLE table_name AS SELECT ... [FROM ...]`. Rewrite
    // pre-parse so the existing CTAS path handles it.
    if contains_ignore_ascii_case(&out, b" into ") {
        out = rewrite_select_into_to_ctas(&out);
    }
    // Track K — PG `GROUP BY ROLLUP (...)` and `GROUP BY CUBE (...)` are
    // syntactic sugar for `GROUP BY GROUPING SETS (...)` with a
    // hierarchical (rollup) or combinatorial (cube) expansion. Lower
    // both to the canonical GROUPING SETS form so the next pass handles
    // them uniformly.
    if contains_ignore_ascii_case(&out, b" group by rollup ")
        || contains_ignore_ascii_case(&out, b" group by cube ")
    {
        out = rewrite_rollup_cube_to_grouping_sets(&out);
    }
    // After ROLLUP/CUBE → GROUPING SETS, expand the GROUPING SETS form
    // itself into N parallel SELECTs combined via UNION ALL. The expansion
    // re-uses the surrounding SELECT body (FROM, WHERE) per grouping set
    // and projects NULL for any non-grouped grouping-key column.
    if contains_ignore_ascii_case(&out, b" group by grouping sets ") {
        out = rewrite_grouping_sets_to_union_all(&out);
    }
    // Track K — `[CROSS|LEFT] JOIN LATERAL (SELECT ...) [AS alias]` is a
    // per-row subquery against the preceding FROM items. We rewrite the
    // two patterns the beyond-portability cases exercise into scalar
    // correlated subqueries promoted to the SELECT projection:
    //   * CROSS JOIN LATERAL (SELECT EXPR AS NAME)  -> inline EXPR
    //   * LEFT  JOIN LATERAL (<one-column query>) ON TRUE -> scalar
    //     correlated subquery
    // The lateral relation reference (`alias.col`) is replaced by the
    // inlined / scalar form; the lateral FROM term is dropped.
    if contains_ignore_ascii_case(&out, b" join lateral ")
        || contains_ignore_ascii_case(&out, b",lateral ")
    {
        out = rewrite_join_lateral_to_subquery(&out);
    }
    // WS-A2f-rewrite: opt-in lowering of `DELETE/UPDATE ... [WHERE ...]
    // [ORDER BY ...] LIMIT n [OFFSET m]` into the rowid-IN-subquery form.
    // Gated by `PRAGMA redline_dml_order_limit_rewrite=ON` so the default
    // surface keeps mirroring the SQLite autoconf amalgamation's rejection
    // (parity case 00220).
    if dml_order_limit_rewrite_enabled()
        && contains_ignore_ascii_case(&out, b"limit ")
        && (contains_ignore_ascii_case(&out, b"delete ")
            || contains_ignore_ascii_case(&out, b"update "))
    {
        out = rewrite_dml_order_limit_to_subquery(&out);
    }
    out
}

/// Track K — Rewrite `SELECT projection INTO table_name [FROM ...]` into
/// `CREATE TABLE table_name AS SELECT projection [FROM ...]`. Conservative:
/// only triggers when SELECT is the leading token of a statement (top-level
/// SELECT) and only handles the simple `INTO <unquoted-ident>` form. The
/// `FROM` clause (if any) is preserved verbatim. Other `INTO` usages
/// (INSERT INTO, MERGE INTO, plpgsql) are left untouched.
pub(crate) fn rewrite_select_into_to_ctas(sql: &str) -> String {
    // Tokenize at statement boundaries (semicolons) to handle multi-statement
    // input. Each statement is rewritten in isolation.
    let mut out = String::with_capacity(sql.len() + 16);
    for (idx, stmt) in split_top_level_statements(sql).into_iter().enumerate() {
        if idx > 0 {
            out.push(';');
        }
        out.push_str(&rewrite_select_into_in_statement(&stmt));
    }
    out
}

pub(crate) fn split_top_level_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
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
            b'\'' | b'"' => in_str = Some(b),
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => {
                out.push(sql[start..i].to_owned());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < bytes.len() {
        out.push(sql[start..].to_owned());
    }
    out
}

pub(crate) fn rewrite_select_into_in_statement(stmt: &str) -> String {
    let trimmed = stmt.trim_start();
    if !trimmed.to_ascii_uppercase().starts_with("SELECT ") {
        return stmt.to_owned();
    }
    let leading_ws = &stmt[..stmt.len() - trimmed.len()];
    let upper = trimmed.to_ascii_uppercase();
    // Find top-level " INTO " (not inside parens/strings).
    let into_at = find_top_level_keyword(&upper, trimmed.as_bytes(), 0, " INTO ");
    let Some(into_pos) = into_at else {
        return stmt.to_owned();
    };
    // SELECT body is `trimmed[7..into_pos]` (after "SELECT "); but it's
    // simpler to keep the original projection (between "SELECT" and " INTO ").
    let after_into = into_pos + " INTO ".len();
    // Find table name: identifier up to next whitespace, ';', or top-level
    // keyword (FROM/WHERE/...).
    let rest = &trimmed[after_into..];
    let upper_rest = &upper[after_into..];
    let mut name_end = 0usize;
    for (idx, ch) in rest.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            name_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if name_end == 0 {
        return stmt.to_owned();
    }
    let name = &rest[..name_end];
    let after_name = &rest[name_end..];
    let after_name_upper = &upper_rest[name_end..];
    // The simple bare form (no TEMP/TABLE qualifiers between the keyword
    // pair and the name) is all we lower; PG-specific variants are left
    // for the parser to reject.
    //
    // The candidate text up to the keyword boundary becomes the body of
    // a new CTAS wrapper; the tail (post-name) is appended verbatim.
    let projection = &trimmed[..into_pos];
    let _ = after_name_upper;
    format!("{leading_ws}CREATE TABLE {name} AS {projection}{after_name}")
}

pub(crate) fn find_top_level_keyword(
    upper: &str,
    bytes: &[u8],
    from: usize,
    kw: &str,
) -> Option<usize> {
    let mut i = from;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while i + kw.len() <= bytes.len() {
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
            _ => {}
        }
        if depth == 0 && &upper[i..i + kw.len()] == kw {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// sqlparser-rs 0.61 chokes on `INSERT INTO t SELECT ... ON CONFLICT ...`
/// because the unwrapped SELECT body cannot be terminated by an ON
/// CONFLICT keyword. Wrap the SELECT body in parens so the parser
/// recognises it as a parenthesised SELECT source followed by the
/// ON CONFLICT trailer.
pub(crate) fn wrap_insert_select_with_upsert(sql: &str) -> String {
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

pub(crate) fn find_top_level_on_conflict(lower: &str, bytes: &[u8], from: usize) -> Option<usize> {
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
        if depth == 0 && on_conflict_keyword_at(lower, bytes, i).is_some() {
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
///   * Collapse chained `ON CONFLICT(...) DO ...` clauses only in the
///     SQL text fed to sqlparser. The DML binder reconstructs the ordered
///     arms from the original statement text so runtime conflict handling
///     still follows SQLite's first-matching-arm semantics.
pub(crate) fn rewrite_on_conflict_clauses(sql: &str) -> String {
    let mut buf = sql.to_owned();
    // Collect all `ON CONFLICT(...) [WHERE ...] DO {NOTHING|UPDATE ...}`
    // segments. The rewritten SQL must keep one parser-compatible arm,
    // but semantic arm ordering is recovered later from the original SQL.
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
        if sql_gap_is_trivia(gap) {
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
    // otherwise the last clause. Strip the rest from parser input only.
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
pub(crate) struct OnConflictSegment {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) is_update: bool,
}

/// Locate every `ON CONFLICT(...) [WHERE ...] DO {NOTHING|UPDATE ...}`
/// chunk in `sql`. Trivia before `ON` stays outside the segment so
/// adjacent arms can be merged by checking the gap between segments.
pub(crate) fn collect_on_conflict_segments(sql: &str) -> Vec<OnConflictSegment> {
    let lower = sql.to_ascii_lowercase();
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let Some((kw_start, after_conflict)) = find_on_conflict_keyword(&lower, bytes, i) else {
            break;
        };
        let mut j = after_conflict;
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

pub(crate) fn contains_on_conflict_clause(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    find_on_conflict_keyword(&lower, sql.as_bytes(), 0).is_some()
}

pub(crate) fn find_on_conflict_keyword(
    lower: &str,
    bytes: &[u8],
    from: usize,
) -> Option<(usize, usize)> {
    let mut i = from;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                if q == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == q {
                    i += 2;
                    continue;
                }
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
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
                continue;
            }
            _ => {}
        }
        if let Some(end) = on_conflict_keyword_at(lower, bytes, i) {
            return Some((i, end));
        }
        i += 1;
    }
    None
}

pub(crate) fn on_conflict_keyword_at(lower: &str, bytes: &[u8], at: usize) -> Option<usize> {
    if !keyword_at_boundary(lower, bytes, at, "on") {
        return None;
    }
    let after_on = at + "on".len();
    if let Some(conflict_at) = skip_sql_trivia(bytes, after_on)
        && conflict_at > after_on
        && keyword_at_boundary(lower, bytes, conflict_at, "conflict")
    {
        return Some(conflict_at + "conflict".len());
    }
    None
}

pub(crate) fn skip_until_keyword(lower: &str, bytes: &[u8], from: usize, kw: &str) -> usize {
    let mut j = from;
    while j < bytes.len() {
        if j + kw.len() <= lower.len() && &lower[j..j + kw.len()] == kw {
            return j;
        }
        j += 1;
    }
    j
}

pub(crate) fn scan_to_clause_boundary(lower: &str, bytes: &[u8], from: usize) -> usize {
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
            if on_conflict_keyword_at(lower, bytes, j).is_some() {
                return j;
            }
            if keyword_at_boundary(lower, bytes, j, "returning") {
                return j;
            }
        }
        j += 1;
    }
    j
}

pub(crate) fn keyword_at_boundary(_lower: &str, bytes: &[u8], at: usize, keyword: &str) -> bool {
    let end = at + keyword.len();
    end <= bytes.len()
        && bytes[at..end].eq_ignore_ascii_case(keyword.as_bytes())
        && (at == 0 || !is_identifier_char(bytes[at - 1]))
        && (end >= bytes.len() || !is_identifier_char(bytes[end]))
}

fn sql_gap_is_trivia(gap: &str) -> bool {
    skip_sql_trivia(gap.as_bytes(), 0) == Some(gap.len())
}

pub(crate) fn skip_sql_trivia(bytes: &[u8], mut i: usize) -> Option<usize> {
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"--" {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"/*" {
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return None;
            }
            i += 2;
            continue;
        }
        return Some(i);
    }
}

pub(crate) fn strip_on_conflict_extras(segment: &str) -> String {
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

pub(crate) fn strip_collate_clauses(inner: &str) -> String {
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
pub(crate) fn has_window_exclude(sql: &str) -> bool {
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
pub(crate) fn rewrite_window_exclude(sql: &str) -> String {
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

pub(crate) fn is_over_open(lower: &[u8], i: usize) -> bool {
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

pub(crate) fn find_matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
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
pub(crate) fn strip_exclude_from_body(
    body: &str,
    body_lower: &str,
) -> Option<(String, &'static str)> {
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
pub(crate) fn inject_partition_marker(body: &str, marker: &str) -> String {
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

pub(crate) fn extract_named_window_spec(sql: &str, name: &str) -> Option<String> {
    let needle = format!(" window {} as (", name.to_ascii_lowercase());
    let start = find_ignore_ascii_case(sql, needle.as_bytes())? + needle.len();
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

pub(crate) fn strip_window_clause(sql: &str, name: &str) -> String {
    let needle = format!(" window {} as (", name.to_ascii_lowercase());
    let Some(start) = find_ignore_ascii_case(sql, needle.as_bytes()) else {
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
    let rest = strip_ignore_ascii_case_prefix(trimmed, b"detach database ")
        .or_else(|| strip_ignore_ascii_case_prefix(trimmed, b"detach "))?;
    let alias = rest.trim();
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
    let rest = strip_ignore_ascii_case_prefix(trimmed, b"attach database ")
        .or_else(|| strip_ignore_ascii_case_prefix(trimmed, b"attach "))?;
    let (path_part, alias_part) = split_attach_path_alias(rest)?;
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

pub(crate) fn split_attach_path_alias(rest: &str) -> Option<(String, &str)> {
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

pub(crate) fn parse_attach_alias(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    strip_ignore_ascii_case_prefix(rest, b"as ")
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

    #[test]
    fn ascii_search_returns_match_offset() {
        assert_eq!(
            find_ignore_ascii_case("SELECT WINDOW win AS (x)", b" window win as ("),
            Some(6)
        );
        assert_eq!(
            find_ignore_ascii_case("SELECT 1", b" window win as ("),
            None
        );
    }

    #[test]
    fn attach_alias_prefix_is_case_insensitive() {
        assert_eq!(parse_attach_alias("  AS aux"), Some("aux"));
        assert_eq!(parse_attach_alias("  as aux"), Some("aux"));
        assert_eq!(parse_attach_alias("  aS aux"), Some("aux"));
    }
}
