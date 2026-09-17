//! Property + invariant tests for the input-boundary and read-only-authz
//! guards. These are the deterministic negative proofs that protect the two
//! security boundaries declared in `agent/boundaries.toml`:
//!
//! * `quote_ident` must neutralise every identifier so it can be interpolated
//!   into SQL without breaking out of its quoted context (input-boundary).
//! * `is_read_only_sql` must reject every mutating statement so a `--read-only`
//!   connection cannot be tricked into a write (authz / data isolation).

use proptest::prelude::*;
use redline_web_server::connector::{is_read_only_sql, quote_ident};

/// A quoted identifier is always wrapped in double quotes and contains no
/// unescaped interior quote (every `"` is doubled), so it cannot terminate the
/// quoted context early. This is the core input-boundary invariant.
fn well_formed_quote(quoted: &str) -> bool {
    let inner = match quoted.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(inner) => inner,
        None => return false,
    };
    // Walk the interior: every `"` must be part of a `""` pair.
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                i += 2;
                continue;
            }
            return false;
        }
        i += 1;
    }
    true
}

proptest! {
    /// For ANY identifier (including injection attempts with quotes, semicolons,
    /// comments) the result is a single well-formed quoted token.
    #[test]
    fn quote_ident_is_always_well_formed(ident in ".*") {
        let quoted = quote_ident(&ident);
        prop_assert!(well_formed_quote(&quoted), "not well-formed: {quoted:?}");
    }

    /// Doubling is reversible: collapsing `""` back to `"` recovers the input.
    #[test]
    fn quote_ident_round_trips(ident in ".*") {
        let quoted = quote_ident(&ident);
        let inner = &quoted[1..quoted.len() - 1];
        let recovered = inner.replace("\"\"", "\"");
        prop_assert_eq!(recovered, ident);
    }

    /// A leading mutating keyword is ALWAYS rejected by the read-only guard,
    /// regardless of surrounding whitespace/casing.
    #[test]
    fn write_statements_are_never_read_only(
        kw in prop::sample::select(vec![
            "insert", "update", "delete", "drop", "create", "alter",
            "replace", "truncate", "vacuum", "reindex", "attach",
        ]),
        lead in "[ \t\n]{0,4}",
    ) {
        let sql = format!("{lead}{kw} something");
        prop_assert!(!is_read_only_sql(&sql), "wrongly allowed: {sql:?}");
    }

    /// SELECT/WITH/EXPLAIN/PRAGMA/VALUES are ALWAYS read-only.
    #[test]
    fn read_statements_are_always_read_only(
        kw in prop::sample::select(vec!["select", "with", "explain", "pragma", "values"]),
    ) {
        let sql = format!("{kw} 1");
        prop_assert!(is_read_only_sql(&sql), "wrongly rejected: {sql:?}");
    }
}

/// Concrete injection regression: a classic identifier breakout is neutralised.
#[test]
fn quote_ident_neutralises_breakout() {
    let evil = r#"users"; DROP TABLE users;--"#;
    let quoted = quote_ident(evil);
    assert!(well_formed_quote(&quoted));
    // The injected closing quote is doubled, so the statement stays one token.
    assert!(quoted.contains("\"\";"));
}

/// Comment-led write is still rejected (negative authz proof).
#[test]
fn commented_write_is_rejected() {
    assert!(!is_read_only_sql("/* hi */ delete from t"));
    assert!(!is_read_only_sql("-- note\nupdate t set a = 1"));
}
