use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_utils::thread;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::*;

pub(super) fn run_connection_limit(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    let baseline_metrics = run_connection_burst(engine, spec, 1, Duration::from_secs(1))?;
    let baseline_p99 = baseline_metrics.latency.p99_us.max(1);

    let connection_limit_max = connection_limit_max();
    let mut low: usize = 1;
    let mut high: usize = connection_limit_max;
    let mut best_stable = 1;
    let mut last_metrics = baseline_metrics.clone();
    let mut last_n = 1;
    let stop_window = 8;

    while high.saturating_sub(low) > stop_window {
        let mid = (low + high) / 2;
        let candidate = run_connection_burst(engine, spec, mid, Duration::from_secs(1))?;
        last_metrics = candidate.clone();
        last_n = mid;
        let total = (candidate.operations + candidate.failures).max(1);
        let error_rate = candidate.failures as f64 / total as f64;
        let busy_ratio =
            (candidate.busy_errors + candidate.locked_errors + candidate.timeout_errors) as f64
                / total as f64;
        let p99_blowup = candidate.latency.p99_us as f64 / baseline_p99 as f64;
        let stable = error_rate <= 0.05 && busy_ratio <= 0.50 && p99_blowup <= 100.0;
        if stable {
            best_stable = mid;
            low = mid;
        } else {
            high = mid;
        }
    }

    let elapsed = wall_started.elapsed().max(Duration::from_millis(1));
    let snapshot = engine.snapshot()?;
    let checksum = engine.checksum()?;
    let mut process = process_metrics::collect_self();
    if snapshot.fsyncs_issued.is_some() {
        process.fsync_count = snapshot.fsyncs_issued;
    }
    if snapshot.fdatasyncs_issued.is_some() {
        process.fdatasync_count = snapshot.fdatasyncs_issued;
    }
    if snapshot.pwrites_issued.is_some() {
        process.pwrite_count = snapshot.pwrites_issued;
    }

    let mut engine_stats = match snapshot.engine_stats {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    engine_stats.insert(
        "max_stable_connections".to_owned(),
        serde_json::json!(best_stable),
    );
    engine_stats.insert(
        "baseline_p99_us".to_owned(),
        serde_json::json!(baseline_p99),
    );
    engine_stats.insert("last_probe_n".to_owned(), serde_json::json!(last_n));
    engine_stats.insert(
        "connection_limit_max".to_owned(),
        serde_json::json!(connection_limit_max),
    );
    engine_stats.insert(
        "wall_elapsed_ms".to_owned(),
        serde_json::json!(elapsed.as_millis() as u64),
    );

    Ok(RunRecord {
        run_id: crate::report::next_run_id(spec.engine, spec.workload),
        engine: spec.engine,
        workload: spec.workload,
        durability: spec.durability,
        threads: best_stable,
        seed: spec.seed,
        cache_bytes: spec.cache_bytes,
        environment: crate::report::collect_environment(),
        metrics: MetricsSummary {
            operations: last_metrics.operations,
            failures: last_metrics.failures,
            busy_errors: last_metrics.busy_errors + last_metrics.locked_errors,
            locked_errors: last_metrics.locked_errors,
            timeout_errors: last_metrics.timeout_errors,
            elapsed_ms: elapsed.as_millis() as u64,
            throughput_ops_per_sec: throughput(last_metrics.operations, elapsed),
            latency: last_metrics.latency,
        },
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: serde_json::Value::Object(engine_stats),
        process_metrics: Some(process),
    })
}

fn connection_limit_max() -> usize {
    std::env::var(CONNECTION_LIMIT_MAX_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 1)
        .unwrap_or(DEFAULT_CONNECTION_LIMIT_MAX)
}

#[derive(Debug, Clone)]
struct ProbeMetrics {
    operations: u64,
    failures: u64,
    busy_errors: u64,
    locked_errors: u64,
    timeout_errors: u64,
    latency: crate::report::LatencySummary,
}

fn run_connection_burst(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    n: usize,
    duration: std::time::Duration,
) -> Result<ProbeMetrics> {
    let n = n.max(1);
    let barrier = Arc::new(Barrier::new(n));
    let deadline = Instant::now() + duration;
    let mut merged = Metrics::new();
    let scope_result = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(n);
        for worker in 0..n {
            let barrier = Arc::clone(&barrier);
            handles.push(scope.spawn(move |_| {
                let mut conn = engine.connect(worker)?;
                let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ worker as u64);
                barrier.wait();
                let mut metrics = Metrics::new();
                while Instant::now() < deadline {
                    let start = Instant::now();
                    let result = point_read(&mut *conn, spec.rows, &mut rng);
                    match result {
                        Ok(()) => metrics.record_success(start.elapsed()),
                        Err(err) => metrics.record_failure(classify_failure(&err)),
                    }
                }
                Ok::<Metrics, anyhow::Error>(metrics)
            }));
        }
        for handle in handles {
            merged.merge(&handle.join().expect("worker panicked")?);
        }
        Ok::<(), anyhow::Error>(())
    });
    match scope_result {
        Ok(result) => result?,
        Err(_) => anyhow::bail!("worker thread panicked"),
    }
    Ok(ProbeMetrics {
        operations: merged.operations(),
        failures: merged.failures(),
        busy_errors: merged.busy_errors(),
        locked_errors: merged.locked_errors(),
        timeout_errors: merged.timeout_errors(),
        latency: merged.latency(),
    })
}
