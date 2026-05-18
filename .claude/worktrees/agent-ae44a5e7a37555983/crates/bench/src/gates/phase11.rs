use crate::config::{EngineKind, WorkloadKind};
use crate::report::RunRecord;

use super::summary::{GateResult, GateSummary};

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
                && record.durability == crate::config::DurabilityKind::Strict
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
