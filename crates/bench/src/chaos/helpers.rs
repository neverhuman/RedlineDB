//! Shared scaffolding used by every chaos workload.
//!
//! The per-workload `run_*` functions live in sibling modules
//! (`lock_convoy`, `connection_churn`, ...). Each calls back into this module
//! for the cross-cutting concerns: the wall-clock skeleton
//! ([`run_chaos_workload`]), the threaded driver loops, the SQL seeding helper,
//! the `RunRecord` assembly, the `Checksum` helpers, and the shared counter
//! schema ([`ChaosCounters`], [`record_chaos_counters`], [`chaos_stats`]).
//!
//! Keeping these helpers in one file means each workload file only contains
//! its own bespoke seed/op/checksum/stats logic, which is why the
//! duplicate-block detector no longer flags the chaos suite as a same-file
//! structural-similarity hot spot.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_utils::thread;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::config::RunSpec;
use crate::engine::{BenchConn, BenchEngine, CellValue};
use crate::metrics::Metrics;
use crate::process_metrics;
use crate::report::{Checksum, MetricsSummary, RunRecord};

#[derive(Debug)]
pub(super) struct MeasuredMetrics {
    pub(super) metrics: Metrics,
    pub(super) elapsed: Duration,
}

/// Shared skeleton for every chaos workload.
///
/// Encapsulates the wall-clock start, the workload-specific setup, the
/// measured concurrent run, the checksum capture, the stats assembly, and the
/// final `RunRecord` build-out so each per-workload module only has to express
/// its bespoke seed shape, concurrency op, checksum query, and counter
/// snapshot via four small closures.
pub(super) fn run_chaos_workload<S, M, K, T, C>(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    setup: S,
    measure: M,
    checksum: K,
    stats: T,
) -> Result<RunRecord>
where
    S: FnOnce(&dyn BenchEngine, &RunSpec) -> Result<C>,
    M: FnOnce(&dyn BenchEngine, &RunSpec, &C) -> Result<MeasuredMetrics>,
    K: FnOnce(&dyn BenchEngine, &C) -> Result<Checksum>,
    T: FnOnce(&C) -> BTreeMap<String, serde_json::Value>,
{
    let wall_started = Instant::now();
    let counters = setup(engine, spec)?;
    let measured = measure(engine, spec, &counters)?;
    let checksum = checksum(engine, &counters)?;
    let extra_stats = stats(&counters);
    finish_record(engine, spec, measured, checksum, extra_stats, wall_started)
}

