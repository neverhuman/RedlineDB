//! Identical-workload Redline/SQLite/PostgreSQL interaction-volume harness.
//!
//! This lane is deliberately separate from SQLite compatibility evidence. It models the bounded
//! Jain session/event interaction shape, preserves every per-run sample, observes storage after
//! work has stopped, and refuses to authorize a claim unless all three real engines run.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{DurabilityKind, EngineKind, RunSpec, WorkloadKind};
use crate::engine::{
    BenchConn, BenchEngine, CellValue, PostgresEngine, RedlineEngine, SqliteEngine, hash_cell,
};
use crate::metrics::{FailureKind, Metrics};
use crate::report::{MetricsSummary, RunEnvironment};

mod evidence;
mod model;
mod oracle;
mod receipt;
use evidence::*;
use model::*;
use oracle::*;
use receipt::*;

const SCHEMA_VERSION: &str = "redline.interaction-volume-cert/v4";
const POSTGRES_URL_ENV: &str = "REDLINEDB_BENCH_POSTGRES_URL";
const PINNED_POSTGRES_IMAGE_DIGEST: &str =
    "sha256:786dab398303b8ce7cb76b407bb21ef2e4dfbbbd4c6abcf3d29b3130467ffdbc";
const STORAGE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const PROGRESS_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
const MAX_LATENCY_SAMPLES: usize = 4_096;
const MAX_INTERACTION_ATTEMPTS: usize = 8;
const INTERACTION_RETRY_DEADLINE: Duration = Duration::from_secs(5);
const KILL_SWITCH_ENV: &str = "REDLINEDB_BENCH_KILL";
const APPROVED_PROFILE_SCHEMA_VERSION: &str = "redline.interaction-volume-profile/v1";
const APPROVED_PROFILE_ID: &str = "jain-session-event-daily-v1";
const APPROVED_DAILY_PROFILE_SHA256: &str =
    "165ffdffd7bec26f4ef7aa361a02f041a61fb62f8a3f58abc7e447ecf890cbb7";

const CANONICAL_THREADS: &[usize] = &[1, 4, 16];
const CANONICAL_OPERATIONS_PER_THREAD: usize = 10_000;
const CANONICAL_REPETITIONS: usize = 7;
const CANONICAL_SESSIONS: usize = 64;
const CANONICAL_PAYLOAD_BYTES: usize = 256;
const CANONICAL_WARMUP_OPERATIONS_PER_THREAD: usize = 100;
const CANONICAL_IDLE_OBSERVATION_SECS: u64 = 5;
const CANONICAL_SOAK_OBSERVATION_SECS: u64 = 300;
const CANONICAL_MAX_IDLE_GROWTH_BYTES: u64 = 16 * 1024 * 1024;
const CANONICAL_MAX_DATA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const ABSOLUTE_MAX_IDLE_GROWTH_BYTES: u64 = CANONICAL_MAX_IDLE_GROWTH_BYTES;
const ABSOLUTE_MAX_DATA_BYTES: u64 = CANONICAL_MAX_DATA_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CertMode {
    /// Fast mechanics/integrity check. Never authorizes release or a performance claim.
    Smoke,
    /// Fail-closed canonical release profile.
    Release,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "interaction-volume-cert")]
pub struct InteractionVolumeArgs {
    /// Directory receiving raw-runs.json and manifest.json.
    #[arg(long)]
    pub out_dir: PathBuf,
    /// Smoke never authorizes release; release requires the immutable canonical profile.
    #[arg(long, value_enum, default_value_t = CertMode::Smoke)]
    pub mode: CertMode,
    /// Digest-bound checked-in profile. Mandatory for release mode.
    #[arg(long)]
    pub approved_profile: Option<PathBuf>,
    /// Wrapper-generated, independently checked execution and storage evidence.
    #[arg(long)]
    pub execution_evidence: PathBuf,
    /// Wall-clock deadline recorded in progress receipts and enforced by the wrapper.
    #[arg(long, default_value_t = 300)]
    pub deadline_secs: u64,
    /// Wrapper-derived absolute deadline; prevents setup time from drifting from timeout evidence.
    #[arg(long)]
    pub deadline_unix_ms: Option<u64>,
    /// Comma-separated concurrent interaction workers.
    #[arg(long, value_delimiter = ',', default_value = "1,4,16")]
    pub threads: Vec<usize>,
    #[arg(long, default_value_t = CANONICAL_OPERATIONS_PER_THREAD)]
    pub operations_per_thread: usize,
    #[arg(long, default_value_t = CANONICAL_REPETITIONS)]
    pub repetitions: usize,
    #[arg(long, default_value_t = CANONICAL_SESSIONS)]
    pub sessions: usize,
    #[arg(long, default_value_t = CANONICAL_PAYLOAD_BYTES)]
    pub payload_bytes: usize,
    #[arg(long, default_value_t = CANONICAL_WARMUP_OPERATIONS_PER_THREAD)]
    pub warmup_operations_per_thread: usize,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    /// Filesystem observation window after the final checkpoint and all customer work has stopped.
    #[arg(long, default_value_t = CANONICAL_IDLE_OBSERVATION_SECS)]
    pub idle_observation_secs: u64,
    /// Redline-only observation after the largest final workload; covers delayed runaway writes.
    #[arg(long, default_value_t = CANONICAL_SOAK_OBSERVATION_SECS)]
    pub soak_observation_secs: u64,
    /// Maximum permitted post-workload data+WAL growth during the idle observation.
    #[arg(long, default_value_t = CANONICAL_MAX_IDLE_GROWTH_BYTES)]
    pub max_idle_growth_bytes: u64,
    /// Maximum accepted on-disk state for any one run.
    #[arg(long, default_value_t = CANONICAL_MAX_DATA_BYTES)]
    pub max_data_bytes: u64,
}

