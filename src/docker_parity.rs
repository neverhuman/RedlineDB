use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::beyond_sqlite::oracle::{RunCasesOptions, run_selected_cases_with};
use crate::cli::args::{DockerParityArgs, ProgressMode, RunArgs, RunMode, Suite};
use crate::compat;

const EVIDENCE_SCHEMA: &str = "redline.docker-parity-evidence/v1";
const CUSTODY_SCHEMA: &str = "redline.docker-custody-receipt/v1";
const SQLITE_CONTRACT: &str = "redline-sqlite-contract/v1";
const POSTGRES_CONTRACT: &str = "redline-postgres-contract/v1";
const SQLITE_FULL_COUNT: u64 = 2445;
const POSTGRES_ORACLE_FULL_COUNT: u64 = 265;
const POSTGRES_TARGET_FULL_COUNT: u64 = 151;
const POSTGRES_EXCLUSION_COUNT: usize = 114;

pub(crate) fn run(args: DockerParityArgs) -> Result<()> {
    let evidence_dir = prepare_evidence_dir(&args.evidence_dir)?;
    if args.mode == RunMode::Release && Path::new("/var/run/docker.sock").exists() {
        bail!("release Docker parity forbids a Docker socket inside the runner");
    }
    let (custody, custody_sha256) = validate_custody(&args.custody_receipt)?;
    validate_postgres_connection(args.mode)?;

    let sqlite_raw = evidence_dir.join("sqlite-contract.raw.jsonl");
    let sqlite_args = contract_args(
        SQLITE_CONTRACT,
        &args.sqlite_cases,
        &args,
        sqlite_raw.clone(),
        Vec::new(),
    );
    compat::run_contract(sqlite_args)?;
    let sqlite_evidence_path = compat::compatibility_evidence_path(&sqlite_raw)?;
    let sqlite_evidence = read_json(&sqlite_evidence_path, "SQLite contract evidence")?;

    let oracle_cases = compat::select_postgres_oracle_cases(&args.postgres_oracle_cases)?;
    let oracle_selected_ids = oracle_cases.iter().map(|case| case.id).collect::<Vec<_>>();
    let (oracle_summary, oracle_outcomes) = run_selected_cases_with(
        oracle_cases,
        RunCasesOptions {
            target_bin: None,
            target_args: Vec::new(),
        },
    )?;
    let postgres_oracle_path = evidence_dir.join("postgres-oracle-evidence.json");
    let oracle_evidence = json!({
        "schema_version": "redline.postgres-oracle-evidence/v1",
        "selector": args.postgres_oracle_cases,
        "selection_explanation": format!(
            "selector={}; corpus=all-published-postgres-cases; selected={}; ordering=case-id-ascending",
            args.postgres_oracle_cases,
            oracle_selected_ids.len()
        ),
        "selected_case_ids": oracle_selected_ids,
        "total": oracle_summary.total,
        "passed": oracle_summary.passed,
        "failed": oracle_summary.failed,
        "skipped_unavailable": oracle_summary.skipped_unavailable,
        "skipped_feature_missing": oracle_summary.skipped_feature_missing,
        "outcomes": oracle_outcomes.iter().map(|outcome| json!({
            "case_id": outcome.case_id,
            "name": outcome.name,
            "category": outcome.category,
            "status": outcome.status,
            "diagnostic": outcome.diagnostic,
        })).collect::<Vec<_>>(),
    });
    write_json(&postgres_oracle_path, &oracle_evidence)?;

    let postgres_raw = evidence_dir.join("postgres-target-contract.raw.jsonl");
    let postgres_args = contract_args(
        POSTGRES_CONTRACT,
        &args.postgres_target_cases,
        &args,
        postgres_raw.clone(),
        args.target_args.clone(),
    );
    compat::run_contract(postgres_args)?;
    let postgres_evidence_path = compat::compatibility_evidence_path(&postgres_raw)?;
    let postgres_evidence = read_json(&postgres_evidence_path, "PostgreSQL target evidence")?;

    let exclusions = compat::postgres_exclusions_json()?;
    let mut failures = campaign_failures(
        &args,
        &sqlite_evidence,
        &oracle_summary,
        &oracle_outcomes,
        &postgres_evidence,
        exclusions.len(),
    );
    let container = container_identity(args.mode, &mut failures);
    let artifact_hashes = json!({
        "sqlite_raw_sha256": sha256_file(&sqlite_raw)?,
        "sqlite_evidence_sha256": sha256_file(&sqlite_evidence_path)?,
        "postgres_oracle_evidence_sha256": sha256_file(&postgres_oracle_path)?,
        "postgres_target_raw_sha256": sha256_file(&postgres_raw)?,
        "postgres_target_evidence_sha256": sha256_file(&postgres_evidence_path)?,
    });
    let contract_hashes = json!({
        "sqlite": sha256_file(&sqlite_evidence_path)?,
        "postgres_oracle": sha256_file(&postgres_oracle_path)?,
        "postgres_target": sha256_file(&postgres_evidence_path)?,
    });
    let status = match (args.mode, failures.is_empty()) {
        (RunMode::Release, true) => "candidate",
        (RunMode::Release, false) => "failed",
        (RunMode::Diagnostic, true) => "diagnostic-pass",
        (RunMode::Diagnostic, false) => "diagnostic-fail",
    };
    let evidence = json!({
        "schema_version": EVIDENCE_SCHEMA,
        "status": status,
        "mode": mode_name(args.mode),
        "provenance_source": "local-jeryu",
        "core": custody.get("core").context("custody receipt lacks core identity")?,
        "testing": custody.get("testing").context("custody receipt lacks Testing identity")?,
        "images": custody.get("images").context("custody receipt lacks image identities")?,
        "docker_engine": custody.get("docker_engine").context("custody receipt lacks Docker engine identity")?,
        "docker_inputs": custody.get("docker_inputs").context("custody receipt lacks Docker input identities")?,
        "cargo_custody": custody.get("cargo_custody").context("custody receipt lacks Cargo custody identity")?,
        "custody_receipt": {
            "path": args.custody_receipt,
            "sha256": custody_sha256,
        },
        "container": container,
        "sqlite": result_counts(&sqlite_evidence)?,
        "postgres_oracle": {
            "total": oracle_summary.total,
            "passed": oracle_summary.passed,
            "failed": oracle_summary.failed,
            "skipped_unavailable": oracle_summary.skipped_unavailable,
            "skipped_feature_missing": oracle_summary.skipped_feature_missing,
        },
        "postgres_target": result_counts(&postgres_evidence)?,
        "postgres_exclusions": exclusions,
        "per_contract_evidence_sha256": contract_hashes,
        "output_sha256": artifact_hashes,
        "release_failures": failures,
        "command": {
            "argv": env::args().collect::<Vec<_>>(),
            "target_args": args.target_args,
            "explanation": "SQLite target comparison covers the SQLite contract; PostgreSQL first self-compares every published case, then compares only the governed non-excluded target set; all orderings are ascending case ID.",
            "sqlite_selection": args.sqlite_cases,
            "postgres_oracle_selection": args.postgres_oracle_cases,
            "postgres_target_selection": args.postgres_target_cases,
        },
    });

    let output = match (args.mode, failures_is_empty(&evidence)) {
        (RunMode::Release, true) => evidence_dir.join("redline-docker-parity-candidate.json"),
        (RunMode::Release, false) => {
            evidence_dir.join("redline-docker-parity-release-failure.json")
        }
        (RunMode::Diagnostic, _) => evidence_dir.join("redline-docker-parity-diagnostic.json"),
    };
    write_checksummed_json(&output, &evidence)?;
    println!("redline Docker parity evidence: {}", output.display());
    if args.mode == RunMode::Release && !failures_is_empty(&evidence) {
        bail!(
            "Docker parity release campaign failed; diagnostic={}",
            output.display()
        );
    }
    Ok(())
}

