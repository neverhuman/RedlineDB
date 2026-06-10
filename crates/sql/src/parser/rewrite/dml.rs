use super::super::{set_dml_order_limit_rewrite_enabled, templates};
use super::is_identifier_char;
use crate::statement::{PreparedKind, PreparedTemplate};
use redlinedb_kernel::catalog::SchemaEpoch;

pub(crate) fn try_parse_dml_order_limit_rewrite_pragma(
    trimmed: &str,
    lower: &str,
    schema_epoch: SchemaEpoch,
) -> Option<PreparedTemplate> {
    const KEY: &str = "redline_dml_order_limit_rewrite";
    let after_pragma = lower.strip_prefix("pragma")?.trim_start();
    let after_key = after_pragma.strip_prefix(KEY)?;
    let after_key = after_key.trim_start();
    let arg_text = if let Some(rest) = after_key.strip_prefix('=') {
        rest.trim()
    } else if let Some(rest) = after_key.strip_prefix('(') {
        let close = rest.find(')')?;
        rest[..close].trim()
    } else if after_key.is_empty() {
        return Some(templates::template(
            trimmed,
            schema_epoch,
            true,
            PreparedKind::Reindex,
        ));
    } else {
        return None;
    };
    let value = match arg_text.trim_end_matches(';').trim() {
        "on" | "true" | "1" => true,
        "off" | "false" | "0" => false,
        _ => return None,
    };
    set_dml_order_limit_rewrite_enabled(value);
    Some(templates::template(
        trimmed,
        schema_epoch,
        true,
        PreparedKind::Reindex,
    ))
}

/// WS-A2f-rewrite: walk the SQL text, locate a top-level `DELETE FROM <t>
/// ... LIMIT n` or `UPDATE <t> SET ... LIMIT n` shape, and rewrite to the
/// `WHERE rowid IN (SELECT rowid FROM t ...)` form that the SQLite
/// amalgamation also accepts. Conservative: bails on multi-table FROM,
/// CTE prefixes, qualified table names (`db.t`), and anything else where
/// the shape is ambiguous, letting the existing parser/dml.rs rejection
/// surface.
pub(crate) fn rewrite_dml_order_limit_to_subquery(sql: &str) -> String {
    let trimmed = sql.trim_start();
    let leading = &sql[..sql.len() - trimmed.len()];
    let bytes = trimmed.as_bytes();
    let upper = trimmed.to_ascii_uppercase();
    let head: String = upper.chars().take_while(|c| !c.is_whitespace()).collect();
    let body = match head.as_str() {
        "DELETE" => rewrite_delete_order_limit(trimmed, bytes, &upper),
        "UPDATE" => rewrite_update_order_limit(trimmed, bytes, &upper),
        _ => None,
    };
    match body {
        Some(body) => format!("{leading}{body}"),
        None => sql.to_owned(),
    }
}

#[derive(Debug, Default)]
pub(crate) struct DmlClauseLayout {
    where_body: Option<(usize, usize)>,
    order_body: Option<(usize, usize)>,
    limit_body: Option<(usize, usize)>,
    offset_body: Option<(usize, usize)>,
}

pub(crate) fn scan_dml_tail(bytes: &[u8], upper: &str, from: usize) -> Option<DmlClauseLayout> {
    let mut layout = DmlClauseLayout::default();
    let mut i = from;
    let end = bytes.len();
    while i < end {
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || bytes[i] == b';' {
            break;
        }
        let kw_end = match_dml_keyword(upper, bytes, i)?;
        let kw = &upper[i..kw_end];
        match kw {
            "WHERE" => {
                let body_start = skip_ws_bytes(bytes, kw_end);
                let body_end = find_next_dml_clause_boundary(bytes, upper, body_start);
                layout.where_body = Some((body_start, body_end));
                i = body_end;
            }
            "ORDER" => {
                let after_order = skip_ws_bytes(bytes, kw_end);
                if !starts_with_dml_keyword(upper, bytes, after_order, "BY") {
                    return None;
                }
                let by_end = after_order + 2;
                let body_start = skip_ws_bytes(bytes, by_end);
                let body_end = find_next_dml_clause_boundary(bytes, upper, body_start);
                layout.order_body = Some((body_start, body_end));
                i = body_end;
            }
            "LIMIT" => {
                let body_start = skip_ws_bytes(bytes, kw_end);
                let body_end = find_next_dml_clause_boundary(bytes, upper, body_start);
                layout.limit_body = Some((body_start, body_end));
                i = body_end;
            }
            "OFFSET" => {
                let body_start = skip_ws_bytes(bytes, kw_end);
                let body_end = find_next_dml_clause_boundary(bytes, upper, body_start);
                layout.offset_body = Some((body_start, body_end));
                i = body_end;
            }
            "RETURNING" => return None,
            _ => return None,
        }
    }
    Some(layout)
}

