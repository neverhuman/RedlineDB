use crate::error::Result;

use sqlparser::ast::{Query, Statement as SqlStatement};

use super::split::is_ident_byte;

fn replace_case_insensitive_once(input: &str, needle: &str, replacement: &str) -> Option<String> {
    let lower_input = input.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let idx = lower_input.find(&lower_needle)?;
    let mut out = String::with_capacity(input.len() - needle.len() + replacement.len());
    out.push_str(&input[..idx]);
    out.push_str(replacement);
    out.push_str(&input[idx + needle.len()..]);
    Some(out)
}

pub(crate) fn strip_cte_materialized_hints(sql: &str) -> String {
    let mut out = sql.to_owned();
    loop {
        if let Some(next) = replace_case_insensitive_once(&out, "AS NOT MATERIALIZED", "AS") {
            out = next;
            continue;
        }
        if let Some(next) = replace_case_insensitive_once(&out, "AS MATERIALIZED", "AS") {
            out = next;
            continue;
        }
        break out;
    }
}

pub(crate) fn strip_alter_add_column_if_not_exists_hint(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let end = quoted_end(bytes, i, bytes[i]);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'[' => {
                let end = bracket_quoted_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                let end = line_comment_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                let end = block_comment_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b if is_ident_start(b) => {
                let word_end = word_end(bytes, i);
                let word = &sql[i..word_end];
                if word.eq_ignore_ascii_case("add")
                    && let Some(end) = alter_add_column_if_not_exists_end(sql, word_end)
                {
                    out.push_str(word);
                    out.push_str(" COLUMN");
                    i = end;
                    changed = true;
                    continue;
                }
                out.push_str(word);
                i = word_end;
            }
            _ => {
                let ch = sql[i..]
                    .chars()
                    .next()
                    .expect("scanner index is on a char boundary");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if changed { out } else { sql.to_owned() }
}

fn alter_add_column_if_not_exists_end(sql: &str, after_add: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let column_start = skip_ws_and_comments(bytes, after_add);
    let column_end = word_end_if(bytes, column_start, "column")?;
    let if_start = skip_ws_and_comments(bytes, column_end);
    let if_end = word_end_if(bytes, if_start, "if")?;
    let not_start = skip_ws_and_comments(bytes, if_end);
    let not_end = word_end_if(bytes, not_start, "not")?;
    let exists_start = skip_ws_and_comments(bytes, not_end);
    let exists_end = word_end_if(bytes, exists_start, "exists")?;
    Some(exists_end)
}

pub(crate) fn strip_sqlite_table_index_hints(sql: &str) -> Result<String> {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let end = quoted_end(bytes, i, bytes[i]);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'[' => {
                let end = bracket_quoted_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                let end = line_comment_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                let end = block_comment_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b if is_ident_start(b) => {
                let word_end = word_end(bytes, i);
                let word = &sql[i..word_end];
                if word.eq_ignore_ascii_case("indexed")
                    && let Some(end) = indexed_by_hint_end(sql, word_end)
                {
                    i = end;
                    changed = true;
                    continue;
                }
                if word.eq_ignore_ascii_case("not")
                    && let Some(end) = not_indexed_hint_end(sql, word_end)
                {
                    i = end;
                    changed = true;
                    continue;
                }
                out.push_str(word);
                i = word_end;
            }
            _ => {
                let ch = sql[i..]
                    .chars()
                    .next()
                    .expect("scanner index is on a char boundary");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if changed { Ok(out) } else { Ok(sql.to_owned()) }
}

fn indexed_by_hint_end(sql: &str, after_indexed: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let by_start = skip_ws_and_comments(bytes, after_indexed);
    let by_end = word_end_if(bytes, by_start, "by")?;
    let ident_start = skip_ws_and_comments(bytes, by_end);
    identifier_end(bytes, ident_start)
}

fn not_indexed_hint_end(sql: &str, after_not: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let indexed_start = skip_ws_and_comments(bytes, after_not);
    word_end_if(bytes, indexed_start, "indexed")
}

fn skip_ws_and_comments(bytes: &[u8], mut i: usize) -> usize {
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i = line_comment_end(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i = block_comment_end(bytes, i);
            continue;
        }
        return i;
    }
}

fn word_end_if(bytes: &[u8], i: usize, expected: &str) -> Option<usize> {
    if i >= bytes.len() || !is_ident_start(bytes[i]) {
        return None;
    }
    let end = word_end(bytes, i);
    let word = std::str::from_utf8(&bytes[i..end]).ok()?;
    if word.eq_ignore_ascii_case(expected) {
        Some(end)
    } else {
        None
    }
}

fn identifier_end(bytes: &[u8], i: usize) -> Option<usize> {
    if i >= bytes.len() {
        return None;
    }
    match bytes[i] {
        b'"' | b'\'' | b'`' => Some(quoted_end(bytes, i, bytes[i])),
        b'[' => Some(bracket_quoted_end(bytes, i)),
        b if is_ident_start(b) => Some(word_end(bytes, i)),
        _ => None,
    }
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            if i + 1 < bytes.len() && bytes[i + 1] == quote {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn bracket_quoted_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b']' {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn line_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2;
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2;
    while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
    }
    if i + 1 < bytes.len() {
        i + 2
    } else {
        bytes.len()
    }
}

fn word_end(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && is_ident_byte(bytes[i]) {
        i += 1;
    }
    i
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

pub(crate) fn apply_cte_materialized_hints(statements: &mut [SqlStatement], sql: &str) {
    let lower = sql.to_ascii_lowercase();
    let hint = if lower.contains("as not materialized") {
        Some(sqlparser::ast::CteAsMaterialized::NotMaterialized)
    } else if lower.contains("as materialized") {
        Some(sqlparser::ast::CteAsMaterialized::Materialized)
    } else {
        None
    };
    let Some(hint) = hint else {
        return;
    };
    for statement in statements {
        if let SqlStatement::Query(query) = statement {
            apply_cte_materialized_hints_to_query(query.as_mut(), hint);
        }
    }
}

fn apply_cte_materialized_hints_to_query(
    query: &mut Query,
    hint: sqlparser::ast::CteAsMaterialized,
) {
    if let Some(with) = &mut query.with {
        for cte in &mut with.cte_tables {
            cte.materialized = Some(hint);
        }
    }
}
