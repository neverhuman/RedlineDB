//! Summarize chaos benchmark raw records and persist versioned JSON.
//!
//! Rust port of the previous `scripts/bench/dick_head_choas_report.py` tool.
//! The output JSON shape matches the Python version byte-for-byte except for
//! the live `generated_at_utc` timestamps, which are emitted in the same
//! ISO-8601 UTC form (`YYYY-MM-DDTHH:MM:SS.mmmuuu+00:00`) the original used.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

const DEFAULT_VERSION_ROOT_REL: &str = "target/bench/versioned";
const DEFAULT_SUITE: &str = "dick-head-choas";

#[derive(Debug)]
struct Args {
    input: PathBuf,
    version_root: PathBuf,
    suite: String,
}

fn parse_args(argv: &[OsString]) -> Result<Args, String> {
    let mut input: Option<PathBuf> = None;
    let mut version_root: Option<PathBuf> = None;
    let mut suite: Option<String> = None;
    let mut iter = argv.iter().skip(1);
    while let Some(raw) = iter.next() {
        let arg = raw.to_string_lossy().into_owned();
        let (key, inline_value) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (arg, None),
        };
        match key.as_str() {
            "--input" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--input requires a value".to_string()),
                    },
                };
                input = Some(PathBuf::from(val));
            }
            "--version-root" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--version-root requires a value".to_string()),
                    },
                };
                version_root = Some(PathBuf::from(val));
            }
            "--suite" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--suite requires a value".to_string()),
                    },
                };
                suite = Some(val);
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let input = match input {
        Some(v) => v,
        None => return Err("--input is required".to_string()),
    };
    let version_root = version_root.unwrap_or_else(default_version_root);
    let suite = suite.unwrap_or_else(|| DEFAULT_SUITE.to_string());
    Ok(Args {
        input,
        version_root,
        suite,
    })
}

fn usage() -> String {
    String::from(
        "usage: chaos_report --input <stamp-dir> \
[--version-root <dir>] [--suite <name>]",
    )
}

fn default_version_root() -> PathBuf {
    repo_root().join(DEFAULT_VERSION_ROOT_REL)
}

