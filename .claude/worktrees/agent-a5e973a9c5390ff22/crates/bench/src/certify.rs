use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::{CertifyArgs, CompareConfig, EngineKind, RunSpec};
use crate::engine::SqliteEngine;
use crate::process_metrics::ProcessMetrics;
use crate::report::{self, RunRecord};
use crate::strace_capture;

#[path = "certify/scheduler.rs"]
mod scheduler;

pub use scheduler::{
    Job, MAX_PARALLEL_THREADS_ENV, RESERVED_CORES, ScheduledOutcome, SchedulerStats,
    available_cores, build_job_queue, dispatch_parallel, dispatch_parallel_with_spawner,
};

#[derive(Debug, Serialize)]
pub struct CertificationReport {
    pub runs: Vec<RunRecord>,
    pub manifest: CertificationManifest,
}

#[derive(Debug, Serialize)]
pub struct CertificationManifest {
    pub out_dir: PathBuf,
    pub config_path: PathBuf,
    pub config_hash: String,
    pub runs_jsonl_hash: String,
    pub summary_csv_hash: String,
    pub ratio_csv_hash: String,
    pub report_md_hash: String,
    pub git_sha: Option<String>,
    pub git_dirty: Option<bool>,
    /// Echo the resolved `--with-strace` decision so manifest consumers
    /// can tell at a glance whether the child wrap was attempted.
    pub with_strace: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pragmas: Option<BTreeMap<String, BTreeMap<String, String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pragma_validation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksums: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strace_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strace_syscall_counts: Option<BTreeMap<String, u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_metrics_per_run: Option<Vec<ProcessMetrics>>,
    /// Lane BH P0 #1: number of warmup rounds executed per
    /// `(engine, workload, durability, threads)` combo before the
    /// measured rounds began. Warmup records are discarded but the
    /// count is preserved so reviewers can confirm the harness
    /// honored `--warmup`.
    pub warmup_runs_per_combo: usize,
    /// Lane BH P0 #1: number of measured rounds executed per
    /// combo. Equals `--repetitions` (clamped to at least 1) and
    /// matches the count of `RunRecord` entries written for that
    /// combo.
    pub measured_runs_per_combo: usize,
}

