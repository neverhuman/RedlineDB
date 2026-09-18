//! Postgres-shape pre-parse rewrites.
#![allow(dead_code)]

use super::*;

pub(crate) fn has_pg_array_literal(sql: &str) -> bool {
    // Substring match is good enough because `array[` is distinct from any
    // SQLite-valid token: SQLite has no ARRAY type, and `[ident]` style
    // identifier quoting is forbidden inside our SQLiteDialect.
    let lower = sql.to_ascii_lowercase();
    lower.contains("array[")
}

/// Rewrite `ARRAY[...]` literals into `json_array(...)` calls. The contents
/// of the bracket pair are preserved verbatim (commas and string literals
/// included), since `json_array` accepts the same comma-separated form.
pub(crate) fn rewrite_pg_array_literal(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 16);
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
            b'A' | b'a' => {
                if matches_keyword_array(bytes, i)
                    && !array_is_anyall_operand(bytes, i)
                    && let Some(end) = find_matching_bracket(bytes, i + 5)
                {
                    // ARRAY[...] → json_array(...)
                    out.push_str("json_array(");
                    out.push_str(std::str::from_utf8(&bytes[i + 6..end]).unwrap_or(""));
                    out.push(')');
                    i = end + 1;
                    continue;
                }
                out.push(b as char);
                i += 1;
            }
            _ => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

/// Returns true when the `ARRAY` keyword at byte offset `pos` is the
/// operand of a `... ANY (ARRAY[...])` / `... ALL (ARRAY[...])` PG
/// construct. The bare-syntax parser rejects these (they're unsupported
/// extensions); without this guard the array rewrite would mask the
/// parse-time error and let the statement reach execution as `... ANY
/// (json_array(...))`, which the negative parity test
/// `like_any_is_unsupported` then accidentally accepts.
pub(crate) fn array_is_anyall_operand(bytes: &[u8], pos: usize) -> bool {
    let mut i = pos;
    while i > 0 && (bytes[i - 1] as char).is_whitespace() {
        i -= 1;
    }
    // Skip an opening paren if the array literal sits in `... ANY (ARRAY[...])`.
    if i > 0 && bytes[i - 1] == b'(' {
        i -= 1;
        while i > 0 && (bytes[i - 1] as char).is_whitespace() {
            i -= 1;
        }
    }
    if i < 3 {
        return false;
    }
    let prev3 = &bytes[i - 3..i];
    if prev3.eq_ignore_ascii_case(b"ANY") || prev3.eq_ignore_ascii_case(b"ALL") {
        if i == 3 {
            return true;
        }
        let prev_prev = bytes[i - 4];
        return !(prev_prev.is_ascii_alphanumeric() || prev_prev == b'_');
    }
    false
}

/// Detect the literal keyword `ARRAY` immediately followed by `[`. Case-
/// insensitive; requires a non-identifier character (or start-of-input)
/// before the `A` so we don't match `arrays[` etc.
pub(crate) fn matches_keyword_array(bytes: &[u8], pos: usize) -> bool {
    if pos + 6 > bytes.len() {
        return false;
    }
    if !bytes[pos..pos + 5].eq_ignore_ascii_case(b"ARRAY") {
        return false;
    }
    if bytes[pos + 5] != b'[' {
        return false;
    }
    // Boundary: previous byte must not be an identifier continuation char.
    if pos > 0 {
        let prev = bytes[pos - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return false;
        }
    }
    true
}

/// Find the matching `]` for the `[` at `bytes[open]`. Returns its offset,
/// or `None` if no balanced match exists (caller leaves the input alone).
pub(crate) fn find_matching_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    if open >= bytes.len() || bytes[open] != b'[' {
        return None;
    }
    let mut depth = 0i32;
    let mut i = open;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_string = Some(b);
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// PG bytea hex literal: `'\xHEXHEX'::bytea` or just `'\xHEXHEX'` in a bytea
/// context. We rewrite the literal to SQLite's `x'HEXHEX'` blob form so the
/// existing blob plumbing handles the rest. Idempotent — input without
/// `'\x` is returned unchanged.
pub(crate) fn has_pg_bytea_literal(sql: &str) -> bool {
    // `'\x` is a very specific 3-byte sequence not present in normal SQLite
    // input (single-quote → backslash → 'x').
    sql.contains("'\\x") || sql.contains("'\\X")
}

