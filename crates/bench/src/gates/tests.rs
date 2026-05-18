use super::{evaluate_phase11_oltp_gap, phase9::gate_single_thread_parity};
use crate::config::{DurabilityKind, EngineKind, WorkloadKind};
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