pub fn run(config: &CompareConfig, args: &CertifyArgs) -> Result<CertificationReport> {
    fs::create_dir_all(&args.out_dir)?;
    let raw_dir = args.out_dir.join("raw");
    fs::create_dir_all(&raw_dir)?;

    // Capture per-engine PRAGMA / setting snapshots before any workload
    // mutates state. SQLite has structured PRAGMAs we can read; redline
    // exposes its settings via engine_stats embedded in run records.
    let pragmas = collect_pragmas(config, args)?;

    let with_strace = args.strace_enabled();

    // Lane BH P0 #1: build the full job queue *before* dispatching so the
    // parallel scheduler can bin-pack mixed-thread jobs onto the
    // available cores. Warmup jobs (records discarded) come first per
    // combo, then the measured rounds. Scheduling is FIFO with a
    // greedy-fit twist — see `dispatch_parallel`.
    let warmup = args.warmup;
    let measured = args.repetitions.max(1);
    let jobs = build_job_queue(config, args, warmup, measured)?;

    let outcomes = dispatch_parallel(jobs, &raw_dir, with_strace, available_cores())?;

    let mut runs = Vec::new();
    let mut strace_child_paths: Vec<PathBuf> = Vec::new();
    for outcome in outcomes {
        // Warmup outcomes are discarded — they exist only to prime
        // caches/disks and the scheduler so the measured rounds
        // start from a steady state.
        if outcome.is_warmup {
            continue;
        }
        runs.push(outcome.record);
        if let Some(path) = outcome.strace_path {
            strace_child_paths.push(path);
        }
    }

    if runs.is_empty() {
        bail!("certify config produced no benchmark runs");
    }

    let runs_jsonl = args.out_dir.join("runs.jsonl");
    write_runs_jsonl(&runs_jsonl, &runs)?;
    let summary_csv = args.out_dir.join("summary.csv");
    write_summary_csv(&summary_csv, &runs)?;
    let ratio_csv = args.out_dir.join("ratio.csv");
    write_ratio_csv(&ratio_csv, &runs)?;
    let report_md = args.out_dir.join("report.md");
    fs::write(&report_md, crate::gates::markdown_summary(&runs))?;

    // Phase 11 wave 1a: when the config exercises any of the new
    // phase-11 workloads, also evaluate the phase-11 OLTP gap gate
    // and stash the result alongside the manifest. The gate is
    // additive — it never replaces or alters the default
    // `evaluate_records` pipeline, so phase-9 lanes stay untouched.
    if is_phase11_oltp_gap_config(args) {
        let phase11_gates = crate::gates::evaluate_phase11_oltp_gap(&runs);
        report::write_json(
            Some(&args.out_dir.join("phase11_oltp_gap_gates.json")),
            &phase11_gates,
        )?;
    }

    let process_metrics_per_run: Vec<ProcessMetrics> = runs
        .iter()
        .filter_map(|run| run.process_metrics.clone())
        .collect();
    let process_metrics_per_run = if process_metrics_per_run.is_empty() {
        None
    } else {
        Some(process_metrics_per_run)
    };

    let checksums = checksum_map(&runs)?;
    let strace = strace_summary(with_strace, &args.out_dir, &strace_child_paths)?;

    let manifest = CertificationManifest {
        out_dir: args.out_dir.clone(),
        config_path: args.config.clone(),
        config_hash: hash_file(&args.config)?,
        runs_jsonl_hash: hash_file(&runs_jsonl)?,
        summary_csv_hash: hash_file(&summary_csv)?,
        ratio_csv_hash: hash_file(&ratio_csv)?,
        report_md_hash: hash_file(&report_md)?,
        git_sha: report::collect_environment().git_sha,
        git_dirty: report::collect_environment().git_dirty,
        with_strace,
        pragma_validation: pragma_validation(&pragmas),
        pragmas: if pragmas.is_empty() {
            None
        } else {
            Some(pragmas)
        },
        checksums: Some(checksums),
        strace_reason: strace.reason,
        strace_syscall_counts: strace.syscall_counts,
        process_metrics_per_run,
        warmup_runs_per_combo: warmup,
        measured_runs_per_combo: measured,
    };
    let manifest_path = args.out_dir.join("manifest.json");
    report::write_json(Some(&manifest_path), &manifest)?;

    let report = CertificationReport { runs, manifest };
    report::write_json(Some(&args.out_dir.join("report.json")), &report)?;

    if is_phase11_oltp_gap_config(args) {
        let phase11_gates = crate::gates::evaluate_phase11_oltp_gap(&report.runs);
        ensure_phase11_gates_pass("phase11-oltp-gap", &phase11_gates)?;
    }

    Ok(report)
}

fn collect_pragmas(
    config: &CompareConfig,
    args: &CertifyArgs,
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut out = BTreeMap::new();
    let probe_dir = args.out_dir.join("probe");
    for &engine in &config.engines {
        match engine {
            EngineKind::Sqlite => {
                let workload = config
                    .workloads
                    .first()
                    .copied()
                    .unwrap_or(crate::config::WorkloadKind::SingleRowInsert);
                let durability = config
                    .durabilities
                    .first()
                    .copied()
                    .unwrap_or(crate::config::DurabilityKind::Strict);
                let threads = config.threads.first().copied().unwrap_or(1);
                let spec: RunSpec =
                    config.run_spec(&engine, &workload, &durability, threads, args.seed)?;
                let probe = probe_dir.join("sqlite-probe");
                fs::create_dir_all(&probe)?;
                let sqlite = SqliteEngine::open(&spec, &probe)?;
                out.insert("sqlite".to_owned(), sqlite.pragmas());
            }
            EngineKind::Redline => {
                // Redline does not expose SQL-level PRAGMAs; surface a
                // marker descriptor so consumers know the probe ran and
                // chose the engine-stats path instead.
                let mut redline_pragmas = BTreeMap::new();
                redline_pragmas.insert("kind".to_owned(), "redline-engine-stats-only".to_owned());
                out.insert("redline".to_owned(), redline_pragmas);
            }
        }
    }
    let _ = fs::remove_dir_all(&probe_dir);
    Ok(out)
}

fn pragma_validation(pragmas: &BTreeMap<String, BTreeMap<String, String>>) -> Option<String> {
    let sqlite = pragmas.get("sqlite")?;
    let journal = sqlite.get("journal_mode").map(String::as_str).unwrap_or("");
    if journal.eq_ignore_ascii_case("wal") {
        Some("ok".to_owned())
    } else {
        Some(format!("journal_mode={journal} (expected wal)"))
    }
}