/// Best-effort repo root. Matches the Python script's
/// `Path(__file__).resolve().parents[2]` semantics by walking up from
/// the binary's manifest dir. Falls back to the current working dir
/// when the layout cannot be detected (e.g. installed binary).
fn repo_root() -> PathBuf {
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if out.status.success() {
            if let Ok(trimmed) = std::str::from_utf8(&out.stdout) {
                let path = PathBuf::from(trimmed.trim_end_matches('\n'));
                if !path.as_os_str().is_empty() {
                    return path;
                }
            }
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn repo_git_sha(repo: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("git rev-parse failed to spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git rev-parse exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let s = String::from_utf8(out.stdout).map_err(|e| format!("git output not utf-8: {e}"))?;
    Ok(s.trim_end_matches('\n').to_string())
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let mut buf = String::new();
    fs::File::open(path)
        .map_err(|e| format!("open {}: {}", path.display(), e))?
        .read_to_string(&mut buf)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    serde_json::from_str(&buf).map_err(|e| format!("parse {}: {}", path.display(), e))
}

fn load_records(stamp_dir: &Path) -> Result<Vec<Value>, String> {
    let raw_dir = stamp_dir.join("raw");
    if !raw_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(&raw_dir).map_err(|e| format!("read_dir {}: {}", raw_dir.display(), e))?
    {
        let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let record_path = p.join("record.json");
        if record_path.exists() {
            paths.push(record_path);
        }
    }
    // Match Python: `sorted(glob.glob(...))` sorts lexicographically.
    paths.sort();
    let mut records = Vec::with_capacity(paths.len());
    for p in paths {
        let mut record = read_json_file(&p)?;
        if let Value::Object(ref mut map) = record {
            let run_name = p
                .parent()
                .and_then(Path::file_name)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            map.insert(
                "_path".to_string(),
                Value::String(p.to_string_lossy().into_owned()),
            );
            map.insert("_run".to_string(), Value::String(run_name));
            records.push(record);
        } else {
            return Err(format!("record at {} is not a JSON object", p.display()));
        }
    }
    Ok(records)
}

fn load_manifest(stamp_dir: &Path) -> Result<Option<Value>, String> {
    let path = stamp_dir.join("manifest.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_json_file(&path)?))
}

/// Replicates Python's `statistics.median`. For an even number of
/// values, the mean of the two middle elements is returned as an f64.
/// Returns NaN on empty input to match `float("nan")`.
fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Median that preserves Python's `statistics.median` typing semantics
/// for JSON output: when the list has an odd length, the middle JSON
/// value is returned as-is (int stays int, float stays float); when
/// the list has an even length, the average of the two middles is
/// always a float. Returns Null for an empty list.
fn median_value(values: &[Value]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let mut idx: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (i, v.as_f64().unwrap_or(f64::NAN)))
        .collect();
    idx.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    let n = idx.len();
    if n % 2 == 1 {
        values[idx[n / 2].0].clone()
    } else {
        let lo = idx[n / 2 - 1].1;
        let hi = idx[n / 2].1;
        let avg = (lo + hi) / 2.0;
        serde_json::Number::from_f64(avg)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn infer_source_paths(workload: &str) -> Vec<String> {
    let chaos = [
        "chaos-lock-convoy",
        "chaos-connection-churn",
        "chaos-checkpoint-thrash",
        "chaos-index-hammer",
        "chaos-sort-spill-convoy",
        "chaos-schema-storm",
    ];
    let index_batch = ["secondary-index-range", "secondary-index-count"];
    let index_access = [
        "secondary-index-read",
        "covered-range-cold",
        "covered-range-warm",
    ];
    let top = ["secondary-index-ordered-limit"];
    let workload_rs = [
        "single-row-insert",
        "batched-insert100",
        "point-read-pk",
        "writers-disjoint",
        "hot-row-update",
        "mixed-oltp",
        "mixed95-read5-write",
        "mixed80-read20-write",
        "mixed50-read50-write",
        "connection-limit",
        "large-sort-spill",
        "json-path-extract",
        "json-path-update",
        "vector-flat-search",
        "vector-ann-search",
        "vector-ann-search-disk",
        "commit-storm-batched",
        "hot-counter-update",
        "queue-mixed",
    ];
    let mut source_paths = Vec::new();
    if chaos.contains(&workload) {
        source_paths.push("crates/bench/src/chaos.rs".to_string());
    } else if index_batch.contains(&workload) {
        source_paths.push("crates/sql/src/exec/index_batch.rs".to_string());
    } else if index_access.contains(&workload) {
        source_paths.push("crates/sql/src/exec/index_access.rs".to_string());
    } else if top.contains(&workload) {
        source_paths.push("crates/sql/src/exec/select_top.rs".to_string());
    } else if workload_rs.contains(&workload) {
        source_paths.push("crates/bench/src/workload.rs".to_string());
    } else {
        source_paths.push("crates/bench/src/workload.rs".to_string());
    }
    source_paths
}

fn record_source_paths(record: &Value) -> Vec<String> {
    if let Some(stats) = record.get("engine_stats").and_then(Value::as_object) {
        if let Some(p) = stats.get("test_code_path").and_then(Value::as_str) {
            if !p.is_empty() {
                return vec![p.to_string()];
            }
        }
    }
    let workload = record.get("workload").and_then(Value::as_str).unwrap_or("");
    infer_source_paths(workload)
}

fn normalize_record(record: &Value, manifest: Option<&Value>) -> Value {
    let mut out = record.clone();
    let workload = record
        .get("workload")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut source_paths: Vec<String> = Vec::new();
    if let Some(stats) = record.get("engine_stats").and_then(Value::as_object) {
        if let Some(p) = stats.get("test_code_path").and_then(Value::as_str) {
            if !p.is_empty() {
                source_paths.push(p.to_string());
            }
        }
    }
    if source_paths.is_empty() {
        source_paths = infer_source_paths(&workload);
    }
    let path = record
        .get("_path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let run = record
        .get("_run")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Value::Object(ref mut map) = out {
        map.insert("raw_path".to_string(), Value::String(path));
        map.insert("run_dir".to_string(), Value::String(run));
        let mut dedup: Vec<String> = source_paths;
        dedup.sort();
        dedup.dedup();
        map.insert(
            "test_code_paths".to_string(),
            Value::Array(dedup.into_iter().map(Value::String).collect()),
        );
        if let Some(m) = manifest {
            if let Some(cp) = m.get("config_path").and_then(Value::as_str) {
                map.insert("config_path".to_string(), Value::String(cp.to_string()));
            }
        }
    }
    out
}

fn collect_pointer(runs: &[&Value], pointer: &str) -> Vec<Value> {
    runs.iter()
        .filter_map(|r| r.pointer(pointer).cloned())
        .collect()
}

fn engine_summary(runs: &[&Value]) -> Value {
    let qps = collect_pointer(runs, "/metrics/throughput_ops_per_sec");
    let p99 = collect_pointer(runs, "/metrics/latency/p99_us");
    let failures = collect_pointer(runs, "/metrics/failures");
    let busy = collect_pointer(runs, "/metrics/busy_errors");
    let locked = collect_pointer(runs, "/metrics/locked_errors");
    let timeout = collect_pointer(runs, "/metrics/timeout_errors");
    let elapsed = collect_pointer(runs, "/metrics/elapsed_ms");
    let raw_paths: Vec<Value> = runs
        .iter()
        .map(|r| {
            r.get("_path")
                .cloned()
                .unwrap_or(Value::String(String::new()))
        })
        .collect();
    let run_ids: Vec<Value> = runs
        .iter()
        .map(|r| r.get("run_id").cloned().unwrap_or(Value::Null))
        .collect();
    json!({
        "runs": runs.len(),
        "median_qps": median_value(&qps),
        "median_p99_us": median_value(&p99),
        "median_failures": median_value(&failures),
        "median_busy_errors": median_value(&busy),
        "median_locked_errors": median_value(&locked),
        "median_timeout_errors": median_value(&timeout),
        "median_elapsed_ms": median_value(&elapsed),
        "raw_record_paths": raw_paths,
        "run_ids": run_ids,
    })
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct ComparisonKey {
    workload: String,
    durability: String,
    threads: i64,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct GroupKey {
    engine: String,
    workload: String,
    durability: String,
    threads: i64,
}

fn extract_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        _ => None,
    }
}

fn build_groups<'a>(records: &'a [Value]) -> BTreeMap<GroupKey, Vec<&'a Value>> {
    let mut grouped: BTreeMap<GroupKey, Vec<&Value>> = BTreeMap::new();
    for record in records {
        let run = record.get("_run").and_then(Value::as_str).unwrap_or("");
        if run.ends_with("-w0") {
            continue;
        }
        let key = GroupKey {
            engine: record
                .get("engine")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            workload: record
                .get("workload")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            durability: record
                .get("durability")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            threads: record.get("threads").and_then(extract_int).unwrap_or(0),
        };
        grouped.entry(key).or_default().push(record);
    }
    grouped
}

fn build_comparisons(records: &[Value]) -> Vec<Value> {
    let grouped = build_groups(records);
    let mut by_workload: BTreeMap<ComparisonKey, BTreeMap<String, Vec<&Value>>> = BTreeMap::new();
    for (key, runs) in grouped {
        let comp_key = ComparisonKey {
            workload: key.workload.clone(),
            durability: key.durability.clone(),
            threads: key.threads,
        };
        by_workload
            .entry(comp_key)
            .or_default()
            .insert(key.engine.clone(), runs);
    }
    let mut comparisons = Vec::new();
    for (cmp_key, engines) in by_workload {
        let empty: Vec<&Value> = Vec::new();
        let redline_runs = engines.get("redline").unwrap_or(&empty);
        let sqlite_runs = engines.get("sqlite").unwrap_or(&empty);
        let mut source_paths: Vec<String> = redline_runs
            .iter()
            .chain(sqlite_runs.iter())
            .flat_map(|r| record_source_paths(r))
            .collect();
        source_paths.sort();
        source_paths.dedup();
        let mut comparison = Map::new();
        comparison.insert(
            "workload".to_string(),
            Value::String(cmp_key.workload.clone()),
        );
        comparison.insert(
            "durability".to_string(),
            Value::String(cmp_key.durability.clone()),
        );
        comparison.insert("threads".to_string(), Value::Number(cmp_key.threads.into()));
        comparison.insert(
            "test_code_paths".to_string(),
            Value::Array(source_paths.into_iter().map(Value::String).collect()),
        );
        if !redline_runs.is_empty() {
            comparison.insert("redline".to_string(), engine_summary(redline_runs));
        }
        if !sqlite_runs.is_empty() {
            comparison.insert("sqlite".to_string(), engine_summary(sqlite_runs));
        }
        if !redline_runs.is_empty() && !sqlite_runs.is_empty() {
            let redline_qps = comparison["redline"]["median_qps"].as_f64();
            let sqlite_qps = comparison["sqlite"]["median_qps"].as_f64();
            let ratio = match (redline_qps, sqlite_qps) {
                (Some(r), Some(s)) if s != 0.0 && !s.is_nan() => {
                    json!(r / s)
                }
                _ => Value::Null,
            };
            comparison.insert("ratio_redline_vs_sqlite".to_string(), ratio);
        }
        comparisons.push(Value::Object(comparison));
    }
    comparisons
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir -p {}: {}", parent.display(), e))?;
    }
    // Python uses `json.dump(value, ..., indent=2, sort_keys=True)`
    // followed by a trailing newline. `serde_json::to_string_pretty`
    // uses 2-space indent; serde_json::Map preserves insertion order so
    // we sort by re-serializing through a BTreeMap.
    let sorted = sort_value_keys(value);
    let text = serde_json::to_string_pretty(&sorted)
        .map_err(|e| format!("serialize {}: {}", path.display(), e))?;
    let mut file =
        fs::File::create(path).map_err(|e| format!("create {}: {}", path.display(), e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    file.write_all(b"\n")
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(())
}

/// Recursively walk a JSON value and rebuild every object as a
/// BTreeMap-backed `serde_json::Map`, so the final serialization is
/// sorted by key like Python's `sort_keys=True`.
fn sort_value_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), sort_value_keys(v));
            }
            // Build a serde_json::Map preserving the BTreeMap order.
            let mut out = Map::with_capacity(sorted.len());
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_value_keys).collect()),
        _ => value.clone(),
    }
}

