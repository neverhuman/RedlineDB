//! LIKE / GLOB pattern-matching helpers used by `eval_scalar` for
//! `Expr::Like`, `Expr::ILike`, and the scalar `glob(...)` function.
//!
//! These are split out from the larger `scalar` module so the
//! pattern-matching state machines live in their own focused file. All
//! exported helpers keep their original `pub(super)` visibility (siblings
//! reach them via the parent `mod.rs` glob re-export).

use super::*;

// A13: take args by reference. The body only needs `value_to_string(&v)`,
// never ownership; previously every LIKE evaluation cloned both operands.
pub(crate) fn like_result(
    value: &SqlValue,
    pattern: &SqlValue,
    negated: bool,
    escape_char: Option<Value>,
    case_insensitive: bool,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(value);
    let pattern = value_to_string(pattern);
    let escape = match escape_char {
        Some(Value::SingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::SingleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleSingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleDoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::EscapedStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::UnicodeStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DollarQuotedString(s)) if s.value.chars().count() == 1 => {
            Some(s.value.chars().next().unwrap())
        }
        None => None,
        Some(other) => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported LIKE escape literal: {other:?}"
            )));
        }
    };
    let matched = like_match(&text, &pattern, escape, case_insensitive);
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

// A13b: take args by reference. ILIKE's per-row `.to_lowercase()` is still
// per-call but at least the outer `SqlValue` clones at the call sites are
// gone. A future commit could push case-folding into `unicode_like_match`
// itself to drop the lowercase clones too — that's a larger refactor.
pub(crate) fn ilike_result(
    value: &SqlValue,
    pattern: &SqlValue,
    negated: bool,
    escape_char: Option<Value>,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(value).to_lowercase();
    let pattern = value_to_string(pattern).to_lowercase();
    let escape = match escape_char {
        Some(Value::SingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::SingleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleSingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleDoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::EscapedStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::UnicodeStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DollarQuotedString(s)) if s.value.chars().count() == 1 => {
            Some(s.value.chars().next().unwrap())
        }
        None => None,
        Some(other) => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported ILIKE escape literal: {other:?}"
            )));
        }
    };
    let matched = unicode_like_match(&text, &pattern, escape);
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

fn like_match(text: &str, pattern: &str, escape: Option<char>, case_insensitive: bool) -> bool {
    // A40: on the case-sensitive path (the common SQLite default), borrow
    // the input strings directly — `like_match_inner` takes `&[u8]` so it
    // doesn't need owned input. The previous code did `text.to_owned()`
    // and `pattern.to_owned()` unconditionally, allocating two Strings
    // per LIKE evaluation even when the case fold was unnecessary.
    if case_insensitive {
        let text = text.to_ascii_lowercase();
        let pattern = pattern.to_ascii_lowercase();
        like_match_inner(
            text.as_bytes(),
            pattern.as_bytes(),
            escape.map(|c| c.to_ascii_lowercase()),
        )
    } else {
        like_match_inner(text.as_bytes(), pattern.as_bytes(), escape)
    }
}

fn unicode_like_match(text: &str, pattern: &str, escape: Option<char>) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    unicode_like_match_inner(&text, &pattern, escape)
}

fn unicode_like_match_inner(text: &[char], pattern: &[char], escape: Option<char>) -> bool {
    fn inner(text: &[char], pattern: &[char], escape: Option<char>) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                '%' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..], escape) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                '_' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch if Some(ch) == escape => {
                    pi += 1;
                    if pi >= pattern.len() || ti >= text.len() || pattern[pi] != text[ti] {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }
    inner(text, pattern, escape)
}

fn like_match_inner(text: &[u8], pattern: &[u8], escape: Option<char>) -> bool {
    fn inner(text: &[u8], pattern: &[u8], escape: Option<u8>) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                b'%' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..], escape) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                b'_' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                b if Some(b) == escape => {
                    pi += 1;
                    if pi >= pattern.len() || ti >= text.len() || pattern[pi] != text[ti] {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }
    inner(text, pattern, escape.map(|c| c as u8))
}

// A14: take args by reference. The body only needs `value_to_string(&v)`,
// never ownership; previously every GLOB evaluation cloned both operands.
pub(crate) fn glob_result(value: &SqlValue, pattern: &SqlValue, negated: bool) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(value);
    let pattern = value_to_string(pattern);
    let matched = glob_match(text.as_bytes(), pattern.as_bytes());
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

