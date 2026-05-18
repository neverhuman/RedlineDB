//! Typed exception surface for agent-friendly repair receipts.
//!
//! Every `DomainError` carries enough context for the next agent to
//! locate the failure, run the right proof lane, and apply a known
//! fix without rereading the source.

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct DomainError {
    pub purpose: &'static str,
    pub reason: String,
    pub common_fixes: &'static [&'static str],
    pub docs_url: &'static str,
    pub repair_hint: &'static str,
    pub source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (docs: {}; repair: {})",
            self.purpose, self.reason, self.docs_url, self.repair_hint,
        )
    }
}

impl Error for DomainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn Error + 'static))
    }
}

impl DomainError {
    pub fn new(
        purpose: &'static str,
        reason: impl Into<String>,
        common_fixes: &'static [&'static str],
        docs_url: &'static str,
        repair_hint: &'static str,
    ) -> Self {
        Self {
            purpose,
            reason: reason.into(),
            common_fixes,
            docs_url,
            repair_hint,
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    const FIXES: &[&str] = &[
        "rerun `just fast` to confirm the failure reproduces",
        "consult docs/audit-rubric.md for the dimension owner",
    ];

    #[test]
    fn new_populates_all_six_fields() {
        let err = DomainError::new(
            "kernel.page_file.read",
            "page id zero is invalid",
            FIXES,
            "docs/audit-rubric.md#observability",
            "verify caller is not passing PageId(0)",
        );

        assert_eq!(err.purpose, "kernel.page_file.read");
        assert_eq!(err.reason, "page id zero is invalid");
        assert_eq!(err.common_fixes, FIXES);
        assert_eq!(err.docs_url, "docs/audit-rubric.md#observability");
        assert_eq!(err.repair_hint, "verify caller is not passing PageId(0)");
        assert!(err.source.is_none());
    }

    #[test]
    fn display_formats_purpose_reason_docs_and_repair() {
        let err = DomainError::new(
            "kernel.checksum",
            "expected 0xdeadbeef got 0x00000000",
            FIXES,
            "docs/testing.md#proof-lanes",
            "rerun integrity::verify on the file",
        );

        let rendered = err.to_string();
        assert_eq!(
            rendered,
            "kernel.checksum: expected 0xdeadbeef got 0x00000000 \
             (docs: docs/testing.md#proof-lanes; repair: rerun integrity::verify on the file)"
        );
    }

    #[test]
    fn source_chains_to_inner_error() {
        let inner = io::Error::new(io::ErrorKind::InvalidData, "checksum mismatch");
        let err = DomainError::new(
            "kernel.checksum",
            "page checksum did not match header",
            FIXES,
            "docs/audit-rubric.md#observability",
            "rerun integrity::verify on the file",
        )
        .with_source(inner);

        let source = err.source().expect("source should be populated");
        assert_eq!(source.to_string(), "checksum mismatch");
        assert!(err.source.is_some());
    }
}