fn refresh_index(version_dir: &Path, git_sha: &str) -> Result<PathBuf, String> {
    let mut reports: Vec<Value> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    if version_dir.exists() {
        for entry in fs::read_dir(version_dir)
            .map_err(|e| format!("read_dir {}: {}", version_dir.display(), e))?
        {
            let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if p.file_name().and_then(|s| s.to_str()) == Some("index.json") {
                continue;
            }
            paths.push(p);
        }
    }
    paths.sort();
    for p in paths {
        let data = read_json_file(&p)?;
        let mut entry = Map::new();
        entry.insert(
            "file".to_string(),
            Value::String(
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
        );
        entry.insert(
            "stamp".to_string(),
            data.get("stamp").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "suite".to_string(),
            data.get("suite").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "profile".to_string(),
            data.get("profile").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "workloads".to_string(),
            data.get("workloads")
                .cloned()
                .unwrap_or(Value::Array(Vec::new())),
        );
        entry.insert(
            "config_path".to_string(),
            data.get("config_path").cloned().unwrap_or(Value::Null),
        );
        reports.push(Value::Object(entry));
    }
    let index = json!({
        "git_sha": git_sha,
        "generated_at_utc": iso_utc_now(),
        "reports": reports,
    });
    let index_path = version_dir.join("index.json");
    write_json(&index_path, &index)?;
    Ok(index_path)
}

/// Emit the current UTC time as an ISO-8601 string with microsecond
/// precision and a `+00:00` suffix, matching Python's
/// `datetime.now(timezone.utc).isoformat()`.
fn iso_utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs() as i64;
    let micros = (now.subsec_micros()) as u32;
    iso_utc_format(total_secs, micros)
}