pub fn run(args: &InteractionVolumeArgs) -> Result<CertManifest> {
    let _output_lock = OutputLock::acquire(&args.out_dir)?;
    let config = validated_config(args)?;
    let approved_profile_sha256 = validate_approved_profile(args, &config)?;
    let artifact_sha256 = current_executable_sha256()?;
    if args.execution_evidence != args.out_dir.join("execution-evidence.json") {
        bail!("execution evidence must be the wrapper-owned file inside the output directory");
    }
    let postgres_url = std::env::var(POSTGRES_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .context("PostgreSQL is mandatory: set REDLINEDB_BENCH_POSTGRES_URL")?;
    let execution_evidence =
        load_execution_evidence(&args.execution_evidence, &artifact_sha256, &postgres_url)?;
    let endpoint_scope = PostgresEngine::endpoint_scope(&postgres_url)?;
    if endpoint_scope != execution_evidence.document.postgres.endpoint_scope {
        bail!(
            "PostgreSQL endpoint scope {endpoint_scope} differs from execution evidence {}",
            execution_evidence.document.postgres.endpoint_scope
        );
    }
    let postgres_image_digest = execution_evidence.document.postgres.image_digest.clone();
    let environment = controlled_environment(&execution_evidence);
    let planned_runs = planned_runs(&config)?;
    let deadline_unix_ms = args
        .deadline_unix_ms
        .unwrap_or_else(|| unix_millis().saturating_add(args.deadline_secs.saturating_mul(1_000)));
    let provenance_bound =
        execution_evidence.source_observation_bound && execution_evidence.postgres_provenance_bound;
    let attempt = AttemptReceipt {
        schema_version: SCHEMA_VERSION.to_owned(),
        status: "in_progress".to_owned(),
        mode: args.mode,
        claim_scope: "attempt started; no performance or customer-load claim is authorized"
            .to_owned(),
        environment: environment.clone(),
        config: config.clone(),
        config_sha256: sha256_json(&config)?,
        artifact_sha256: artifact_sha256.clone(),
        approved_profile_sha256: approved_profile_sha256.clone(),
        postgres_image_digest: postgres_image_digest.clone(),
        execution_evidence: execution_evidence.document.clone(),
        execution_evidence_sha256: execution_evidence.sha256.clone(),
        live_postgres_observation: execution_evidence.live_postgres.clone(),
        provenance_bound,
        ci_trigger_bound: execution_evidence.ci_trigger_bound,
        storage_contract: execution_evidence.document.storage.clone(),
        storage_comparison_eligible: execution_evidence.storage_comparison_eligible,
        planned_runs: planned_runs.clone(),
        safety_ceilings: safety_ceilings(),
    };
    let attempt_path = args.out_dir.join("attempt.json");
    atomic_write_json(&attempt_path, &attempt)?;
    let attempt_receipt_sha256 = sha256_file(&attempt_path)?;

    let mut runs = Vec::new();
    ProgressTracker::matrix(&args.out_dir, planned_runs.len(), &runs, deadline_unix_ms).write(
        "in_progress",
        "matrix_start",
        None,
        None,
    )?;
    if args.mode == CertMode::Release
        && (!execution_evidence.source_observation_bound
            || environment.git_dirty != Some(false)
            || !execution_evidence.postgres_provenance_bound
            || !execution_evidence.storage_comparison_eligible
            || !execution_evidence.ci_trigger_bound)
    {
        let reason = "release preflight requires clean repository-observed source, a live digest-verified owned PostgreSQL container, one shared durable host bind, and runtime CI trigger evidence"
            .to_owned();
        ProgressTracker::matrix(&args.out_dir, planned_runs.len(), &runs, deadline_unix_ms).write(
            "failed",
            "release_preflight",
            Some(reason.clone()),
            None,
        )?;
        write_failed_attempt(FailedAttempt {
            mode: args.mode,
            out_dir: &args.out_dir,
            environment: &environment,
            config: &config,
            runs: &runs,
            approved_profile_sha256,
            execution_evidence: &execution_evidence,
            attempt_receipt_sha256,
            reason: reason.clone(),
        })?;
        bail!(reason);
    }
    let database_root = args.out_dir.join("dbs");
    fs::create_dir_all(&database_root)?;
    let max_threads = config.threads.iter().copied().max().unwrap_or(1);
    for (thread_index, &threads) in config.threads.iter().enumerate() {
        let plan = interaction_plan(&config, threads);
        let plan_sha256 = sha256_json(&plan)?;
        for repetition in 0..config.repetitions {
            for (execution_order_position, engine) in
                engine_order(thread_index, repetition, config.seed)
                    .into_iter()
                    .enumerate()
            {
                let run_dir =
                    database_root.join(format!("{}-t{threads}-r{repetition}", engine.as_str()));
                let delayed_growth_soak = engine == EngineLabel::Redline
                    && threads == max_threads
                    && repetition + 1 == config.repetitions;
                let idle_observation_secs = if delayed_growth_soak {
                    config.soak_observation_secs
                } else {
                    config.idle_observation_secs
                };
                let progress = ProgressTracker::matrix(
                    &args.out_dir,
                    planned_runs.len(),
                    &runs,
                    deadline_unix_ms,
                )
                .active(ProgressPoint {
                    engine,
                    threads,
                    repetition,
                    execution_order_position,
                });
                progress.write("in_progress", "run_start", None, None)?;
                let planned_run = PlannedRun {
                    engine,
                    threads,
                    repetition,
                    execution_order_position,
                    plan_sha256: plan_sha256.clone(),
                    delayed_growth_soak,
                };
                let result = run_engine(
                    &planned_run,
                    &plan,
                    &run_dir,
                    EngineRunContext {
                        config: &config,
                        postgres_url: &postgres_url,
                        idle_observation_secs,
                        shared_storage_contract: execution_evidence.storage_comparison_eligible,
                        progress,
                    },
                );
                match result {
                    Ok(run) => {
                        runs.push(run);
                        ProgressTracker::matrix(
                            &args.out_dir,
                            planned_runs.len(),
                            &runs,
                            deadline_unix_ms,
                        )
                        .write(
                            "in_progress",
                            "run_complete",
                            None,
                            None,
                        )?;
                    }
                    Err(error) => {
                        let reason = format!(
                            "{} t{} r{} failed: {error:#}",
                            engine.as_str(),
                            threads,
                            repetition
                        );
                        progress.write("failed", "run_failed", Some(reason.clone()), None)?;
                        write_failed_attempt(FailedAttempt {
                            mode: args.mode,
                            out_dir: &args.out_dir,
                            environment: &environment,
                            config: &config,
                            runs: &runs,
                            approved_profile_sha256: approved_profile_sha256.clone(),
                            execution_evidence: &execution_evidence,
                            attempt_receipt_sha256: attempt_receipt_sha256.clone(),
                            reason: reason.clone(),
                        })?;
                        return Err(error).with_context(|| reason);
                    }
                }
            }
        }
    }

    let raw = RawReceipt {
        schema_version: SCHEMA_VERSION.to_owned(),
        environment: environment.clone(),
        execution_evidence_sha256: execution_evidence.sha256.clone(),
        live_postgres_observation: execution_evidence.live_postgres.clone(),
        storage_contract: execution_evidence.document.storage.clone(),
        config: config.clone(),
        runs: runs.clone(),
    };
    let raw_path = args.out_dir.join("raw-runs.json");
    atomic_write_json(&raw_path, &raw)?;
    let raw_receipt_sha256 = sha256_file(&raw_path)?;
    let comparisons = comparisons(&runs, &config.threads)?;
    let mut failure_reasons = validate_runs(&runs, &config);
    failure_reasons.extend(validate_attempt_plan(&planned_runs, &runs));
    let reference_cleanup_verified = match PostgresEngine::benchmark_schema_count(&postgres_url) {
        Ok(0) => true,
        Ok(remaining) => {
            failure_reasons.push(format!(
                "PostgreSQL benchmark cleanup left {remaining} schemas"
            ));
            false
        }
        Err(error) => {
            failure_reasons.push(format!(
                "could not verify PostgreSQL benchmark cleanup: {error:#}"
            ));
            false
        }
    };
    let mechanics_passed = failure_reasons.is_empty();
    for comparison in &comparisons {
        if !comparison.bounded_result_eligible {
            failure_reasons.push(format!(
                "t{} Redline did not beat both references on throughput and p99 in every repetition",
                comparison.threads
            ));
        }
    }
    let canonical_profile = is_canonical_profile(&config);
    if args.mode == CertMode::Release && !canonical_profile {
        failure_reasons.push("release mode requires the immutable canonical profile".to_owned());
    }
    if args.mode == CertMode::Release {
        if !execution_evidence.source_observation_bound {
            failure_reasons
                .push("release evidence lacks a repository-observed source commit".to_owned());
        }
        if environment.git_dirty != Some(false) {
            failure_reasons.push("release evidence requires a clean source tree".to_owned());
        }
        if !execution_evidence.postgres_provenance_bound {
            failure_reasons.push(
                "release evidence lacks verified PostgreSQL image/isolation provenance".to_owned(),
            );
        }
        if !execution_evidence.storage_comparison_eligible {
            failure_reasons.push(
                "release evidence requires one durable shared host mount for all three engines"
                    .to_owned(),
            );
        }
        if !execution_evidence.ci_trigger_bound {
            failure_reasons.push(
                "release evidence requires a runtime-bound CI schedule/manual trigger receipt"
                    .to_owned(),
            );
        }
    } else {
        failure_reasons.push("smoke mode is informational and cannot authorize release".to_owned());
        if !execution_evidence.storage_comparison_eligible {
            failure_reasons.push(
                "smoke storage is unmatched; no bounded reference-win claim is authorized"
                    .to_owned(),
            );
        }
    }
    failure_reasons.sort();
    failure_reasons.dedup();
    let release_eligible = args.mode == CertMode::Release && failure_reasons.is_empty();
    let max_observed_data_bytes = runs
        .iter()
        .flat_map(all_storage_samples)
        .map(storage_total)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    let config_sha256 = sha256_json(&config)?;
    let manifest = CertManifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        mode: args.mode,
        status: match (args.mode, release_eligible) {
            (CertMode::Release, true) => "pass",
            (CertMode::Release, false) => "fail",
            (CertMode::Smoke, _) if mechanics_passed => "smoke_complete",
            (CertMode::Smoke, _) => "smoke_failed",
        }
        .to_owned(),
        mechanics_passed,
        release_eligible,
        bounded_reference_win_eligible: release_eligible,
        claim_scope: match args.mode {
            CertMode::Release => "exact seeded closed-loop Jain interaction points only; not a general database or untested customer-load claim",
            CertMode::Smoke => "mechanics and integrity exercise only; no competitive, bounded-win, release, or customer-load claim is authorized",
        }
        .to_owned(),
        canonical_profile,
        approved_profile_sha256,
        postgres_image_digest,
        execution_evidence: execution_evidence.document.clone(),
        execution_evidence_sha256: execution_evidence.sha256.clone(),
        live_postgres_observation: execution_evidence.live_postgres.clone(),
        provenance_bound,
        ci_trigger_bound: execution_evidence.ci_trigger_bound,
        storage_contract: execution_evidence.document.storage.clone(),
        storage_comparison_eligible: execution_evidence.storage_comparison_eligible,
        config_sha256,
        artifact_sha256,
        attempt_receipt: "attempt.json".to_owned(),
        attempt_receipt_sha256,
        cleanup_receipt: None,
        cleanup_receipt_sha256: None,
        failure_reasons,
        environment,
        config: config.clone(),
        raw_receipt: "raw-runs.json".to_owned(),
        raw_receipt_sha256,
        comparisons,
        tested_workload: Some(TestedWorkload {
            exact_concurrent_worker_points: config.threads.clone(),
            operations_per_worker: config.operations_per_thread,
            total_operations_at_each_point: config
                .threads
                .iter()
                .map(|threads| config.operations_per_thread as u64 * *threads as u64)
                .collect(),
            sessions: config.sessions,
            payload_bytes: config.payload_bytes,
            seed: config.seed,
            append_percent: 70,
            point_read_percent: 20,
            replay_percent: 10,
            max_observed_data_bytes,
            storage_stop_threshold_bytes: storage_stop_threshold(config.max_data_bytes),
            storage_class: execution_evidence.document.storage.class.clone(),
            durability: config.durability.clone(),
            delayed_growth_soak_secs: config.soak_observation_secs,
        }),
        reference_cleanup_verified,
        storage_claim_scope: "PostgreSQL measures recursive PGDATA apparent bytes plus Docker SizeRw; this is an absolute safety scope, not a complete PostgreSQL, container, host, or cross-engine footprint comparison".to_owned(),
    };
    atomic_write_json(&args.out_dir.join("manifest.json"), &manifest)?;
    let final_progress =
        ProgressTracker::matrix(&args.out_dir, planned_runs.len(), &runs, deadline_unix_ms);
    if (args.mode == CertMode::Release && !manifest.release_eligible)
        || (args.mode == CertMode::Smoke && !manifest.mechanics_passed)
    {
        final_progress.write(
            "failed",
            "matrix_complete",
            manifest.failure_reasons.first().cloned(),
            None,
        )?;
    } else {
        final_progress.write("completed", "matrix_complete", None, None)?;
    }
    if (args.mode == CertMode::Release && !manifest.release_eligible)
        || (args.mode == CertMode::Smoke && !manifest.mechanics_passed)
    {
        bail!(
            "interaction-volume run failed; inspect {}/manifest.json",
            args.out_dir.display()
        );
    }
    Ok(manifest)
}

