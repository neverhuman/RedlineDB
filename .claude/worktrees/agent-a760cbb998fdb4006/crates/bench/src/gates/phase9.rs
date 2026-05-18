use std::collections::BTreeMap;

use crate::config::{DurabilityKind, EngineKind, WorkloadKind};
use crate::failpoint_matrix::FailpointMatrixReport;
use crate::report::RunRecord;

use super::summary::GateResult;

pub(super) fn gate_nonzero(records: &[RunRecord]) -> GateResult {
    let passed = records.iter().all(|record| record.metrics.operations > 0);
    GateResult {
        name: "harness_sanity".to_owned(),
        passed,
        detail: "all runs produced at least one successful operation".to_owned(),
    }
}

pub(super) fn gate_checksums_match(records: &[RunRecord]) -> GateResult {
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

pub(super) fn gate_single_thread_parity(records: &[RunRecord]) -> GateResult {
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

pub(super) fn gate_writer_advantage(records: &[RunRecord]) -> GateResult {
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