pub(crate) fn rewrite_pg_bytea_literal(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    while i < bytes.len() {
        // Look for the start of a `'\x` literal at the current position.
        if bytes[i] == b'\'' && i + 2 < bytes.len() && bytes[i + 1] == b'\\' {
            let marker = bytes[i + 2];
            if marker == b'x' || marker == b'X' {
                // Find the closing quote (no escaping inside PG hex bytea
                // literals).
                let mut j = i + 3;
                while j < bytes.len() && bytes[j] != b'\'' {
                    j += 1;
                }
                if j < bytes.len() {
                    let hex = std::str::from_utf8(&bytes[i + 3..j]).unwrap_or("");
                    if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                        out.push_str(&format!("x'{hex}'"));
                        i = j + 1;
                        // Strip an immediately-following `::bytea` cast since
                        // the value is now already in blob form.
                        let rest = &sql[i..];
                        let lower = rest.to_ascii_lowercase();
                        if lower.starts_with("::bytea") {
                            i += "::bytea".len();
                        }
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Rewrite PG `&&` (array-overlap) operator into a `pg_array_overlap(...)`
/// function call. Track F's `@>` / `<@` containment operators are already
/// handled at the binary-op layer (`exec::expr::coerce::binary::AtArrow`),
/// so this rewriter only addresses `&&` — SQLite has no `&&` operator at
/// all, so the rewrite is unambiguous.
pub(crate) fn rewrite_pg_array_overlap(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut ops: Vec<usize> = Vec::new();
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_string = Some(b);
                i += 1;
            }
            b'&' if i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                ops.push(i);
                i += 2;
            }
            _ => i += 1,
        }
    }
    if ops.is_empty() {
        return sql.to_owned();
    }
    // Splice in reverse so earlier offsets remain valid.
    let mut buf: Vec<u8> = sql.as_bytes().to_vec();
    for pos in ops.into_iter().rev() {
        let (lhs_start, lhs_end) = match expr_to_left(&buf, pos) {
            Some(span) => span,
            None => continue,
        };
        let mut rhs_off = pos + 2;
        while rhs_off < buf.len() && (buf[rhs_off] as char).is_whitespace() {
            rhs_off += 1;
        }
        let (rhs_start, rhs_end) = match expr_to_right(&buf, rhs_off) {
            Some(span) => span,
            None => continue,
        };
        let lhs = std::str::from_utf8(&buf[lhs_start..lhs_end])
            .unwrap_or("")
            .trim();
        let rhs = std::str::from_utf8(&buf[rhs_start..rhs_end])
            .unwrap_or("")
            .trim();
        if lhs.is_empty() || rhs.is_empty() {
            continue;
        }
        let replacement = format!("pg_array_overlap({lhs}, {rhs})");
        buf.splice(lhs_start..rhs_end, replacement.bytes());
    }
    String::from_utf8(buf).unwrap_or_else(|_| sql.to_owned())
}

/// Walk backward from `end` (exclusive) to find the start of the longest
/// balanced expression. Stops at:
///   * paren/bracket boundaries (when at depth 0),
///   * punctuation that introduces a new expression (`,;=<>+*/%`),
///   * the right edge of any SQL keyword (`SELECT`, `FROM`, `WHERE`,
///     `AND`, `OR`, etc.).
/// Returns the (start, end) byte span; `end` is the first non-whitespace
/// byte before `pos`.
pub(crate) fn expr_to_left(bytes: &[u8], pos: usize) -> Option<(usize, usize)> {
    let mut end = pos;
    while end > 0 && (bytes[end - 1] as char).is_whitespace() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut i = end;
    while i > 0 {
        let b = bytes[i - 1];
        match b {
            b')' => depth_paren += 1,
            b'(' => {
                if depth_paren == 0 {
                    break;
                }
                depth_paren -= 1;
            }
            b']' => depth_bracket += 1,
            b'[' => {
                if depth_bracket == 0 {
                    break;
                }
                depth_bracket -= 1;
            }
            b',' | b';' | b'=' | b'<' | b'>' | b'+' | b'*' | b'/' | b'%' => {
                if depth_paren == 0 && depth_bracket == 0 {
                    break;
                }
            }
            _ => {}
        }
        // At depth 0, stop if we're about to back into a SQL keyword.
        if depth_paren == 0
            && depth_bracket == 0
            && (b as char).is_whitespace()
            && let Some(kw_end) = keyword_just_left_of(bytes, i - 1)
            && kw_end < i
        {
            // Stop at the keyword's right edge (exclusive). Keep
            // `i` pointing just after the keyword's trailing whitespace.
            break;
        }
        i -= 1;
    }
    // Trim leading whitespace from the captured span.
    while i < end && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= end {
        return None;
    }
    Some((i, end))
}

/// If a SQL keyword's last byte sits at or just-before `pos` (skipping any
/// trailing whitespace), return the byte index immediately after the
/// keyword. Otherwise `None`.
pub(crate) fn keyword_just_left_of(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut end = pos + 1;
    while end > 0 && (bytes[end - 1] as char).is_whitespace() {
        end -= 1;
    }
    keyword_ends_at_index(bytes, end).map(|_| end)
}

/// Cheap gate for the postfix-index rewriter: a `)[` sequence somewhere in
/// the SQL is a necessary (not sufficient) prerequisite for the rewrite.
pub(crate) fn has_postfix_index(sql: &str) -> bool {
    sql.contains(")[")
}

/// Rewrite `(EXPR)[N]` and `(EXPR)[N1:N2]` to `json_extract(EXPR, '$[N-1]')`
/// (PG 1-based → JSON 0-based shift). Slice form `[N1:N2]` is left untouched
/// because RedlineDB has no equivalent (PG slices are out of scope here).
pub(crate) fn rewrite_postfix_index(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_string {
            out.push(b as char);
            if b == q {
                in_string = None;
            }
            i += 1;
            continue;
        }
        if matches!(b, b'\'' | b'"' | b'`') {
            in_string = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'[' && !out.is_empty() && out.ends_with(')') {
            // Find the matching `]` at depth 0 (within strings is unlikely
            // for an index expression but we still track them for safety).
            if let Some(close) = find_matching_bracket(bytes, i) {
                let inside = std::str::from_utf8(&bytes[i + 1..close])
                    .unwrap_or("")
                    .trim();
                // Skip slices — they contain `:`.
                if inside.contains(':') {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                // Parse the index as an integer; non-integer expressions are
                // also out of scope (PG allows them but we keep the rewrite
                // conservative).
                if let Ok(n) = inside.parse::<i64>() {
                    let zero_based = n - 1;
                    // Pull the parenthesised LHS out of `out`: find the
                    // matching `(` at the end.
                    let out_bytes = out.as_bytes();
                    if let Some(open) = find_matching_open_paren_at_end(out_bytes) {
                        let lhs = std::str::from_utf8(&out_bytes[open + 1..out_bytes.len() - 1])
                            .unwrap_or("")
                            .trim()
                            .to_owned();
                        out.truncate(open);
                        out.push_str(&format!("json_extract({lhs}, '$[{zero_based}]')"));
                        i = close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(b as char);
        i += 1;
    }
    out
}

/// Find the matching `(` for the `)` at the very end of `bytes`. Returns
/// the offset of the `(` or `None` when the prefix has no balanced match.
pub(crate) fn find_matching_open_paren_at_end(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() || *bytes.last().unwrap() != b')' {
        return None;
    }
    let mut depth = 0i32;
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `Some(len)` when a recognised SQL keyword ends at byte position `end`
/// (exclusive), else `None`. Word boundaries are enforced on both sides.
pub(crate) fn keyword_ends_at_index(bytes: &[u8], end: usize) -> Option<usize> {
    const KWS: &[&[u8]] = &[
        b"SELECT",
        b"FROM",
        b"WHERE",
        b"GROUP",
        b"HAVING",
        b"ORDER",
        b"LIMIT",
        b"OFFSET",
        b"AND",
        b"OR",
        b"NOT",
        b"BY",
        b"ON",
        b"WHEN",
        b"THEN",
        b"ELSE",
        b"END",
        b"CASE",
        b"AS",
        b"IN",
        b"IS",
        b"LIKE",
        b"BETWEEN",
        b"RETURNING",
        b"JOIN",
        b"INNER",
        b"LEFT",
        b"RIGHT",
        b"FULL",
        b"CROSS",
        b"UNION",
        b"INTERSECT",
        b"EXCEPT",
        b"VALUES",
        b"WITH",
        b"DISTINCT",
        b"INSERT",
        b"UPDATE",
        b"DELETE",
        b"SET",
    ];
    for kw in KWS {
        if end < kw.len() {
            continue;
        }
        let start = end - kw.len();
        if !bytes[start..end].eq_ignore_ascii_case(kw) {
            continue;
        }
        if start > 0 {
            let prev = bytes[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }
        if end < bytes.len() {
            let next = bytes[end];
            if next.is_ascii_alphanumeric() || next == b'_' {
                continue;
            }
        }
        return Some(kw.len());
    }
    None
}

/// Forward dual of `expr_to_left`: walk from `start` to the end of the
/// longest balanced expression.
pub(crate) fn expr_to_right(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if start >= bytes.len() {
        return None;
    }
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut i = start;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_string {
            if b == q {
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_string = Some(b);
            }
            b'(' => depth_paren += 1,
            b')' => {
                if depth_paren == 0 {
                    break;
                }
                depth_paren -= 1;
            }
            b'[' => depth_bracket += 1,
            b']' => {
                if depth_bracket == 0 {
                    break;
                }
                depth_bracket -= 1;
            }
            b',' | b';' | b'=' | b'<' | b'>' => {
                if depth_paren == 0 && depth_bracket == 0 {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if i == start {
        return None;
    }
    Some((start, i))
}

/// Rewrite `array_length(arr, 1)` to `json_array_length(arr)`. The second
/// argument (dimension) is dropped because RedlineDB only supports
/// single-dimensional arrays via the JSON surface; PG `array_length(x, N)`
/// for N>1 already returns NULL on flat arrays so the behaviour matches.
///
/// Identifier boundary is enforced on the LEFT (so `json_array_length(` is
/// not rewritten as `json_` + `array_length(`).
pub(crate) fn rewrite_array_length_function(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let bytes = sql.as_bytes();
    while i < sql.len() {
        if lower[i..].starts_with("array_length(") {
            let prev_is_ident =
                i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            if !prev_is_ident {
                let open = i + "array_length".len();
                if let Some(close) = find_matching_paren(bytes, open) {
                    let inside = &sql[open + 1..close];
                    // Drop the trailing `, N` dimension argument.
                    let first_arg = match split_top_level_comma(inside) {
                        Some(idx) => &inside[..idx],
                        None => inside,
                    };
                    out.push_str("json_array_length(");
                    out.push_str(first_arg.trim());
                    out.push(')');
                    i = close + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Find the comma that separates the top-level arguments of a parenthesised
/// expression body (paren-depth 0, ignoring strings and bracketed groups).
pub(crate) fn split_top_level_comma(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth_p = 0i32;
    let mut depth_b = 0i32;
    let mut in_string: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(q) = in_string {
            if b == q {
                in_string = None;
            }
            continue;
        }
        match b {
            b'\'' | b'"' => in_string = Some(b),
            b'(' => depth_p += 1,
            b')' => depth_p -= 1,
            b'[' => depth_b += 1,
            b']' => depth_b -= 1,
            b',' if depth_p == 0 && depth_b == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Rewrite `array_agg(EXPR [ORDER BY ...])` to `json_group_array(EXPR [ORDER
/// BY ...])`. RedlineDB's `json_group_array` already accepts an in-aggregate
/// `ORDER BY`, so the rewriter is just a name swap. Identifier boundary is
/// enforced on the LEFT so qualified names with `array_agg` as a suffix
/// aren't rewritten.
pub(crate) fn rewrite_array_agg_function(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let bytes = sql.as_bytes();
    while i < sql.len() {
        if lower[i..].starts_with("array_agg(") {
            let prev_is_ident =
                i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            if !prev_is_ident {
                let open = i + "array_agg".len();
                if let Some(close) = find_matching_paren(bytes, open) {
                    let inside = &sql[open + 1..close];
                    out.push_str("json_group_array(");
                    out.push_str(inside);
                    out.push(')');
                    i = close + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Rewrite PG's `INTERVAL 'N units'` literal into a SQLite `'+N units'`
/// string. The single-quoted argument is reused verbatim with a leading
/// `+` so downstream datetime arithmetic can feed it to `datetime(date,
/// modifier)`. PG intervals with multiple parts (`'1 day 2 hours'`) and
/// signed forms (`'-3 days'`) flow through unchanged after the `+` prefix.
pub(crate) fn rewrite_pg_interval_literal(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if lower[i..].starts_with("interval ") {
            // Word boundary before.
            let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            if prev_ok {
                // Find the literal — skip whitespace then expect `'`.
                let mut j = i + "interval ".len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'\'' {
                    let start = j;
                    j += 1;
                    while j < bytes.len() && bytes[j] != b'\'' {
                        j += 1;
                    }
                    if j < bytes.len() {
                        let body = std::str::from_utf8(&bytes[start + 1..j])
                            .unwrap_or("")
                            .trim();
                        // Prefix `+` if not already signed.
                        let prefixed = if body.starts_with('-') || body.starts_with('+') {
                            body.to_owned()
                        } else {
                            format!("+{body}")
                        };
                        out.push_str(&format!("'{prefixed}'"));
                        i = j + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Rewrite `EXPR + 'modifier'` / `EXPR - 'modifier'` for date-arithmetic
/// modifiers (`+N days`, `-N months`, etc.) into `datetime(EXPR, 'modifier')`.
/// Only triggers when the right-hand string literal looks like a SQLite
/// datetime modifier so plain string concatenation is untouched.
pub(crate) fn rewrite_date_arith_with_modifier(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_string {
            out.push(b as char);
            if b == q {
                in_string = None;
            }
            i += 1;
            continue;
        }
        if matches!(b, b'\'' | b'"' | b'`') {
            in_string = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }
        if (b == b'+' || b == b'-')
            && let Some(mod_span) = peek_modifier_literal_after(bytes, i + 1)
        {
            // Pull the parenthesised LHS expression from `out`.
            let out_bytes = out.as_bytes();
            let lhs_span = trailing_expr_span(out_bytes);
            if let Some((lhs_start, lhs_end)) = lhs_span {
                let lhs = std::str::from_utf8(&out_bytes[lhs_start..lhs_end])
                    .unwrap_or("")
                    .trim()
                    .to_owned();
                if !lhs.is_empty() {
                    let modifier_text =
                        std::str::from_utf8(&bytes[mod_span.0..mod_span.1]).unwrap_or("");
                    // Inject the sign into the modifier ('+5 days' / '-5 days').
                    let inner = &modifier_text[1..modifier_text.len() - 1];
                    let signed: String = if inner.starts_with('+') || inner.starts_with('-') {
                        inner.to_owned()
                    } else if b == b'-' {
                        format!("-{inner}")
                    } else {
                        format!("+{inner}")
                    };
                    out.truncate(lhs_start);
                    out.push_str(&format!("datetime({lhs}, '{signed}')"));
                    i = mod_span.1;
                    continue;
                }
            }
        }
        out.push(b as char);
        i += 1;
    }
    out
}

/// Find a single-quoted literal starting at byte `start` (after skipping
/// whitespace) whose body looks like a SQLite datetime modifier — i.e.,
/// matches `[+-]?\d+\s+(year|month|day|hour|minute|second)s?`. Returns the
/// (start, end) of the quote pair, or `None`.
pub(crate) fn peek_modifier_literal_after(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut j = start;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'\'' {
        return None;
    }
    let lit_start = j;
    j += 1;
    while j < bytes.len() && bytes[j] != b'\'' {
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    let body = std::str::from_utf8(&bytes[lit_start + 1..j]).ok()?;
    if !looks_like_datetime_modifier(body) {
        return None;
    }
    Some((lit_start, j + 1))
}

pub(crate) fn looks_like_datetime_modifier(body: &str) -> bool {
    let lower = body.trim().to_ascii_lowercase();
    let bytes = lower.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut i = 0usize;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        i += 1;
    }
    let digit_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digit_start {
        return false;
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    let unit = &lower[i..];
    matches!(
        unit,
        "year"
            | "years"
            | "month"
            | "months"
            | "day"
            | "days"
            | "hour"
            | "hours"
            | "minute"
            | "minutes"
            | "second"
            | "seconds"
    )
}

/// Find the byte span of the rightmost expression at the end of `out`.
/// Walks back through balanced parens / brackets, stops at top-level
/// punctuation. String literals are pre-marked via `forward_string_mask`
/// so embedded punctuation never falsely terminates the expression.
///
/// Returns the (start, end) span where `end == out.len()` (after whitespace
/// trim).
pub(crate) fn trailing_expr_span(out: &[u8]) -> Option<(usize, usize)> {
    let mut end = out.len();
    while end > 0 && (out[end - 1] as char).is_whitespace() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let in_string = forward_string_mask(out);
    let mut depth_p = 0i32;
    let mut depth_b = 0i32;
    let mut i = end;
    while i > 0 {
        let pos = i - 1;
        if in_string[pos] {
            i -= 1;
            continue;
        }
        let b = out[pos];
        match b {
            b')' => depth_p += 1,
            b'(' => {
                if depth_p == 0 {
                    break;
                }
                depth_p -= 1;
            }
            b']' => depth_b += 1,
            b'[' => {
                if depth_b == 0 {
                    break;
                }
                depth_b -= 1;
            }
            b',' | b';' | b'=' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'%' => {
                if depth_p == 0 && depth_b == 0 {
                    break;
                }
            }
            _ => {}
        }
        if depth_p == 0
            && depth_b == 0
            && (b as char).is_whitespace()
            && let Some(_) = keyword_just_left_of(out, pos)
        {
            break;
        }
        i -= 1;
    }
    while i < end && (out[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= end {
        return None;
    }
    Some((i, end))
}

/// Mark each byte in `out` with `true` when it falls inside a single-quoted
/// (or double/back-quoted) string literal, including the quote bytes
/// themselves. Used by `trailing_expr_span` so the backward walker treats
/// strings as opaque.
pub(crate) fn forward_string_mask(out: &[u8]) -> Vec<bool> {
    let mut mask = vec![false; out.len()];
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < out.len() {
        let b = out[i];
        if let Some(q) = in_string {
            mask[i] = true;
            if b == q {
                if i + 1 < out.len() && out[i + 1] == q {
                    mask[i + 1] = true;
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        if matches!(b, b'\'' | b'"' | b'`') {
            mask[i] = true;
            in_string = Some(b);
        }
        i += 1;
    }
    mask
}

/// Strip a trailing `AT TIME ZONE 'TZ'` clause from a SQL expression.
/// RedlineDB is tz-naive — every timestamp is treated as UTC — so dropping
/// the clause is the right thing to do for the common `'<ts>'::timestamptz
/// AT TIME ZONE 'UTC'` shape. The single-quoted TZ argument is parsed
/// literally so it doesn't matter what the timezone string is; we always
/// drop it. The trailing `'+HH[:MM]'` offset on the literal itself is
/// stripped one layer deeper, by `datetime::parse::strip_tz_suffix`.
pub(crate) fn rewrite_at_time_zone(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if lower[i..].starts_with("at time zone") {
            // Word boundary before.
            let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            if prev_ok {
                let mut j = i + "at time zone".len();
                // Skip whitespace.
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                // The TZ argument is a single-quoted string literal — find
                // and skip past the closing quote.
                if j < bytes.len() && bytes[j] == b'\'' {
                    j += 1;
                    while j < bytes.len() && bytes[j] != b'\'' {
                        j += 1;
                    }
                    if j < bytes.len() {
                        j += 1; // closing quote
                    }
                    // Drop everything between `i` and `j`. The expression's
                    // preceding whitespace stays so the surrounding SQL
                    // parses cleanly.
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    i = j;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