fn validate_approved_profile(
    args: &InteractionVolumeArgs,
    config: &CertConfig,
) -> Result<Option<String>> {
    let Some(path) = args.approved_profile.as_deref() else {
        if args.mode == CertMode::Release {
            bail!("release mode requires --approved-profile with the digest-bound daily profile");
        }
        return Ok(None);
    };
    let digest =
        sha256_file(path).with_context(|| format!("hash approved profile {}", path.display()))?;
    if digest != APPROVED_DAILY_PROFILE_SHA256 {
        bail!(
            "approved profile digest mismatch: expected {APPROVED_DAILY_PROFILE_SHA256}, got {digest}"
        );
    }
    let document: ApprovedProfileDocument = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("read approved profile {}", path.display()))?,
    )
    .with_context(|| format!("decode approved profile {}", path.display()))?;
    if document.schema_version != APPROVED_PROFILE_SCHEMA_VERSION
        || document.profile_id != APPROVED_PROFILE_ID
    {
        bail!("approved profile identity does not match the certified daily profile");
    }
    if document.config != *config {
        bail!("command-line configuration differs from the approved daily profile");
    }
    Ok(Some(digest))
}

fn planned_runs(config: &CertConfig) -> Result<Vec<PlannedRun>> {
    let max_threads = config.threads.iter().copied().max().unwrap_or(1);
    let mut planned = Vec::with_capacity(config.threads.len() * config.repetitions * 3);
    for (thread_index, &threads) in config.threads.iter().enumerate() {
        let plan_sha256 = sha256_json(&interaction_plan(config, threads))?;
        for repetition in 0..config.repetitions {
            for (execution_order_position, engine) in
                engine_order(thread_index, repetition, config.seed)
                    .into_iter()
                    .enumerate()
            {
                planned.push(PlannedRun {
                    engine,
                    threads,
                    repetition,
                    execution_order_position,
                    plan_sha256: plan_sha256.clone(),
                    delayed_growth_soak: engine == EngineLabel::Redline
                        && threads == max_threads
                        && repetition + 1 == config.repetitions,
                });
            }
        }
    }
    Ok(planned)
}

fn validated_config(args: &InteractionVolumeArgs) -> Result<CertConfig> {
    if args.deadline_secs == 0 || args.deadline_secs > 86_400 {
        bail!("deadline-secs must be in 1..=86400");
    }
    if let Some(deadline) = args.deadline_unix_ms {
        let now = unix_millis();
        if deadline <= now || deadline > now.saturating_add(86_400_000) {
            bail!("deadline-unix-ms must be in the next 24 hours");
        }
    }
    let mut threads = args.threads.clone();
    threads.sort_unstable();
    threads.dedup();
    if threads.is_empty()
        || threads
            .iter()
            .any(|value| *value == 0 || *value > MAX_THREADS)
    {
        bail!("threads must contain values in 1..={MAX_THREADS}");
    }
    if args.operations_per_thread == 0 || args.operations_per_thread > MAX_OPERATIONS_PER_THREAD {
        bail!("operations-per-thread must be in 1..={MAX_OPERATIONS_PER_THREAD}");
    }
    if args.repetitions == 0 || args.repetitions > MAX_REPETITIONS {
        bail!("repetitions must be in 1..={MAX_REPETITIONS}");
    }
    if args.sessions == 0 || args.sessions > MAX_SESSIONS {
        bail!("sessions must be in 1..={MAX_SESSIONS}");
    }
    if args.payload_bytes == 0 || args.payload_bytes > MAX_PAYLOAD_BYTES {
        bail!("payload-bytes must be in 1..={MAX_PAYLOAD_BYTES}");
    }
    if args.warmup_operations_per_thread > MAX_OPERATIONS_PER_THREAD {
        bail!("warmup-operations-per-thread must be in 0..={MAX_OPERATIONS_PER_THREAD}");
    }
    if args.idle_observation_secs == 0 || args.idle_observation_secs > MAX_IDLE_OBSERVATION_SECS {
        bail!("idle-observation-secs must be in 1..={MAX_IDLE_OBSERVATION_SECS}");
    }
    if args.soak_observation_secs == 0 || args.soak_observation_secs > MAX_SOAK_OBSERVATION_SECS {
        bail!("soak-observation-secs must be in 1..={MAX_SOAK_OBSERVATION_SECS}");
    }
    if args.max_idle_growth_bytes > ABSOLUTE_MAX_IDLE_GROWTH_BYTES {
        bail!(
            "max-idle-growth-bytes must not exceed the absolute ceiling {ABSOLUTE_MAX_IDLE_GROWTH_BYTES}"
        );
    }
    if args.max_data_bytes == 0 || args.max_data_bytes > ABSOLUTE_MAX_DATA_BYTES {
        bail!("max-data-bytes must be in 1..={ABSOLUTE_MAX_DATA_BYTES} (absolute safety ceiling)");
    }
    if args.max_idle_growth_bytes > args.max_data_bytes {
        bail!("max-idle-growth-bytes must not exceed max-data-bytes");
    }
    let max_threads = threads.iter().copied().max().unwrap_or(1);
    let materialized_plan_bytes = max_threads
        .checked_mul(args.operations_per_thread)
        .and_then(|value| value.checked_mul(args.payload_bytes.saturating_add(128)))
        .context("interaction plan byte calculation overflowed")?;
    if materialized_plan_bytes > MAX_MATERIALIZED_PLAN_BYTES {
        bail!(
            "materialized interaction plan would use at least {materialized_plan_bytes} bytes (limit {MAX_MATERIALIZED_PLAN_BYTES})"
        );
    }
    Ok(CertConfig {
        threads,
        operations_per_thread: args.operations_per_thread,
        repetitions: args.repetitions,
        sessions: args.sessions,
        payload_bytes: args.payload_bytes,
        warmup_operations_per_thread: args.warmup_operations_per_thread,
        seed: args.seed,
        durability: "strict".to_owned(),
        idle_observation_secs: args.idle_observation_secs,
        soak_observation_secs: args.soak_observation_secs,
        max_idle_growth_bytes: args.max_idle_growth_bytes,
        max_data_bytes: args.max_data_bytes,
    })
}