fn contract_args(
    contract: &str,
    cases: &str,
    docker: &DockerParityArgs,
    output: PathBuf,
    target_args: Vec<String>,
) -> RunArgs {
    RunArgs {
        contract: Some(contract.to_owned()),
        cases: cases.to_owned(),
        mode: RunMode::Diagnostic,
        suite: Suite::All,
        target_bin: docker.target_bin.clone(),
        target_args,
        sqlite_bin: docker.sqlite_bin.display().to_string(),
        workers: docker.workers.clone(),
        tmp_root: docker.evidence_dir.join("tmp").display().to_string(),
        output,
        repetitions: 1,
        warmup: 0,
        progress: ProgressMode::Never,
        memory_samples: false,
    }
}

fn campaign_failures(
    args: &DockerParityArgs,
    sqlite: &Value,
    oracle: &crate::beyond_sqlite::oracle::OracleSummary,
    oracle_outcomes: &[crate::beyond_sqlite::oracle::CaseOutcome],
    postgres: &Value,
    exclusion_count: usize,
) -> Vec<String> {
    let mut failures = Vec::new();
    append_contract_failures("SQLite", sqlite, &mut failures);
    append_contract_failures("PostgreSQL target", postgres, &mut failures);
    let unique = oracle_outcomes
        .iter()
        .map(|outcome| outcome.case_id)
        .collect::<BTreeSet<_>>();
    if unique.len() != oracle_outcomes.len() {
        failures.push("PostgreSQL oracle emitted duplicate case IDs".to_owned());
    }
    if oracle.total != oracle_outcomes.len()
        || oracle.failed != 0
        || oracle.skipped_unavailable != 0
        || oracle.skipped_feature_missing != 0
        || oracle.passed != oracle.total
        || oracle_outcomes
            .iter()
            .any(|outcome| outcome.status != "passed")
    {
        failures.push(format!(
            "PostgreSQL oracle is not complete: total={} passed={} failed={} unavailable={} feature_skips={} outcomes={}",
            oracle.total,
            oracle.passed,
            oracle.failed,
            oracle.skipped_unavailable,
            oracle.skipped_feature_missing,
            oracle_outcomes.len()
        ));
    }
    if exclusion_count != POSTGRES_EXCLUSION_COUNT {
        failures.push(format!(
            "PostgreSQL exclusion drift: expected={POSTGRES_EXCLUSION_COUNT} actual={exclusion_count}"
        ));
    }
    if args.mode == RunMode::Release {
        if args.sqlite_cases != "all"
            || args.postgres_oracle_cases != "all"
            || args.postgres_target_cases != "all"
        {
            failures.push("release mode requires all three case selectors to be `all`".to_owned());
        }
        require_count(
            sqlite,
            "total",
            SQLITE_FULL_COUNT,
            "SQLite total",
            &mut failures,
        );
        require_count(
            sqlite,
            "passed",
            SQLITE_FULL_COUNT,
            "SQLite passed",
            &mut failures,
        );
        if oracle.total as u64 != POSTGRES_ORACLE_FULL_COUNT
            || oracle.passed as u64 != POSTGRES_ORACLE_FULL_COUNT
        {
            failures.push(format!(
                "PostgreSQL oracle count drift: expected={POSTGRES_ORACLE_FULL_COUNT} total={} passed={}",
                oracle.total, oracle.passed
            ));
        }
        require_count(
            postgres,
            "total",
            POSTGRES_TARGET_FULL_COUNT,
            "PostgreSQL governed target total",
            &mut failures,
        );
        require_count(
            postgres,
            "target_passed",
            POSTGRES_TARGET_FULL_COUNT,
            "PostgreSQL governed target passed",
            &mut failures,
        );
    }
    failures
}

