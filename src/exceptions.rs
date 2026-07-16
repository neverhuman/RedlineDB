//! Typed, agent-friendly exception surface for the harness.
//!
//! Runtime failures in `redline-testing` are plain `anyhow::Error` values
//! threaded up through the suites. This module classifies a failed run into a
//! typed [`HarnessException`] that carries, for every known failure mode, a
//! `purpose`, a `reason`, a list of common fixes, a `docs_url`, and a
//! `repair_hint`. That turns an opaque stderr line into a repair receipt the
//! next agent (or human) can act on without re-deriving context.

use std::fmt;

/// A classified, agent-friendly description of a harness failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessException {
    /// Stable machine code for the failure class (e.g. `MISSING_REFERENCE_CLI`).
    pub code: &'static str,
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

impl HarnessException {
    /// Classify an `anyhow` error into a typed exception by matching the
    /// lowercased message against known failure signatures, falling back to a
    /// generic-but-still-actionable record.
    pub fn classify(error: &anyhow::Error) -> Self {
        let message = format!("{error:#}").to_ascii_lowercase();
        for candidate in CATALOG {
            if candidate
                .signatures
                .iter()
                .any(|needle| message.contains(needle))
            {
                return candidate.exception;
            }
        }
        GENERIC
    }

    /// Render the exception as a multi-line repair receipt.
    pub fn receipt(&self) -> String {
        let fixes = self
            .common_fixes
            .iter()
            .map(|fix| format!("    - {fix}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "error[{code}]: {purpose}\n  reason: {reason}\n  common fixes:\n{fixes}\n  docs: {docs}\n  repair: {hint}",
            code = self.code,
            purpose = self.purpose,
            reason = self.reason,
            fixes = fixes,
            docs = self.docs_url,
            hint = self.repair_hint,
        )
    }
}

impl fmt::Display for HarnessException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.receipt())
    }
}

struct Classified {
    signatures: &'static [&'static str],
    exception: HarnessException,
}

const GENERIC: HarnessException = HarnessException {
    code: "UNCLASSIFIED",
    purpose: "run a conformance / benchmark suite to completion",
    reason: "the underlying step returned an error the harness has no specific repair record for",
    common_fixes: &[
        "re-run the failing lane with `bash ops/ci/pr-ci.sh` to reproduce locally",
        "inspect the raw JSONL under the suite output directory for the first failing case",
    ],
    docs_url: "docs/testing.md",
    repair_hint: "reproduce with `bash ops/ci/pr-ci.sh`, then narrow to the failing suite",
};

const CATALOG: &[Classified] = &[
    Classified {
        signatures: &[
            "no such file",
            "not found",
            "cannot find",
            "no such program",
            "canonicalize executable",
        ],
        exception: HarnessException {
            code: "MISSING_REFERENCE_CLI",
            purpose: "resolve and probe the reference / target database CLI",
            reason: "the reference or target binary could not be located on PATH",
            common_fixes: &[
                "install the reference CLI and ensure it is on PATH",
                "pass an explicit `--reference-bin` / `--target-bin` path",
            ],
            docs_url: "docs/architecture.md",
            repair_hint: "run `<bin> --version` to confirm the binary resolves before re-running the suite",
        },
    },
    Classified {
        signatures: &[
            "create tmp root",
            "permission denied",
            "read-only",
            "create dir",
        ],
        exception: HarnessException {
            code: "OUTPUT_DIR_UNWRITABLE",
            purpose: "prepare the per-suite output directory for raw JSONL",
            reason: "the output / tmp root could not be created or written",
            common_fixes: &[
                "pass `--output` to a writable directory",
                "ensure the tmp root (or /dev/shm) is writable by the current user",
            ],
            docs_url: "docs/operations.md",
            repair_hint: "set `--output` to a writable path and re-run",
        },
    },
    Classified {
        signatures: &["postgres", "psql", "oracle"],
        exception: HarnessException {
            code: "ORACLE_UNAVAILABLE",
            purpose: "validate beyond-SQLite cases via the psql self-compare oracle",
            reason: "the PostgreSQL oracle (psql / server) was unavailable",
            common_fixes: &[
                "start a local Postgres and set `REDLINE_TESTING_POSTGRES_URL`",
                "skip the beyond_sqlite suite when no oracle is provisioned",
            ],
            docs_url: "docs/boundaries.md",
            repair_hint: "export `REDLINE_TESTING_POSTGRES_URL` then re-run `cargo test --test beyond_sqlite_smoke`",
        },
    },
];

/// Render an `anyhow` error as a classified repair receipt for stderr.
pub fn render(error: &anyhow::Error) -> String {
    format!(
        "{}\n  detail: {error:#}",
        HarnessException::classify(error).receipt()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn classifies_missing_binary() {
        let exc = HarnessException::classify(&anyhow!("No such file or directory (reference cli)"));
        assert_eq!(exc.code, "MISSING_REFERENCE_CLI");
        assert!(exc.receipt().contains("repair:"));
        assert!(exc.receipt().contains("common fixes"));
    }

    #[test]
    fn classifies_unwritable_output() {
        let exc = HarnessException::classify(&anyhow!("create tmp root /ro: Permission denied"));
        assert_eq!(exc.code, "OUTPUT_DIR_UNWRITABLE");
    }

    #[test]
    fn falls_back_to_generic() {
        let error = anyhow!("something unexpected happened");
        let exc = HarnessException::classify(&error);
        assert_eq!(exc.code, "UNCLASSIFIED");
        assert!(!exc.common_fixes.is_empty());
        assert!(render(&error).contains("detail: something unexpected happened"));
    }
}
