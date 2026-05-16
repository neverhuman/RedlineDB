//! Summarize chaos benchmark raw records and persist versioned JSON.
//!
//! Rust port of the previous `scripts/bench/dick_head_choas_report.py` tool.
//! The output JSON shape matches the Python version byte-for-byte except for
//! the live `generated_at_utc` timestamps, which are emitted in the same
//! ISO-8601 UTC form (`YYYY-MM-DDTHH:MM:SS.mmmuuu+00:00`) the original used.

mod args;
mod compare;
mod normalize;
mod read;
mod write;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write as _};
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::args::{parse_args, repo_root};
use crate::compare::{build_comparisons, build_groups, extract_int, median};
use crate::normalize::normalize_record;
use crate::read::{load_manifest, load_records, repo_git_sha};
use crate::write::{iso_utc_now, refresh_index, write_json};

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

    let suite_name = match manifest
        .as_ref()
        .and_then(|m| m.get("suite"))
        .and_then(Value::as_str)
    {
        Some(s) => s.to_string(),
        None => args.suite.clone(),
    };

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
    use std::path::Path;
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