fn is_canonical_profile(config: &CertConfig) -> bool {
    config.threads == CANONICAL_THREADS
        && config.operations_per_thread == CANONICAL_OPERATIONS_PER_THREAD
        && config.repetitions == CANONICAL_REPETITIONS
        && config.sessions == CANONICAL_SESSIONS
        && config.payload_bytes == CANONICAL_PAYLOAD_BYTES
        && config.warmup_operations_per_thread == CANONICAL_WARMUP_OPERATIONS_PER_THREAD
        && config.seed == 7
        && config.idle_observation_secs == CANONICAL_IDLE_OBSERVATION_SECS
        && config.soak_observation_secs == CANONICAL_SOAK_OBSERVATION_SECS
        && config.max_idle_growth_bytes == CANONICAL_MAX_IDLE_GROWTH_BYTES
        && config.max_data_bytes == CANONICAL_MAX_DATA_BYTES
        && config.durability == "strict"
}

fn engine_order(thread_index: usize, repetition: usize, seed: u64) -> [EngineLabel; 3] {
    let base = [
        EngineLabel::Redline,
        EngineLabel::Sqlite,
        EngineLabel::Postgres,
    ];
    let offset = (thread_index + repetition + seed as usize) % base.len();
    std::array::from_fn(|index| base[(index + offset) % base.len()])
}

fn interaction_plan(config: &CertConfig, threads: usize) -> Vec<Vec<Interaction>> {
    (0..threads)
        .map(|worker| {
            let mut rng = ChaCha8Rng::seed_from_u64(
                config.seed ^ (worker as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
            );
            (0..config.operations_per_thread)
                .map(|operation| {
                    let selector = rng.next_u32() % 100;
                    let session_id = if selector < 20 {
                        0
                    } else {
                        (rng.next_u64() % config.sessions as u64) as i64
                    };
                    if selector < 70 {
                        Interaction::Append {
                            event_id: format!("w{worker:03}-o{operation:08}"),
                            session_id,
                            sequence: (worker * config.operations_per_thread + operation) as i64,
                            payload: deterministic_payload(&mut rng, config.payload_bytes),
                        }
                    } else if selector < 90 {
                        Interaction::ReadSession { session_id }
                    } else {
                        Interaction::ReplayEvents { session_id }
                    }
                })
                .collect()
        })
        .collect()
}

fn warmup_plan(config: &CertConfig, threads: usize) -> Vec<Vec<Interaction>> {
    (0..threads)
        .map(|worker| {
            (0..config.warmup_operations_per_thread)
                .map(|operation| {
                    let session_id = ((worker * config.warmup_operations_per_thread + operation)
                        % config.sessions) as i64;
                    if operation % 10 == 0 {
                        Interaction::ReplayEvents { session_id }
                    } else {
                        Interaction::ReadSession { session_id }
                    }
                })
                .collect()
        })
        .collect()
}

fn expected_integrity(plan: &[Vec<Interaction>], sessions: usize) -> Result<IntegritySnapshot> {
    let mut event_rows = Vec::new();
    let mut session_updates = vec![0_i64; sessions];
    for interaction in plan.iter().flatten() {
        if let Interaction::Append {
            event_id,
            session_id,
            sequence,
            payload,
        } = interaction
        {
            let session_index = usize::try_from(*session_id)
                .ok()
                .filter(|index| *index < sessions)
                .context("operation plan contains an invalid session id")?;
            session_updates[session_index] = session_updates[session_index].saturating_add(1);
            event_rows.push(vec![
                CellValue::Text(event_id.clone()),
                CellValue::Integer(*session_id),
                CellValue::Integer(*sequence),
                CellValue::Text("training.progress".to_owned()),
                CellValue::Text(payload.clone()),
            ]);
        }
    }
    event_rows.sort_by(|left, right| match (left.first(), right.first()) {
        (Some(CellValue::Text(left)), Some(CellValue::Text(right))) => left.cmp(right),
        _ => std::cmp::Ordering::Equal,
    });
    let session_rows = session_updates
        .iter()
        .enumerate()
        .map(|(id, updates)| {
            vec![
                CellValue::Integer(id as i64),
                CellValue::Integer(*updates),
                CellValue::Text("training".to_owned()),
            ]
        })
        .collect::<Vec<_>>();
    Ok(IntegritySnapshot {
        events: event_rows.len() as u64,
        sessions: sessions as u64,
        last_sequence_sum: session_updates.iter().sum(),
        content_sha256: hash_rows(&event_rows),
        session_state_sha256: hash_rows(&session_rows),
    })
}

fn deterministic_payload(rng: &mut ChaCha8Rng, bytes: usize) -> String {
    const ALPHABET: &[u8; 16] = b"0123456789abcdef";
    (0..bytes)
        .map(|_| ALPHABET[(rng.next_u32() as usize) & 15] as char)
        .collect()
}

struct EngineRunContext<'a> {
    config: &'a CertConfig,
    postgres_url: &'a str,
    idle_observation_secs: u64,
    shared_storage_contract: bool,
    progress: ProgressTracker<'a>,
}

