//! SQLite-shape pre-parse rewrites (UPSERT, windows, GLOB, JSON, STRICT).
#![allow(dead_code)]

use super::*;

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

pub(crate) fn sql_gap_is_trivia(gap: &str) -> bool {
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

pub(crate) fn rewrite_glob_to_function(input: &str) -> String {
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

pub(crate) fn scan_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
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

pub(crate) fn matches_keyword_ci(bytes: &[u8], pos: usize, keyword: &[u8]) -> bool {
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

pub(crate) fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Working backwards from `end`, find the start of an "atom" suitable as a
/// GLOB operand: a quoted string, NULL literal, parenthesized group,
/// numeric literal, or simple identifier. Returns `None` if the preceding
/// text doesn't look like a clean atom (e.g. mid-expression).
pub(crate) fn find_atom_start(bytes: &[u8], end: usize) -> Option<usize> {
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
                let prev = if candidate > 0 {
                    bytes[candidate - 1]
                } else {
                    0
                };
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
    if j < i { Some(j) } else { None }
}

/// Forward equivalent of `find_atom_start`: pick out the end of an atom
/// starting at `start`. Returns `None` if no recognisable atom is present.
pub(crate) fn find_atom_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start >= bytes.len() {
        return None;
    }
    let first = bytes[start];
    if first == b'\'' || first == b'"' {
        return Some(scan_quoted(bytes, start, first));
    }
    // `x'01ab'` style blob literal.
    if (first == b'x' || first == b'X') && bytes.get(start + 1) == Some(&b'\'') {
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
pub(crate) fn trim_trailing_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
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
pub(crate) fn strip_create_index_using_clause(sql: &str) -> String {
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
                    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
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

pub(crate) fn matches_word_ci(lower: &[u8], start: usize, needle: &[u8]) -> bool {
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
pub(crate) fn has_jsonb_question_op(sql: &str) -> bool {
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
                if jsonb_question_op_shape(bytes, i) {
                    return true;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}

/// True if the `?` byte at position `i` looks like a JSONB containment
/// operator (`?`, `?|`, `?&`) rather than a SQL positional placeholder.
///
/// Disambiguation rule: only treat `?` as a JSONB operator when the
/// right-hand side is one of the documented JSONB RHS shapes — a string
/// literal for `?`, or `ARRAY[...]` for `?|` / `?&`. Anything else
/// (including bare `?` followed by a SQL keyword, closing paren, comma,
/// or end-of-input) is the parameter placeholder and must be left alone.
pub(crate) fn jsonb_question_op_shape(bytes: &[u8], i: usize) -> bool {
    let next = bytes.get(i + 1).copied();
    let after_op = match next {
        Some(b'|') | Some(b'&') => i + 2,
        _ => i + 1,
    };
    let mut j = after_op;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= bytes.len() {
        return false;
    }
    match next {
        Some(b'|') | Some(b'&') => {
            // `?|` / `?&` require `ARRAY[`.
            let prefix = b"ARRAY[";
            if j + prefix.len() > bytes.len() {
                return false;
            }
            bytes[j..j + prefix.len()]
                .iter()
                .zip(prefix.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        }
        _ => bytes[j] == b'\'',
    }
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
pub(crate) fn rewrite_jsonb_question_ops(sql: &str) -> String {
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
                // Skip bare `?` placeholders (anything that doesn't match
                // the JSONB RHS shape — see jsonb_question_op_shape).
                if !jsonb_question_op_shape(bytes, i) {
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
pub(crate) fn find_jsonb_lhs_start(prefix: &str) -> Option<usize> {
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
        b"select", b"where", b"from", b"group", b"order", b"having", b"limit", b"on", b"by",
        b"when", b"then", b"else", b"and", b"or", b"not", b"in", b"is", b"as", b"case", b"join",
        b"using", b"set", b"values",
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
pub(crate) fn collect_jsonb_rhs(
    bytes: &[u8],
    start: usize,
    is_array: bool,
) -> Option<(String, usize)> {
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
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.')
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

pub(crate) fn rewrite_strict_without_rowid_combo(sql: &str) -> String {
    if !contains_ignore_ascii_case(sql, b"strict")
        || !contains_ignore_ascii_case(sql, b"without rowid")
    {
        return sql.to_owned();
    }

    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut last = 0usize;
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                i = scan_quoted(bytes, i, bytes[i]);
                continue;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            _ => {}
        }

        if let Some(end) = parse_strict_without_rowid_options(bytes, i) {
            out.push_str(&sql[last..i]);
            out.push_str("WITHOUT ROWID STRICT");
            last = end;
            i = end;
            changed = true;
            continue;
        }
        if let Some(end) = parse_without_rowid_strict_options(bytes, i) {
            out.push_str(&sql[last..i]);
            out.push_str("WITHOUT ROWID STRICT");
            last = end;
            i = end;
            changed = true;
            continue;
        }

        i += 1;
    }

    if changed {
        out.push_str(&sql[last..]);
        out
    } else {
        sql.to_owned()
    }
}

pub(crate) fn parse_strict_without_rowid_options(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut i = parse_strict_option(bytes, pos)?;
    i = skip_ascii_ws(bytes, i);
    if bytes.get(i) != Some(&b',') {
        return None;
    }
    i = skip_ascii_ws(bytes, i + 1);
    parse_without_rowid_option(bytes, i)
}

pub(crate) fn parse_without_rowid_strict_options(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut i = parse_without_rowid_option(bytes, pos)?;
    i = skip_ascii_ws(bytes, i);
    if bytes.get(i) != Some(&b',') {
        return None;
    }
    i = skip_ascii_ws(bytes, i + 1);
    parse_strict_option(bytes, i)
}

pub(crate) fn parse_strict_option(bytes: &[u8], pos: usize) -> Option<usize> {
    if matches_keyword_ci_bounded(bytes, pos, b"STRICT") {
        Some(pos + b"STRICT".len())
    } else {
        None
    }
}

pub(crate) fn parse_without_rowid_option(bytes: &[u8], pos: usize) -> Option<usize> {
    if !matches_keyword_ci_bounded(bytes, pos, b"WITHOUT") {
        return None;
    }
    let after_without = pos + b"WITHOUT".len();
    let rowid_pos = skip_ascii_ws(bytes, after_without);
    if rowid_pos == after_without || !matches_keyword_ci_bounded(bytes, rowid_pos, b"ROWID") {
        return None;
    }
    Some(rowid_pos + b"ROWID".len())
}

pub(crate) fn matches_keyword_ci_bounded(bytes: &[u8], pos: usize, keyword: &[u8]) -> bool {
    matches_keyword_ci(bytes, pos, keyword)
        && pos
            .checked_sub(1)
            .is_none_or(|prev| !is_word_char(bytes[prev]))
        && bytes
            .get(pos + keyword.len())
            .is_none_or(|next| !is_word_char(*next))
}

pub(crate) fn skip_ascii_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}
// ---------------------------------------------------------------------------
// Track H — beyond-SQLite (Postgres parity) pre-parse rewrites.
//
// The rewriters below translate a small but high-leverage slice of PG's
// surface syntax into RedlineDB's existing JSON / scalar surface so that
// `psql -A -t` output for the beyond_sqlite oracle's BEYOND_RICH_TYPES
// cases byte-matches under the runner's normalizer pipeline.
//
// Conventions:
//   * each `has_*` predicate runs first and is cheap (substring scan only)
//     so the rewriter cost is paid only when the surface form is present;
//   * each `rewrite_*` walks the bytes with a string-context tracker so
//     the rewrite is safe inside literal text;
//   * the output of each rewriter is itself valid SQLite-dialect SQL so
//     sqlparser-rs parses it without further hints.
// ---------------------------------------------------------------------------
