//! Versioned requirement denominator, separate from corpus instance counts.
use std::collections::HashSet;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::case::Case;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    schema_version: u32,
    pub profile_id: String,
    sqlite_version: String,
    pub inventory_complete: bool,
    comparison_policy_version: String,
    requirements: Vec<Requirement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    id: String,
    scope: String,
    upstream: String,
    applicability: String,
    test_ids: Vec<usize>,
    status: String,
    evidence: Vec<String>,
}

impl Manifest {
    pub fn load() -> Result<Self> {
        Ok(serde_json::from_str(include_str!(
            "../../profiles/sqlite-3.53.1-app-v1.json"
        ))?)
    }

    pub fn validate(&self, cases: &[Case]) -> Result<()> {
        if self.schema_version != 1
            || self.sqlite_version != "3.53.1"
            || self.comparison_policy_version != "2"
            || self.profile_id != "sqlite-3.53.1-app-v1"
            || self.requirements.is_empty()
        {
            bail!("invalid compatibility manifest identity or empty requirements");
        }
        let known: HashSet<_> = cases.iter().map(|case| case.id).collect();
        let mut ids = HashSet::new();
        for requirement in &self.requirements {
            if requirement.id.trim().is_empty()
                || !ids.insert(&requirement.id)
                || requirement.scope.trim().is_empty()
                || requirement.upstream.trim().is_empty()
            {
                bail!("invalid or duplicate requirement {}", requirement.id);
            }
            let valid_status = match requirement.applicability.as_str() {
                "required" => matches!(requirement.status.as_str(), "open" | "verified"),
                "excluded" => requirement.status == "excluded",
                _ => false,
            };
            if !valid_status {
                bail!("invalid requirement disposition {}", requirement.id);
            }
            let mut test_ids = HashSet::new();
            for id in &requirement.test_ids {
                if !known.contains(id) || !test_ids.insert(id) {
                    bail!("unknown or duplicate case {id} in {}", requirement.id);
                }
            }
            if requirement.status == "verified"
                && (requirement.test_ids.is_empty()
                    || requirement.evidence.is_empty()
                    || requirement
                        .evidence
                        .iter()
                        .any(|entry| entry.trim().is_empty()))
            {
                bail!(
                    "verified requirement lacks cases/evidence: {}",
                    requirement.id
                );
            }
        }
        Ok(())
    }

    pub fn counts(&self) -> (usize, usize) {
        let required = self
            .requirements
            .iter()
            .filter(|r| r.applicability == "required");
        (
            required.clone().filter(|r| r.status == "verified").count(),
            required.count(),
        )
    }

    pub fn qualify(&self, cases: &[Case]) -> Result<()> {
        self.validate(cases)?;
        let (verified, required) = self.counts();
        if !self.inventory_complete || verified != required {
            bail!(
                "profile {} is not qualified: inventory_complete={}, requirements={verified}/{required}",
                self.profile_id,
                self.inventory_complete
            );
        }
        Ok(())
    }
}

const SOURCE_ID: &str =
    "2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9";

pub(super) fn validate_oracle(identity: &super::engine::BinaryIdentity) -> Result<()> {
    let path = std::path::Path::new(&identity.executable_path);
    let prefix = path
        .parent()
        .and_then(std::path::Path::parent)
        .context("oracle binary must have qualified prefix/bin layout")?;
    let receipt_path = prefix.join("oracle-identity.json");
    let receipt = std::fs::read_to_string(&receipt_path).with_context(|| {
        format!(
            "qualified oracle receipt required: {}",
            receipt_path.display()
        )
    })?;
    validate_oracle_receipt(&receipt, &identity.executable_sha256, &identity.version)?;
    Ok(())
}

fn validate_oracle_receipt(receipt: &str, binary_hash: &str, version: &str) -> Result<()> {
    let receipt: serde_json::Value = serde_json::from_str(receipt)?;
    if receipt["schema_version"] != 1
        || receipt["sqlite_version"] != "3.53.1"
        || receipt["source_id"] != SOURCE_ID
        || receipt["profile"] != "extended"
        || receipt["probes"]["cli_and_library"] != "passed"
        || receipt["artifacts"]["bin/sqlite3"].as_str() != Some(binary_hash)
        || version.split_whitespace().next() != Some("3.53.1")
        || !version.contains(SOURCE_ID)
    {
        bail!(
            "oracle identity mismatch: expected pinned SQLite 3.53.1 extended reference and matching receipt/binary hash"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_manifest_is_valid_but_cannot_claim_qualification() {
        let manifest = Manifest::load().unwrap();
        let cases = super::super::catalog::all_cases().unwrap();
        manifest.validate(&cases).unwrap();
        assert!(manifest.qualify(&cases).is_err());
        assert_eq!(manifest.counts().0, 0);
    }

    #[test]
    fn manifest_rejects_fabricated_verification_and_case_references() {
        let cases = super::super::catalog::all_cases().unwrap();
        let mut manifest = Manifest::load().unwrap();
        manifest.requirements[0].status = "verified".into();
        assert!(manifest.validate(&cases).is_err());
        manifest.requirements[0].status = "open".into();
        manifest.requirements[0].test_ids = vec![usize::MAX];
        assert!(manifest.validate(&cases).is_err());
        manifest.requirements[0].test_ids.clear();
        manifest.requirements[1].id = manifest.requirements[0].id.clone();
        assert!(manifest.validate(&cases).is_err());
        manifest.requirements.clear();
        assert!(manifest.validate(&cases).is_err());
    }
    #[test]
    fn oracle_receipts_reject_missing_wrong_version_source_and_hash() {
        let receipt = serde_json::json!({"schema_version":1,"sqlite_version":"3.53.1",
            "source_id":SOURCE_ID,"profile":"extended","probes":{"cli_and_library":"passed"},
            "artifacts":{"bin/sqlite3":"actual-hash"}});
        let version = format!("3.53.1 {SOURCE_ID} (64-bit)");
        assert!(validate_oracle_receipt(&receipt.to_string(), "actual-hash", &version).is_ok());
        assert!(validate_oracle_receipt(&receipt.to_string(), "wrong-hash", &version).is_err());
        assert!(validate_oracle_receipt(&receipt.to_string(), "actual-hash", "3.45.1").is_err());
        for field in ["sqlite_version", "source_id", "profile"] {
            let mut wrong = receipt.clone();
            wrong[field] = "wrong".into();
            assert!(validate_oracle_receipt(&wrong.to_string(), "actual-hash", &version).is_err());
        }
        assert!(
            validate_oracle(&super::super::engine::BinaryIdentity {
                executable_path: "/nonexistent-qualified-reference/bin/sqlite3".into(),
                executable_sha256: "actual-hash".into(),
                version,
            })
            .is_err()
        );
        let mut manifest = Manifest::load().unwrap();
        manifest.profile_id = "renamed".into();
        assert!(
            manifest
                .validate(&super::super::catalog::all_cases().unwrap())
                .is_err()
        );
    }
}
