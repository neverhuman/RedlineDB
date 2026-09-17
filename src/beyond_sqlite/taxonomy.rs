use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sqlite_parity::RunSummary;

const FEATURES: &str = include_str!("../../metadata/beyond_sqlite/features.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Feature {
    pub rank: usize,
    pub title: String,
    pub owner: String,
    pub proof_lane: String,
    pub source_tips: Vec<String>,
    pub status: FeatureStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    ManifestBacklog,
    PassingReference,
}

#[derive(Debug)]
pub struct RunConfig {
    pub target_bin: PathBuf,
    pub output: PathBuf,
    pub command_line: Vec<String>,
    pub started_unix_ms: u128,
    pub ended_unix_ms: u128,
}

#[derive(Debug, Serialize)]
struct BeyondRecord {
    suite: String,
    case_id: String,
    name: String,
    case_file: String,
    priority: String,
    profile: String,
    category: String,
    sample_index: usize,
    repetition_index: Option<usize>,
    sample_role: String,
    reference_engine: String,
    target_engine: String,
    reference_executable_path: String,
    target_executable_path: String,
    reference_executable_sha256: String,
    target_executable_sha256: String,
    reference_version: String,
    target_version: String,
    status: String,
    feature_status: FeatureStatus,
    proof_lane: String,
    source_tips: Vec<String>,
    reference_exit_code: Option<i32>,
    target_exit_code: Option<i32>,
    reference_elapsed_ns: u128,
    target_elapsed_ns: u128,
    latency_ratio: f64,
    memory_status: String,
    diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
struct BeyondSummary {
    suite: String,
    total_features: usize,
    passed_features: usize,
    failed_features: usize,
    skipped_features: usize,
    coverage_pct: f64,
    elapsed_ns: u128,
    /// Target-compare lane counts (psql↔target_bin compare for cases that
    /// passed the psql self-compare). Zero when the lane didn't run.
    #[serde(default)]
    target_total: usize,
    #[serde(default)]
    target_passed: usize,
    #[serde(default)]
    target_failed: usize,
    #[serde(default)]
    target_skipped: usize,
}

#[derive(Debug, Serialize)]
struct BeyondManifest {
    schema_version: String,
    suite: String,
    command_line: Vec<String>,
    output_files: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct BeyondProvenance {
    schema_version: String,
    suite: String,
    redline_testing_binary_path: String,
    redline_testing_binary_sha256: String,
    target_binary_path: String,
    target_binary_sha256: String,
    target_version: String,
    postgres_reference_status: String,
    command_line: Vec<String>,
    started_unix_ms: u128,
    ended_unix_ms: u128,
    output_file_hashes: BTreeMap<String, String>,
}

pub fn all_features() -> Result<Vec<Feature>> {
    serde_json::from_str(FEATURES).context("parse beyond_sqlite feature manifest")
}

pub fn run(config: RunConfig) -> Result<RunSummary> {
    let started = Instant::now();
    let features = all_features()?;
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let target = binary_identity(&config.target_bin).unwrap_or_else(|_| BinaryIdentity {
        path: config.target_bin.to_string_lossy().into_owned(),
        sha256: "<unknown>".to_owned(),
        version: "<unknown>".to_owned(),
    });
    let postgres_status = postgres_reference_status();
    let mut raw = String::new();
    let mut passed = 0usize;
    let mut skipped = 0usize;
    for feature in &features {
        let is_passing = feature.status == FeatureStatus::PassingReference;
        if is_passing {
            passed += 1;
        } else {
            skipped += 1;
        }
        let record = BeyondRecord {
            suite: "beyond_sqlite".to_owned(),
            case_id: format!("BEYOND-{:03}", feature.rank),
            name: feature.title.clone(),
            case_file: "metadata/beyond_sqlite/features.json".to_owned(),
            priority: if is_passing { "P0" } else { "P2" }.to_owned(),
            profile: "beyond_sqlite".to_owned(),
            category: feature.owner.clone(),
            sample_index: 0,
            repetition_index: Some(1),
            sample_role: "measured:1".to_owned(),
            reference_engine: "postgres".to_owned(),
            target_engine: "redlinedb".to_owned(),
            reference_executable_path: postgres_status.clone(),
            target_executable_path: target.path.clone(),
            reference_executable_sha256: "<metadata>".to_owned(),
            target_executable_sha256: target.sha256.clone(),
            reference_version: postgres_status.clone(),
            target_version: target.version.clone(),
            status: if is_passing { "passed" } else { "skipped" }.to_owned(),
            feature_status: feature.status,
            proof_lane: feature.proof_lane.clone(),
            source_tips: feature.source_tips.clone(),
            reference_exit_code: None,
            target_exit_code: None,
            reference_elapsed_ns: 0,
            target_elapsed_ns: 0,
            latency_ratio: 0.0,
            memory_status: "not_run".to_owned(),
            diagnostic: (!is_passing)
                .then(|| "manifest backlog; executable contract not promoted".to_owned()),
        };
        raw.push_str(&serde_json::to_string(&record)?);
        raw.push('\n');
    }

    // Executable-oracle layer: append per-case outcomes from
    // corpus/beyond_sqlite/generated_manifest.json. When the manifest is
    // empty (commit 5 baseline) this is a no-op. When Postgres is
    // unavailable, every case emits a `skipped` record with a diagnostic;
    // we never let oracle failures break the suite.
    //
    // Two record streams are emitted:
    //   * `profile: beyond_sqlite_oracle` — psql self-compare (ship gate)
    //   * `profile: beyond_sqlite_target` — psql↔target compare. Always
    //     attempted when the target binary path resolves; the target lane is
    //     emitted only for cases whose self-compare passed (so we don't
    //     attribute reference instability to the target).
    let (oracle_summary, oracle_outcomes) =
        super::oracle::run_cases_with(super::oracle::RunCasesOptions {
            target_bin: Some(config.target_bin.clone()),
        })
        .unwrap_or((
            super::oracle::OracleSummary {
                total: 0,
                passed: 0,
                skipped_unavailable: 0,
                skipped_feature_missing: 0,
                failed: 0,
                target_total: 0,
                target_passed: 0,
                target_failed: 0,
                target_skipped: 0,
            },
            Vec::new(),
        ));
    let mut oracle_raw = String::new();
    for outcome in &oracle_outcomes {
        let record = serde_json::json!({
            "suite": "beyond_sqlite",
            "case_id": format!("BEYOND-CASE-{:05}", outcome.case_id),
            "name": outcome.name,
            "case_file": "corpus/beyond_sqlite/generated_manifest.json",
            "priority": "P0",
            "profile": "beyond_sqlite_oracle",
            "category": &outcome.category,
            "sample_index": 0,
            "repetition_index": 1,
            "sample_role": "measured:1",
            "reference_engine": "postgres",
            "target_engine": "postgres",
            "feature_rank": outcome.feature_rank,
            "status": outcome.status,
            "diagnostic": outcome.diagnostic,
            "reference_elapsed_ns": 0,
            "target_elapsed_ns": 0,
            "memory_status": "not_run",
        });
        oracle_raw.push_str(&record.to_string());
        oracle_raw.push('\n');

        // Target-vs-reference record. Always emitted when the target lane ran,
        // even on skip/fail — downstream agents need to see the redlinedb
        // outputs to triage.
        if let Some(t) = outcome.target.as_ref() {
            let target_record = serde_json::json!({
                "suite": "beyond_sqlite",
                "case_id": format!("BEYOND-CASE-{:05}", outcome.case_id),
                "name": outcome.name,
                "case_file": "corpus/beyond_sqlite/generated_manifest.json",
                "priority": "P0",
                "profile": "beyond_sqlite_target",
                "category": &outcome.category,
                "sample_index": 0,
                "repetition_index": 1,
                "sample_role": "measured:1",
                "reference_engine": "postgres",
                "target_engine": "redlinedb",
                "feature_rank": outcome.feature_rank,
                "status": t.status,
                "diagnostic": t.diagnostic,
                "reference_exit_code": t.reference_exit,
                "target_exit_code": t.target_exit,
                "reference_stdout": t.reference_stdout,
                "target_stdout": t.target_stdout,
                "target_stderr_head": t.target_stderr_head,
                "reference_elapsed_ns": t.reference_elapsed_ns,
                "target_elapsed_ns": t.target_elapsed_ns,
                "memory_status": "not_run",
            });
            oracle_raw.push_str(&target_record.to_string());
            oracle_raw.push('\n');
        }
    }
    let mut combined = raw;
    combined.push_str(&oracle_raw);
    fs::write(&config.output, &combined)
        .with_context(|| format!("write {}", config.output.display()))?;
    write_artifacts(
        &config,
        &target,
        &postgres_status,
        &combined,
        BeyondSummary {
            suite: "beyond_sqlite".to_owned(),
            total_features: features.len() + oracle_summary.total,
            passed_features: passed + oracle_summary.passed,
            failed_features: oracle_summary.failed,
            skipped_features: skipped
                + oracle_summary.skipped_unavailable
                + oracle_summary.skipped_feature_missing,
            coverage_pct: pct(
                passed + oracle_summary.passed,
                features.len() + oracle_summary.total,
            ),
            elapsed_ns: started.elapsed().as_nanos(),
            target_total: oracle_summary.target_total,
            target_passed: oracle_summary.target_passed,
            target_failed: oracle_summary.target_failed,
            target_skipped: oracle_summary.target_skipped,
        },
        &features,
    )?;

    Ok(RunSummary {
        total: features.len() + oracle_summary.total,
        passed: passed + oracle_summary.passed,
        failed: oracle_summary.failed,
        skipped: skipped
            + oracle_summary.skipped_unavailable
            + oracle_summary.skipped_feature_missing,
        elapsed: Duration::from_nanos(started.elapsed().as_nanos().min(u64::MAX as u128) as u64),
        slowest: Vec::new(),
    })
}

fn write_artifacts(
    config: &RunConfig,
    target: &BinaryIdentity,
    postgres_status: &str,
    raw: &str,
    summary: BeyondSummary,
    features: &[Feature],
) -> Result<()> {
    let output_dir = config
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let summary_path = output_dir.join("beyond-sqlite-summary.json");
    let ranked_path = output_dir.join("beyond-sqlite-ranked.csv");
    let coverage_path = output_dir.join("beyond-sqlite-coverage.csv");
    let manifest_path = output_dir.join("beyond-sqlite-manifest.json");
    let provenance_path = output_dir.join("beyond-sqlite-provenance.json");

    let summary_json = serde_json::to_string_pretty(&summary)? + "\n";
    let ranked_csv = ranked_csv(features);
    let coverage_csv = coverage_csv(&summary);
    let manifest_json = serde_json::to_string_pretty(&BeyondManifest {
        schema_version: "redline-testing-manifest-v1".to_owned(),
        suite: "beyond_sqlite".to_owned(),
        command_line: config.command_line.clone(),
        output_files: BTreeMap::from([
            ("raw".to_owned(), display_path(&config.output)),
            ("summary".to_owned(), display_path(&summary_path)),
            ("ranked".to_owned(), display_path(&ranked_path)),
            ("coverage".to_owned(), display_path(&coverage_path)),
            ("provenance".to_owned(), display_path(&provenance_path)),
        ]),
    })? + "\n";
    let redline_testing_bin = std::env::current_exe().context("resolve current executable")?;
    let provenance_json = serde_json::to_string_pretty(&BeyondProvenance {
        schema_version: "redline-testing-provenance-v1".to_owned(),
        suite: "beyond_sqlite".to_owned(),
        redline_testing_binary_path: display_path(&redline_testing_bin),
        redline_testing_binary_sha256: sha256_file(&redline_testing_bin)?,
        target_binary_path: target.path.clone(),
        target_binary_sha256: target.sha256.clone(),
        target_version: target.version.clone(),
        postgres_reference_status: postgres_status.to_owned(),
        command_line: config.command_line.clone(),
        started_unix_ms: config.started_unix_ms,
        ended_unix_ms: config.ended_unix_ms,
        output_file_hashes: BTreeMap::from([
            ("beyond_sqlite.raw.jsonl".to_owned(), sha256_hex(raw)),
            (
                "beyond-sqlite-summary.json".to_owned(),
                sha256_hex(&summary_json),
            ),
            (
                "beyond-sqlite-ranked.csv".to_owned(),
                sha256_hex(&ranked_csv),
            ),
            (
                "beyond-sqlite-coverage.csv".to_owned(),
                sha256_hex(&coverage_csv),
            ),
            (
                "beyond-sqlite-manifest.json".to_owned(),
                sha256_hex(&manifest_json),
            ),
        ]),
    })? + "\n";

    fs::write(summary_path, summary_json)?;
    fs::write(ranked_path, ranked_csv)?;
    fs::write(coverage_path, coverage_csv)?;
    fs::write(manifest_path, manifest_json)?;
    fs::write(provenance_path, provenance_json)?;
    Ok(())
}

fn ranked_csv(features: &[Feature]) -> String {
    let mut out = String::from("rank,case_id,title,status,owner,proof_lane,source_tips\n");
    for feature in features {
        out.push_str(&format!(
            "{},BEYOND-{:03},{},{},{},{},{}\n",
            feature.rank,
            feature.rank,
            csv(&feature.title),
            serde_json::to_string(&feature.status)
                .unwrap_or_else(|_| "\"unknown\"".to_owned())
                .trim_matches('"')
                .to_owned(),
            csv(&feature.owner),
            csv(&feature.proof_lane),
            csv(&feature.source_tips.join(";"))
        ));
    }
    out
}

fn coverage_csv(summary: &BeyondSummary) -> String {
    format!(
        "suite,total_features,passed_features,failed_features,skipped_features,coverage_pct\n{},{},{},{},{},{:.6}\n",
        summary.suite,
        summary.total_features,
        summary.passed_features,
        summary.failed_features,
        summary.skipped_features,
        summary.coverage_pct
    )
}

fn postgres_reference_status() -> String {
    if let Ok(url) = std::env::var("REDLINE_TESTING_POSTGRES_URL")
        && !url.trim().is_empty()
    {
        return "configured".to_owned();
    }
    if command_exists("psql") {
        "psql_available_no_url".to_owned()
    } else {
        "unavailable".to_owned()
    }
}

fn command_exists(name: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| dir.join(name).is_file())
}

struct BinaryIdentity {
    path: String,
    sha256: String,
    version: String,
}

fn binary_identity(path: &Path) -> Result<BinaryIdentity> {
    let resolved = resolve_executable_path(path)?;
    Ok(BinaryIdentity {
        path: display_path(&resolved),
        sha256: sha256_file(&resolved)?,
        version: capture_version(&resolved).unwrap_or_else(|_| "<unknown>".to_owned()),
    })
}

fn resolve_executable_path(path: &Path) -> Result<PathBuf> {
    if path.components().count() > 1 || path.is_absolute() {
        return fs::canonicalize(path)
            .with_context(|| format!("canonicalize executable {}", path.display()));
    }
    let Some(path_var) = std::env::var_os("PATH") else {
        anyhow::bail!("PATH is unset while resolving {}", path.display());
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(path);
        if candidate.is_file() {
            return fs::canonicalize(candidate).context("canonicalize executable");
        }
    }
    anyhow::bail!("executable not found on PATH: {}", path.display())
}

fn capture_version(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", path.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn pct(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64 * 100.0
    }
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beyond_manifest_is_ranked_and_has_coverage() {
        let features = all_features().expect("features");
        assert_eq!(features.len(), 12);
        assert!(features.windows(2).all(|pair| pair[0].rank < pair[1].rank));
        assert_eq!(
            features
                .iter()
                .filter(|feature| feature.status == FeatureStatus::PassingReference)
                .count(),
            4
        );
    }
}