pub(crate) fn match_dml_keyword(upper: &str, bytes: &[u8], from: usize) -> Option<usize> {
    let mut end = from;
    while end < bytes.len() && (bytes[end].is_ascii_alphabetic() || bytes[end] == b'_') {
        end += 1;
    }
    if end == from {
        return None;
    }
    let kw = &upper[from..end];
    match kw {
        "WHERE" | "ORDER" | "LIMIT" | "OFFSET" | "RETURNING" => Some(end),
        _ => None,
    }
}

pub(crate) fn starts_with_dml_keyword(upper: &str, bytes: &[u8], from: usize, kw: &str) -> bool {
    let end = from + kw.len();
    if end > bytes.len() {
        return false;
    }
    if &upper[from..end] != kw {
        return false;
    }
    end == bytes.len() || !is_identifier_char(bytes[end])
}

pub(crate) fn skip_ws_bytes(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

pub(crate) fn find_next_dml_clause_boundary(bytes: &[u8], upper: &str, from: usize) -> usize {
    let end = bytes.len();
    let mut i = from;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while i < end {
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
            b'(' => {
                depth += 1;
                i += 1;
                continue;
            }
            b')' => {
                depth -= 1;
                i += 1;
                continue;
            }
            b';' if depth == 0 => {
                let mut back = i;
                while back > from && bytes[back - 1].is_ascii_whitespace() {
                    back -= 1;
                }
                return back;
            }
            _ => {}
        }
        if depth == 0
            && (i == from || !is_identifier_char(bytes[i - 1]))
            && bytes[i].is_ascii_alphabetic()
        {
            let mut j = i;
            while j < end && (bytes[j].is_ascii_alphabetic() || bytes[j] == b'_') {
                j += 1;
            }
            let kw = &upper[i..j];
            let is_boundary = matches!(kw, "WHERE" | "ORDER" | "LIMIT" | "OFFSET" | "RETURNING")
                && (j == end || !is_identifier_char(bytes[j]));
            if is_boundary {
                let mut back = i;
                while back > from && bytes[back - 1].is_ascii_whitespace() {
                    back -= 1;
                }
                return back;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    let mut back = end;
    while back > from && bytes[back - 1].is_ascii_whitespace() {
        back -= 1;
    }
    back
}

pub(crate) fn parse_unqualified_ident(bytes: &[u8], from: usize) -> Option<(String, usize)> {
    let mut i = from;
    let start;
    let end;
    if i < bytes.len() && bytes[i] == b'"' {
        i += 1;
        start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        end = i;
        i += 1;
    } else {
        start = from;
        while i < bytes.len() && is_identifier_char(bytes[i]) {
            i += 1;
        }
        if i == from {
            return None;
        }
        end = i;
    }
    if start == end {
        return None;
    }
    Some((std::str::from_utf8(&bytes[start..end]).ok()?.to_owned(), i))
}

pub(crate) fn rewrite_delete_order_limit(text: &str, bytes: &[u8], upper: &str) -> Option<String> {
    let mut i = skip_ws_bytes(bytes, "DELETE".len());
    if !starts_with_dml_keyword(upper, bytes, i, "FROM") {
        return None;
    }
    i = skip_ws_bytes(bytes, i + "FROM".len());
    let (table, after_name) = parse_unqualified_ident(bytes, i)?;
    let j = skip_ws_bytes(bytes, after_name);
    if j < bytes.len() && bytes[j] == b'.' {
        return None;
    }
    if j < bytes.len() && bytes[j].is_ascii_alphabetic() {
        let mut k = j;
        while k < bytes.len() && (bytes[k].is_ascii_alphabetic() || bytes[k] == b'_') {
            k += 1;
        }
        let next_kw = &upper[j..k];
        if !matches!(
            next_kw,
            "WHERE" | "ORDER" | "LIMIT" | "OFFSET" | "RETURNING"
        ) {
            return None;
        }
    }
    let layout = scan_dml_tail(bytes, upper, j)?;
    let (limit_start, limit_end) = layout.limit_body?;
    let limit_text = text[limit_start..limit_end].trim();
    if limit_text.is_empty() {
        return None;
    }
    let where_text = layout.where_body.map(|(s, e)| text[s..e].trim().to_owned());
    let order_text = layout.order_body.map(|(s, e)| text[s..e].trim().to_owned());
    let offset_text = layout
        .offset_body
        .map(|(s, e)| text[s..e].trim().to_owned());
    Some(build_dml_subquery_form(
        DmlRewriteKind::Delete { table },
        where_text,
        order_text,
        limit_text.to_owned(),
        offset_text,
    ))
}

pub(crate) fn rewrite_update_order_limit(text: &str, bytes: &[u8], upper: &str) -> Option<String> {
    let i = skip_ws_bytes(bytes, "UPDATE".len());
    let (table, after_name) = parse_unqualified_ident(bytes, i)?;
    let mut j = skip_ws_bytes(bytes, after_name);
    if j < bytes.len() && bytes[j] == b'.' {
        return None;
    }
    if !starts_with_dml_keyword(upper, bytes, j, "SET") {
        return None;
    }
    j = skip_ws_bytes(bytes, j + "SET".len());
    let assignments_start = j;
    let assignments_end = find_next_dml_clause_boundary(bytes, upper, j);
    let assignments = text[assignments_start..assignments_end].trim().to_owned();
    if assignments.is_empty() {
        return None;
    }
    let layout = scan_dml_tail(bytes, upper, assignments_end)?;
    let (limit_start, limit_end) = layout.limit_body?;
    let limit_text = text[limit_start..limit_end].trim();
    if limit_text.is_empty() {
        return None;
    }
    let where_text = layout.where_body.map(|(s, e)| text[s..e].trim().to_owned());
    let order_text = layout.order_body.map(|(s, e)| text[s..e].trim().to_owned());
    let offset_text = layout
        .offset_body
        .map(|(s, e)| text[s..e].trim().to_owned());
    Some(build_dml_subquery_form(
        DmlRewriteKind::Update { table, assignments },
        where_text,
        order_text,
        limit_text.to_owned(),
        offset_text,
    ))
}

pub(crate) enum DmlRewriteKind {
    Delete { table: String },
    Update { table: String, assignments: String },
}

pub(crate) fn build_dml_subquery_form(
    kind: DmlRewriteKind,
    where_text: Option<String>,
    order_text: Option<String>,
    limit_text: String,
    offset_text: Option<String>,
) -> String {
    let (head, table) = match &kind {
        DmlRewriteKind::Delete { table } => (format!("DELETE FROM {table}"), table.clone()),
        DmlRewriteKind::Update { table, assignments } => {
            (format!("UPDATE {table} SET {assignments}"), table.clone())
        }
    };
    let inner_where = where_text
        .as_ref()
        .map(|w| format!(" WHERE ({w})"))
        .unwrap_or_default();
    let inner_order = order_text
        .as_ref()
        .map(|o| format!(" ORDER BY {o}"))
        .unwrap_or_default();
    let inner_offset = offset_text
        .as_ref()
        .map(|o| format!(" OFFSET {o}"))
        .unwrap_or_default();
    let inner = format!(
        "SELECT rowid FROM {table}{inner_where}{inner_order} LIMIT {limit_text}{inner_offset}"
    );
    let outer_where = match where_text {
        Some(w) => format!(" WHERE ({w}) AND rowid IN ({inner})"),
        None => format!(" WHERE rowid IN ({inner})"),
    };
    format!("{head}{outer_where}")
}
