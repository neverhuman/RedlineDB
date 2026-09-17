//! Agent-friendly typed exception surface.
//!
//! Every error the API can return carries a [`RepairHint`]: a structured,
//! machine-readable explanation with a `purpose`, the concrete `reason`, the
//! `common_fixes` to try, a `docs_url`, and a one-line `repair_hint` telling the
//! next agent exactly where to rerun proof. This keeps failures local and
//! debuggable instead of opaque. The hint is logged on every error response
//! (see `api::err_response`) and surfaced in structured form for observability.

use serde::Serialize;

use crate::connector::ConnectorError;

/// A structured, agent-readable repair hint attached to a failure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairHint {
    /// What this error class is for (the boundary it guards).
    pub purpose: &'static str,
    /// The concrete reason this instance fired.
    pub reason: String,
    /// Ordered list of fixes to try first.
    pub common_fixes: Vec<&'static str>,
    /// Where to read more.
    pub docs_url: &'static str,
    /// One-line instruction for the next agent: where to rerun proof.
    pub repair_hint: &'static str,
}

impl ConnectorError {
    /// Build the typed [`RepairHint`] for this error.
    pub fn repair(&self) -> RepairHint {
        match self {
            ConnectorError::NotFound(_) => RepairHint {
                purpose: "guards table/object lookups",
                reason: self.to_string(),
                common_fixes: vec![
                    "check the object name against GET /api/schema",
                    "the name is case-sensitive and must already exist",
                ],
                docs_url: "CONTRACT.md#endpoints",
                repair_hint: "rerun: cargo test --workspace --all-targets --locked",
            },
            ConnectorError::InvalidArgument(_) => RepairHint {
                purpose: "guards query/paging arguments",
                reason: self.to_string(),
                common_fixes: vec![
                    "orderBy must name a real column",
                    "limit/offset must be non-negative integers",
                ],
                docs_url: "CONTRACT.md#endpoints",
                repair_hint: "rerun: cargo test --workspace --all-targets --locked",
            },
            ConnectorError::ReadOnly(_) => RepairHint {
                purpose: "enforces the --read-only data-isolation boundary",
                reason: self.to_string(),
                common_fixes: vec![
                    "restart without --read-only to allow writes",
                    "issue only SELECT/EXPLAIN/PRAGMA/WITH/VALUES statements",
                ],
                docs_url: "docs/security.md#read-only-authorization",
                repair_hint: "rerun: cargo test -p redline-web-server --test property",
            },
            ConnectorError::Timeout => RepairHint {
                purpose: "bounds per-query execution time",
                reason: self.to_string(),
                common_fixes: vec![
                    "narrow the query or add an index",
                    "raise --query-timeout-ms for legitimately slow work",
                ],
                docs_url: "docs/operations.md#timeouts",
                repair_hint: "rerun: cargo test --workspace --all-targets --locked",
            },
            _ => RepairHint {
                purpose: "general database boundary failure",
                reason: self.to_string(),
                common_fixes: vec![
                    "inspect the server log for the structured repair hint",
                    "confirm the database file/target binary is reachable",
                ],
                docs_url: "docs/operations.md#repair-receipts",
                repair_hint: "rerun: bash ops/ci/pr-ci.sh",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_hint_points_at_the_authz_proof() {
        let hint = ConnectorError::ReadOnly("write rejected".into()).repair();
        assert_eq!(
            hint.purpose,
            "enforces the --read-only data-isolation boundary"
        );
        assert!(hint.repair_hint.contains("property"));
        assert!(!hint.common_fixes.is_empty());
    }
}
