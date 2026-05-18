//! SAVEPOINT / RELEASE / ROLLBACK TO grammar.
//!
//! These commands are intercepted before the generic SQL parser path because
//! they sit at the connection layer — they snapshot/restore session-level tx
//! state rather than producing a kernel-level statement plan. We deliberately
//! keep the grammar permissive (matching SQLite's tolerant identifier rules)
//! and surface a parsed `SavepointAction` that `Connection` consumes
//! directly.

use crate::error::{Error, Result};

/// One of the three SQLite-compatible savepoint commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavepointAction {
    Savepoint(String),
    Release(String),
    RollbackTo(String),
}

/// Try to recognise a savepoint command in `sql`. Returns `None` if the SQL
/// does not start with one of the three keywords. Returns `Err` only when the
/// keyword matches but the rest of the statement is malformed (e.g. missing
/// name).
pub(crate) fn try_parse_savepoint(sql: &str) -> Result<Option<SavepointAction>> {
    let trimmed = sql.trim();
    let trimmed = trimmed.trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let lower = trimmed.to_ascii_lowercase();

    if let Some(rest) = strip_keyword(&lower, "savepoint") {
        let original_rest = &trimmed[trimmed.len() - rest.len()..];
        let name = parse_savepoint_name(original_rest)?;
        return Ok(Some(SavepointAction::Savepoint(name)));
    }

    if let Some(rest) = strip_keyword(&lower, "release") {
        // RELEASE [SAVEPOINT] name
        let original_rest = &trimmed[trimmed.len() - rest.len()..];
        let original_rest = strip_optional_savepoint_keyword(original_rest);
        let name = parse_savepoint_name(original_rest)?;
        return Ok(Some(SavepointAction::Release(name)));
    }

    // ROLLBACK [TRANSACTION] TO [SAVEPOINT] name  -- only matches when "TO" is
    // present. A bare ROLLBACK falls back to the regular ROLLBACK path.
    if let Some(rest) = strip_keyword(&lower, "rollback") {
        // Optional "TRANSACTION"
        let lower_rest = rest.trim_start();
        let lower_rest = strip_keyword(lower_rest, "transaction").unwrap_or(lower_rest);
        if let Some(after_to) = strip_keyword(lower_rest, "to") {
            // Recover the original-case slice corresponding to `after_to`.
            let original_rest = &trimmed[trimmed.len() - after_to.len()..];
            let original_rest = strip_optional_savepoint_keyword(original_rest);
            let name = parse_savepoint_name(original_rest)?;
            return Ok(Some(SavepointAction::RollbackTo(name)));
        }
    }

    Ok(None)
}

fn strip_keyword<'a>(s: &'a str, keyword: &str) -> Option<&'a str> {
    let trimmed = s.trim_start();
    if trimmed.len() < keyword.len() {
        return None;
    }
    let (head, tail) = trimmed.split_at(keyword.len());
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    // Must be followed by whitespace, end, or punctuation — not by another
    // identifier character. This prevents accidentally matching "RELEASEME".
    if let Some(next) = tail.chars().next()
        && (next.is_alphanumeric() || next == '_')
    {
        return None;
    }
    Some(tail)
}

fn strip_optional_savepoint_keyword(s: &str) -> &str {
    let trimmed = s.trim_start();
    if let Some(rest) = strip_keyword(trimmed, "savepoint") {
        rest
    } else {
        trimmed
    }
}

fn parse_savepoint_name(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::UnsupportedSql(
            "savepoint command missing name".to_owned(),
        ));
    }
    // Strip optional surrounding double quotes (SQLite identifier quoting).
    if let Some(rest) = s.strip_prefix('"')
        && let Some(name) = rest.strip_suffix('"')
    {
        if name.is_empty() {
            return Err(Error::UnsupportedSql(
                "savepoint name cannot be empty".to_owned(),
            ));
        }
        return Ok(name.to_owned());
    }
    // Unquoted: must be a single identifier-like token (no whitespace/semis).
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty");
    if !(first.is_alphabetic() || first == '_') {
        return Err(Error::UnsupportedSql(format!(
            "invalid savepoint name '{s}'"
        )));
    }
    for ch in chars {
        if !(ch.is_alphanumeric() || ch == '_') {
            return Err(Error::UnsupportedSql(format!(
                "invalid savepoint name '{s}'"
            )));
        }
    }
    Ok(s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_savepoint() {
        assert_eq!(
            try_parse_savepoint("SAVEPOINT sp1").unwrap(),
            Some(SavepointAction::Savepoint("sp1".to_owned()))
        );
        assert_eq!(
            try_parse_savepoint("SAVEPOINT sp1;").unwrap(),
            Some(SavepointAction::Savepoint("sp1".to_owned()))
        );
        assert_eq!(
            try_parse_savepoint("  savepoint   foo_bar  ").unwrap(),
            Some(SavepointAction::Savepoint("foo_bar".to_owned()))
        );
    }

    #[test]
    fn parses_release_with_optional_keyword() {
        assert_eq!(
            try_parse_savepoint("RELEASE sp1").unwrap(),
            Some(SavepointAction::Release("sp1".to_owned()))
        );
        assert_eq!(
            try_parse_savepoint("RELEASE SAVEPOINT sp1").unwrap(),
            Some(SavepointAction::Release("sp1".to_owned()))
        );
    }

    #[test]
    fn parses_rollback_to() {
        assert_eq!(
            try_parse_savepoint("ROLLBACK TO sp1").unwrap(),
            Some(SavepointAction::RollbackTo("sp1".to_owned()))
        );
        assert_eq!(
            try_parse_savepoint("ROLLBACK TRANSACTION TO SAVEPOINT sp1").unwrap(),
            Some(SavepointAction::RollbackTo("sp1".to_owned()))
        );
    }

    #[test]
    fn bare_rollback_is_not_a_savepoint() {
        assert_eq!(try_parse_savepoint("ROLLBACK").unwrap(), None);
        assert_eq!(try_parse_savepoint("ROLLBACK;").unwrap(), None);
    }

    #[test]
    fn does_not_match_keyword_prefix() {
        // RELEASEME is not RELEASE.
        assert_eq!(try_parse_savepoint("RELEASEME sp1").unwrap(), None);
        assert_eq!(try_parse_savepoint("SAVEPOINTS x").unwrap(), None);
    }

    #[test]
    fn rejects_missing_name() {
        assert!(try_parse_savepoint("SAVEPOINT").is_err());
        assert!(try_parse_savepoint("RELEASE").is_err());
        assert!(try_parse_savepoint("ROLLBACK TO").is_err());
    }

    #[test]
    fn accepts_quoted_name() {
        assert_eq!(
            try_parse_savepoint("SAVEPOINT \"my sp\"").unwrap(),
            Some(SavepointAction::Savepoint("my sp".to_owned()))
        );
    }
}
