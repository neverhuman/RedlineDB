use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

pub(super) const MAX_THREADS: usize = 64;
pub(super) const MAX_OPERATIONS_PER_THREAD: usize = 100_000;
pub(super) const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub(super) const MAX_SESSIONS: usize = 4_096;
pub(super) const MAX_REPETITIONS: usize = 10;
pub(super) const MAX_IDLE_OBSERVATION_SECS: u64 = 60;
pub(super) const MAX_SOAK_OBSERVATION_SECS: u64 = 900;
pub(super) const MAX_MATERIALIZED_PLAN_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SafetyCeilings {
    max_threads: usize,
    max_operations_per_thread: usize,
    max_repetitions: usize,
    max_sessions: usize,
    max_payload_bytes: usize,
    max_materialized_plan_bytes: usize,
    max_idle_observation_secs: u64,
    max_soak_observation_secs: u64,
    max_idle_growth_bytes: u64,
    max_data_bytes: u64,
    storage_stop_threshold_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AttemptReceipt {
    pub schema_version: String,
    pub status: String,
    pub mode: CertMode,
    pub claim_scope: String,
    pub environment: RunEnvironment,
    pub config: CertConfig,
    pub config_sha256: String,
    pub artifact_sha256: String,
    pub approved_profile_sha256: Option<String>,
    pub postgres_image_digest: String,
    pub execution_evidence: ExecutionEvidence,
    pub execution_evidence_sha256: String,
    pub provenance_bound: bool,
    pub storage_contract: StorageContract,
    pub storage_comparison_eligible: bool,
    pub planned_runs: Vec<PlannedRun>,
    pub safety_ceilings: SafetyCeilings,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(super) struct ProgressPoint {
    pub engine: EngineLabel,
    pub threads: usize,
    pub repetition: usize,
    pub execution_order_position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ProgressReceipt {
    schema_version: String,
    status: String,
    completed_runs: usize,
    planned_runs: usize,
    active_engine: Option<EngineLabel>,
    active_point: Option<ProgressPoint>,
    lifecycle_phase: String,
    heartbeat_unix_ms: u64,
    deadline_unix_ms: u64,
    cause: Option<String>,
    last_storage_sample: Option<StorageSample>,
    runs: Vec<EngineRun>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProgressTracker<'a> {
    out_dir: &'a Path,
    planned_runs: usize,
    completed_runs: &'a [EngineRun],
    deadline_unix_ms: u64,
    active_point: Option<ProgressPoint>,
}

impl<'a> ProgressTracker<'a> {
    pub(super) fn matrix(
        out_dir: &'a Path,
        planned_runs: usize,
        completed_runs: &'a [EngineRun],
        deadline_unix_ms: u64,
    ) -> Self {
        Self {
            out_dir,
            planned_runs,
            completed_runs,
            deadline_unix_ms,
            active_point: None,
        }
    }

    pub(super) fn active(self, point: ProgressPoint) -> Self {
        Self {
            active_point: Some(point),
            ..self
        }
    }

    pub(super) fn write(
        &self,
        status: &str,
        lifecycle_phase: &str,
        cause: Option<String>,
        last_storage_sample: Option<StorageSample>,
    ) -> Result<()> {
        atomic_write_json(
            &self.out_dir.join("progress.json"),
            &ProgressReceipt {
                schema_version: SCHEMA_VERSION.to_owned(),
                status: status.to_owned(),
                completed_runs: self.completed_runs.len(),
                planned_runs: self.planned_runs,
                active_engine: self.active_point.as_ref().map(|point| point.engine),
                active_point: self.active_point.clone(),
                lifecycle_phase: lifecycle_phase.to_owned(),
                heartbeat_unix_ms: unix_millis(),
                deadline_unix_ms: self.deadline_unix_ms,
                cause,
                last_storage_sample,
                runs: self.completed_runs.to_vec(),
            },
        )
    }

    pub(super) fn ensure_within_deadline(&self, phase: &str) -> Result<()> {
        if unix_millis() >= self.deadline_unix_ms {
            bail!("certificate deadline exceeded during {phase}");
        }
        Ok(())
    }
}

pub(super) fn safety_ceilings() -> SafetyCeilings {
    SafetyCeilings {
        max_threads: MAX_THREADS,
        max_operations_per_thread: MAX_OPERATIONS_PER_THREAD,
        max_repetitions: MAX_REPETITIONS,
        max_sessions: MAX_SESSIONS,
        max_payload_bytes: MAX_PAYLOAD_BYTES,
        max_materialized_plan_bytes: MAX_MATERIALIZED_PLAN_BYTES,
        max_idle_observation_secs: MAX_IDLE_OBSERVATION_SECS,
        max_soak_observation_secs: MAX_SOAK_OBSERVATION_SECS,
        max_idle_growth_bytes: ABSOLUTE_MAX_IDLE_GROWTH_BYTES,
        max_data_bytes: ABSOLUTE_MAX_DATA_BYTES,
        storage_stop_threshold_bytes: storage_stop_threshold(ABSOLUTE_MAX_DATA_BYTES),
    }
}

pub(super) fn validate_attempt_plan(planned: &[PlannedRun], runs: &[EngineRun]) -> Vec<String> {
    let mut failures = Vec::new();
    if planned.len() != runs.len() {
        failures.push(format!(
            "attempt planned {} runs but final evidence contains {}",
            planned.len(),
            runs.len()
        ));
    }
    for expected in planned {
        let matches = runs
            .iter()
            .filter(|run| {
                run.engine == expected.engine
                    && run.threads == expected.threads
                    && run.repetition == expected.repetition
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            failures.push(format!(
                "attempt plan expected exactly one {} t{} r{} run, found {}",
                expected.engine.as_str(),
                expected.threads,
                expected.repetition,
                matches.len()
            ));
            continue;
        }
        let run = matches[0];
        if run.execution_order_position != expected.execution_order_position
            || run.plan_sha256 != expected.plan_sha256
            || run.delayed_growth_soak != expected.delayed_growth_soak
        {
            failures.push(format!(
                "{} t{} r{} final run differs from the immutable attempt plan",
                expected.engine.as_str(),
                expected.threads,
                expected.repetition
            ));
        }
    }
    failures
}

pub(super) fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("receipt path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("receipt"),
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub(super) fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

pub(super) fn sha256_json(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

pub(super) fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

pub(super) fn current_executable_sha256() -> Result<String> {
    let executable = std::env::current_exe().context("resolve certification executable")?;
    sha256_file(&executable)
        .with_context(|| format!("hash certification executable {}", executable.display()))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_failed_attempt(
    mode: CertMode,
    out_dir: &Path,
    environment: &RunEnvironment,
    config: &CertConfig,
    runs: &[EngineRun],
    approved_profile_sha256: Option<String>,
    execution_evidence: &ValidatedEvidence,
    attempt_receipt_sha256: String,
    reason: String,
) -> Result<()> {
    let raw = RawReceipt {
        schema_version: SCHEMA_VERSION.to_owned(),
        environment: environment.clone(),
        execution_evidence_sha256: execution_evidence.sha256.clone(),
        storage_contract: execution_evidence.document.storage.clone(),
        config: config.clone(),
        runs: runs.to_vec(),
    };
    let raw_path = out_dir.join("raw-runs.json");
    atomic_write_json(&raw_path, &raw)?;
    let manifest = CertManifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        mode,
        status: "fail".to_owned(),
        mechanics_passed: false,
        release_eligible: false,
        bounded_reference_win_eligible: false,
        claim_scope:
            "failed/incomplete attempt; no performance or customer-load claim is authorized"
                .to_owned(),
        canonical_profile: is_canonical_profile(config),
        approved_profile_sha256,
        postgres_image_digest: execution_evidence.document.postgres.image_digest.clone(),
        execution_evidence: execution_evidence.document.clone(),
        execution_evidence_sha256: execution_evidence.sha256.clone(),
        provenance_bound: execution_evidence.source_observation_bound
            && execution_evidence.postgres_provenance_bound,
        storage_contract: execution_evidence.document.storage.clone(),
        storage_comparison_eligible: execution_evidence.storage_comparison_eligible,
        config_sha256: sha256_json(config)?,
        artifact_sha256: current_executable_sha256()?,
        attempt_receipt: "attempt.json".to_owned(),
        attempt_receipt_sha256,
        failure_reasons: vec![reason],
        environment: environment.clone(),
        config: config.clone(),
        raw_receipt: "raw-runs.json".to_owned(),
        raw_receipt_sha256: sha256_file(&raw_path)?,
        comparisons: Vec::new(),
        tested_workload: None,
        reference_cleanup_verified: false,
        storage_claim_scope:
            "failed attempt; storage accounting is safety-only and authorizes no comparison"
                .to_owned(),
    };
    atomic_write_json(&out_dir.join("manifest.json"), &manifest)
}