fn run_engine(
    planned: &PlannedRun,
    plan: &[Vec<Interaction>],
    run_dir: &Path,
    context: EngineRunContext<'_>,
) -> Result<EngineRun> {
    let EngineRunContext {
        config,
        postgres_url,
        idle_observation_secs,
        shared_storage_contract,
        progress,
    } = context;
    let label = planned.engine;
    let threads = planned.threads;
    progress.ensure_within_deadline("engine setup")?;
    if run_dir.exists() {
        fs::remove_dir_all(run_dir)?;
    }
    fs::create_dir_all(run_dir)?;
    let mut cleanup = RunCleanup {
        label,
        run_dir,
        postgres_url,
        armed: true,
    };
    let spec = run_spec(label, config, threads, run_dir);
    let (engine, version) = create_engine(label, &spec, run_dir, postgres_url)?;
    setup_interaction_schema(&*engine, config.sessions)?;
    if config.warmup_operations_per_thread > 0 {
        let warmup = warmup_plan(config, threads);
        progress.write("in_progress", "warmup", None, None)?;
        let _ = execute_plan(
            &*engine,
            &warmup,
            config.max_data_bytes,
            &progress,
            "warmup",
            config,
        )?;
    }
    let mut lifecycle_storage_samples = Vec::new();
    let storage_before =
        checked_storage_sample(&*engine, 0, config.max_data_bytes, "setup_complete")?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "setup_complete".to_owned(),
        sample: storage_before.clone(),
    });
    progress.write(
        "in_progress",
        "setup_complete",
        None,
        Some(storage_before.clone()),
    )?;
    let expected_integrity = expected_integrity(plan, config.sessions)?;
    let execution = execute_plan(
        &*engine,
        plan,
        config.max_data_bytes,
        &progress,
        "timed_workload",
        config,
    )?;
    let WorkerResult {
        metrics,
        retry_attempts,
        latency_sample_us,
        failure_samples,
        verification,
    } = execution.worker;
    let workload_storage_samples = execution.workload_storage_samples;
    let elapsed = execution.elapsed;
    let workload_complete = workload_storage_samples
        .last()
        .cloned()
        .context("post-workload storage watchdog produced no terminal sample")?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "workload_complete".to_owned(),
        sample: workload_complete.clone(),
    });
    progress.write(
        "in_progress",
        "workload_complete",
        None,
        Some(workload_complete),
    )?;
    progress.ensure_within_deadline("checkpoint")?;
    ensure_not_killed("before checkpoint")?;
    engine.checkpoint()?;
    let storage_after_checkpoint = checked_storage_sample(
        &*engine,
        elapsed.as_millis() as u64,
        config.max_data_bytes,
        "checkpoint_complete",
    )?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "checkpoint_complete".to_owned(),
        sample: storage_after_checkpoint.clone(),
    });
    progress.write(
        "in_progress",
        "checkpoint_complete",
        None,
        Some(storage_after_checkpoint.clone()),
    )?;
    let idle_storage_samples = observe_idle_storage(
        &*engine,
        idle_observation_secs,
        storage_after_checkpoint.clone(),
        config.max_data_bytes,
        &progress,
    )?;
    if let Some(sample) = idle_storage_samples.last().cloned() {
        lifecycle_storage_samples.push(PhaseStorageSample {
            phase: "idle_observation_complete".to_owned(),
            sample,
        });
    }
    let idle_growth_bytes = idle_growth(&idle_storage_samples)?;
    let before_integrity = checked_storage_sample(
        &*engine,
        elapsed.as_millis() as u64,
        config.max_data_bytes,
        "before_integrity",
    )?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "before_integrity".to_owned(),
        sample: before_integrity,
    });
    let integrity_before_reopen = interaction_integrity(&*engine)?;
    let (after_integrity, engine_stats) = checked_storage_snapshot(
        &*engine,
        elapsed.as_millis() as u64,
        config.max_data_bytes,
        "integrity_complete",
    )?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "integrity_complete".to_owned(),
        sample: after_integrity.clone(),
    });
    progress.write(
        "in_progress",
        "integrity_complete",
        None,
        Some(after_integrity),
    )?;
    drop(engine);

    ensure_not_killed("before reopen")?;
    let reopened = reopen_engine(label, &spec, run_dir, postgres_url)?;
    let reopened_sample = checked_storage_sample(
        &*reopened,
        elapsed.as_millis() as u64,
        config.max_data_bytes,
        "reopen_complete",
    )?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "reopen_complete".to_owned(),
        sample: reopened_sample.clone(),
    });
    progress.write(
        "in_progress",
        "reopen_complete",
        None,
        Some(reopened_sample),
    )?;
    let integrity_after_reopen = interaction_integrity(&*reopened)?;
    let reopen_integrity_sample = checked_storage_sample(
        &*reopened,
        elapsed.as_millis() as u64,
        config.max_data_bytes,
        "reopen_integrity_complete",
    )?;
    lifecycle_storage_samples.push(PhaseStorageSample {
        phase: "reopen_integrity_complete".to_owned(),
        sample: reopen_integrity_sample,
    });
    let reopen_verified = integrity_before_reopen == integrity_after_reopen;
    let plan_integrity_verified = integrity_before_reopen == expected_integrity
        && integrity_after_reopen == expected_integrity;
    drop(reopened);
    cleanup.cleanup()?;

    let elapsed_ms = elapsed.as_millis().max(1) as u64;
    let throughput = metrics.operations() as f64 / elapsed.as_secs_f64().max(0.000_001);
    let summary = MetricsSummary {
        operations: metrics.operations(),
        failures: metrics.failures(),
        busy_errors: metrics
            .busy_errors()
            .saturating_add(metrics.locked_errors()),
        locked_errors: metrics.locked_errors(),
        timeout_errors: metrics.timeout_errors(),
        elapsed_ms,
        throughput_ops_per_sec: throughput,
        latency: metrics.latency(),
    };
    Ok(EngineRun {
        engine: label,
        engine_version: version,
        threads,
        repetition: planned.repetition,
        execution_order_position: planned.execution_order_position,
        plan_sha256: planned.plan_sha256.clone(),
        attempted_operations: (config.operations_per_thread * threads) as u64,
        metrics: summary,
        retry_attempts,
        failure_samples,
        latency_sample_us,
        point_reads_verified: verification.point_reads,
        replays_verified: verification.replays,
        replay_rows_verified: verification.replay_rows,
        storage_before,
        workload_storage_samples,
        storage_after_checkpoint,
        idle_storage_samples,
        lifecycle_storage_samples,
        storage_semantics: storage_semantics(label, config.max_data_bytes, shared_storage_contract),
        delayed_growth_soak: planned.delayed_growth_soak,
        idle_growth_bytes,
        expected_integrity,
        integrity_before_reopen,
        integrity_after_reopen,
        plan_integrity_verified,
        reopen_verified,
        engine_stats,
    })
}

fn run_spec(label: EngineLabel, config: &CertConfig, threads: usize, run_dir: &Path) -> RunSpec {
    RunSpec {
        engine: match label {
            EngineLabel::Sqlite => EngineKind::Sqlite,
            EngineLabel::Redline | EngineLabel::Postgres => EngineKind::Redline,
        },
        workload: WorkloadKind::MixedOltp,
        durability: DurabilityKind::Strict,
        threads,
        rows: config.sessions,
        duration: Duration::from_secs(1),
        cache_bytes: 64 * 1024 * 1024,
        seed: config.seed,
        base_dir: run_dir.to_path_buf(),
    }
}

fn create_engine(
    label: EngineLabel,
    spec: &RunSpec,
    run_dir: &Path,
    postgres_url: &str,
) -> Result<(Box<dyn BenchEngine>, String)> {
    match label {
        EngineLabel::Redline => Ok((
            Box::new(RedlineEngine::open(spec, run_dir)?),
            env!("CARGO_PKG_VERSION").to_owned(),
        )),
        EngineLabel::Sqlite => Ok((
            Box::new(SqliteEngine::open(spec, run_dir)?),
            rusqlite::version().to_owned(),
        )),
        EngineLabel::Postgres => {
            let engine = PostgresEngine::create(spec, run_dir, postgres_url)?;
            let version = engine.server_version()?;
            Ok((Box::new(engine), version))
        }
    }
}

fn reopen_engine(
    label: EngineLabel,
    spec: &RunSpec,
    run_dir: &Path,
    postgres_url: &str,
) -> Result<Box<dyn BenchEngine>> {
    match label {
        EngineLabel::Redline => Ok(Box::new(RedlineEngine::open(spec, run_dir)?)),
        EngineLabel::Sqlite => Ok(Box::new(SqliteEngine::open(spec, run_dir)?)),
        EngineLabel::Postgres => Ok(Box::new(PostgresEngine::reopen(
            spec,
            run_dir,
            postgres_url,
        )?)),
    }
}

fn setup_interaction_schema(engine: &dyn BenchEngine, sessions: usize) -> Result<()> {
    let mut conn = engine.connect(0)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS interaction_sessions(\
             id BIGINT PRIMARY KEY, last_seq INTEGER NOT NULL, state TEXT NOT NULL, updated_at BIGINT NOT NULL\
         )",
        &[],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS interaction_events(\
             event_id TEXT PRIMARY KEY, session_id BIGINT NOT NULL, seq BIGINT NOT NULL, \
             kind TEXT NOT NULL, payload TEXT NOT NULL\
         )",
        &[],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS interaction_events_session_seq \
         ON interaction_events(session_id, seq)",
        &[],
    )?;
    conn.begin_immediate()?;
    for session in 0..sessions {
        let inserted = conn.execute(
            "INSERT INTO interaction_sessions(id, last_seq, state, updated_at) \
             VALUES (?1, 0, 'training', 0)",
            &[CellValue::Integer(session as i64)],
        )?;
        if inserted != 1 {
            bail!("session seed insert affected {inserted} rows, expected 1");
        }
    }
    conn.commit()
}

struct PlanExecution {
    worker: WorkerResult,
    workload_storage_samples: Vec<StorageSample>,
    elapsed: Duration,
}

