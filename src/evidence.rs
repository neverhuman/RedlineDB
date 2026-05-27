use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sqlite_parity::RunSummary;

#[derive(Debug)]
pub struct EvidenceConfig {
    pub suite: String,
    pub output: PathBuf,
    pub target_bin: PathBuf,
    pub sqlite_bin: PathBuf,
    pub tmp_root: PathBuf,
    pub workers: String,
    pub repetitions: usize,
    pub warmup: usize,
    pub memory_samples: bool,
    pub command_line: Vec<String>,
    pub started_unix_ms: u128,
    pub ended_unix_ms: u128,
    pub summary: RunSummary,
}

#[derive(Debug)]
pub struct OfficialSuiteEvidence {
    pub name: String,
    pub raw_path: PathBuf,
    pub summary_path: PathBuf,
    pub ranked_path: PathBuf,
    pub manifest_path: PathBuf,
    pub provenance_path: PathBuf,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl OfficialSuiteEvidence {
    pub fn new(
        name: impl Into<String>,
        raw_path: PathBuf,
        summary_path: PathBuf,
        ranked_path: PathBuf,
        manifest_path: PathBuf,
        provenance_path: PathBuf,
        summary: &RunSummary,
    ) -> Self {
        Self {
            name: name.into(),
            raw_path,
            summary_path,
            ranked_path,
            manifest_path,
            provenance_path,
            total: summary.total,
            passed: summary.passed,
            failed: summary.failed,
            skipped: summary.skipped,
        }
    }
}

#[derive(Debug)]
pub struct OfficialEvidenceConfig {
    pub output_dir: PathBuf,
    pub all_output: PathBuf,
    pub all_manifest: PathBuf,
    pub target_bin: PathBuf,
    pub sqlite_bin: PathBuf,
    pub tmp_root: PathBuf,
    pub workers: String,
    pub repetitions: usize,
    pub warmup: usize,
    pub memory_samples: bool,
    pub command_line: Vec<String>,
    pub generated_at_unix_ms: u128,
    pub suites: Vec<OfficialSuiteEvidence>,
}

#[derive(Debug, Serialize)]
struct BinaryEvidence {
    path: String,
    sha256: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct RunnerEvidence {
    binary_path: String,
    binary_sha256: String,
    release_binary_sha256: String,
    release_tarball_sha256: Option<String>,
    version: String,
}

#[derive(Debug, Serialize)]
struct OfficialSuiteJson {
    name: String,
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    raw_path: String,
    summary_path: String,
    ranked_path: String,
    manifest_path: String,
    provenance_path: String,
}

#[derive(Debug, Serialize)]
struct OfficialEvidenceJson {
    schema_version: String,
    runner: RunnerEvidence,
    target: BinaryEvidence,
    sqlite: BinaryEvidence,
    suites: BTreeMap<String, OfficialSuiteJson>,
    status: String,
    command_line: Vec<String>,
    generated_at_unix_ms: u128,
    output_file_hashes: BTreeMap<String, String>,
    workers: String,
    repetitions: usize,
    warmup: usize,
    memory_samples: bool,
    tmp_root: String,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    case_id: String,
    name: String,
    #[serde(default)]
    case_file: String,
    priority: String,
    profile: String,
    category: String,
    #[serde(default)]
    sample_role: String,
    #[serde(default)]
    repetition_index: Option<usize>,
    status: String,
    reference_elapsed_ns: u128,
    target_elapsed_ns: u128,
}

#[derive(Debug)]
struct RankedCase {
    case_id: String,
    name: String,
    case_file: String,
    priority: String,
    profile: String,
    category: String,
    sqlite_median_ns: u128,
    redline_median_ns: u128,
    improvement_pct: f64,
    samples: usize,
}

#[derive(Debug, Serialize)]
struct SummaryJson {
    suite: String,
    total_cases: usize,
    passed_cases: usize,
    failed_cases: usize,
    skipped_cases: usize,
    elapsed_ns: u128,
    measured_samples: usize,
    warmup_samples: usize,
    ranked_cases: usize,
    repetitions: usize,
    warmup: usize,
}

#[derive(Debug, Serialize)]
struct ManifestJson {
    schema_version: String,
    suite: String,
    command_line: Vec<String>,
    workers: String,
    repetitions: usize,
    warmup: usize,
    memory_samples: bool,
    output_files: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ProvenanceJson {
    schema_version: String,
    suite: String,
    redlinedb_git_sha: String,
    redlinedb_git_dirty: bool,
    target_binary_path: String,
    target_binary_sha256: String,
    target_version: String,
    redline_testing_binary_path: String,
    redline_testing_binary_sha256: String,
    redline_testing_release_binary_sha256: String,
    redline_testing_release_tarball_sha256: Option<String>,
    sqlite_binary_path: String,
    sqlite_binary_sha256: String,
    sqlite_version: String,
    command_line: Vec<String>,
    worker_count: String,
    repetitions: usize,
    warmup: usize,
    memory_samples: bool,
    tmp_root: String,
    os: String,
    arch: String,
    cpu: String,
    available_parallelism: usize,
    started_unix_ms: u128,
    ended_unix_ms: u128,
    output_file_hashes: BTreeMap<String, String>,
}

pub fn write_sqlite_parity_evidence(config: EvidenceConfig) -> Result<()> {
    let raw_text = fs::read_to_string(&config.output)
        .with_context(|| format!("read raw output {}", config.output.display()))?;
    let raw_records = parse_raw_records(&raw_text)?;
    let ranked = ranked_cases(&raw_records);
    let output_dir = config
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let summary_path = output_dir.join(suite_artifact_name(&config.suite, "summary.json"));
    let ranked_path = output_dir.join(suite_artifact_name(&config.suite, "ranked.csv"));
    let manifest_path = output_dir.join(suite_artifact_name(&config.suite, "manifest.json"));
    let provenance_path = output_dir.join(suite_artifact_name(&config.suite, "provenance.json"));

    let summary_json = serde_json::to_string_pretty(&SummaryJson {
        suite: config.suite.clone(),
        total_cases: config.summary.total,
        passed_cases: config.summary.passed,
        failed_cases: config.summary.failed,
        skipped_cases: config.summary.skipped,
        elapsed_ns: config.summary.elapsed.as_nanos(),
        measured_samples: raw_records
            .iter()
            .filter(|record| is_measured(record))
            .count(),
        warmup_samples: raw_records
            .iter()
            .filter(|record| record.sample_role == "warmup")
            .count(),
        ranked_cases: ranked.len(),
        repetitions: config.repetitions,
        warmup: config.warmup,
    })? + "\n";
    let ranked_csv = ranked_csv(&ranked);
    let manifest_json = serde_json::to_string_pretty(&ManifestJson {
        schema_version: "redline-testing-manifest-v1".to_owned(),
        suite: config.suite.clone(),
        command_line: config.command_line.clone(),
        workers: config.workers.clone(),
        repetitions: config.repetitions,
        warmup: config.warmup,
        memory_samples: config.memory_samples,
        output_files: BTreeMap::from([
            ("raw".to_owned(), display_path(&config.output)),
            ("summary".to_owned(), display_path(&summary_path)),
            ("ranked".to_owned(), display_path(&ranked_path)),
            ("provenance".to_owned(), display_path(&provenance_path)),
        ]),
    })? + "\n";

    fs::write(&summary_path, summary_json)
        .with_context(|| format!("write {}", summary_path.display()))?;
    fs::write(&ranked_path, ranked_csv)
        .with_context(|| format!("write {}", ranked_path.display()))?;
    fs::write(&manifest_path, manifest_json)
        .with_context(|| format!("write {}", manifest_path.display()))?;

    let redline_testing_bin = std::env::current_exe().context("resolve current executable")?;
    let redline_testing_binary_sha256 = sha256_file(&redline_testing_bin)?;
    let release_binary_sha256 = env_sha("CI_REDLINE_TESTING_RELEASE_BINARY_SHA256")
        .or_else(|| env_sha("CI_REDLINE_TESTING_BIN_SHA256"))
        .unwrap_or_else(|| redline_testing_binary_sha256.clone());
    let raw_hash = sha256_file(&config.output)?;
    let output_hashes = BTreeMap::from([
        (file_name(&config.output), raw_hash.clone()),
        (display_path(&config.output), raw_hash),
        (file_name(&summary_path), sha256_file(&summary_path)?),
        (file_name(&ranked_path), sha256_file(&ranked_path)?),
        (file_name(&manifest_path), sha256_file(&manifest_path)?),
    ]);
    let provenance_json = serde_json::to_string_pretty(&ProvenanceJson {
        schema_version: "redline-testing-provenance-v1".to_owned(),
        suite: config.suite,
        redlinedb_git_sha: git_output(["rev-parse", "HEAD"])
            .unwrap_or_else(|| "<unknown>".to_owned()),
        redlinedb_git_dirty: git_dirty(),
        target_binary_path: canonical_display(&config.target_bin),
        target_binary_sha256: sha256_file(&resolve_executable_path(&config.target_bin)?)?,
        target_version: capture_version(&config.target_bin)?,
        redline_testing_binary_path: display_path(&redline_testing_bin),
        redline_testing_binary_sha256,
        redline_testing_release_binary_sha256: release_binary_sha256,
        redline_testing_release_tarball_sha256: env_sha(
            "CI_REDLINE_TESTING_RELEASE_TARBALL_SHA256",
        ),
        sqlite_binary_path: canonical_display(&config.sqlite_bin),
        sqlite_binary_sha256: sha256_file(&resolve_executable_path(&config.sqlite_bin)?)?,
        sqlite_version: capture_version(&config.sqlite_bin)?,
        command_line: config.command_line,
        worker_count: config.workers,
        repetitions: config.repetitions,
        warmup: config.warmup,
        memory_samples: config.memory_samples,
        tmp_root: display_path(&config.tmp_root),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        cpu: cpu_model(),
        available_parallelism: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        started_unix_ms: config.started_unix_ms,
        ended_unix_ms: config.ended_unix_ms,
        output_file_hashes: output_hashes,
    })? + "\n";
    fs::write(&provenance_path, provenance_json)
        .with_context(|| format!("write {}", provenance_path.display()))?;
    Ok(())
}

pub fn suite_artifact_path(output_dir: &Path, suite: &str, base: &str) -> PathBuf {
    output_dir.join(suite_artifact_name(suite, base))
}

fn suite_artifact_name(suite: &str, base: &str) -> String {
    match suite {
        "sqlite_parity" | "all" => base.to_owned(),
        "rql_phase1" => format!("rql-phase1-{base}"),
        "memory" => format!("memory-{base}"),
        other => format!("{}-{base}", other.replace('_', "-")),
    }
}

pub fn write_official_evidence(config: OfficialEvidenceConfig) -> Result<()> {
    let redline_testing_bin = std::env::current_exe().context("resolve current executable")?;
    let redline_testing_binary_sha256 = sha256_file(&redline_testing_bin)?;
    let release_binary_sha256 = env_sha("CI_REDLINE_TESTING_RELEASE_BINARY_SHA256")
        .or_else(|| env_sha("CI_REDLINE_TESTING_BIN_SHA256"))
        .unwrap_or_else(|| redline_testing_binary_sha256.clone());
    let runner = RunnerEvidence {
        binary_path: display_path(&redline_testing_bin),
        binary_sha256: redline_testing_binary_sha256,
        release_binary_sha256,
        release_tarball_sha256: env_sha("CI_REDLINE_TESTING_RELEASE_TARBALL_SHA256"),
        version: format!("redline-testing {}", env!("CARGO_PKG_VERSION")),
    };
    let target = BinaryEvidence {
        path: canonical_display(&config.target_bin),
        sha256: sha256_file(&resolve_executable_path(&config.target_bin)?)?,
        version: capture_version(&config.target_bin)?,
    };
    let sqlite = BinaryEvidence {
        path: canonical_display(&config.sqlite_bin),
        sha256: sha256_file(&resolve_executable_path(&config.sqlite_bin)?)?,
        version: capture_version(&config.sqlite_bin)?,
    };

    let mut output_file_hashes = BTreeMap::new();
    insert_hash(
        &mut output_file_hashes,
        &config.output_dir,
        &config.all_output,
    )?;
    insert_hash(
        &mut output_file_hashes,
        &config.output_dir,
        &config.all_manifest,
    )?;

    let mut suites = BTreeMap::new();
    let mut failed = 0usize;
    for suite in config.suites {
        failed = failed.saturating_add(suite.failed);
        insert_hash(&mut output_file_hashes, &config.output_dir, &suite.raw_path)?;
        insert_hash(
            &mut output_file_hashes,
            &config.output_dir,
            &suite.summary_path,
        )?;
        insert_hash(
            &mut output_file_hashes,
            &config.output_dir,
            &suite.ranked_path,
        )?;
        insert_hash(
            &mut output_file_hashes,
            &config.output_dir,
            &suite.manifest_path,
        )?;
        insert_hash(
            &mut output_file_hashes,
            &config.output_dir,
            &suite.provenance_path,
        )?;
        suites.insert(
            suite.name.clone(),
            OfficialSuiteJson {
                name: suite.name,
                total: suite.total,
                passed: suite.passed,
                failed: suite.failed,
                skipped: suite.skipped,
                raw_path: relative_display(&config.output_dir, &suite.raw_path),
                summary_path: relative_display(&config.output_dir, &suite.summary_path),
                ranked_path: relative_display(&config.output_dir, &suite.ranked_path),
                manifest_path: relative_display(&config.output_dir, &suite.manifest_path),
                provenance_path: relative_display(&config.output_dir, &suite.provenance_path),
            },
        );
    }

    let evidence = OfficialEvidenceJson {
        schema_version: "redline-testing-official-evidence-v1".to_owned(),
        runner,
        target,
        sqlite,
        suites,
        status: if failed == 0 { "passed" } else { "failed" }.to_owned(),
        command_line: config.command_line,
        generated_at_unix_ms: config.generated_at_unix_ms,
        output_file_hashes,
        workers: config.workers,
        repetitions: config.repetitions,
        warmup: config.warmup,
        memory_samples: config.memory_samples,
        tmp_root: display_path(&config.tmp_root),
    };
    let evidence_path = config.output_dir.join("official-evidence.json");
    fs::write(
        &evidence_path,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )
    .with_context(|| format!("write {}", evidence_path.display()))
}

fn insert_hash(
    output_file_hashes: &mut BTreeMap<String, String>,
    output_dir: &Path,
    path: &Path,
) -> Result<()> {
    output_file_hashes.insert(relative_display(output_dir, path), sha256_file(path)?);
    Ok(())
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(display_path)
        .unwrap_or_else(|_| display_path(path))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| display_path(path))
}

pub fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn parse_raw_records(raw_text: &str) -> Result<Vec<RawRecord>> {
    let mut records = Vec::new();
    for (index, line) in raw_text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        records.push(
            serde_json::from_str(line)
                .with_context(|| format!("parse raw JSONL line {}", index.saturating_add(1)))?,
        );
    }
    Ok(records)
}

fn ranked_cases(records: &[RawRecord]) -> Vec<RankedCase> {
    let mut grouped = BTreeMap::<String, Vec<&RawRecord>>::new();
    for record in records.iter().filter(|record| is_measured(record)) {
        grouped
            .entry(record.case_id.clone())
            .or_default()
            .push(record);
    }
    let mut ranked = Vec::new();
    for (case_id, records) in grouped {
        let first = records[0];
        let sqlite_median_ns = median(records.iter().map(|record| record.reference_elapsed_ns));
        let redline_median_ns = median(records.iter().map(|record| record.target_elapsed_ns));
        ranked.push(RankedCase {
            case_id,
            name: first.name.clone(),
            case_file: first.case_file.clone(),
            priority: first.priority.clone(),
            profile: first.profile.clone(),
            category: first.category.clone(),
            sqlite_median_ns,
            redline_median_ns,
            improvement_pct: improvement_pct(sqlite_median_ns, redline_median_ns),
            samples: records.len(),
        });
    }
    ranked.sort_by(|left, right| {
        left.improvement_pct
            .total_cmp(&right.improvement_pct)
            .then_with(|| left.case_id.cmp(&right.case_id))
    });
    ranked
}

fn ranked_csv(ranked: &[RankedCase]) -> String {
    let mut out = String::from(
        "rank,case_id,name,case_file,priority,profile,category,sqlite_median_ns,redline_median_ns,improvement_pct,samples\n",
    );
    for (index, row) in ranked.iter().enumerate() {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.6},{}\n",
            index.saturating_add(1),
            row.case_id,
            csv(&row.name),
            csv(&row.case_file),
            row.priority,
            row.profile,
            csv(&row.category),
            row.sqlite_median_ns,
            row.redline_median_ns,
            row.improvement_pct,
            row.samples
        ));
    }
    out
}