fn checksum_map(runs: &[RunRecord]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for record in runs {
        let key = format!(
            "{:?}:{}:{}:t{}",
            record.engine,
            record.workload.as_str(),
            record.durability.as_str(),
            record.threads
        );
        let payload = serde_json::to_vec(&record.checksum).with_context(|| {
            format!(
                "serialise checksum for {:?}/{}/{}/t{}",
                record.engine,
                record.workload.as_str(),
                record.durability.as_str(),
                record.threads
            )
        })?;
        let digest = Sha256::digest(&payload);
        out.insert(key, format!("{digest:x}"));
    }
    Ok(out)
}

/// Aggregate the per-child strace summaries collected during the run.
///
/// Earlier versions of this harness attached `strace` to the parent
/// process *after* every child had exited. On Linux that path can hang
/// because `strace -p $$` is asking strace to attach to the same
/// process that is then waiting for strace to detach. The new contract
/// is: each spawned `redlinedb-bench run` child is wrapped with
/// `strace -c -o <path>` directly — see `run_child` — and we sum the
/// per-child summaries here.
fn strace_summary(
    with_strace: bool,
    out_dir: &Path,
    child_paths: &[PathBuf],
) -> Result<crate::strace_capture::StraceCapture> {
    if !with_strace {
        return Ok(crate::strace_capture::StraceCapture {
            syscall_counts: None,
            reason: Some(if cfg!(target_os = "linux") {
                "disabled (pass --with-strace or set REDLINEDB_BENCH_WITH_STRACE=1 to enable)"
                    .to_owned()
            } else {
                "linux required".to_owned()
            }),
            output_path: None,
        });
    }
    if !cfg!(target_os = "linux") {
        return Ok(crate::strace_capture::StraceCapture {
            syscall_counts: None,
            reason: Some("linux required".to_owned()),
            output_path: None,
        });
    }
    if child_paths.is_empty() {
        return Ok(crate::strace_capture::StraceCapture {
            syscall_counts: None,
            reason: Some("no strace output files were produced by children".to_owned()),
            output_path: None,
        });
    }

    // Sum the per-child summaries into a single map.
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut empties = 0usize;
    for path in child_paths {
        match fs::read_to_string(path) {
            Ok(raw) => {
                let parsed = strace_capture::parse_strace_summary(&raw);
                if parsed.is_empty() {
                    empties += 1;
                    continue;
                }
                for (name, calls) in parsed {
                    *totals.entry(name).or_insert(0) += calls;
                }
            }
            Err(_) => empties += 1,
        }
    }

    let aggregate_path = out_dir.join("strace.txt");
    if !totals.is_empty() {
        let mut text = String::from(
            "% time     seconds  usecs/call     calls    errors syscall\n\
             ------ ----------- ----------- --------- --------- ----------------\n",
        );
        for (name, calls) in &totals {
            text.push_str(&format!(
                "   0.00    0.000000           0 {calls:>9}         0 {name}\n"
            ));
        }
        let _ = fs::write(&aggregate_path, text);
    }

    Ok(crate::strace_capture::StraceCapture {
        syscall_counts: if totals.is_empty() {
            None
        } else {
            Some(totals)
        },
        reason: if empties > 0 {
            Some(format!(
                "{empties} of {} child strace files were empty or unreadable",
                child_paths.len()
            ))
        } else {
            None
        },
        output_path: if aggregate_path.exists() {
            Some(aggregate_path)
        } else {
            None
        },
    })
}

fn write_runs_jsonl(path: &Path, runs: &[RunRecord]) -> Result<()> {
    let mut out = fs::File::create(path)?;
    for run in runs {
        writeln!(out, "{}", serde_json::to_string(run)?)?;
    }
    Ok(())
}

/// Lane BH: test-only re-export of [`write_summary_csv`]. The
/// internal writer is module-private; tests in
/// `crates/bench/tests/` use this thin wrapper to keep the
/// production surface unchanged.
pub fn write_summary_csv_for_test(path: &Path, runs: &[RunRecord]) -> Result<()> {
    write_summary_csv(path, runs)
}

/// Test-only re-export of [`write_ratio_csv`].
pub fn write_ratio_csv_for_test(path: &Path, runs: &[RunRecord]) -> Result<()> {
    write_ratio_csv(path, runs)
}