fn glob_match(text: &[u8], pattern: &[u8]) -> bool {
    // SQLite GLOB grammar:
    //   *           — matches zero or more characters
    //   ?           — matches exactly one character
    //   [abc]       — character class (any of)
    //   [a-z]       — character range
    //   [!abc]      — negated class (matches one char NOT in abc)
    //   [^abc]      — also a negated class (compatibility)
    //   anything else — literal (case-sensitive, unlike LIKE)
    // An unterminated `[` is treated as a literal `[`.
    fn inner(text: &[u8], pattern: &[u8]) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                b'*' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..]) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                b'?' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                b'[' => {
                    if let Some((matched, advance)) = match_glob_class(&pattern[pi..], text.get(ti))
                    {
                        if !matched {
                            return false;
                        }
                        ti += 1;
                        pi += advance;
                    } else {
                        // Unterminated class: treat `[` as a literal.
                        if ti >= text.len() || text[ti] != b'[' {
                            return false;
                        }
                        ti += 1;
                        pi += 1;
                    }
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }
    inner(text, pattern)
}

/// Try to match a `[...]` character class at the start of `pattern` against
/// the optional next byte of the input. Returns `(matched, pattern_advance)`
/// on success; `None` when the class is unterminated (caller should treat
/// the leading `[` as a literal).
fn match_glob_class(pattern: &[u8], target: Option<&u8>) -> Option<(bool, usize)> {
    debug_assert!(pattern.first() == Some(&b'['));
    let mut idx = 1usize;
    let negate = matches!(pattern.get(idx), Some(&b'!') | Some(&b'^'));
    if negate {
        idx += 1;
    }
    let class_start = idx;
    let mut matched = false;
    let target_byte = match target {
        Some(&b) => b,
        None => 0,
    };

    // SQLite allows a literal `]` only as the first character of the class.
    // So `[]abc]` matches `]`, `a`, `b`, or `c`.
    if pattern.get(idx) == Some(&b']') {
        if target.is_some() && target_byte == b']' {
            matched = true;
        }
        idx += 1;
    }

    while idx < pattern.len() && pattern[idx] != b']' {
        let lo = pattern[idx];
        if idx + 2 < pattern.len() && pattern[idx + 1] == b'-' && pattern[idx + 2] != b']' {
            let hi = pattern[idx + 2];
            if target.is_some() && target_byte >= lo.min(hi) && target_byte <= lo.max(hi) {
                matched = true;
            }
            idx += 3;
        } else {
            if target.is_some() && target_byte == lo {
                matched = true;
            }
            idx += 1;
        }
    }

    if idx >= pattern.len() {
        // No closing `]` — pattern is malformed. Caller falls back to literal.
        if class_start == idx {
            return None;
        }
        return None;
    }

    let final_match = if target.is_none() {
        false
    } else if negate {
        !matched
    } else {
        matched
    };

    Some((final_match, idx + 1))
}

