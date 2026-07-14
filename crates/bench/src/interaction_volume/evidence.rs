use std::path::Path;
use std::process::Command;

use super::*;
use crate::report::collect_environment;

pub(super) const EXECUTION_EVIDENCE_SCHEMA: &str =
    "redline.interaction-volume-execution-evidence/v1";
const EVIDENCE_GENERATOR: &str = "ops/ci/interaction-volume-cert.sh";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresExecutionEvidence {
    pub image_digest: String,
    pub digest_observation: String,
    pub digest_verified: bool,
    pub backend: String,
    pub isolation_verified: bool,
    pub dedicated_instance: bool,
    pub endpoint_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageContract {
    pub class: String,
    pub local_database_root: String,
    pub postgres_data_root: String,
    pub local_mount_identity: String,
    pub postgres_mount_identity: String,
    pub same_mount: bool,
    pub durable: bool,
    pub postgres_bind_mode: String,
}

impl StorageContract {
    pub(super) fn comparison_eligible(&self, postgres: &PostgresExecutionEvidence) -> bool {
        self.class == "shared_host_durable_bind"
            && self.same_mount
            && self.durable
            && self.postgres_bind_mode == "rw"
            && self.local_mount_identity == self.postgres_mount_identity
            && postgres.backend == "docker_bind"
            && postgres.digest_verified
            && postgres.isolation_verified
            && postgres.dedicated_instance
            && postgres.endpoint_scope == "loopback"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvidence {
    pub schema_version: String,
    pub generator: String,
    pub source_commit: String,
    pub source_dirty: bool,
    pub binary_sha256: String,
    pub postgres: PostgresExecutionEvidence,
    pub storage: StorageContract,
}

#[derive(Debug, Clone)]
pub(super) struct ValidatedEvidence {
    pub document: ExecutionEvidence,
    pub sha256: String,
    pub source_observation_bound: bool,
    pub postgres_provenance_bound: bool,
    pub storage_comparison_eligible: bool,
}

pub(super) fn load_execution_evidence(
    path: &Path,
    executable_sha256: &str,
) -> Result<ValidatedEvidence> {
    let bytes =
        fs::read(path).with_context(|| format!("read execution evidence {}", path.display()))?;
    let document: ExecutionEvidence = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode execution evidence {}", path.display()))?;
    if document.schema_version != EXECUTION_EVIDENCE_SCHEMA {
        bail!(
            "execution evidence schema must be {EXECUTION_EVIDENCE_SCHEMA}, got {}",
            document.schema_version
        );
    }
    if document.generator != EVIDENCE_GENERATOR {
        bail!(
            "execution evidence generator must be {EVIDENCE_GENERATOR}, got {}",
            document.generator
        );
    }
    if document.binary_sha256 != executable_sha256 {
        bail!(
            "execution evidence binary digest {} differs from running executable {}",
            document.binary_sha256,
            executable_sha256
        );
    }
    if document.postgres.image_digest != PINNED_POSTGRES_IMAGE_DIGEST {
        bail!(
            "execution evidence PostgreSQL digest differs from pinned {}",
            PINNED_POSTGRES_IMAGE_DIGEST
        );
    }
    validate_source_commit(&document.source_commit)?;
    let controlled_source = controlled_git_source();
    if let Some((commit, dirty)) = &controlled_source {
        if commit != &document.source_commit || dirty != &document.source_dirty {
            bail!(
                "wrapper source evidence does not match the checked-out repository (commit {commit}, dirty {dirty})"
            );
        }
    }
    let postgres_provenance_bound = document.postgres.digest_verified
        && document.postgres.digest_observation == "docker_image_inspect_repo_digest"
        && document.postgres.backend == "docker_bind"
        && document.postgres.isolation_verified
        && document.postgres.dedicated_instance
        && document.postgres.endpoint_scope == "loopback";
    let storage_comparison_eligible = document.storage.comparison_eligible(&document.postgres);
    Ok(ValidatedEvidence {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        document,
        source_observation_bound: controlled_source.is_some(),
        postgres_provenance_bound,
        storage_comparison_eligible,
    })
}

pub(super) fn controlled_environment(evidence: &ValidatedEvidence) -> RunEnvironment {
    let mut environment = collect_environment();
    environment.git_sha = evidence
        .source_observation_bound
        .then(|| evidence.document.source_commit.clone());
    environment.git_dirty = evidence
        .source_observation_bound
        .then_some(evidence.document.source_dirty);
    // The generic report environment accepts a caller-provided image digest. This certificate
    // records image identity only through the validated execution evidence above.
    environment.image_digest = None;
    environment
}

fn validate_source_commit(commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("execution evidence source commit must be a full 40-character hexadecimal SHA");
    }
    Ok(())
}

fn controlled_git_source() -> Option<(String, bool)> {
    let commit = command_output(&["rev-parse", "HEAD"])?;
    validate_source_commit(&commit).ok()?;
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .ok()?;
    if !status.status.success() {
        return None;
    }
    Some((commit, !status.stdout.is_empty()))
}

fn command_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