fn write_summary_csv(path: &Path, runs: &[RunRecord]) -> Result<()> {
    let mut out = fs::File::create(path)?;
    // Lane BH P1 #7: pre-Lane-BH summary.csv only carried `p99_us`
    // and `p999_us`; reviewers asked for the full tail so dashboards
    // can plot p50/p95/max alongside the existing percentiles.
    // Column order: existing identity columns first, then ops/error
    // counts, throughput, the latency block in ascending percentile
    // order, then capacity counters at the tail.
    writeln!(
        out,
        "engine,workload,durability,threads,ops,failures,busy_errors,locked_errors,timeout_errors,throughput_ops_per_sec,p50_us,p95_us,p99_us,p999_us,max_us,data_bytes,wal_bytes"
    )?;
    for run in runs {
        writeln!(
            out,
            "{:?},{},{},{},{},{},{},{},{},{:.2},{},{},{},{},{},{},{}",
            run.engine,
            run.workload.as_str(),
            run.durability.as_str(),
            run.threads,
            run.metrics.operations,
            run.metrics.failures,
            run.metrics.busy_errors,
            run.metrics.locked_errors,
            run.metrics.timeout_errors,
            run.metrics.throughput_ops_per_sec,
            run.metrics.latency.p50_us,
            run.metrics.latency.p95_us,
            run.metrics.latency.p99_us,
            run.metrics.latency.p999_us,
            run.metrics.latency.max_us,
            run.data_bytes,
            run.wal_bytes,
        )?;
    }
    Ok(())
}

fn write_ratio_csv(path: &Path, runs: &[RunRecord]) -> Result<()> {
    let mut groups: BTreeMap<
        (
            crate::config::WorkloadKind,
            crate::config::DurabilityKind,
            usize,
        ),
        Vec<&RunRecord>,
    > = BTreeMap::new();
    for run in runs {
        groups
            .entry((run.workload, run.durability, run.threads))
            .or_default()
            .push(run);
    }

    let mut out = fs::File::create(path)?;
    writeln!(
        out,
        "workload,durability,threads,redline_median_qps,sqlite_median_qps,ratio,redline_p95_us,redline_p99_us,sqlite_p95_us,sqlite_p99_us,redline_failures,sqlite_failures,redline_busy_errors,sqlite_busy_errors,redline_locked_errors,sqlite_locked_errors,redline_timeout_errors,sqlite_timeout_errors,redline_fdatasync_count,sqlite_fdatasync_count,redline_pwrite_count,sqlite_pwrite_count,redline_raw_hash,sqlite_raw_hash"
    )?;

    for ((workload, durability, threads), records) in groups {
        let redline = records_for_engine(&records, EngineKind::Redline);
        let sqlite = records_for_engine(&records, EngineKind::Sqlite);
        let redline_qps = median_f64(
            redline
                .iter()
                .map(|run| run.metrics.throughput_ops_per_sec)
                .collect(),
        );
        let sqlite_qps = median_f64(
            sqlite
                .iter()
                .map(|run| run.metrics.throughput_ops_per_sec)
                .collect(),
        );
        let ratio = match (redline_qps, sqlite_qps) {
            (Some(redline), Some(sqlite)) if sqlite > 0.0 => Some(redline / sqlite),
            _ => None,
        };
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            workload.as_str(),
            durability.as_str(),
            threads,
            fmt_opt_f64(redline_qps),
            fmt_opt_f64(sqlite_qps),
            fmt_opt_f64(ratio),
            fmt_opt_u64(median_u64(
                redline
                    .iter()
                    .map(|run| run.metrics.latency.p95_us)
                    .collect()
            )),
            fmt_opt_u64(median_u64(
                redline
                    .iter()
                    .map(|run| run.metrics.latency.p99_us)
                    .collect()
            )),
            fmt_opt_u64(median_u64(
                sqlite
                    .iter()
                    .map(|run| run.metrics.latency.p95_us)
                    .collect()
            )),
            fmt_opt_u64(median_u64(
                sqlite
                    .iter()
                    .map(|run| run.metrics.latency.p99_us)
                    .collect()
            )),
            sum_u64(redline.iter().map(|run| run.metrics.failures)),
            sum_u64(sqlite.iter().map(|run| run.metrics.failures)),
            sum_u64(redline.iter().map(|run| run.metrics.busy_errors)),
            sum_u64(sqlite.iter().map(|run| run.metrics.busy_errors)),
            sum_u64(redline.iter().map(|run| run.metrics.locked_errors)),
            sum_u64(sqlite.iter().map(|run| run.metrics.locked_errors)),
            sum_u64(redline.iter().map(|run| run.metrics.timeout_errors)),
            sum_u64(sqlite.iter().map(|run| run.metrics.timeout_errors)),
            sum_process_counter(&redline, |metrics| metrics.fdatasync_count),
            sum_process_counter(&sqlite, |metrics| metrics.fdatasync_count),
            sum_process_counter(&redline, |metrics| metrics.pwrite_count),
            sum_process_counter(&sqlite, |metrics| metrics.pwrite_count),
            hash_records(&redline),
            hash_records(&sqlite),
        )?;
    }
    Ok(())
}