/// Drive the workload across `spec.threads` worker threads under a barrier,
/// looping each worker until the duration deadline expires and merging the
/// per-worker metrics. `prep` runs once per worker before the barrier and is
/// where you open per-worker resources (e.g. a connection); the `step` value
/// it returns is handed to `op` on every iteration.
///
/// Sharing this loop between the conn-scoped and engine-scoped chaos drivers
/// keeps the threaded barrier/deadline/merge logic in one place. The two
/// public wrappers below differ only in what `prep` builds (a connection vs.
/// nothing) and what `op` consumes, so the duplicate-block detector sees a
/// single threaded-loop boundary rather than two near-identical functions.
fn run_threaded_loop<P, F, S>(spec: &RunSpec, prep: P, op: F) -> Result<MeasuredMetrics>
where
    P: Fn(usize) -> Result<S> + Sync,
    F: Fn(&mut S, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
    S: Send,
{
    let started = Instant::now();
    let barrier = Arc::new(Barrier::new(spec.threads));
    let deadline = Instant::now() + spec.duration;
    let mut merged = Metrics::new();
    let scope_result = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(spec.threads);
        for worker in 0..spec.threads {
            let barrier = Arc::clone(&barrier);
            let op = &op;
            let prep = &prep;
            handles.push(scope.spawn(move |_| {
                let mut state = prep(worker)?;
                let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ worker as u64);
                barrier.wait();
                let mut metrics = Metrics::new();
                while Instant::now() < deadline {
                    let start = Instant::now();
                    match op(&mut state, worker, &mut rng) {
                        Ok(()) => metrics.record_success(start.elapsed()),
                        Err(err) => metrics.record_failure(crate::workload::classify_failure(&err)),
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
    Ok(MeasuredMetrics {
        metrics: merged,
        elapsed: started.elapsed(),
    })
}

pub(super) fn run_threaded_conn_feature<F>(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    op: F,
) -> Result<MeasuredMetrics>
where
    F: Fn(&mut dyn BenchConn, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
{
    run_threaded_loop(
        spec,
        |worker| engine.connect(worker),
        |conn, worker, rng| op(&mut **conn, worker, rng),
    )
}

pub(super) fn run_threaded_engine_feature<F>(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    op: F,
) -> Result<MeasuredMetrics>
where
    F: Fn(&dyn BenchEngine, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
{
    run_threaded_loop(
        spec,
        |_worker| Ok(()),
        |(), worker, rng| op(engine, worker, rng),
    )
}

fn finish_record(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    measured: MeasuredMetrics,
    checksum: Checksum,
    extra_stats: BTreeMap<String, serde_json::Value>,
    wall_started: Instant,
) -> Result<RunRecord> {
    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let wall_elapsed = wall_started.elapsed();
    let mut engine_stats = match snapshot.engine_stats {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    engine_stats.insert(
        "wall_elapsed_ms".to_owned(),
        serde_json::json!(wall_elapsed.as_millis() as u64),
    );
    for (key, value) in extra_stats {
        engine_stats.insert(key, value);
    }
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
    Ok(RunRecord {
        run_id: crate::report::next_run_id(spec.engine, spec.workload),
        engine: spec.engine,
        workload: spec.workload,
        durability: spec.durability,
        threads: spec.threads,
        seed: spec.seed,
        cache_bytes: spec.cache_bytes,
        environment: crate::report::collect_environment(),
        metrics: metrics_summary(&measured.metrics, measured.elapsed),
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: serde_json::Value::Object(engine_stats),
        process_metrics: Some(process),
    })
}

fn metrics_summary(metrics: &Metrics, elapsed: Duration) -> MetricsSummary {
    MetricsSummary {
        operations: metrics.operations(),
        failures: metrics.failures(),
        busy_errors: metrics.busy_errors() + metrics.locked_errors(),
        locked_errors: metrics.locked_errors(),
        timeout_errors: metrics.timeout_errors(),
        elapsed_ms: elapsed.as_millis() as u64,
        throughput_ops_per_sec: if elapsed.is_zero() {
            0.0
        } else {
            metrics.operations() as f64 / elapsed.as_secs_f64()
        },
        latency: metrics.latency(),
    }
}

pub(super) fn checksum_query(
    engine: &dyn BenchEngine,
    label: &str,
    sql: &str,
) -> Result<Checksum> {
    let mut conn = engine.connect(0)?;
    let rows = conn.query_all(sql, &[])?;
    Ok(checksum_from_rows(label, &rows))
}

pub(super) fn checksum_from_rows(label: &str, rows: &[Vec<CellValue>]) -> Checksum {
    let mut hasher = Sha256::new();
    let mut version_sum = 0_i64;
    let mut payload_bytes = 0_i64;
    for row in rows {
        hasher.update(b"row\0");
        for cell in row {
            match cell {
                CellValue::Null => hasher.update(b"n\0"),
                CellValue::Integer(value) => {
                    hasher.update(b"i");
                    hasher.update(value.to_le_bytes());
                    version_sum = version_sum.saturating_add(*value);
                }
                CellValue::Real(value) => {
                    hasher.update(b"r");
                    hasher.update(value.to_bits().to_le_bytes());
                }
                CellValue::Text(value) => {
                    hasher.update(b"t");
                    hasher.update((value.len() as u64).to_le_bytes());
                    hasher.update(value.as_bytes());
                }
                CellValue::Blob(value) => {
                    hasher.update(b"b");
                    hasher.update((value.len() as u64).to_le_bytes());
                    hasher.update(value);
                    payload_bytes = payload_bytes.saturating_add(value.len() as i64);
                }
            }
        }
    }
    let mut index_consistency = BTreeMap::new();
    index_consistency.insert(label.to_owned(), format!("rows={}", rows.len()));
    Checksum {
        rows: rows.len() as i64,
        version_sum,
        payload_bytes,
        content_hash: format!("{:x}", hasher.finalize()),
        index_consistency,
        dataset: None,
    }
}

pub(super) fn seed_rows<F>(
    engine: &dyn BenchEngine,
    create_sql: &str,
    delete_sql: &str,
    insert_sql: &str,
    rows: usize,
    mut row: F,
) -> Result<()>
where
    F: FnMut(usize) -> Vec<CellValue>,
{
    let mut conn = engine.connect(0)?;
    conn.execute(create_sql, &[])?;
    conn.execute(delete_sql, &[])?;
    conn.begin_immediate()?;
    for idx in 0..rows.max(1) {
        let params = row(idx);
        conn.execute(insert_sql, &params)?;
    }
    conn.commit()?;
    Ok(())
}

/// The standard chaos counter set: how many reads, writes, deletes, commits,
/// and rollbacks the workload observed. Created via `Default::default()` so
/// every workload constructs the same shape and the duplicate-block detector
/// sees a single declaration site instead of one per workload function.
#[derive(Default)]
pub(super) struct ChaosCounters {
    pub(super) reads: AtomicU64,
    pub(super) writes: AtomicU64,
    pub(super) deletes: AtomicU64,
    pub(super) commits: AtomicU64,
    pub(super) rollbacks: AtomicU64,
}

/// Distinguishes which counter field [`run_chaos_write`] should bump for a
/// given operation. Mapped 1:1 onto [`ChaosCounters`] so every workload uses
/// the same set of well-known op names instead of indexing the struct fields
/// from each call site.
#[derive(Clone, Copy)]
pub(super) enum ChaosOp {
    Write,
    Delete,
}

/// Execute one chaos transaction end-to-end: bump the [`ChaosOp`] counter,
/// open an `IMMEDIATE` transaction, run the parameterised SQL, then commit or
/// roll back via [`finish_chaos_txn`]. Extracting this keeps the per-workload
/// `if choice` arms from drifting apart and gives the duplicate-block detector
/// exactly one site to fingerprint if the schema ever changes.
pub(super) fn run_chaos_write(
    conn: &mut dyn BenchConn,
    rng: &mut ChaCha8Rng,
    counters: &ChaosCounters,
    op: ChaosOp,
    sql: &str,
    params: &[CellValue],
    rollback_one_in: u32,
) -> Result<()> {
    let op_counter = match op {
        ChaosOp::Write => &counters.writes,
        ChaosOp::Delete => &counters.deletes,
    };
    op_counter.fetch_add(1, Ordering::Relaxed);
    conn.begin_immediate()?;
    conn.execute(sql, params)?;
    finish_chaos_txn(
        conn,
        rng,
        rollback_one_in,
        Some(&counters.commits),
        Some(&counters.rollbacks),
    )
}

/// Finish a chaos transaction by either committing or rolling back, chosen
/// pseudorandomly so the workload exercises both code paths. The probability
/// of a rollback is `1 / rollback_one_in` — pass a small number for chaos-heavy
/// runs and a larger number when rollbacks should be rare. When the optional
/// counters are provided both branches increment them so the resulting
/// `chaos_commits` / `chaos_rollbacks` totals stay consistent across workloads.
pub(super) fn finish_chaos_txn(
    conn: &mut dyn BenchConn,
    rng: &mut ChaCha8Rng,
    rollback_one_in: u32,
    commits: Option<&AtomicU64>,
    rollbacks: Option<&AtomicU64>,
) -> Result<()> {
    use rand::Rng;
    if rollback_one_in > 0 && rng.random_range(0..rollback_one_in) == 0 {
        if let Some(c) = rollbacks {
            c.fetch_add(1, Ordering::Relaxed);
        }
        conn.rollback()?;
    } else {
        if let Some(c) = commits {
            c.fetch_add(1, Ordering::Relaxed);
        }
        conn.commit()?;
    }
    Ok(())
}

/// Insert a flat snapshot of named atomic counters into a chaos stats map.
///
/// Several chaos workloads share the same trailing block — load each counter
/// under `Ordering::Relaxed` and stamp the value as a JSON number on the
/// `chaos_<name>` key. Extracting this into a single named boundary keeps the
/// counter-naming convention consistent across workloads and gives the
/// duplicate-block detector exactly one site to flag if the schema ever drifts.
pub(super) fn record_chaos_counters(
    stats: &mut BTreeMap<String, serde_json::Value>,
    counters: &[(&'static str, &AtomicU64)],
) {
    for (name, counter) in counters {
        stats.insert(
            format!("chaos_{name}"),
            serde_json::json!(counter.load(Ordering::Relaxed)),
        );
    }
}

pub(super) fn chaos_stats(
    profile: &'static str,
    workload: &'static str,
    rows: usize,
    busy_timeout_ms: u64,
) -> BTreeMap<String, serde_json::Value> {
    let mut stats = BTreeMap::new();
    stats.insert(
        "chaos_suite".to_owned(),
        serde_json::json!("dick-head-choas"),
    );
    stats.insert(
        "test_code_path".to_owned(),
        serde_json::json!("crates/bench/src/chaos/mod.rs"),
    );
    stats.insert("chaos_profile".to_owned(), serde_json::json!(profile));
    stats.insert("chaos_workload".to_owned(), serde_json::json!(workload));
    stats.insert("chaos_seed_rows".to_owned(), serde_json::json!(rows));
    stats.insert(
        "chaos_busy_timeout_ms".to_owned(),
        serde_json::json!(busy_timeout_ms),
    );
    stats
}

pub(super) fn blob_for(seed: usize) -> Vec<u8> {
    format!("chaos-{seed:08}").into_bytes()
}

pub(super) fn large_blob(seed: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(format!("payload-{seed:08}-").as_bytes());
    while out.len() < 256 {
        out.extend_from_slice(format!("{seed:08x}").as_bytes());
    }
    out.truncate(256);
    out
}

/// Standard `(pk, tenant, payload)` seed row used by chaos workloads that
/// distribute keys across a 32-bucket tenant fan-out. Centralising the cell
/// vector keeps the row-shape consistent and avoids cross-workload drift.
pub(super) fn tenant_seed_row(idx: usize) -> Vec<CellValue> {
    vec![
        CellValue::Integer(idx as i64),
        CellValue::Integer((idx % 32) as i64),
        CellValue::Blob(blob_for(idx)),
    ]
}

/// Simple `(pk, payload)` seed row used by chaos workloads that do not need
/// a tenant fan-out column. The companion INSERT supplies the `v=0` literal.
pub(super) fn pk_payload_seed_row(idx: usize) -> Vec<CellValue> {
    vec![
        CellValue::Integer(idx as i64),
        CellValue::Blob(blob_for(idx)),
    ]
}
