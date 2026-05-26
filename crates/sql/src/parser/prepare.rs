use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::Result;
use crate::statement::TableAccessHint;

use sqlparser::ast::{Query, Statement as SqlStatement};

use super::split::is_ident_byte;

thread_local! {
    /// Phase 5 WS-A2e: per-statement registry of SQLite table-hint
    /// captures produced by `strip_sqlite_table_index_hints`. Keyed by
    /// the lowercased identifier (alias if the user typed one, else the
    /// table name) that immediately preceded `INDEXED BY` / `NOT
    /// INDEXED`. The FROM binder pops each entry as it walks the
    /// `TableFactor`s, so by the time the prepare returns the map should
    /// be empty for well-formed inputs.
    static TABLE_INDEX_HINTS: RefCell<HashMap<String, TableAccessHint>> =
        RefCell::new(HashMap::new());
    /// Phase 5 WS-A2e: stash for the `SelectSource::Table(Arc<TableDef>)`
    /// collapse path — the parser's FROM binder pushes a single-table
    /// hint here and `bind_query` lifts it onto `SelectPlan::table_hint`.
    static SINGLE_TABLE_HINT: RefCell<Option<TableAccessHint>> = const { RefCell::new(None) };
}

/// Reset both hint thread-locals. Called by the prepare entry point
/// before each statement so leftover state from a previous (errored)
/// prepare cannot leak.
pub(crate) fn reset_table_index_hints() {
    TABLE_INDEX_HINTS.with(|cell| cell.borrow_mut().clear());
    SINGLE_TABLE_HINT.with(|cell| *cell.borrow_mut() = None);
}

/// Insert a hint captured during the strip pass. Public-in-crate so the
/// strip function can populate it; tests do not need to call this.
pub(crate) fn register_table_index_hint(key_lower: String, hint: TableAccessHint) {
    TABLE_INDEX_HINTS.with(|cell| {
        cell.borrow_mut().insert(key_lower, hint);
    });
}

/// Consume the hint for `alias` (preferred) or `name`. Called by
/// `bind_select_table_factor`. Returns `None` when neither key is
/// present — i.e. no hint applies.
pub(crate) fn take_table_index_hint(
    alias: Option<&str>,
    name: Option<&str>,
) -> Option<TableAccessHint> {
    TABLE_INDEX_HINTS.with(|cell| {
        let mut map = cell.borrow_mut();
        if let Some(key) = alias
            && let Some(hint) = map.remove(&key.to_ascii_lowercase())
        {
            return Some(hint);
        }
        if let Some(key) = name
            && let Some(hint) = map.remove(&key.to_ascii_lowercase())
        {
            return Some(hint);
        }
        None
    })
}

pub(crate) fn stash_single_table_hint(hint: Option<TableAccessHint>) {
    SINGLE_TABLE_HINT.with(|cell| *cell.borrow_mut() = hint);
}

pub(crate) fn take_single_table_hint() -> Option<TableAccessHint> {
    SINGLE_TABLE_HINT.with(|cell| cell.borrow_mut().take())
}

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
    // Phase 5 WS-A2e: track the most-recent bare identifier we emitted
    // into `out`. When we recognise an `INDEXED BY ...` / `NOT INDEXED`
    // hint, that identifier is the table-name or alias the hint targets
    // (SQLite syntax: `FROM t INDEXED BY i`, `FROM t alias NOT INDEXED`).
    let mut last_ident: Option<String> = None;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let end = quoted_end(bytes, i, bytes[i]);
                // Quoted identifiers can be table aliases — record the
                // payload (without quotes) for the hint-binding step.
                if end > i + 1 {
                    if let Ok(inner) = std::str::from_utf8(&bytes[i + 1..end - 1]) {
                        last_ident = Some(inner.to_owned());
                    }
                }
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'[' => {
                let end = bracket_quoted_end(bytes, i);
                if end > i + 1 {
                    if let Ok(inner) = std::str::from_utf8(&bytes[i + 1..end - 1]) {
                        last_ident = Some(inner.to_owned());
                    }
                }
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
                let word_end_idx = word_end(bytes, i);
                let word = &sql[i..word_end_idx];
                if word.eq_ignore_ascii_case("indexed")
                    && let Some((end, index_name)) = indexed_by_hint_end_with_name(sql, word_end_idx)
                {
                    if let Some(target) = last_ident.take() {
                        register_table_index_hint(
                            target.to_ascii_lowercase(),
                            TableAccessHint::IndexedBy(std::sync::Arc::from(index_name.as_str())),
                        );
                    }
                    i = end;
                    changed = true;
                    continue;
                }
                if word.eq_ignore_ascii_case("not")
                    && let Some(end) = not_indexed_hint_end(sql, word_end_idx)
                {
                    if let Some(target) = last_ident.take() {
                        register_table_index_hint(
                            target.to_ascii_lowercase(),
                            TableAccessHint::NotIndexed,
                        );
                    }
                    i = end;
                    changed = true;
                    continue;
                }
                // Track identifiers (skip a small allow-list of keywords
                // that always appear between a table name and the hint
                // keyword so the hint still binds to the table). The
                // SQLite grammar allows `FROM t AS alias NOT INDEXED` —
                // `AS` must not overwrite `t` because the alias `alias`
                // wins. The simple rule: any identifier-shaped token
                // updates `last_ident`, except the noise word `AS`.
                if !word.eq_ignore_ascii_case("as") {
                    last_ident = Some(word.to_owned());
                }
                out.push_str(word);
                i = word_end_idx;
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

/// Variant of `indexed_by_hint_end` that also returns the named index.
/// Used by `strip_sqlite_table_index_hints` to populate
/// `TableAccessHint::IndexedBy(...)` while the strip pass is rewriting
/// the SQL.
fn indexed_by_hint_end_with_name(sql: &str, after_indexed: usize) -> Option<(usize, String)> {
    let bytes = sql.as_bytes();
    let by_start = skip_ws_and_comments(bytes, after_indexed);
    let by_end = word_end_if(bytes, by_start, "by")?;
    let ident_start = skip_ws_and_comments(bytes, by_end);
    let end = identifier_end(bytes, ident_start)?;
    let name_bytes = match bytes.get(ident_start) {
        Some(b'"') | Some(b'`') | Some(b'\'') if end > ident_start + 1 => {
            &bytes[ident_start + 1..end - 1]
        }
        Some(b'[') if end > ident_start + 1 => &bytes[ident_start + 1..end - 1],
        _ => &bytes[ident_start..end],
    };
    let name = std::str::from_utf8(name_bytes).ok()?.to_owned();
    Some((end, name))
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