/// Evaluator for the `SIMILAR TO` operator. Translates the SQL-standard
/// pattern (Postgres flavour) into a POSIX regex anchored at both ends,
/// then delegates to the shared regex engine. NULL on either side
/// propagates. An invalid pattern surfaces as `UnsupportedSql`.
pub(crate) fn similar_to_result(
    value: SqlValue,
    pattern: SqlValue,
    negated: bool,
    escape_char: Option<Value>,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern_text = value_to_string(&pattern);
    let escape = extract_single_char_escape(escape_char.as_ref(), "SIMILAR TO")?;
    let regex_pattern = similar_to_regex(&pattern_text, escape)
        .map_err(|err| Error::UnsupportedSql(format!("invalid SIMILAR TO pattern: {err}")))?;
    let matched = crate::regexp::regex_match(&text, &regex_pattern)?;
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

/// Postgres `POSITION(sub IN str)` — 1-based offset of the first
/// occurrence of `sub` in `str`, or 0 if absent. NULL on either side
/// propagates. The empty needle returns 1 (matches PG and SQLite's
/// `instr`).
pub(crate) fn position_result(sub: SqlValue, haystack: SqlValue) -> Result<SqlValue> {
    if matches!(sub, SqlValue::Null) || matches!(haystack, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let needle = value_to_string(&sub);
    let hay = value_to_string(&haystack);
    if needle.is_empty() {
        return Ok(SqlValue::Integer(1));
    }
    let needle_str: &str = needle.as_ref();
    let position = hay
        .char_indices()
        .enumerate()
        .find(|(_, (byte_pos, _))| hay[*byte_pos..].starts_with(needle_str))
        .map(|(char_pos, _)| char_pos as i64 + 1)
        .unwrap_or(0);
    Ok(SqlValue::Integer(position))
}

/// Helper shared by ILIKE / LIKE / SIMILAR TO escape-clause parsing —
/// pulls a single character out of an arbitrary sqlparser string literal
/// node. Multi-character literals are rejected (PG and SQLite reject
/// them too).
fn extract_single_char_escape(escape_char: Option<&Value>, op_name: &str) -> Result<Option<char>> {
    Ok(match escape_char {
        None => None,
        Some(Value::SingleQuotedString(s))
        | Some(Value::DoubleQuotedString(s))
        | Some(Value::SingleQuotedRawStringLiteral(s))
        | Some(Value::DoubleQuotedRawStringLiteral(s))
        | Some(Value::TripleSingleQuotedString(s))
        | Some(Value::TripleDoubleQuotedString(s))
        | Some(Value::EscapedStringLiteral(s))
        | Some(Value::UnicodeStringLiteral(s)) => {
            if s.chars().count() != 1 {
                return Err(Error::UnsupportedSql(format!(
                    "{op_name} escape must be a single character"
                )));
            }
            Some(s.chars().next().unwrap())
        }
        Some(Value::DollarQuotedString(s)) => {
            if s.value.chars().count() != 1 {
                return Err(Error::UnsupportedSql(format!(
                    "{op_name} escape must be a single character"
                )));
            }
            Some(s.value.chars().next().unwrap())
        }
        Some(other) => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported {op_name} escape literal: {other:?}"
            )));
        }
    })
}

/// Translate a Postgres-flavoured `SIMILAR TO` pattern into an anchored
/// POSIX regex. Handles `%` (→ `.*`), `_` (→ `.`), the regex
/// metacharacters `| ( ) [ ] + * ? { }` (passed through), and bracket
/// expressions (passed through verbatim). Regex specials that have no
/// SIMILAR-TO meaning (`. ^ $ \\`) are escaped so they match literally.
/// Returns an `anchored = true` regex (so the whole input must match)
/// to mirror Postgres semantics.
fn similar_to_regex(pattern: &str, escape: Option<char>) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(pattern.len() + 4);
    out.push('^');
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;
    let mut bracket_depth: i32 = 0;
    while i < chars.len() {
        let ch = chars[i];
        if Some(ch) == escape {
            // The next char is taken literally.
            i += 1;
            if i >= chars.len() {
                return Err("dangling escape at end of pattern".to_owned());
            }
            let lit = chars[i];
            push_regex_literal(&mut out, lit);
            i += 1;
            continue;
        }
        if bracket_depth > 0 {
            // Inside a `[...]` class: passthrough, but track close-bracket.
            out.push(ch);
            if ch == ']' {
                bracket_depth -= 1;
            }
            i += 1;
            continue;
        }
        match ch {
            '%' => out.push_str(".*"),
            '_' => out.push('.'),
            '[' => {
                out.push('[');
                bracket_depth += 1;
            }
            // SIMILAR TO shares these metacharacters with POSIX regex.
            '|' | '(' | ')' | '+' | '*' | '?' | '{' | '}' => out.push(ch),
            // Regex specials with no SIMILAR-TO meaning — must be escaped.
            '.' | '^' | '$' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            other => push_regex_literal(&mut out, other),
        }
        i += 1;
    }
    if bracket_depth != 0 {
        return Err("unterminated character class".to_owned());
    }
    out.push('$');
    Ok(out)
}

fn push_regex_literal(out: &mut String, ch: char) {
    // Anything that could be a regex metachar (defensive — even though
    // the main loop already handles the SIMILAR-TO-specific cases). The
    // `regex` crate's regex grammar treats this set as syntax characters.
    if matches!(
        ch,
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$'
    ) {
        out.push('\\');
    }
    out.push(ch);
}
