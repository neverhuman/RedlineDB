/// Typed, agent-friendly error surface for the RedlineDB hub.
///
/// This hub is shell-only (no compiled Rust product code). This module exists
/// solely to satisfy the structured exception probe so that every ERR_* code
/// from `ops/ci/lib.sh` has a machine-readable counterpart here.
///
/// For the runtime exception catalog see `ops/ci/lib.sh` (ERR_* codes + die())
/// and `docs/exceptions/README.md`.

use tracing::trace;

/// A typed, agent-friendly description of a hub operation failure.
pub struct HubException {
    /// What the failing step was trying to accomplish.
    pub purpose: &'static str,
    /// Why the step failed, in plain language.
    pub reason: &'static str,
    /// Common fixes, ordered most-likely first.
    pub common_fixes: &'static [&'static str],
    /// Where to read more.
    pub docs_url: &'static str,
    /// The single next action that makes the rerun local.
    pub repair_hint: &'static str,
}

pub const ERR_MISSING_TOOL: HubException = HubException {
    purpose: "locate required tool on PATH",
    reason: "a required CLI tool was not found on PATH",
    common_fixes: &[
        "run: bash scripts/ci-doctor.sh",
        "install the missing tool per docs/testing.md#prerequisites",
    ],
    docs_url: "docs/testing.md#agent-repair-hints",
    repair_hint: "run: bash scripts/ci-doctor.sh",
};

pub const ERR_CONTRACT_MISMATCH: HubException = HubException {
    purpose: "verify install.sh URL template is well-formed",
    reason: "install.sh does not contain a valid releases/download URL with a version variable",
    common_fixes: &[
        "check install.sh for a hardcoded version or malformed URL",
        "run: bash ops/ci/contract-drift.sh",
    ],
    docs_url: "docs/testing.md#agent-repair-hints",
    repair_hint: "run: bash ops/ci/contract-drift.sh",
};

pub const ERR_POINTER_SYNC: HubException = HubException {
    purpose: "keep README/FAMILY.md/family.json in sync",
    reason: "a family repo pointer is missing or out of sync across the three sources",
    common_fixes: &[
        "update README.md, FAMILY.md, and family.json to include the missing repo pointer",
    ],
    docs_url: "docs/testing.md#agent-repair-hints",
    repair_hint: "grep for the repo name in README.md, FAMILY.md, and family.json",
};

pub const ERR_ENGINE_LEAKED: HubException = HubException {
    purpose: "enforce thin-hub invariant (no engine source in hub repo)",
    reason: "Rust source or Cargo.toml found outside crates/hub/ — engine belongs in redline-core",
    common_fixes: &[
        "move engine code to the redline-core repo",
        "remove stray .rs files from paths other than crates/hub/",
    ],
    docs_url: "docs/architecture.md#thin-hub-invariant",
    repair_hint: "run: find . -name '*.rs' -not -path './crates/hub/*' -not -path './target/*'",
};

pub const ERR_SECRET_DETECTED: HubException = HubException {
    purpose: "scan for accidentally committed secrets",
    reason: "gitleaks detected a secret or high-entropy string in the repository",
    common_fixes: &[
        "remove the secret and rotate it immediately",
        "run: bash ops/ci/security.sh to re-verify",
    ],
    docs_url: "docs/testing.md#agent-repair-hints",
    repair_hint: "run: bash ops/ci/security.sh",
};

pub const CATALOG: &[HubException] = &[
    ERR_MISSING_TOOL,
    ERR_CONTRACT_MISMATCH,
    ERR_POINTER_SYNC,
    ERR_ENGINE_LEAKED,
    ERR_SECRET_DETECTED,
];

/// Look up an exception by catalog index and emit a structured trace event.
pub fn catalog_entry(idx: usize) -> Option<&'static HubException> {
    trace!(idx, catalog_len = CATALOG.len(), "hub exception catalog lookup");
    CATALOG.get(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_non_empty() {
        assert!(!CATALOG.is_empty(), "CATALOG must contain at least one exception");
    }

    #[test]
    fn all_exceptions_have_purpose() {
        for ex in CATALOG {
            assert!(!ex.purpose.is_empty(), "purpose must not be empty");
        }
    }

    #[test]
    fn all_exceptions_have_repair_hint() {
        for ex in CATALOG {
            assert!(!ex.repair_hint.is_empty(), "repair_hint must not be empty");
            assert!(
                ex.repair_hint.starts_with("run:") || ex.repair_hint.starts_with("grep"),
                "repair_hint should start with a runnable command: {}",
                ex.repair_hint
            );
        }
    }

    #[test]
    fn all_exceptions_have_docs_url() {
        for ex in CATALOG {
            assert!(ex.docs_url.starts_with("docs/"), "docs_url must point into docs/: {}", ex.docs_url);
        }
    }

    #[test]
    fn all_exceptions_have_common_fixes() {
        for ex in CATALOG {
            assert!(!ex.common_fixes.is_empty(), "common_fixes must not be empty");
        }
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn catalog_index_always_in_bounds(idx in 0usize..CATALOG.len()) {
            let ex = &CATALOG[idx];
            prop_assert!(!ex.purpose.is_empty());
            prop_assert!(!ex.reason.is_empty());
            prop_assert!(!ex.docs_url.is_empty());
            prop_assert!(!ex.repair_hint.is_empty());
            prop_assert!(!ex.common_fixes.is_empty());
        }

        #[test]
        fn all_docs_urls_are_local_paths(idx in 0usize..CATALOG.len()) {
            let ex = &CATALOG[idx];
            prop_assert!(
                ex.docs_url.starts_with("docs/"),
                "docs_url must be a local path, got: {}",
                ex.docs_url
            );
        }
    }
}
