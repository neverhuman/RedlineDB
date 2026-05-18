//! LIKE / GLOB pattern-matching helpers used by `eval_scalar` for
//! `Expr::Like`, `Expr::ILike`, and the scalar `glob(...)` function.
//!
//! These are split out from the larger `scalar` module so the
//! pattern-matching state machines live in their own focused file. All
//! exported helpers keep their original `pub(super)` visibility (siblings
//! reach them via the parent `mod.rs` glob re-export).

use super::*;

pub(crate) fn like_result(
    value: SqlValue,
    pattern: SqlValue,
    negated: bool,
    escape_char: Option<Value>,
    case_insensitive: bool,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern = value_to_string(&pattern);
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

fn like_match(text: &str, pattern: &str, escape: Option<char>, case_insensitive: bool) -> bool {
    let text = if case_insensitive {
        text.to_ascii_lowercase()
    } else {
        text.to_owned()
    };
    let pattern = if case_insensitive {
        pattern.to_ascii_lowercase()
    } else {
        pattern.to_owned()
    };
    like_match_inner(
        text.as_bytes(),
        pattern.as_bytes(),
        escape.map(|c| c.to_ascii_lowercase()),
    )
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

pub(crate) fn glob_result(value: SqlValue, pattern: SqlValue, negated: bool) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern = value_to_string(&pattern);
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