fn records_for_engine<'a>(records: &[&'a RunRecord], engine: EngineKind) -> Vec<&'a RunRecord> {
    records
        .iter()
        .copied()
        .filter(|run| run.engine == engine)
        .collect()
}

fn median_f64(mut values: Vec<f64>) -> Option<f64> {
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    Some(if values.len() % 2 == 1 {
        values[values.len() / 2]
    } else {
        let upper = values.len() / 2;
        (values[upper - 1] + values[upper]) / 2.0
    })
}

fn median_u64(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(if values.len() % 2 == 1 {
        values[values.len() / 2]
    } else {
        let upper = values.len() / 2;
        values[upper - 1].saturating_add(values[upper]) / 2
    })
}

/// Render an optional f64 for the CSV summary cell. An absent value renders
/// as the empty string so the cell stays empty in the column rather than
/// emitting a numeric sentinel.
fn fmt_opt_f64(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.6}"),
        None => String::new(),
    }
}

/// Same convention as [`fmt_opt_f64`] for integer-valued cells: `None` →
/// empty cell, `Some(v)` → decimal digits.
fn fmt_opt_u64(value: Option<u64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

fn sum_u64(values: impl Iterator<Item = u64>) -> u64 {
    values.fold(0_u64, u64::saturating_add)
}

fn sum_process_counter(
    records: &[&RunRecord],
    getter: impl Fn(&ProcessMetrics) -> Option<u64>,
) -> u64 {
    records
        .iter()
        .filter_map(|run| run.process_metrics.as_ref())
        .filter_map(getter)
        .fold(0_u64, u64::saturating_add)
}

fn hash_records(records: &[&RunRecord]) -> String {
    if records.is_empty() {
        return String::new();
    }
    let mut hasher = Sha256::new();
    for record in records {
        if let Ok(bytes) = serde_json::to_vec(record) {
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

/// Phase 11 wave 1a gates are bound to the dedicated certification
/// config, not to individual workload names. Smoke and phase-9 lanes
/// intentionally share workloads such as point-read-pk and
/// writers-disjoint but must keep their older gate semantics.
fn is_phase11_oltp_gap_config(args: &CertifyArgs) -> bool {
    args.config
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "phase11-oltp-gap.toml")
}

fn ensure_phase11_gates_pass(label: &str, gates: &crate::gates::GateSummary) -> Result<()> {
    let failures: Vec<&crate::gates::GateResult> =
        gates.gates.iter().filter(|gate| !gate.passed).collect();
    if failures.is_empty() {
        return Ok(());
    }
    let detail = failures
        .iter()
        .map(|gate| format!("{}: {}", gate.name, gate.detail))
        .collect::<Vec<_>>()
        .join("; ");
    bail!("{label} certification failed: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase11_gate_failure_bubbles_up() {
        let gates = crate::gates::GateSummary {
            gates: vec![
                crate::gates::GateResult {
                    name: "ok".to_owned(),
                    passed: true,
                    detail: "all good".to_owned(),
                },
                crate::gates::GateResult {
                    name: "phase11_oltp_gap::covered_range_cold::t1".to_owned(),
                    passed: false,
                    detail: "covered-range ratio below floor 0.40".to_owned(),
                },
            ],
        };
        let err = ensure_phase11_gates_pass("phase11-oltp-gap", &gates).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("phase11-oltp-gap"));
        assert!(text.contains("phase11_oltp_gap::covered_range_cold::t1"));
        assert!(text.contains("below floor"));
    }

    #[test]
    fn phase11_gate_activation_is_config_scoped() {
        let phase11 = CertifyArgs {
            config: PathBuf::from("crates/bench/bench/phase11-oltp-gap.toml"),
            out_dir: PathBuf::from("target/bench/phase11-oltp-gap"),
            seed: 7,
            repetitions: 3,
            warmup: 1,
            with_strace: false,
        };
        let smoke = CertifyArgs {
            config: PathBuf::from("crates/bench/bench/smoke.toml"),
            out_dir: PathBuf::from("target/bench/certify-smoke"),
            seed: 7,
            repetitions: 1,
            warmup: 0,
            with_strace: false,
        };

        assert!(is_phase11_oltp_gap_config(&phase11));
        assert!(!is_phase11_oltp_gap_config(&smoke));
    }
}
