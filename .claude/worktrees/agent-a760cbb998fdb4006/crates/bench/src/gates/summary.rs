use serde::Serialize;

use crate::report::RunRecord;

use super::phase9::{
    gate_checksums_match, gate_nonzero, gate_single_thread_parity, gate_writer_advantage,
};

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
