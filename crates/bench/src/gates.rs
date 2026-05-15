use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::{DurabilityKind, EngineKind, WorkloadKind};
use crate::failpoint_matrix::FailpointMatrixReport;
use crate::report::RunRecord;

#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateSummary {
    pub gates: Vec<GateResult>,
}

pub fn evaluate_records(records: &[RunRecord]) -> GateSummary {
    GateSummary {
        gates: vec![
            gate_nonzero(records),
            gate_checksums_match(records),
            gate_single_thread_parity(records),
            gate_writer_advantage(records),
        ],
    }
}

pub fn markdown_summary(records: &[RunRecord]) -> String {
    let summary = evaluate_records(records);
    // Lane BH P1 #7: keep parity with summary.csv — surface the
    // full latency block (p50/p95/p99/p999/max) instead of just
    // p99/p999 so the report and CSV agree.
    let mut out = String::from(
        "| workload | engine | durability | threads | ops/s | p50 us | p95 us | p99 us | p999 us | max us | busy | locked | timeout | data bytes | wal bytes |\n",
    );
    out.push_str(
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    for record in records {
        out.push_str(&format!(
            "| {} | {:?} | {} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            record.workload.as_str(),
            record.engine,
            record.durability.as_str(),
            record.threads,
            record.metrics.throughput_ops_per_sec,
            record.metrics.latency.p50_us,
            record.metrics.latency.p95_us,
            record.metrics.latency.p99_us,
            record.metrics.latency.p999_us,
            record.metrics.latency.max_us,
            record.metrics.busy_errors,
            record.metrics.locked_errors,
            record.metrics.timeout_errors,
            record.data_bytes,
            record.wal_bytes
        ));
    }
    out.push_str("\n## Gates\n");
    for gate in summary.gates {
        out.push_str(&format!(
            "- {}: {} ({})\n",
            gate.name,
            if gate.passed { "PASS" } else { "FAIL" },
            gate.detail
        ));
    }
    out
}

fn gate_nonzero(records: &[RunRecord]) -> GateResult {
    let passed = records.iter().all(|record| record.metrics.operations > 0);
    GateResult {
        name: "harness_sanity".to_owned(),
        passed,
        detail: "all runs produced at least one successful operation".to_owned(),
    }
}

fn gate_checksums_match(records: &[RunRecord]) -> GateResult {
    let mut seen =
        BTreeMap::<(WorkloadKind, DurabilityKind, usize), crate::report::Checksum>::new();
    let mut passed = true;
    for record in records {
        let key = (record.workload, record.durability, record.threads);
        if let Some(existing) = seen.get(&key) {
            if existing != &record.checksum {
                passed = false;
                break;
            }
        } else {
            seen.insert(key, record.checksum.clone());
        }
    }
    GateResult {
        name: "checksum_match".to_owned(),
        passed,
        detail: "matching engines agree on final workload checksum".to_owned(),
    }
}

fn gate_single_thread_parity(records: &[RunRecord]) -> GateResult {
    let outcome = compare_ratio(records, 1, 0.90, WorkloadKind::PointReadPk);
    GateResult {
        name: "single_thread_parity".to_owned(),
        passed: outcome.unwrap_or(true),
        detail: match outcome {
            Some(_) => "redline point-read throughput stays within 90% of sqlite".to_owned(),
            None => "skipped: point-read sqlite/redline comparison rows absent".to_owned(),
        },
    }
}

fn gate_writer_advantage(records: &[RunRecord]) -> GateResult {
    let outcome = compare_ratio(records, 8, 1.50, WorkloadKind::WritersDisjoint);
    GateResult {
        name: "concurrent_writer_advantage".to_owned(),
        passed: outcome.unwrap_or(true),
        detail: match outcome {
            Some(_) => {
                "redline writers-disjoint throughput exceeds sqlite by 1.5x at 8 threads".to_owned()
            }
            None => "skipped: writers-disjoint sqlite/redline comparison rows absent".to_owned(),
        },
    }
}

/// Lane E gate: every redline-strict failpoint matrix case must report
/// `lost_acked_commits == 0`. Returned as a [`GateResult`] so it
/// composes with the rest of the bench harness's gate pipeline.
pub fn gate_zero_lost_acked_commits(report: &FailpointMatrixReport) -> GateResult {
    let mut offenders: Vec<String> = Vec::new();
    for run in &report.runs {
        if run.engine == EngineKind::Redline
            && run.durability == DurabilityKind::Strict
            && run.lost_acked_commits > 0
        {
            offenders.push(format!(
                "{} (failpoint={}, kill_after_n_hits={}, acked={}, recovered={}, lost={})",
                run.case,
                run.failpoint,
                run.kill_after_n_hits,
                run.acknowledged,
                run.recovered,
                run.lost_acked_commits
            ));
        }
    }
    let passed = offenders.is_empty();
    let detail = if passed {
        format!(
            "all {} redline-strict failpoint cases reported zero lost acked commits",
            report
                .runs
                .iter()
                .filter(|run| run.engine == EngineKind::Redline
                    && run.durability == DurabilityKind::Strict)
                .count()
        )
    } else {
        format!(
            "{} redline-strict cases lost acked commits: [{}]",
            offenders.len(),
            offenders.join("; ")
        )
    };
    GateResult {
        name: "failpoint_zero_lost_acked_commits".to_owned(),
        passed,
        detail,
    }
}

/// Phase 11 wave 1a OLTP gap gate.
///
/// Evaluates the wave-1a perf-gap floors that Wave 1b is expected to
/// drive past. These are *abort floors*, not targets — the wave goal
/// in each row is documented inline as a comment, but the gate fails
/// only when the redline/sqlite ratio drops below the listed floor.
///
/// The gate is intentionally additive: it is **not** wired into the
/// default `evaluate_records` pipeline so existing phase-9 lanes keep
/// the same pass/fail semantics. Callers that want phase-11 floors
/// invoke this directly via [`evaluate_phase11_oltp_gap`].
pub fn phase11_oltp_gap_gate(records: &[RunRecord]) -> GateSummary {
    // (workload, threads, floor, wave-target). The floor is what the
    // gate enforces today; the target is what wave 1b should hit.
    let floors = [
        // Wave 1b target 0.50; abort floor 0.30.
        (WorkloadKind::SecondaryIndexRange, 8usize, 0.30f64),
        // Covered secondary read; wave target 0.70.
        (WorkloadKind::SecondaryIndexRead, 8, 0.40),
        // Wave target 0.85.
        (WorkloadKind::HotRowUpdate, 8, 0.60),
        // W1-E count-only leaf walk; no heap recheck should be needed.
        (WorkloadKind::SecondaryIndexCount, 8, 0.30),
        // W1-D ordered range with LIMIT early-stop.
        (WorkloadKind::SecondaryIndexOrderedLimit, 8, 0.30),
        // W1-E covering reads; canonical cold/warm comparison is t1.
        (WorkloadKind::CoveredRangeCold, 1, 0.40),
        (WorkloadKind::CoveredRangeWarm, 1, 0.40),
        // W1 write baseline for future hot-counter combiner work.
        (WorkloadKind::HotCounterUpdate, 1, 0.60),
        // No-regression floor; sqlite owns this fixture today.
        (WorkloadKind::PointReadPk, 1, 0.85),
        // Concurrent writers — Redline's flagship win.
        (WorkloadKind::WritersDisjoint, 8, 1.30),
    ];
    let mut gates = Vec::with_capacity(floors.len());
    for (workload, threads, floor) in floors {
        let outcome = median_ratio(records, threads, workload);
        let passed = match &outcome {
            Some(ratio) => ratio.ratio >= floor,
            None => true,
        };
        gates.push(GateResult {
            name: format!(
                "phase11_oltp_gap::{}::t{}",
                workload.as_str().replace('-', "_"),
                threads
            ),
            passed,
            detail: match outcome {
                Some(ratio) => format!(
                    "{} ratio {:.6} at {} threads {} floor {:.2} (redline_median_qps={:.6}, sqlite_median_qps={:.6}, redline_samples={}, sqlite_samples={})",
                    workload.as_str(),
                    ratio.ratio,
                    threads,
                    if passed { ">=" } else { "below" },
                    floor,
                    ratio.redline_median_qps,
                    ratio.sqlite_median_qps,
                    ratio.redline_samples,
                    ratio.sqlite_samples
                ),
                None => format!(
                    "skipped: {} sqlite/redline rows at {} threads absent",
                    workload.as_str(),
                    threads
                ),
            },
        });
    }
    GateSummary { gates }
}

/// Wave 1a entry point for the phase 11 OLTP gap evaluation. Kept as
/// a thin wrapper so future wave-1b/c/d/e gates can add their own
/// `evaluate_phase11_*` siblings without touching the wave-1a
/// definition.
pub fn evaluate_phase11_oltp_gap(records: &[RunRecord]) -> GateSummary {
    phase11_oltp_gap_gate(records)
}

#[derive(Debug, Clone, Copy)]
struct MedianRatio {
    redline_median_qps: f64,
    sqlite_median_qps: f64,
    ratio: f64,
    redline_samples: usize,
    sqlite_samples: usize,
}

fn median_ratio(
    records: &[RunRecord],
    threads: usize,
    workload: WorkloadKind,
) -> Option<MedianRatio> {
    let redline_values = throughput_values(records, EngineKind::Redline, threads, workload);
    let sqlite_values = throughput_values(records, EngineKind::Sqlite, threads, workload);
    let redline_samples = redline_values.len();
    let sqlite_samples = sqlite_values.len();
    let redline_median_qps = median_f64(redline_values)?;
    let sqlite_median_qps = median_f64(sqlite_values)?;
    if sqlite_median_qps <= 0.0 {
        return Some(MedianRatio {
            redline_median_qps,
            sqlite_median_qps,
            ratio: 0.0,
            redline_samples,
            sqlite_samples,
        });
    }
    Some(MedianRatio {
        redline_median_qps,
        sqlite_median_qps,
        ratio: redline_median_qps / sqlite_median_qps,
        redline_samples,
        sqlite_samples,
    })
}

fn throughput_values(
    records: &[RunRecord],
    engine: EngineKind,
    threads: usize,
    workload: WorkloadKind,
) -> Vec<f64> {
    records
        .iter()
        .filter(|record| {
            record.engine == engine
                && record.threads == threads
                && record.durability == DurabilityKind::Strict
                && record.workload == workload
        })
        .map(|record| record.metrics.throughput_ops_per_sec)
        .filter(|value| value.is_finite())
        .collect()
}

fn median_f64(mut values: Vec<f64>) -> Option<f64> {
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

fn compare_ratio(
    records: &[RunRecord],
    threads: usize,
    min_ratio: f64,
    workload: WorkloadKind,
) -> Option<bool> {
    let sqlite = records.iter().find(|record| {
        record.engine == EngineKind::Sqlite
            && record.threads == threads
            && record.durability == DurabilityKind::Strict
            && record.workload == workload
    });
    let redline = records.iter().find(|record| {
        record.engine == EngineKind::Redline
            && record.threads == threads
            && record.durability == DurabilityKind::Strict
            && record.workload == workload
    });
    match (sqlite, redline) {
        (Some(sqlite), Some(redline)) => {
            if sqlite.metrics.throughput_ops_per_sec == 0.0 {
                Some(false)
            } else {
                Some(
                    (redline.metrics.throughput_ops_per_sec
                        / sqlite.metrics.throughput_ops_per_sec)
                        >= min_ratio,
                )
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{LatencySummary, MetricsSummary, RunRecord};

    fn record(engine: EngineKind, throughput: f64) -> RunRecord {
        record_for(engine, WorkloadKind::PointReadPk, 1, throughput)
    }

    fn record_for(
        engine: EngineKind,
        workload: WorkloadKind,
        threads: usize,
        throughput: f64,
    ) -> RunRecord {
        RunRecord {
            run_id: "run".to_owned(),
            engine,
            workload,
            durability: DurabilityKind::Strict,
            threads,
            seed: 7,
            cache_bytes: 1024,
            environment: crate::report::RunEnvironment {
                hostname: "test".to_owned(),
                git_sha: None,
                git_dirty: None,
                rustc_version: None,
                sqlite_version: None,
                logical_cpus: 1,
                memory_mib: None,
                image_digest: None,
            },
            metrics: MetricsSummary {
                operations: 10,
                failures: 0,
                busy_errors: 0,
                locked_errors: 0,
                timeout_errors: 0,
                elapsed_ms: 10,
                throughput_ops_per_sec: throughput,
                latency: LatencySummary {
                    p50_us: 1,
                    p95_us: 1,
                    p99_us: 1,
                    p999_us: 1,
                    max_us: 1,
                },
            },
            checksum: crate::report::Checksum::default(),
            data_bytes: 1,
            wal_bytes: 1,
            engine_stats: serde_json::json!({}),
            process_metrics: None,
        }
    }

    #[test]
    fn parity_gate_passes_when_ratio_is_met() {
        let records = vec![
            record(EngineKind::Sqlite, 100.0),
            record(EngineKind::Redline, 95.0),
        ];
        assert!(gate_single_thread_parity(&records).passed);
    }

    #[test]
    fn phase11_gate_uses_median_ratio_not_first_repetition() {
        let workload = WorkloadKind::CoveredRangeCold;
        let records = vec![
            record_for(EngineKind::Sqlite, workload, 1, 100.0),
            record_for(EngineKind::Redline, workload, 1, 1.0),
            record_for(EngineKind::Sqlite, workload, 1, 100.0),
            record_for(EngineKind::Redline, workload, 1, 41.0),
            record_for(EngineKind::Sqlite, workload, 1, 100.0),
            record_for(EngineKind::Redline, workload, 1, 50.0),
        ];

        let summary = evaluate_phase11_oltp_gap(&records);
        let gate = summary
            .gates
            .iter()
            .find(|gate| gate.name == "phase11_oltp_gap::covered_range_cold::t1")
            .expect("covered range cold gate");

        assert!(gate.passed, "{}", gate.detail);
        assert!(gate.detail.contains("ratio 0.410000"), "{}", gate.detail);
        assert!(gate.detail.contains("redline_median_qps=41.000000"));
        assert!(gate.detail.contains("sqlite_median_qps=100.000000"));
        assert!(gate.detail.contains("redline_samples=3"));
        assert!(gate.detail.contains("sqlite_samples=3"));
    }
}
