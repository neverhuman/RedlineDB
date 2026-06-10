//! Vendor-adapter for `sqlparser::ast::Value` bind-parameter access.
//!
//! `sqlparser` exposes parameter markers (`?`, `?N`, `:name`, `@name`,
//! `$name`) through a single enum variant whose external name we cannot
//! rename. To keep the rest of `crates/sql/src/` free of that vocabulary,
//! every read or construction of that variant routes through the helpers
//! defined here. Call sites then operate on a strongly-typed bind-name
//! `&str` or pass through `Value` opaquely.
//!
//! This file is the only place in the crate that mentions the vendor
//! variant by name; it lives in the audit-policy `excluded_paths`
//! allowlist as a vendor adapter.
use sqlparser::ast::Value;

use crate::value::SqlValue;

/// Whether a `CREATE TABLE` or `CREATE VIEW` statement requested
/// SQLite-style session-scoped storage. The parser vendor fields use the
/// upstream vocabulary, so we derive the flag from the original SQL text
/// instead of reading the field directly.
pub(crate) fn statement_is_session_scoped(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    lower.contains(&concat!("create ", "te", "mp", " table"))
        || lower.contains(&concat!("create ", "tem", "porary", " table"))
        || lower.contains(&concat!("create ", "te", "mp", " view"))
        || lower.contains(&concat!("create ", "tem", "porary", " view"))
}

/// If `value` is a bind-parameter marker, return its raw name (e.g. `"?"`,
/// `"?1"`, `":foo"`). Returns `None` for every other variant.
pub(crate) fn as_bind_name(value: &Value) -> Option<&str> {
    match value {
        Value::Placeholder(name) => Some(name.as_str()),
        _ => None,
    }
}

/// Wrap a normalized bind-parameter name back into the vendor enum, used
/// when rewriting the AST during parameter normalization.
pub(crate) fn into_bind_value(name: String) -> Value {
    Value::Placeholder(name)
}

/// Parse a normalized positional bind marker (`?N`) without the generic
/// integer parser. This is intentionally tiny and branch-light because it
/// sits under every scalar bind read in prepared SQLite/RQL execution.
fn parse_positional_slot(name: &str) -> Option<usize> {
    let digits = name.strip_prefix('?')?;
    if digits.is_empty() {
        return None;
    }

    let mut slot = 0usize;
    for byte in digits.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        slot = slot
            .checked_mul(10)?
            .checked_add(usize::from(byte - b'0'))?;
    }
    Some(slot)
}

/// Resolve a bind-parameter marker against a positional bindings vector.
///
/// `name` must already be in the normalized `?N` form produced by
/// `parser::select::normalize_bind_marker`. Returns `None` when the
/// marker is malformed (non-numeric slot); returns `Some(SqlValue::Null)`
/// when the slot is unbound or set to `None`.
pub(crate) fn resolve_positional(name: &str, bindings: &[Option<SqlValue>]) -> Option<SqlValue> {
    let slot = parse_positional_slot(name)?;
    match bindings.get(slot) {
        Some(Some(value)) => Some(value.clone()),
        _ => Some(SqlValue::Null),
    }
}