fn is_measured(record: &RawRecord) -> bool {
    record.status == "passed"
        && (record.repetition_index.is_some() || record.sample_role.starts_with("measured"))
}

fn median(values: impl Iterator<Item = u128>) -> u128 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_unstable();
    values[values.len() / 2]
}

fn improvement_pct(sqlite_median_ns: u128, redline_median_ns: u128) -> f64 {
    let effective_sqlite_ns = sqlite_median_ns.max(3_000_000);
    (effective_sqlite_ns as f64 - redline_median_ns as f64) / effective_sqlite_ns.max(1) as f64
        * 100.0
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn env_sha(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn capture_version(path: &Path) -> Result<String> {
    let resolved = resolve_executable_path(path)?;
    let output = Command::new(&resolved)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", resolved.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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
            return fs::canonicalize(&candidate)
                .with_context(|| format!("canonicalize executable {}", candidate.display()));
        }
    }
    anyhow::bail!("executable not found on PATH: {}", path.display())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn canonical_display(path: &Path) -> String {
    resolve_executable_path(path)
        .map(|path| display_path(&path))
        .unwrap_or_else(|_| display_path(path))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git_dirty() -> bool {
    !Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .is_ok_and(|status| status.success())
        || !Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .status()
            .is_ok_and(|status| status.success())
}

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| {
                    line.strip_prefix("model name")
                        .or_else(|| line.strip_prefix("Hardware"))
                })
                .and_then(|line| {
                    line.split_once(':')
                        .map(|(_, value)| value.trim().to_owned())
                })
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "<unknown>".to_owned())
}