/// Format a Unix epoch (seconds + microseconds) into the same string
/// shape Python emits: `YYYY-MM-DDTHH:MM:SS.uuuuuu+00:00` (or no
/// `.uuuuuu` if microseconds are zero, also matching Python).
fn iso_utc_format(epoch_secs: i64, micros: u32) -> String {
    let (year, month, day, hour, minute, second) = civil_from_epoch(epoch_secs);
    if micros == 0 {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
            year, month, day, hour, minute, second
        )
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            year, month, day, hour, minute, second, micros
        )
    }
}

/// Convert a Unix epoch in UTC seconds to a (Y, M, D, h, m, s) tuple.
/// Algorithm from Howard Hinnant's date library, ported to integer math.
fn civil_from_epoch(epoch_secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let mut secs = epoch_secs % 86_400;
    let mut days = epoch_secs.div_euclid(86_400);
    if secs < 0 {
        secs += 86_400;
        days -= 1;
    }
    let hour = (secs / 3600) as u32;
    let minute = ((secs % 3600) / 60) as u32;
    let second = (secs % 60) as u32;

    // Shift epoch so that day 0 is March 1, year 0 (Hinnant's "days from civil").
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32, hour, minute, second)
}

fn run() -> Result<(), String> {
    let argv: Vec<OsString> = env::args_os().collect();
    let args = parse_args(&argv)?;
    let manifest = load_manifest(&args.input)?;
    let records = load_records(&args.input)?;
    if records.is_empty() {
        println!("no records found under {}", args.input.display());
        return Err(String::new());
    }

    let grouped = build_groups(&records);

    let git_sha = match manifest
        .as_ref()
        .and_then(|m| m.get("git_sha"))
        .and_then(Value::as_str)
    {
        Some(s) => s.to_string(),
        None => repo_git_sha(&repo_root())?,
    };

    let version_dir = args.version_root.join(&git_sha);
    fs::create_dir_all(&version_dir)
        .map_err(|e| format!("mkdir -p {}: {}", version_dir.display(), e))?;

    let stamp_name = args
        .input
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    println!("stamp: {stamp_name}");
    println!("records: {}", records.len());
    for (key, runs) in &grouped {
        let qps: Vec<f64> = runs
            .iter()
            .filter_map(|r| {
                r.pointer("/metrics/throughput_ops_per_sec")
                    .and_then(Value::as_f64)
            })
            .collect();
        let p99: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.pointer("/metrics/latency/p99_us").and_then(Value::as_f64))
            .collect();
        let failures: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.pointer("/metrics/failures").and_then(Value::as_f64))
            .collect();
        let busy: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.pointer("/metrics/busy_errors").and_then(Value::as_f64))
            .collect();
        let locked: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.pointer("/metrics/locked_errors").and_then(Value::as_f64))
            .collect();
        let timeout: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.pointer("/metrics/timeout_errors").and_then(Value::as_f64))
            .collect();
        println!(
            "{:7} {:38} {:7} t{:<4} qps={:10.2} p99_us={:8.0} fail={:4.0} busy={:4.0} locked={:4.0} timeout={:4.0}",
            key.engine,
            key.workload,
            key.durability,
            key.threads,
            median(qps),
            median(p99),
            median(failures),
            median(busy),
            median(locked),
            median(timeout),
        );
    }

    println!();
    println!("redline-vs-sqlite median qps ratios:");
    let comparisons = build_comparisons(&records);
    for comparison in &comparisons {
        let redline = comparison.get("redline");
        let sqlite = comparison.get("sqlite");
        if redline.is_some() && sqlite.is_some() {
            let ratio = comparison.get("ratio_redline_vs_sqlite");
            let ratio_display = match ratio.and_then(Value::as_f64) {
                Some(r) => format!("{:8.3}", r),
                None => "   null".to_string(),
            };
            let workload = comparison
                .get("workload")
                .and_then(Value::as_str)
                .unwrap_or("");
            let durability = comparison
                .get("durability")
                .and_then(Value::as_str)
                .unwrap_or("");
            let threads = comparison.get("threads").and_then(extract_int).unwrap_or(0);
            let redline_qps = redline
                .and_then(|v| v.get("median_qps"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let sqlite_qps = sqlite
                .and_then(|v| v.get("median_qps"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            println!(
                "{:38} {:7} t{:<4} ratio={} redline={:10.2} sqlite={:10.2}",
                workload, durability, threads, ratio_display, redline_qps, sqlite_qps
            );
        }
    }

    let normalized_records: Vec<Value> = records
        .iter()
        .map(|r| normalize_record(r, manifest.as_ref()))
        .collect();
    let mut workloads: Vec<String> = records
        .iter()
        .map(|r| {
            r.get("workload")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect();
    workloads.sort();
    workloads.dedup();
    let mut source_paths: Vec<String> = normalized_records
        .iter()
        .flat_map(|r| {
            r.get("test_code_paths")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();
    source_paths.sort();
    source_paths.dedup();

    let suite_name = manifest
        .as_ref()
        .and_then(|m| m.get("suite"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| args.suite.clone());

    let config_path_value = manifest
        .as_ref()
        .and_then(|m| m.get("config_path"))
        .cloned()
        .unwrap_or(Value::Null);
    let config_hash_value = manifest
        .as_ref()
        .and_then(|m| m.get("config_hash"))
        .cloned()
        .unwrap_or(Value::Null);
    let manifest_value = manifest.clone().unwrap_or(Value::Null);

    let report = json!({
        "schema_version": 1,
        "suite": suite_name,
        "stamp": stamp_name,
        "git_sha": git_sha,
        "generated_at_utc": iso_utc_now(),
        "input_dir": args.input.to_string_lossy(),
        "config_path": config_path_value,
        "config_hash": config_hash_value,
        "workloads": workloads,
        "source_paths": source_paths,
        "records": normalized_records,
        "comparisons": comparisons,
        "manifest": manifest_value,
    });

    let stamp_report_path = args.input.join("versioned-results.json");
    write_json(&stamp_report_path, &report)?;

    let version_report_path = version_dir.join(format!("{}.json", stamp_name));
    write_json(&version_report_path, &report)?;
    let index_path = refresh_index(&version_dir, &git_sha)?;

    println!();
    println!("versioned_json: {}", version_report_path.display());
    println!("stamp_json: {}", stamp_report_path.display());
    println!("version_index: {}", index_path.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                let _ = writeln!(io::stderr(), "error: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_record(
        run: &str,
        engine: &str,
        workload: &str,
        durability: &str,
        threads: i64,
        qps: f64,
        p99: f64,
    ) -> Value {
        json!({
            "run_id": format!("{engine}-{workload}-{threads}-{run}"),
            "engine": engine,
            "workload": workload,
            "durability": durability,
            "threads": threads,
            "metrics": {
                "throughput_ops_per_sec": qps,
                "failures": 0,
                "busy_errors": 0,
                "locked_errors": 0,
                "timeout_errors": 0,
                "elapsed_ms": 1000,
                "latency": { "p99_us": p99 }
            },
            "engine_stats": {
                "test_code_path": "crates/bench/src/chaos.rs"
            }
        })
    }

    fn write_record(dir: &Path, name: &str, record: &Value) {
        let run_dir = dir.join("raw").join(name);
        fs::create_dir_all(&run_dir).unwrap();
        let mut f = fs::File::create(run_dir.join("record.json")).unwrap();
        f.write_all(serde_json::to_string(record).unwrap().as_bytes())
            .unwrap();
    }

    #[test]
    fn median_matches_python_semantics() {
        assert!(median(vec![]).is_nan());
        assert_eq!(median(vec![1.0]), 1.0);
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        // Even length averages the two middle values, like statistics.median.
        assert_eq!(median(vec![1.0, 2.0, 3.0, 4.0]), 2.5);
    }

    #[test]
    fn iso_utc_format_matches_python_isoformat() {
        // 2024-01-02T03:04:05+00:00, no fractional part.
        let secs: i64 = 1704164645;
        assert_eq!(iso_utc_format(secs, 0), "2024-01-02T03:04:05+00:00");
        // Same instant with microseconds.
        assert_eq!(
            iso_utc_format(secs, 123456),
            "2024-01-02T03:04:05.123456+00:00"
        );
        // Leap year boundary.
        assert_eq!(iso_utc_format(1709251200, 0), "2024-03-01T00:00:00+00:00");
    }

    #[test]
    fn infer_source_paths_routes_workloads_to_owners() {
        assert_eq!(
            infer_source_paths("chaos-lock-convoy"),
            vec!["crates/bench/src/chaos.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-range"),
            vec!["crates/sql/src/exec/index_batch.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-read"),
            vec!["crates/sql/src/exec/index_access.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-ordered-limit"),
            vec!["crates/sql/src/exec/select_top.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("commit-storm-batched"),
            vec!["crates/bench/src/workload.rs".to_string()]
        );
        // Unknown workload falls through to the workload.rs catch-all.
        assert_eq!(
            infer_source_paths("nonsense-workload"),
            vec!["crates/bench/src/workload.rs".to_string()]
        );
    }

    #[test]
    fn round_trip_tiny_record_set_emits_expected_report_shape() {
        let dir = TempDir::new().unwrap();
        let stamp = dir.path();
        // Three runs of the same key plus a -w0 warmup that must be dropped.
        write_record(
            stamp,
            "Redline-chaos-r0",
            &make_record(
                "0",
                "redline",
                "chaos-lock-convoy",
                "normal",
                4,
                10.0,
                100.0,
            ),
        );
        write_record(
            stamp,
            "Redline-chaos-r1",
            &make_record(
                "1",
                "redline",
                "chaos-lock-convoy",
                "normal",
                4,
                20.0,
                200.0,
            ),
        );
        write_record(
            stamp,
            "Sqlite-chaos-r0",
            &make_record("0", "sqlite", "chaos-lock-convoy", "normal", 4, 5.0, 80.0),
        );
        write_record(
            stamp,
            "Redline-chaos-w0",
            &make_record(
                "w0",
                "redline",
                "chaos-lock-convoy",
                "normal",
                4,
                999.0,
                9999.0,
            ),
        );

        let records = load_records(stamp).unwrap();
        assert_eq!(records.len(), 4);

        let manifest = load_manifest(stamp).unwrap();
        assert!(manifest.is_none());

        let groups = build_groups(&records);
        // The warmup row drops out; we keep two engines × one key.
        assert_eq!(groups.len(), 2);

        let comparisons = build_comparisons(&records);
        assert_eq!(comparisons.len(), 1);
        let cmp = &comparisons[0];
        assert_eq!(cmp["workload"], "chaos-lock-convoy");
        assert_eq!(cmp["durability"], "normal");
        assert_eq!(cmp["threads"], 4);
        // redline median qps over [10, 20] = 15.0; ratio vs sqlite 5.0 = 3.0.
        assert_eq!(cmp["redline"]["median_qps"].as_f64().unwrap(), 15.0);
        assert_eq!(cmp["sqlite"]["median_qps"].as_f64().unwrap(), 5.0);
        assert_eq!(cmp["ratio_redline_vs_sqlite"].as_f64().unwrap(), 3.0);
        assert_eq!(cmp["test_code_paths"], json!(["crates/bench/src/chaos.rs"]));

        // Now write the full report and assert the on-disk JSON has the
        // expected top-level keys, types, and sort-order.
        let version_dir = dir.path().join("versioned").join("deadbeef");
        fs::create_dir_all(&version_dir).unwrap();
        let normalized: Vec<Value> = records
            .iter()
            .map(|r| normalize_record(r, manifest.as_ref()))
            .collect();
        let mut workloads: Vec<String> = records
            .iter()
            .map(|r| r["workload"].as_str().unwrap().to_string())
            .collect();
        workloads.sort();
        workloads.dedup();
        let mut source_paths: Vec<String> = normalized
            .iter()
            .flat_map(|r| {
                r["test_code_paths"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        source_paths.sort();
        source_paths.dedup();
        let report = json!({
            "schema_version": 1,
            "suite": "dick-head-choas",
            "stamp": "tiny",
            "git_sha": "deadbeef",
            "generated_at_utc": "2024-01-02T03:04:05+00:00",
            "input_dir": stamp.to_string_lossy(),
            "config_path": Value::Null,
            "config_hash": Value::Null,
            "workloads": workloads,
            "source_paths": source_paths,
            "records": normalized,
            "comparisons": comparisons,
            "manifest": Value::Null,
        });
        let out_path = version_dir.join("tiny.json");
        write_json(&out_path, &report).unwrap();

        // Read it back and verify the keys are present, sorted, and the
        // file ends with a newline.
        let raw = fs::read_to_string(&out_path).unwrap();
        assert!(raw.ends_with("\n"), "report file missing trailing newline");
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        let top: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        let mut expected = vec![
            "comparisons",
            "config_hash",
            "config_path",
            "generated_at_utc",
            "git_sha",
            "input_dir",
            "manifest",
            "records",
            "schema_version",
            "source_paths",
            "stamp",
            "suite",
            "workloads",
        ];
        expected.sort();
        let got: Vec<String> = top.into_iter().cloned().collect();
        assert_eq!(got, expected);

        // The normalized record carries raw_path / run_dir / test_code_paths.
        let rec0 = &parsed["records"][0];
        assert!(rec0.get("raw_path").is_some());
        assert!(rec0.get("run_dir").is_some());
        assert_eq!(
            rec0["test_code_paths"],
            json!(["crates/bench/src/chaos.rs"])
        );

        // refresh_index produces an index.json with the new report listed.
        let index_path = refresh_index(&version_dir, "deadbeef").unwrap();
        let index: Value = serde_json::from_str(&fs::read_to_string(&index_path).unwrap()).unwrap();
        assert_eq!(index["git_sha"], "deadbeef");
        let reports = index["reports"].as_array().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["file"], "tiny.json");
        assert_eq!(reports[0]["stamp"], "tiny");
        assert_eq!(reports[0]["suite"], "dick-head-choas");
    }
}