fn execute_plan(
    engine: &dyn BenchEngine,
    plan: &[Vec<Interaction>],
    max_data_bytes: u64,
    progress: &ProgressTracker<'_>,
    lifecycle_phase: &str,
    config: &CertConfig,
) -> Result<PlanExecution> {
    if plan.is_empty() {
        bail!("interaction plan must contain at least one worker");
    }
    let barrier = Arc::new(Barrier::new(plan.len() + 2));
    let epoch = Arc::new(OnceLock::<Instant>::new());
    let stop = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicUsize::new(0));
    let validation = ResultValidation::for_plan(plan, config)?;
    let (worker_results, workload_storage_samples, elapsed) =
        std::thread::scope(|scope| -> Result<_> {
            let monitor_epoch = Arc::clone(&epoch);
            let monitor_stop = Arc::clone(&stop);
            let monitor_completed = Arc::clone(&completed);
            let monitor_barrier = Arc::clone(&barrier);
            let monitor_progress = *progress;
            let monitor = scope.spawn(move || -> Result<Vec<StorageSample>> {
                let epoch = loop {
                    if let Some(epoch) = monitor_epoch.get() {
                        break *epoch;
                    }
                    std::thread::yield_now();
                };
                monitor_barrier.wait();
                let mut samples = Vec::new();
                let mut last_progress = Instant::now()
                    .checked_sub(PROGRESS_HEARTBEAT_INTERVAL)
                    .unwrap_or_else(Instant::now);
                loop {
                    monitor_progress.ensure_within_deadline(lifecycle_phase)?;
                    if kill_requested() {
                        monitor_stop.store(true, Ordering::Release);
                        bail!("{KILL_SWITCH_ENV}=1 requested benchmark termination");
                    }
                    let sample = match checked_storage_sample(
                        engine,
                        epoch.elapsed().as_millis() as u64,
                        max_data_bytes,
                        lifecycle_phase,
                    ) {
                        Ok(sample) => sample,
                        Err(error) => {
                            monitor_stop.store(true, Ordering::Release);
                            return Err(error).context("continuous storage watchdog failed");
                        }
                    };
                    samples.push(sample.clone());
                    let workers_complete = monitor_completed.load(Ordering::Acquire) >= plan.len();
                    if last_progress.elapsed() >= PROGRESS_HEARTBEAT_INTERVAL || workers_complete {
                        if let Err(error) = monitor_progress.write(
                            "in_progress",
                            lifecycle_phase,
                            None,
                            Some(sample),
                        ) {
                            monitor_stop.store(true, Ordering::Release);
                            return Err(error).context("write workload progress heartbeat");
                        }
                        last_progress = Instant::now();
                    }
                    if workers_complete {
                        if samples.len() == 1 {
                            samples.push(checked_storage_sample(
                                engine,
                                epoch.elapsed().as_millis() as u64,
                                max_data_bytes,
                                lifecycle_phase,
                            )?);
                        }
                        break Ok(samples);
                    }
                    std::thread::sleep(STORAGE_POLL_INTERVAL);
                }
            });

            let mut handles = Vec::with_capacity(plan.len());
            for (worker, operations) in plan.iter().enumerate() {
                let barrier = Arc::clone(&barrier);
                let stop = Arc::clone(&stop);
                let completed = Arc::clone(&completed);
                let validation = &validation;
                handles.push(scope.spawn(move || -> Result<WorkerResult> {
                    let connection = engine.connect(worker).and_then(|mut connection| {
                        connection.set_busy_timeout(Duration::from_secs(5))?;
                        Ok(connection)
                    });
                    barrier.wait();
                    let _completion = WorkerCompletion { completed };
                    let mut connection = connection?;
                    let base_budget = MAX_LATENCY_SAMPLES / plan.len();
                    let remainder = MAX_LATENCY_SAMPLES % plan.len();
                    let sample_budget = base_budget + usize::from(worker < remainder);
                    let stride = if sample_budget == 0 {
                        usize::MAX
                    } else {
                        operations.len().div_ceil(sample_budget).max(1)
                    };
                    let mut metrics = Metrics::new();
                    let mut retry_attempts = 0_u64;
                    let mut latency_sample_us = Vec::with_capacity(sample_budget);
                    let mut failure_samples = Vec::new();
                    let mut verification = InteractionVerification::default();
                    for (index, operation) in operations.iter().enumerate() {
                        if kill_requested() {
                            stop.store(true, Ordering::Release);
                            bail!("{KILL_SWITCH_ENV}=1 requested benchmark termination");
                        }
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        let began = Instant::now();
                        match execute_interaction_with_retry(
                            &mut *connection,
                            operation,
                            validation,
                        ) {
                            Ok((retries, operation_verification)) => {
                                retry_attempts = retry_attempts.saturating_add(retries);
                                verification.point_reads = verification
                                    .point_reads
                                    .saturating_add(operation_verification.point_reads);
                                verification.replays = verification
                                    .replays
                                    .saturating_add(operation_verification.replays);
                                verification.replay_rows = verification
                                    .replay_rows
                                    .saturating_add(operation_verification.replay_rows);
                                let elapsed = began.elapsed();
                                metrics.record_success(elapsed);
                                if index % stride == 0 && latency_sample_us.len() < sample_budget {
                                    latency_sample_us
                                        .push(elapsed.as_micros().min(u128::from(u64::MAX)) as u64);
                                }
                            }
                            Err(error) => {
                                metrics.record_failure(classify_failure(&error));
                                if failure_samples.len() < 8 {
                                    failure_samples.push(format!("{error:#}"));
                                }
                            }
                        }
                    }
                    Ok(WorkerResult {
                        metrics,
                        retry_attempts,
                        latency_sample_us,
                        failure_samples,
                        verification,
                    })
                }));
            }
            let started = Instant::now();
            epoch
                .set(started)
                .map_err(|_| anyhow!("interaction timer was initialized more than once"))?;
            barrier.wait();
            let worker_results = handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| anyhow!("interaction worker panicked"))?
                })
                .collect::<Result<Vec<_>>>();
            let elapsed = started.elapsed();
            let workload_storage_samples = monitor
                .join()
                .map_err(|_| anyhow!("storage watchdog panicked"))??;
            Ok((worker_results?, workload_storage_samples, elapsed))
        })?;
    let mut metrics = Metrics::new();
    let mut retry_attempts = 0_u64;
    let mut latency_sample_us = Vec::new();
    let mut failure_samples = Vec::new();
    let mut verification = InteractionVerification::default();
    for worker in worker_results {
        metrics.merge(&worker.metrics);
        retry_attempts = retry_attempts.saturating_add(worker.retry_attempts);
        latency_sample_us.extend(worker.latency_sample_us);
        failure_samples.extend(worker.failure_samples);
        verification.point_reads = verification
            .point_reads
            .saturating_add(worker.verification.point_reads);
        verification.replays = verification
            .replays
            .saturating_add(worker.verification.replays);
        verification.replay_rows = verification
            .replay_rows
            .saturating_add(worker.verification.replay_rows);
    }
    failure_samples.truncate(32);
    debug_assert!(latency_sample_us.len() <= MAX_LATENCY_SAMPLES);
    Ok(PlanExecution {
        worker: WorkerResult {
            metrics,
            retry_attempts,
            latency_sample_us,
            failure_samples,
            verification,
        },
        workload_storage_samples,
        elapsed,
    })
}

fn classify_failure(error: &anyhow::Error) -> FailureKind {
    let message = format!("{error:#}").to_ascii_lowercase();
    if message.contains("timeout") || message.contains("timed out") {
        FailureKind::Timeout
    } else if message.contains("locked") || message.contains("lock wait") {
        FailureKind::Locked
    } else if message.contains("busy") || message.contains("serialization") {
        FailureKind::Busy
    } else {
        FailureKind::Other
    }
}

fn interaction_integrity(engine: &dyn BenchEngine) -> Result<IntegritySnapshot> {
    let mut conn = engine.connect(0)?;
    let event_rows = conn.query_all(
        "SELECT event_id, session_id, seq, kind, payload FROM interaction_events ORDER BY event_id",
        &[],
    )?;
    let session_rows = conn.query_all(
        "SELECT id, last_seq, state FROM interaction_sessions ORDER BY id",
        &[],
    )?;
    let sessions = scalar_i64(&mut *conn, "SELECT COUNT(*) FROM interaction_sessions")?;
    let last_sequence_sum = scalar_i64(
        &mut *conn,
        "SELECT COALESCE(SUM(last_seq), 0) FROM interaction_sessions",
    )?;
    Ok(IntegritySnapshot {
        events: event_rows.len() as u64,
        sessions: sessions.max(0) as u64,
        last_sequence_sum,
        content_sha256: hash_rows(&event_rows),
        session_state_sha256: hash_rows(&session_rows),
    })
}

fn hash_rows(rows: &[Vec<CellValue>]) -> String {
    let mut digest = Sha256::new();
    for row in rows {
        digest.update(b"row\0");
        for cell in row {
            hash_cell(&mut digest, cell);
        }
    }
    format!("{:x}", digest.finalize())
}

fn scalar_i64(conn: &mut dyn BenchConn, sql: &str) -> Result<i64> {
    match conn.query_row(sql, &[])?.first() {
        Some(CellValue::Integer(value)) => Ok(*value),
        other => Err(anyhow!("expected integer scalar for {sql}, got {other:?}")),
    }
}

fn storage_sample(engine: &dyn BenchEngine, offset_ms: u64) -> Result<StorageSample> {
    let snapshot = engine.snapshot()?;
    Ok(StorageSample {
        offset_ms,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
    })
}

fn checked_storage_sample(
    engine: &dyn BenchEngine,
    offset_ms: u64,
    max_data_bytes: u64,
    phase: &str,
) -> Result<StorageSample> {
    ensure_not_killed(phase)?;
    let sample = storage_sample(engine, offset_ms)
        .with_context(|| format!("storage watchdog snapshot failed during {phase}"))?;
    ensure_storage_within_limit(&sample, max_data_bytes)
        .with_context(|| format!("storage watchdog failed during {phase}"))?;
    ensure_not_killed(phase)?;
    Ok(sample)
}