fn append_contract_failures(label: &str, evidence: &Value, failures: &mut Vec<String>) {
    match evidence.get("release_failures").and_then(Value::as_array) {
        Some(values) => {
            for value in values {
                failures.push(format!(
                    "{label}: {}",
                    value.as_str().unwrap_or("malformed contract failure")
                ));
            }
        }
        None => failures.push(format!("{label}: missing release_failures array")),
    }
}

fn require_count(
    evidence: &Value,
    field: &str,
    expected: u64,
    label: &str,
    failures: &mut Vec<String>,
) {
    let actual = evidence
        .pointer(&format!("/results/{field}"))
        .and_then(Value::as_u64);
    if actual != Some(expected) {
        failures.push(format!(
            "{label} count drift: expected={expected} actual={actual:?}"
        ));
    }
}

fn result_counts(evidence: &Value) -> Result<Value> {
    evidence
        .get("results")
        .cloned()
        .context("contract evidence lacks results")
}

fn validate_postgres_connection(mode: RunMode) -> Result<()> {
    let connection = env::var("REDLINE_TESTING_POSTGRES_URL")
        .context("Docker parity requires REDLINE_TESTING_POSTGRES_URL")?;
    if connection.trim().is_empty() {
        bail!("Docker parity PostgreSQL connection is empty");
    }
    if mode == RunMode::Release
        && !connection.contains("@postgres:")
        && !connection.contains("host=postgres")
    {
        bail!("release Docker parity PostgreSQL connection must name Compose service `postgres`");
    }
    Ok(())
}

fn container_identity(mode: RunMode, failures: &mut Vec<String>) -> Value {
    let runner = fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let postgres = env::var("REDLINE_DOCKER_POSTGRES_CONTAINER_ID").unwrap_or_default();
    let health = env::var("REDLINE_DOCKER_POSTGRES_HEALTH").unwrap_or_default();
    if mode == RunMode::Release {
        if !is_container_id(&runner) {
            failures.push("runner container ID is missing or malformed".to_owned());
        }
        if !is_container_id(&postgres) {
            failures.push("PostgreSQL container ID is missing or malformed".to_owned());
        }
        if health != "healthy" {
            failures.push(format!(
                "PostgreSQL health is `{health}`, expected `healthy`"
            ));
        }
    }
    json!({
        "runner_id": runner,
        "postgres_id": postgres,
        "postgres_health": health,
        "compose_project": env::var("REDLINE_DOCKER_COMPOSE_PROJECT").unwrap_or_default(),
    })
}

