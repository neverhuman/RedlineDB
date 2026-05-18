use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{DurabilityKind, EngineKind, WorkloadKind};
use crate::process_metrics::ProcessMetrics;
use crate::{ensure_parent, gates};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    pub rows: i64,
    pub version_sum: i64,
    pub payload_bytes: i64,
    pub content_hash: String,
    pub index_consistency: BTreeMap<String, String>,
    /// Lane INT: deterministic three-axis dataset fingerprint
    /// (`row_count` / `key_xor` / `payload_hash`). Replaces the old
    /// `MAX(k)` / `COUNT(*)` placeholder for the certification manifest's
    /// `checksums` field with something that surfaces row drift,
    /// key drift, and value drift independently. Optional so older
    /// JSONL records still round-trip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<crate::checksum::DatasetChecksum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencySummary {
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub p999_us: u64,
    pub max_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub operations: u64,
    pub failures: u64,
    /// Combined BUSY + LOCKED count, kept for one minor cycle so older
    /// dashboards continue to render. New consumers should prefer the
    /// split `locked_errors` and dedicated `timeout_errors` fields.
    pub busy_errors: u64,
    /// Errors whose message indicates the engine reported the
    /// destination as locked (e.g. `database is locked`,
    /// `lock_wait` expired). Reviewer Finding #7.
    #[serde(default)]
    pub locked_errors: u64,
    /// Errors that timed out waiting for a resource — distinct from
    /// busy/locked because the engine never refused outright but the
    /// caller tripped its deadline.
    #[serde(default)]
    pub timeout_errors: u64,
    pub elapsed_ms: u64,
    pub throughput_ops_per_sec: f64,
    pub latency: LatencySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub engine: EngineKind,
    pub workload: WorkloadKind,
    pub durability: DurabilityKind,
    pub threads: usize,
    pub seed: u64,
    pub cache_bytes: usize,
    pub environment: RunEnvironment,
    pub metrics: MetricsSummary,
    pub checksum: Checksum,
    pub data_bytes: u64,
    pub wal_bytes: u64,
    pub engine_stats: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_metrics: Option<ProcessMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEnvironment {
    pub hostname: String,
    pub git_sha: Option<String>,
    pub git_dirty: Option<bool>,
    pub rustc_version: Option<String>,
    pub sqlite_version: Option<String>,
    pub logical_cpus: usize,
    pub memory_mib: Option<u64>,
    pub image_digest: Option<String>,
}

pub fn write_run_output(path: Option<&Path>, record: &RunRecord) -> Result<()> {
    if let Some(path) = path {
        write_json(Some(path), record)
    } else {
        println!("{}", serde_json::to_string(record)?);
        Ok(())
    }
}

pub fn write_compare_output(
    out: Option<&Path>,
    report: Option<&Path>,
    records: &[RunRecord],
) -> Result<()> {
    if let Some(out) = out {
        ensure_parent(Some(out))?;
        let mut file = fs::File::create(out)?;
        for record in records {
            writeln!(file, "{}", serde_json::to_string(record)?)?;
        }
    }
    if let Some(report) = report {
        ensure_parent(Some(report))?;
        let markdown = gates::markdown_summary(records);
        fs::write(report, markdown)?;
    }
    if out.is_none() && report.is_none() {
        for record in records {
            println!("{}", serde_json::to_string(record)?);
        }
    }
    Ok(())
}

pub fn write_json(path: Option<&Path>, value: &impl Serialize) -> Result<()> {
    if let Some(path) = path {
        ensure_parent(Some(path))?;
        fs::write(path, serde_json::to_vec_pretty(value)?)?;
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

pub fn append_jsonl(path: &Path, record: &RunRecord) -> Result<()> {
    ensure_parent(Some(path))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open jsonl output {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

pub fn read_jsonl(path: &Path) -> Result<Vec<RunRecord>> {
    let file = fs::File::open(path)?;
    let mut records = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(&line)?);
    }
    Ok(records)
}

pub fn next_run_id(engine: EngineKind, workload: WorkloadKind) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    format!("{engine:?}-{workload:?}-{millis}")
}

pub fn collect_environment() -> RunEnvironment {
    RunEnvironment {
        hostname: hostname(),
        // Lane BH P1 #4: prefer caller-provided env vars so the
        // remote/Docker path (where `.git` is intentionally excluded
        // from the rsync) still records the host-side commit. Only
        // fall back to shelling out when neither is set; that keeps
        // local interactive runs working unchanged while making the
        // manifest's `git_sha` / `git_dirty` no longer silently None
        // on the certify host.
        git_sha: env_git_sha().or_else(|| command_output(["git", "rev-parse", "HEAD"])),
        git_dirty: env_git_dirty().or_else(|| {
            command_status(["git", "status", "--porcelain"]).map(|output| !output.is_empty())
        }),
        rustc_version: command_output(["rustc", "-V"]),
        sqlite_version: Some(rusqlite::version().to_owned()),
        logical_cpus: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
        memory_mib: total_memory_mib(),
        image_digest: std::env::var("REDLINEDB_BENCH_IMAGE_DIGEST").ok(),
    }
}

/// Read `REDLINEDB_BENCH_GIT_SHA` if set and non-empty.
///
/// This is the primary path for remote/Docker runs because the
/// container often has no `.git` directory — the host script captures
/// the SHA before issuing `docker run`.
fn env_git_sha() -> Option<String> {
    std::env::var("REDLINEDB_BENCH_GIT_SHA")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Read `REDLINEDB_BENCH_GIT_DIRTY` and parse it as a boolean. The
/// host script writes the literal string `true` or `false` so we
/// accept those exactly; any other value (including unset) yields
/// `None` and the caller falls back to `git status` if available.
fn env_git_dirty() -> Option<bool> {
    let raw = std::env::var("REDLINEDB_BENCH_GIT_DIRTY").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn hostname() -> String {
    command_output(["hostname"]).unwrap_or_else(|| "unknown".to_owned())
}

fn command_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let (cmd, tail) = args.split_first()?;
    let output = Command::new(cmd).args(tail).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}

fn command_status<const N: usize>(args: [&str; N]) -> Option<String> {
    let (cmd, tail) = args.split_first()?;
    let output = Command::new(cmd).args(tail).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}

fn total_memory_mib() -> Option<u64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            let kib = value
                .split_whitespace()
                .next()
                .and_then(|number| number.parse::<u64>().ok())?;
            return Some(kib / 1024);
        }
    }
    None
}