fn checked_storage_snapshot(
    engine: &dyn BenchEngine,
    offset_ms: u64,
    max_data_bytes: u64,
    phase: &str,
) -> Result<(StorageSample, serde_json::Value)> {
    ensure_not_killed(phase)?;
    let snapshot = engine
        .snapshot()
        .with_context(|| format!("storage watchdog snapshot failed during {phase}"))?;
    let sample = StorageSample {
        offset_ms,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
    };
    ensure_storage_within_limit(&sample, max_data_bytes)
        .with_context(|| format!("storage watchdog failed during {phase}"))?;
    ensure_not_killed(phase)?;
    Ok((sample, snapshot.engine_stats))
}

fn storage_total(sample: &StorageSample) -> Result<u64> {
    sample
        .data_bytes
        .checked_add(sample.wal_bytes)
        .context("aggregate data+WAL byte accounting overflowed u64")
}

fn ensure_storage_within_limit(sample: &StorageSample, max_data_bytes: u64) -> Result<()> {
    let total = storage_total(sample)?;
    let stop_threshold = storage_stop_threshold(max_data_bytes);
    if total > max_data_bytes {
        bail!(
            "aggregate storage hard cap exceeded: observed {total} bytes above {max_data_bytes} bytes"
        );
    }
    if total >= stop_threshold {
        bail!(
            "storage watchdog stopped at {total} bytes before the hard {max_data_bytes}-byte cap (stop threshold {stop_threshold})"
        );
    }
    Ok(())
}

fn storage_stop_threshold(max_data_bytes: u64) -> u64 {
    // Reserve 25% (bounded to 8..=512 MiB) and sample every 25 ms. The reserve is also
    // physically allocated by the wrapper on the shared filesystem, leaving recovery space even
    // if a defective engine grows between watchdog samples.
    let headroom = (max_data_bytes / 4)
        .clamp(8 * 1024 * 1024, 512 * 1024 * 1024)
        .min(max_data_bytes / 2);
    max_data_bytes.saturating_sub(headroom)
}

fn observe_idle_storage(
    engine: &dyn BenchEngine,
    seconds: u64,
    initial: StorageSample,
    max_data_bytes: u64,
    progress: &ProgressTracker<'_>,
) -> Result<Vec<StorageSample>> {
    ensure_storage_within_limit(&initial, max_data_bytes)?;
    let initial_offset_ms = initial.offset_ms;
    let started = Instant::now();
    let mut samples = vec![initial];
    for _ in 1..=seconds {
        std::thread::sleep(Duration::from_secs(1));
        progress.ensure_within_deadline("idle observation")?;
        let sample = checked_storage_sample(
            engine,
            initial_offset_ms.saturating_add(started.elapsed().as_millis() as u64),
            max_data_bytes,
            "idle observation",
        )?;
        progress.write(
            "in_progress",
            "idle_observation",
            None,
            Some(sample.clone()),
        )?;
        samples.push(sample);
    }
    Ok(samples)
}

fn kill_requested() -> bool {
    std::env::var(KILL_SWITCH_ENV)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
}

fn ensure_not_killed(phase: &str) -> Result<()> {
    if kill_requested() {
        bail!("{KILL_SWITCH_ENV}=1 requested benchmark termination during {phase}");
    }
    Ok(())
}

fn storage_semantics(
    label: EngineLabel,
    hard_limit_bytes: u64,
    shared_storage_contract: bool,
) -> StorageSemantics {
    let accounting = match label {
        EngineLabel::Redline => {
            "recursive Redline database-directory bytes split into data and WAL"
        }
        EngineLabel::Sqlite => "SQLite database-file bytes plus WAL-file bytes",
        EngineLabel::Postgres => {
            "dedicated PostgreSQL default-tablespace bytes plus current WAL-directory bytes"
        }
    };
    StorageSemantics {
        accounting: accounting.to_owned(),
        purpose: "absolute runaway-growth safety bound only; never a ranking metric".to_owned(),
        cross_engine_comparable: false,
        sampled_inside_timed_window: true,
        continuously_sampled: true,
        hard_limit_bytes,
        stop_threshold_bytes: storage_stop_threshold(hard_limit_bytes),
        shared_storage_contract,
    }
}

fn all_storage_samples(run: &EngineRun) -> impl Iterator<Item = &StorageSample> {
    std::iter::once(&run.storage_before)
        .chain(run.workload_storage_samples.iter())
        .chain(std::iter::once(&run.storage_after_checkpoint))
        .chain(run.idle_storage_samples.iter())
        .chain(
            run.lifecycle_storage_samples
                .iter()
                .map(|phase| &phase.sample),
        )
}

fn idle_growth(samples: &[StorageSample]) -> Result<u64> {
    let Some(first) = samples.first() else {
        return Ok(0);
    };
    let baseline = storage_total(first)?;
    Ok(samples
        .iter()
        .map(storage_total)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(baseline)
        .saturating_sub(baseline))
}

fn expected_verification_counts(plan: &[Vec<Interaction>]) -> (u64, u64) {
    plan.iter()
        .flatten()
        .fold((0, 0), |counts, operation| match operation {
            Interaction::ReadSession { .. } => (counts.0 + 1, counts.1),
            Interaction::ReplayEvents { .. } => (counts.0, counts.1 + 1),
            Interaction::Append { .. } => counts,
        })
}

fn validate_runs(runs: &[EngineRun], config: &CertConfig) -> Vec<String> {
    let mut failures = Vec::new();
    let expected_runs = config.threads.len() * config.repetitions * 3;
    if runs.len() != expected_runs {
        failures.push(format!(
            "incomplete engine matrix: expected {expected_runs} runs, found {}",
            runs.len()
        ));
    }
    for run in runs {
        let expected_verifications =
            expected_verification_counts(&interaction_plan(config, run.threads));
        if (run.point_reads_verified, run.replays_verified) != expected_verifications {
            failures.push(format!(
                "{} t{} r{} correctness-gated {}/{} point reads and {}/{} replays",
                run.engine.as_str(),
                run.threads,
                run.repetition,
                run.point_reads_verified,
                expected_verifications.0,
                run.replays_verified,
                expected_verifications.1
            ));
        }
        if run.metrics.failures > 0 {
            failures.push(format!(
                "{} t{} r{} recorded {} failed interactions",
                run.engine.as_str(),
                run.threads,
                run.repetition,
                run.metrics.failures
            ));
        }
        if run.metrics.operations != run.attempted_operations {
            failures.push(format!(
                "{} t{} r{} completed {}/{} interactions",
                run.engine.as_str(),
                run.threads,
                run.repetition,
                run.metrics.operations,
                run.attempted_operations
            ));
        }
        if !run.reopen_verified {
            failures.push(format!(
                "{} t{} r{} failed checkpoint/reopen integrity",
                run.engine.as_str(),
                run.threads,
                run.repetition
            ));
        }
        if !run.plan_integrity_verified {
            failures.push(format!(
                "{} t{} r{} did not match the immutable operation plan",
                run.engine.as_str(),
                run.threads,
                run.repetition
            ));
        }
        if run.workload_storage_samples.len() < 2 {
            failures.push(format!(
                "{} t{} r{} lacks continuous workload storage samples",
                run.engine.as_str(),
                run.threads,
                run.repetition
            ));
        }
        if run.storage_semantics.cross_engine_comparable
            || !run.storage_semantics.sampled_inside_timed_window
            || !run.storage_semantics.continuously_sampled
        {
            failures.push(format!(
                "{} t{} r{} has invalid continuous safety-accounting semantics",
                run.engine.as_str(),
                run.threads,
                run.repetition
            ));
        }
        let phases = run
            .lifecycle_storage_samples
            .iter()
            .map(|sample| sample.phase.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for required in [
            "setup_complete",
            "workload_complete",
            "checkpoint_complete",
            "idle_observation_complete",
            "before_integrity",
            "integrity_complete",
            "reopen_complete",
            "reopen_integrity_complete",
        ] {
            if !phases.contains(required) {
                failures.push(format!(
                    "{} t{} r{} lacks bounded storage proof for phase {required}",
                    run.engine.as_str(),
                    run.threads,
                    run.repetition
                ));
            }
        }
        if run.idle_growth_bytes > config.max_idle_growth_bytes {
            failures.push(format!(
                "{} t{} r{} grew {} bytes after work stopped (limit {})",
                run.engine.as_str(),
                run.threads,
                run.repetition,
                run.idle_growth_bytes,
                config.max_idle_growth_bytes
            ));
        }
        let max_storage = all_storage_samples(run).try_fold(0_u64, |maximum, sample| {
            storage_total(sample).map(|total| maximum.max(total))
        });
        let max_storage = match max_storage {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!(
                    "{} t{} r{} storage accounting failed: {error:#}",
                    run.engine.as_str(),
                    run.threads,
                    run.repetition
                ));
                continue;
            }
        };
        if max_storage > config.max_data_bytes {
            failures.push(format!(
                "{} t{} r{} used {} bytes (limit {})",
                run.engine.as_str(),
                run.threads,
                run.repetition,
                max_storage,
                config.max_data_bytes
            ));
        }
    }
    let soak_runs = runs
        .iter()
        .filter(|run| run.delayed_growth_soak)
        .collect::<Vec<_>>();
    if soak_runs.len() != 1 || soak_runs[0].engine != EngineLabel::Redline {
        failures.push("matrix must contain exactly one Redline delayed-growth soak".to_owned());
    }
    for &threads in &config.threads {
        let digests = runs
            .iter()
            .filter(|run| run.threads == threads)
            .map(|run| run.plan_sha256.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if digests.len() != 1 {
            failures.push(format!(
                "t{threads} runs did not share one immutable operation plan"
            ));
        }
        for repetition in 0..config.repetitions {
            let integrity = runs
                .iter()
                .filter(|run| run.threads == threads && run.repetition == repetition)
                .map(|run| &run.integrity_after_reopen)
                .collect::<Vec<_>>();
            if integrity.len() != 3 || integrity.windows(2).any(|pair| pair[0] != pair[1]) {
                failures.push(format!(
                    "t{threads} r{repetition} cross-engine integrity mismatch"
                ));
            }
        }
    }
    failures
}

