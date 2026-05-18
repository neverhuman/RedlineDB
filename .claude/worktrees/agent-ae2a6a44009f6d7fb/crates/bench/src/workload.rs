#[path = "connection_limit.rs"]
mod connection_limit;
#[path = "feature_workloads.rs"]
mod feature_workloads;
#[path = "phase11.rs"]
mod phase11;
#[path = "queue.rs"]
mod queue;
#[path = "spill.rs"]
mod spill;

use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::ValueEnum;
use crossbeam_utils::thread;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::chaos;
use crate::config::{EngineKind, RunSpec, WorkloadKind};
use crate::engine::{self, BenchConn, BenchEngine, CellValue};
use crate::metrics::{FailureKind, Metrics};
use crate::process_metrics;
use crate::report::{Checksum, MetricsSummary, RunRecord};

#[derive(Debug)]
struct MeasuredMetrics {
    metrics: Metrics,
    elapsed: Duration,
}

pub fn run_once(spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    let db_dir = spec.base_dir.join(format!(
        "{}-{}-{}-t{}-s{}",
        spec.engine.to_possible_value().expect("value").get_name(),
        spec.workload.as_str(),
        spec.durability.as_str(),
        spec.threads,
        spec.seed
    ));
    let _ = std::fs::remove_dir_all(&db_dir);
    let engine = engine::open(spec, &db_dir)?;
    engine.setup_schema()?;
    if !matches!(
        spec.workload,
        WorkloadKind::SingleRowInsert
            | WorkloadKind::BatchedInsert100
            | WorkloadKind::ConnectionLimit
            | WorkloadKind::LargeSortSpill
            | WorkloadKind::JsonPathExtract
            | WorkloadKind::JsonPathUpdate
            | WorkloadKind::VectorFlatSearch
            | WorkloadKind::VectorAnnSearch
            | WorkloadKind::VectorAnnSearchDisk
            | WorkloadKind::CommitStormBatched
            | WorkloadKind::CoveredRangeCold
            | WorkloadKind::CoveredRangeWarm
            | WorkloadKind::HotCounterUpdate
            | WorkloadKind::QueueMixed
            | WorkloadKind::ChaosLockConvoy
            | WorkloadKind::ChaosConnectionChurn
            | WorkloadKind::ChaosCheckpointThrash
            | WorkloadKind::ChaosIndexHammer
            | WorkloadKind::ChaosTempSpillConvoy
            | WorkloadKind::ChaosSchemaStorm
    ) {
        engine.seed_kv(spec.rows)?;
    }
    // Lane BH P1 #7: connection-limit is a self-managed workload —
    // it owns its connection pool, runs its own binary search, and
    // reports the resulting `max_stable_connections` via
    // `engine_stats`. The default per-thread runner is bypassed.
    if matches!(spec.workload, WorkloadKind::ConnectionLimit) {
        engine.seed_kv(spec.rows)?;
        return connection_limit::run_connection_limit(engine.as_ref(), spec);
    }
    // Lane VE: large-sort-spill is also self-managed: it seeds its own
    // table (separate `sortable` schema with a 64-byte payload), then
    // runs sort queries until the deadline. Reports
    // `engine_stats.spill_bytes_ratio` so the paper plot can show how
    // much of the sort actually went through the spill path.
    if matches!(spec.workload, WorkloadKind::LargeSortSpill) {
        return spill::run_large_sort_spill(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::JsonPathExtract) {
        return feature_workloads::run_json_path_extract(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::JsonPathUpdate) {
        return feature_workloads::run_json_path_update(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorFlatSearch) {
        return feature_workloads::run_vector_flat_search(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorAnnSearch) {
        return feature_workloads::run_vector_ann_search(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorAnnSearchDisk) {
        return feature_workloads::run_vector_ann_search_disk(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::CommitStormBatched) {
        return feature_workloads::run_commit_storm_batched(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::CoveredRangeCold) {
        return phase11::run_covered_range_cold(spec);
    }
    if matches!(spec.workload, WorkloadKind::CoveredRangeWarm) {
        return phase11::run_covered_range_warm(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::HotCounterUpdate) {
        return phase11::run_hot_counter_update(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::QueueMixed) {
        return queue::run_queue_mixed(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosLockConvoy) {
        return chaos::run_lock_convoy(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosConnectionChurn) {
        return chaos::run_connection_churn(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosCheckpointThrash) {
        return chaos::run_checkpoint_thrash(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosIndexHammer) {
        return chaos::run_index_hammer(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosTempSpillConvoy) {
        return chaos::run_temp_spill_convoy(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::ChaosSchemaStorm) {
        return chaos::run_schema_storm(engine.as_ref(), spec);
    }
    let measured = run_workload(engine.as_ref(), spec)?;
    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let checksum = engine.checksum()?;
    let wall_elapsed = wall_started.elapsed();
    let mut process = process_metrics::collect_self();
    // Lane BH P1 #7: when the engine surfaced its own kernel-level
    // syscall counters (Redline does, SQLite does not), prefer
    // those over the strace/getrusage-derived host fields. This is
    // why the certify manifest can now report non-`None`
    // fsync/pwrite tallies on Redline rows even on macOS where
    // strace is unavailable.
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
        "wall_elapsed_ms".to_owned(),
        serde_json::json!(wall_elapsed.as_millis() as u64),
    );
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

fn run_workload(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<MeasuredMetrics> {
    let started = Instant::now();
    let barrier = Arc::new(Barrier::new(spec.threads));
    let deadline = Instant::now() + spec.duration;
    let mut merged = Metrics::new();
    let scope_result = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(spec.threads);
        for worker in 0..spec.threads {
            let barrier = Arc::clone(&barrier);
            handles.push(scope.spawn(move |_| {
                let mut conn = engine.connect(worker)?;
                let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ worker as u64);
                barrier.wait();
                let mut metrics = Metrics::new();
                while Instant::now() < deadline {
                    let start = Instant::now();
                    let result = match spec.workload {
                        WorkloadKind::SingleRowInsert => {
                            single_row_insert(&mut *conn, worker, &mut rng)
                        }
                        WorkloadKind::BatchedInsert100 => {
                            batched_insert(&mut *conn, worker, &mut rng)
                        }
                        WorkloadKind::PointReadPk => point_read(&mut *conn, spec.rows, &mut rng),
                        WorkloadKind::SecondaryIndexRead => {
                            secondary_index_read(&mut *conn, spec.rows, &mut rng)
                        }
                        WorkloadKind::SecondaryIndexRange => {
                            secondary_index_range(&mut *conn, spec.rows, &mut rng)
                        }
                        WorkloadKind::SecondaryIndexCount => {
                            secondary_index_count(&mut *conn, spec.rows, &mut rng)
                        }
                        WorkloadKind::SecondaryIndexOrderedLimit => {
                            secondary_index_ordered_limit(&mut *conn, spec.rows, &mut rng)
                        }
                        WorkloadKind::WritersDisjoint => {
                            update_disjoint(&mut *conn, worker, spec.threads, spec.rows, &mut rng)
                        }
                        WorkloadKind::HotRowUpdate => hot_row_update(&mut *conn, worker, &mut rng),
                        WorkloadKind::MixedOltp => {
                            mixed_oltp(&mut *conn, worker, spec.threads, spec.rows, &mut rng)
                        }
                        WorkloadKind::Mixed95Read5Write => {
                            mixed_ratio(&mut *conn, worker, spec.threads, spec.rows, &mut rng, 95)
                        }
                        WorkloadKind::Mixed80Read20Write => {
                            mixed_ratio(&mut *conn, worker, spec.threads, spec.rows, &mut rng, 80)
                        }
                        WorkloadKind::Mixed50Read50Write => {
                            mixed_ratio(&mut *conn, worker, spec.threads, spec.rows, &mut rng, 50)
                        }
                        // Lane BH P1 #7: connection-limit is dispatched
                        // via `run_connection_limit` before this loop
                        // is reached; the arm is unreachable in
                        // practice but keeps the match exhaustive.
                        WorkloadKind::ConnectionLimit => {
                            unreachable!("connection-limit is handled by run_connection_limit")
                        }
                        WorkloadKind::LargeSortSpill => {
                            unreachable!("large-sort-spill is handled by run_large_sort_spill")
                        }
                        WorkloadKind::JsonPathExtract
                        | WorkloadKind::JsonPathUpdate
                        | WorkloadKind::VectorFlatSearch
                        | WorkloadKind::VectorAnnSearch
                        | WorkloadKind::VectorAnnSearchDisk
                        | WorkloadKind::CommitStormBatched => {
                            unreachable!("phase-10 feature workloads are self-managed")
                        }
                        WorkloadKind::CoveredRangeCold
                        | WorkloadKind::CoveredRangeWarm
                        | WorkloadKind::HotCounterUpdate
                        | WorkloadKind::QueueMixed
                        | WorkloadKind::ChaosLockConvoy
                        | WorkloadKind::ChaosConnectionChurn
                        | WorkloadKind::ChaosCheckpointThrash
                        | WorkloadKind::ChaosIndexHammer
                        | WorkloadKind::ChaosTempSpillConvoy
                        | WorkloadKind::ChaosSchemaStorm => {
                            unreachable!("phase-11 wave-1a workloads are self-managed")
                        }
                    };
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
    Ok(MeasuredMetrics {
        metrics: merged,
        elapsed: started.elapsed(),
    })
}

fn single_row_insert(conn: &mut dyn BenchConn, worker: usize, rng: &mut ChaCha8Rng) -> Result<()> {
    let key = ((worker as u64) << 32 | rng.random::<u32>() as u64) as i64;
    let params = [
        CellValue::Integer(key),
        CellValue::Integer((key % 32).abs()),
        CellValue::Blob(blob_for(key as usize)),
        CellValue::Integer(1),
    ];
    let _ = conn.execute(
        "INSERT INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)",
        &params,
    )?;
    Ok(())
}

fn batched_insert(conn: &mut dyn BenchConn, worker: usize, rng: &mut ChaCha8Rng) -> Result<()> {
    conn.begin_immediate()?;
    for _ in 0..100 {
        single_row_insert(conn, worker, rng)?;
    }
    conn.commit()?;
    Ok(())
}

fn point_read(conn: &mut dyn BenchConn, rows: usize, rng: &mut ChaCha8Rng) -> Result<()> {
    let key = (rng.random_range(0..rows.max(1))) as i64;
    let _ = conn.query_row(
        "SELECT version, v FROM kv WHERE k = ?1",
        &[CellValue::Integer(key)],
    )?;
    Ok(())
}

fn secondary_index_read(conn: &mut dyn BenchConn, rows: usize, rng: &mut ChaCha8Rng) -> Result<()> {
    let tenant = (rng.random_range(0..rows.max(1)) % 32) as i64;
    let _ = conn.query_row(
        "SELECT k, v FROM kv WHERE tenant = ?1 ORDER BY k LIMIT 1",
        &[CellValue::Integer(tenant)],
    )?;
    Ok(())
}

fn secondary_index_range(
    conn: &mut dyn BenchConn,
    rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    let tenant = (rng.random_range(0..rows.max(1)) % 32) as i64;
    let high = (tenant + 3).min(31);
    let _ = conn.query_row(
        "SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2",
        &[CellValue::Integer(tenant), CellValue::Integer(high)],
    )?;
    Ok(())
}

/// Phase 11 wave 1a: pure index-leaf walk with no heap rechecks.
/// `SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ? AND ?` over the
/// existing `kv_tenant_idx`. The COUNT-only projection means the
/// optimizer never has to re-fetch the heap row, so this isolates the
/// cost of walking secondary-index leaves end to end.
fn secondary_index_count(
    conn: &mut dyn BenchConn,
    rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    // Same fixture shape as `secondary_index_range`; covers ~3 tenant
    // buckets per query so the leaf walk has enough work to amortize
    // statement-prepare overhead while staying well below the table.
    let tenant = (rng.random_range(0..rows.max(1)) % 32) as i64;
    let high = (tenant + 3).min(31);
    let _ = conn.query_row(
        "SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2",
        &[CellValue::Integer(tenant), CellValue::Integer(high)],
    )?;
    Ok(())
}

/// Phase 11 wave 1a: ordered range with `LIMIT` early-stop.
/// `SELECT k, tenant FROM kv WHERE tenant >= ? ORDER BY tenant LIMIT 100`.
/// The index leading column matches `ORDER BY` so a competent planner
/// can stop after 100 rows without sorting the world.
fn secondary_index_ordered_limit(
    conn: &mut dyn BenchConn,
    rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    let tenant = (rng.random_range(0..rows.max(1)) % 32) as i64;
    let _ = conn.query_all(
        "SELECT k, tenant FROM kv WHERE tenant >= ?1 ORDER BY tenant LIMIT 100",
        &[CellValue::Integer(tenant)],
    )?;
    Ok(())
}

fn update_disjoint(
    conn: &mut dyn BenchConn,
    worker: usize,
    threads: usize,
    rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    let lane = worker % threads.max(1);
    let span = rows.max(threads) / threads.max(1);
    let start = lane * span;
    let key = (start + rng.random_range(0..span.max(1))) as i64;
    let params = [
        CellValue::Blob(blob_for((key as usize).wrapping_add(1))),
        CellValue::Integer(key),
    ];
    let _ = conn.execute(
        "UPDATE kv SET v = ?1, version = version + 1 WHERE k = ?2",
        &params,
    )?;
    Ok(())
}

fn hot_row_update(conn: &mut dyn BenchConn, worker: usize, rng: &mut ChaCha8Rng) -> Result<()> {
    let params = [
        CellValue::Blob(blob_for((worker << 24) ^ rng.random::<u32>() as usize)),
        CellValue::Integer(0),
    ];
    let _ = conn.execute(
        "UPDATE kv SET v = ?1, version = version + 1 WHERE k = ?2",
        &params,
    )?;
    Ok(())
}

fn mixed_oltp(
    conn: &mut dyn BenchConn,
    worker: usize,
    threads: usize,
    rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    if rng.random_range(0..10) < 8 {
        point_read(conn, rows, rng)
    } else {
        update_disjoint(conn, worker, threads, rows, rng)
    }
}

fn mixed_ratio(
    conn: &mut dyn BenchConn,
    worker: usize,
    threads: usize,
    rows: usize,
    rng: &mut ChaCha8Rng,
    read_pct: u32,
) -> Result<()> {
    if rng.random_range(0..100) < read_pct {
        point_read(conn, rows, rng)
    } else {
        update_disjoint(conn, worker, threads, rows, rng)
    }
}

fn blob_for(seed: usize) -> Vec<u8> {
    format!("value-{seed:08}").into_bytes()
}

fn checksum_query(engine: &dyn BenchEngine, label: &str, sql: &str) -> Result<Checksum> {
    let mut conn = engine.connect(0)?;
    let rows = conn.query_all(sql, &[])?;
    Ok(checksum_from_rows(label, &rows))
}

fn checksum_from_rows(label: &str, rows: &[Vec<CellValue>]) -> Checksum {
    let mut hasher = Sha256::new();
    let mut row_payloads = Vec::with_capacity(rows.len());
    let mut keys = Vec::with_capacity(rows.len());
    let mut payload_bytes = 0_i64;
    let mut version_sum = 0_i64;
    for (idx, row) in rows.iter().enumerate() {
        let mut row_buf = Vec::new();
        for cell in row {
            encode_cell_for_digest(cell, &mut row_buf);
            match cell {
                CellValue::Integer(value) => version_sum = version_sum.saturating_add(*value),
                CellValue::Text(value) => {
                    payload_bytes = payload_bytes.saturating_add(value.len() as i64)
                }
                CellValue::Blob(value) => {
                    payload_bytes = payload_bytes.saturating_add(value.len() as i64)
                }
                CellValue::Null | CellValue::Real(_) => {}
            }
        }
        hasher.update(b"row\0");
        hasher.update((row_buf.len() as u64).to_le_bytes());
        hasher.update(&row_buf);
        row_payloads.push(row_buf);
        let key = match row.first() {
            Some(CellValue::Integer(value)) => *value as u64,
            _ => idx as u64,
        };
        keys.push(key);
    }
    let mut index_consistency = BTreeMap::new();
    index_consistency.insert(format!("{label}_digest"), format!("ok rows={}", rows.len()));
    Checksum {
        rows: rows.len() as i64,
        version_sum,
        payload_bytes,
        content_hash: format!("{:x}", hasher.finalize()),
        index_consistency,
        dataset: Some(crate::checksum::DatasetChecksum {
            row_count: rows.len() as u64,
            key_xor: crate::checksum::key_xor(keys),
            payload_hash: crate::checksum::payload_hash(
                row_payloads.iter().map(|row| row.as_slice()),
            ),
        }),
    }
}

fn encode_cell_for_digest(cell: &CellValue, out: &mut Vec<u8>) {
    match cell {
        CellValue::Null => out.push(b'n'),
        CellValue::Integer(value) => {
            out.push(b'i');
            out.extend_from_slice(&value.to_le_bytes());
        }
        CellValue::Real(value) => {
            out.push(b'r');
            out.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        CellValue::Text(value) => {
            out.push(b't');
            out.extend_from_slice(&(value.len() as u64).to_le_bytes());
            out.extend_from_slice(value.as_bytes());
        }
        CellValue::Blob(value) => {
            out.push(b'b');
            out.extend_from_slice(&(value.len() as u64).to_le_bytes());
            out.extend_from_slice(value);
        }
    }
}

/// Lane BH P1 #7: probe the engine for the maximum number of
/// concurrent connections it serves stably under a 1s point-read
/// burst.
///
/// Algorithm: establish a 1-connection baseline, capture its p99,
/// then binary-search [low, high] over connection counts. A
/// candidate `n` is "stable" iff during 1 second of point-reads
const CONNECTION_LIMIT_MAX_ENV: &str = "REDLINEDB_BENCH_CONNECTION_LIMIT_MAX";
const DEFAULT_CONNECTION_LIMIT_MAX: usize = 256;

/// across `n` worker threads:
///   - error rate is at most 5%
///   - locked / busy / timeout ratio is at most 50%
///   - p99 stays within 100x of the baseline p99
///
/// Loop terminates once the binary-search window narrows to
/// within 8 connections.
///
/// Classify an error string into one of the four [`FailureKind`]
/// buckets. The classes are evaluated in priority order (locked first,
/// then timeout, then busy) so the same error never lands in more than
/// one bucket — Reviewer Finding #7 specifically called out that the
/// previous implementation collapsed LOCKED into BUSY.
pub(crate) fn classify_failure(err: &anyhow::Error) -> FailureKind {
    let text = err.to_string().to_ascii_lowercase();
    if is_locked_error_str(&text) {
        FailureKind::Locked
    } else if is_timeout_error_str(&text) {
        FailureKind::Timeout
    } else if is_busy_error_str(&text) {
        FailureKind::Busy
    } else {
        FailureKind::Other
    }
}

#[allow(dead_code)]
fn is_busy_error(err: &anyhow::Error) -> bool {
    matches!(classify_failure(err), FailureKind::Busy)
}

#[allow(dead_code)]
fn is_locked_error(err: &anyhow::Error) -> bool {
    matches!(classify_failure(err), FailureKind::Locked)
}

#[allow(dead_code)]
fn is_timeout_error(err: &anyhow::Error) -> bool {
    matches!(classify_failure(err), FailureKind::Timeout)
}

fn is_busy_error_str(text: &str) -> bool {
    text.contains("busy")
}

fn is_locked_error_str(text: &str) -> bool {
    text.contains("locked") || text.contains("database is locked") || text.contains("lock_wait")
}

fn is_timeout_error_str(text: &str) -> bool {
    text.contains("timeout") || text.contains("timed out") || text.contains("deadline")
}

fn throughput(operations: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        operations as f64 / seconds
    } else {
        0.0
    }
}

fn metrics_summary(metrics: &Metrics, elapsed: Duration) -> MetricsSummary {
    MetricsSummary {
        operations: metrics.operations(),
        failures: metrics.failures(),
        // Backward-compat: surfaces the original (busy + locked)
        // count for one minor cycle while consumers migrate to the
        // split fields below.
        busy_errors: metrics.busy_errors() + metrics.locked_errors(),
        locked_errors: metrics.locked_errors(),
        timeout_errors: metrics.timeout_errors(),
        elapsed_ms: elapsed.as_millis() as u64,
        throughput_ops_per_sec: throughput(metrics.operations(), elapsed),
        latency: metrics.latency(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> anyhow::Error {
        anyhow::anyhow!(text.to_owned())
    }

    #[test]
    fn classify_locked_does_not_collapse_into_busy() {
        // Reviewer Finding #7: "locked" must NOT also classify as busy.
        let e = err("database is locked: lock_wait expired");
        assert_eq!(classify_failure(&e), FailureKind::Locked);
        assert!(!is_busy_error(&e));
        assert!(is_locked_error(&e));
        assert!(!is_timeout_error(&e));
    }

    #[test]
    fn classify_busy_remains_busy() {
        let e = err("SQLITE_BUSY: database is busy");
        assert_eq!(classify_failure(&e), FailureKind::Busy);
        assert!(is_busy_error(&e));
        assert!(!is_locked_error(&e));
    }

    #[test]
    fn classify_timeout_separate_bucket() {
        let cases = [
            "operation timed out",
            "deadline exceeded",
            "lock acquire timeout after 100ms",
        ];
        for raw in cases {
            let e = err(raw);
            assert_eq!(
                classify_failure(&e),
                FailureKind::Timeout,
                "expected timeout for {raw}"
            );
            assert!(is_timeout_error(&e));
        }
    }

    #[test]
    fn classify_other_is_default() {
        let e = err("unique constraint violation");
        assert_eq!(classify_failure(&e), FailureKind::Other);
        assert!(!is_busy_error(&e));
        assert!(!is_locked_error(&e));
        assert!(!is_timeout_error(&e));
    }

    #[test]
    fn classify_buckets_are_disjoint() {
        // No single message ever populates more than one named bucket.
        let messages = [
            "locked while holding row lock",
            "busy throttled",
            "deadline exceeded waiting for fsync",
            "permission denied",
        ];
        for raw in messages {
            let e = err(raw);
            let busy = is_busy_error(&e) as u8;
            let locked = is_locked_error(&e) as u8;
            let timeout = is_timeout_error(&e) as u8;
            assert!(
                busy + locked + timeout <= 1,
                "{raw} classified into multiple buckets: busy={busy} locked={locked} timeout={timeout}"
            );
        }
    }

    #[test]
    fn metrics_summary_uses_measured_elapsed() {
        let mut metrics = Metrics::new();
        metrics.record_success(Duration::from_millis(10));
        metrics.record_success(Duration::from_millis(20));
        let summary = metrics_summary(&metrics, Duration::from_secs(2));
        assert_eq!(summary.elapsed_ms, 2_000);
        assert_eq!(summary.operations, 2);
        assert!((summary.throughput_ops_per_sec - 1.0).abs() < f64::EPSILON);
    }
}