fn is_container_id(value: &str) -> bool {
    (12..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_custody(path: &Path) -> Result<(Value, String)> {
    require_physical_file(path, "Docker custody receipt")?;
    verify_sidecar(path)?;
    let value = read_json(path, "Docker custody receipt")?;
    if value.get("schema_version").and_then(Value::as_str) != Some(CUSTODY_SCHEMA)
        || value.get("status").and_then(Value::as_str) != Some("pass")
        || value.get("provenance_source").and_then(Value::as_str) != Some("local-jeryu")
    {
        bail!("Docker custody receipt is not a passing local-Jeryu v1 receipt");
    }
    let artifacts = value
        .get("runtime_artifacts")
        .and_then(Value::as_array)
        .context("Docker custody receipt lacks runtime_artifacts")?;
    for artifact in artifacts {
        let artifact_path = PathBuf::from(
            artifact
                .get("path")
                .and_then(Value::as_str)
                .context("runtime artifact lacks path")?,
        );
        let expected = artifact
            .get("sha256")
            .and_then(Value::as_str)
            .context("runtime artifact lacks sha256")?;
        require_physical_file(&artifact_path, "Docker runtime artifact")?;
        if sha256_file(&artifact_path)? != expected {
            bail!(
                "Docker runtime artifact identity changed: {}",
                artifact_path.display()
            );
        }
    }
    let manifest_path = PathBuf::from(
        value
            .pointer("/images/manifest/path")
            .and_then(Value::as_str)
            .context("Docker custody receipt lacks image manifest path")?,
    );
    let manifest = fs::read_to_string(&manifest_path)?;
    validate_locked_image_manifest(&manifest)?;
    Ok((value, sha256_file(path)?))
}

fn validate_locked_image_manifest(body: &str) -> Result<()> {
    if body.contains(":latest") || body.contains("pull_policy = \"always\"") {
        bail!("Docker image manifest contains an unpinned or pull-enabled image");
    }
    let digest_refs = body.matches("@sha256:").count();
    if digest_refs != 2 {
        bail!("Docker image manifest must contain exactly two digest-qualified references");
    }
    Ok(())
}

fn prepare_evidence_dir(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!(
            "evidence directory is not a physical directory: {}",
            path.display()
        );
    }
    fs::canonicalize(path).context("resolve evidence directory")
}

fn require_physical_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} at {}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("{label} is not a physical regular file: {}", path.display());
    }
    Ok(())
}

fn verify_sidecar(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("receipt path has no UTF-8 filename")?;
    let sidecar = path.with_file_name(format!("{name}.sha256"));
    require_physical_file(&sidecar, "Docker custody checksum sidecar")?;
    let body = fs::read_to_string(&sidecar)?;
    let expected = body
        .split_whitespace()
        .next()
        .context("Docker custody checksum sidecar is empty")?;
    if expected != sha256_file(path)? || !body.trim_end().ends_with(name) {
        bail!("Docker custody checksum sidecar does not bind the receipt");
    }
    Ok(())
}

fn read_json(path: &Path, label: &str) -> Result<Value> {
    serde_json::from_slice(&fs::read(path)?).with_context(|| format!("parse {label}"))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut body = serde_json::to_vec_pretty(value)?;
    body.push(b'\n');
    fs::write(path, body)?;
    Ok(())
}

fn write_checksummed_json(path: &Path, value: &Value) -> Result<()> {
    write_json(path, value)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("evidence.json");
    fs::write(
        path.with_file_name(format!("{name}.sha256")),
        format!("{}  {name}\n", sha256_file(path)?),
    )?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn mode_name(mode: RunMode) -> &'static str {
    match mode {
        RunMode::Diagnostic => "diagnostic",
        RunMode::Release => "release",
    }
}

fn failures_is_empty(evidence: &Value) -> bool {
    evidence
        .get("release_failures")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_manifest_requires_two_digest_qualified_images() {
        assert!(validate_locked_image_manifest(
            "runner = \"ubuntu@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\npostgres = \"postgres@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\n"
        ).is_ok());
        assert!(validate_locked_image_manifest("runner = \"ubuntu:latest\"").is_err());
        assert!(validate_locked_image_manifest("runner = \"ubuntu@sha256:a\"").is_err());
    }

    #[test]
    fn container_ids_are_lowercase_hex_and_bounded() {
        assert!(is_container_id("0123456789ab"));
        assert!(!is_container_id("0123456789AB"));
        assert!(!is_container_id("short"));
    }
}