fn comparisons(runs: &[EngineRun], threads: &[usize]) -> Result<Vec<Comparison>> {
    threads
        .iter()
        .map(|threads| {
            let throughput = |engine| -> Result<f64> {
                median_f64(values_for(runs, *threads, engine, |run| {
                    run.metrics.throughput_ops_per_sec
                }))
            };
            let p99 = |engine| {
                median_u64(values_for(runs, *threads, engine, |run| {
                    run.metrics.latency.p99_us
                }))
            };
            let redline_throughput = throughput(EngineLabel::Redline)?;
            let sqlite_throughput = throughput(EngineLabel::Sqlite)?;
            let postgres_throughput = throughput(EngineLabel::Postgres)?;
            let redline_p99 = p99(EngineLabel::Redline)?;
            let sqlite_p99 = p99(EngineLabel::Sqlite)?;
            let postgres_p99 = p99(EngineLabel::Postgres)?;
            let redline_to_sqlite_throughput_ratio =
                throughput_ratio(redline_throughput, sqlite_throughput);
            let redline_to_postgres_throughput_ratio =
                throughput_ratio(redline_throughput, postgres_throughput);
            let redline_to_sqlite_p99_ratio = latency_ratio(redline_p99 as f64, sqlite_p99 as f64);
            let redline_to_postgres_p99_ratio =
                latency_ratio(redline_p99 as f64, postgres_p99 as f64);
            let worst_redline_to_sqlite_throughput_ratio = min_ratio(paired_ratios(
                runs,
                *threads,
                EngineLabel::Redline,
                EngineLabel::Sqlite,
                |run| run.metrics.throughput_ops_per_sec,
                throughput_ratio,
            )?)?;
            let worst_redline_to_postgres_throughput_ratio = min_ratio(paired_ratios(
                runs,
                *threads,
                EngineLabel::Redline,
                EngineLabel::Postgres,
                |run| run.metrics.throughput_ops_per_sec,
                throughput_ratio,
            )?)?;
            let worst_redline_to_sqlite_p99_ratio = max_ratio(paired_ratios(
                runs,
                *threads,
                EngineLabel::Redline,
                EngineLabel::Sqlite,
                |run| run.metrics.latency.p99_us as f64,
                latency_ratio,
            )?)?;
            let worst_redline_to_postgres_p99_ratio = max_ratio(paired_ratios(
                runs,
                *threads,
                EngineLabel::Redline,
                EngineLabel::Postgres,
                |run| run.metrics.latency.p99_us as f64,
                latency_ratio,
            )?)?;
            Ok(Comparison {
                threads: *threads,
                redline_median_ops_per_sec: redline_throughput,
                sqlite_median_ops_per_sec: sqlite_throughput,
                postgres_median_ops_per_sec: postgres_throughput,
                redline_to_sqlite_throughput_ratio,
                redline_to_postgres_throughput_ratio,
                redline_median_p99_us: redline_p99,
                sqlite_median_p99_us: sqlite_p99,
                postgres_median_p99_us: postgres_p99,
                redline_to_sqlite_p99_ratio,
                redline_to_postgres_p99_ratio,
                worst_redline_to_sqlite_throughput_ratio,
                worst_redline_to_postgres_throughput_ratio,
                worst_redline_to_sqlite_p99_ratio,
                worst_redline_to_postgres_p99_ratio,
                bounded_result_eligible: worst_redline_to_sqlite_throughput_ratio >= 1.0
                    && worst_redline_to_postgres_throughput_ratio >= 1.0
                    && worst_redline_to_sqlite_p99_ratio <= 1.0
                    && worst_redline_to_postgres_p99_ratio <= 1.0,
            })
        })
        .collect()
}

fn paired_ratios(
    runs: &[EngineRun],
    threads: usize,
    numerator: EngineLabel,
    denominator: EngineLabel,
    value: impl Fn(&EngineRun) -> f64,
    ratio: fn(f64, f64) -> f64,
) -> Result<Vec<f64>> {
    let repetitions = runs
        .iter()
        .filter(|run| run.threads == threads && run.engine == numerator)
        .map(|run| run.repetition)
        .collect::<std::collections::BTreeSet<_>>();
    repetitions
        .into_iter()
        .map(|repetition| {
            let numerator_run = runs
                .iter()
                .find(|run| {
                    run.threads == threads
                        && run.repetition == repetition
                        && run.engine == numerator
                })
                .context("missing numerator run for paired comparison")?;
            let denominator_run = runs
                .iter()
                .find(|run| {
                    run.threads == threads
                        && run.repetition == repetition
                        && run.engine == denominator
                })
                .context("missing denominator run for paired comparison")?;
            Ok(ratio(value(numerator_run), value(denominator_run)))
        })
        .collect()
}

fn min_ratio(values: Vec<f64>) -> Result<f64> {
    values
        .into_iter()
        .min_by(f64::total_cmp)
        .context("missing paired ratio samples")
}

fn max_ratio(values: Vec<f64>) -> Result<f64> {
    values
        .into_iter()
        .max_by(f64::total_cmp)
        .context("missing paired ratio samples")
}

fn values_for<T>(
    runs: &[EngineRun],
    threads: usize,
    engine: EngineLabel,
    value: impl Fn(&EngineRun) -> T,
) -> Vec<T> {
    runs.iter()
        .filter(|run| run.threads == threads && run.engine == engine)
        .map(value)
        .collect()
}

fn median_f64(mut values: Vec<f64>) -> Result<f64> {
    if values.is_empty() {
        bail!("missing engine throughput samples");
    }
    values.sort_by(f64::total_cmp);
    Ok(values[values.len() / 2])
}

fn median_u64(mut values: Vec<u64>) -> Result<u64> {
    if values.is_empty() {
        bail!("missing engine latency samples");
    }
    values.sort_unstable();
    Ok(values[values.len() / 2])
}

fn throughput_ratio(numerator: f64, denominator: f64) -> f64 {
    if !numerator.is_finite() || !denominator.is_finite() || numerator <= 0.0 || denominator <= 0.0
    {
        0.0
    } else {
        numerator / denominator
    }
}

fn latency_ratio(numerator: f64, denominator: f64) -> f64 {
    if !numerator.is_finite() || !denominator.is_finite() || numerator < 0.0 || denominator <= 0.0 {
        // Keep receipts valid finite JSON while making an invalid/zero reference latency a
        // categorical loss. A shared generic zero-denominator fallback used to report 0.0,
        // accidentally satisfying the <= 1.0 latency-win gate.
        f64::MAX
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests;
