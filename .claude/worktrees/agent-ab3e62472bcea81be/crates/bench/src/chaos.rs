use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_utils::thread;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::config::RunSpec;
use crate::engine::{BenchConn, BenchEngine, CellValue};
use crate::metrics::Metrics;
use crate::process_metrics;
use crate::report::{Checksum, MetricsSummary, RunRecord};

#[derive(Debug)]
struct MeasuredMetrics {
    metrics: Metrics,
    elapsed: Duration,
}

pub(crate) fn run_lock_convoy(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    seed_rows(
        engine,
        "CREATE TABLE IF NOT EXISTS dick_head_choas_lock_convoy(pk INTEGER PRIMARY KEY, v INTEGER, payload BLOB)",
        "DELETE FROM dick_head_choas_lock_convoy",
        "INSERT INTO dick_head_choas_lock_convoy(pk, v, payload) VALUES (?1, 0, ?2)",
        spec.rows.max(1),
        |idx| {
            vec![
                CellValue::Integer(idx as i64),
                CellValue::Blob(blob_for(idx)),
            ]
        },
    )?;
    let commits = AtomicU64::new(0);
    let rollbacks = AtomicU64::new(0);
    let busy_timeout_ms = 25_u64;
    let measured = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
        conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
        conn.begin_immediate()?;
        conn.execute(
            "UPDATE dick_head_choas_lock_convoy SET v = v + 1, payload = ?1 WHERE pk = 0",
            &[CellValue::Blob(blob_for(
                (worker << 16) ^ rng.random::<u32>() as usize,
            ))],
        )?;
        std::thread::sleep(Duration::from_micros(
            500 + rng.random_range(0..2000) as u64,
        ));
        if rng.random_range(0..8) == 0 {
            rollbacks.fetch_add(1, Ordering::Relaxed);
            conn.rollback()?;
        } else {
            commits.fetch_add(1, Ordering::Relaxed);
            conn.commit()?;
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "dick-head-choas-lock-convoy",
        "SELECT pk, v, payload FROM dick_head_choas_lock_convoy ORDER BY pk",
    )?;
    let mut stats = chaos_stats("bounded", "lock-convoy", spec.rows.max(1), busy_timeout_ms);
    stats.insert(
        "chaos_commits".to_owned(),
        serde_json::json!(commits.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_rollbacks".to_owned(),
        serde_json::json!(rollbacks.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(crate) fn run_connection_churn(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    seed_rows(
        engine,
        "CREATE TABLE IF NOT EXISTS dick_head_choas_connection_churn(pk INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER, payload BLOB)",
        "DELETE FROM dick_head_choas_connection_churn",
        "INSERT INTO dick_head_choas_connection_churn(pk, tenant, v, payload) VALUES (?1, ?2, 0, ?3)",
        spec.rows.max(1),
        |idx| {
            vec![
                CellValue::Integer(idx as i64),
                CellValue::Integer((idx % 32) as i64),
                CellValue::Blob(blob_for(idx)),
            ]
        },
    )?;
    let connection_opens = AtomicU64::new(0);
    let commits = AtomicU64::new(0);
    let rollbacks = AtomicU64::new(0);
    let reads = AtomicU64::new(0);
    let writes = AtomicU64::new(0);
    let measured = run_threaded_engine_feature(engine, spec, |engine, worker, rng| {
        connection_opens.fetch_add(1, Ordering::Relaxed);
        let mut conn = engine.connect(worker)?;
        conn.set_busy_timeout(Duration::from_millis(10))?;
        if rng.random_range(0..100) < 70 {
            reads.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            let _ = conn.query_row(
                "SELECT v FROM dick_head_choas_connection_churn WHERE pk = ?1",
                &[CellValue::Integer(key)],
            )?;
        } else {
            writes.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            conn.begin_immediate()?;
            conn.execute(
                "UPDATE dick_head_choas_connection_churn SET v = v + 1, payload = ?1 WHERE pk = ?2",
                &[
                    CellValue::Blob(blob_for(
                        (worker << 20) ^ key as usize ^ rng.random::<u32>() as usize,
                    )),
                    CellValue::Integer(key),
                ],
            )?;
            if rng.random_range(0..10) == 0 {
                rollbacks.fetch_add(1, Ordering::Relaxed);
                conn.rollback()?;
            } else {
                commits.fetch_add(1, Ordering::Relaxed);
                conn.commit()?;
            }
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "dick-head-choas-connection-churn",
        "SELECT pk, tenant, v, payload FROM dick_head_choas_connection_churn ORDER BY pk",
    )?;
    let mut stats = chaos_stats("bounded", "connection-churn", spec.rows.max(1), 10);
    stats.insert(
        "chaos_connection_opens".to_owned(),
        serde_json::json!(connection_opens.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_reads".to_owned(),
        serde_json::json!(reads.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_writes".to_owned(),
        serde_json::json!(writes.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_commits".to_owned(),
        serde_json::json!(commits.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_rollbacks".to_owned(),
        serde_json::json!(rollbacks.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(crate) fn run_checkpoint_thrash(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    seed_rows(
        engine,
        "CREATE TABLE IF NOT EXISTS dick_head_choas_checkpoint_thrash(pk INTEGER PRIMARY KEY, v INTEGER, payload BLOB)",
        "DELETE FROM dick_head_choas_checkpoint_thrash",
        "INSERT INTO dick_head_choas_checkpoint_thrash(pk, v, payload) VALUES (?1, 0, ?2)",
        spec.rows.max(1),
        |idx| {
            vec![
                CellValue::Integer(idx as i64),
                CellValue::Blob(blob_for(idx)),
            ]
        },
    )?;
    let checkpoint_calls = AtomicU64::new(0);
    let reads = AtomicU64::new(0);
    let writes = AtomicU64::new(0);
    let measured = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
        conn.set_busy_timeout(Duration::from_millis(15))?;
        if worker == 0 && rng.random_range(0..4) != 0 {
            checkpoint_calls.fetch_add(1, Ordering::Relaxed);
            engine.checkpoint()?;
            return Ok(());
        }
        if rng.random_range(0..100) < 55 {
            reads.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            let _ = conn.query_row(
                "SELECT COUNT(*) FROM dick_head_choas_checkpoint_thrash WHERE pk BETWEEN ?1 AND ?2",
                &[
                    CellValue::Integer(key),
                    CellValue::Integer((key + 32).min(spec.rows.max(1) as i64)),
                ],
            )?;
        } else {
            writes.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            conn.begin_immediate()?;
            conn.execute(
                "UPDATE dick_head_choas_checkpoint_thrash SET v = v + 1, payload = ?1 WHERE pk = ?2",
                &[
                    CellValue::Blob(blob_for(
                        (worker << 12) ^ key as usize ^ rng.random::<u32>() as usize,
                    )),
                    CellValue::Integer(key),
                ],
            )?;
            if rng.random_range(0..5) == 0 {
                conn.rollback()?;
            } else {
                conn.commit()?;
            }
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "dick-head-choas-checkpoint-thrash",
        "SELECT pk, v, payload FROM dick_head_choas_checkpoint_thrash ORDER BY pk",
    )?;
    let mut stats = chaos_stats("bounded", "checkpoint-thrash", spec.rows.max(1), 15);
    stats.insert(
        "chaos_checkpoint_calls".to_owned(),
        serde_json::json!(checkpoint_calls.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_reads".to_owned(),
        serde_json::json!(reads.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_writes".to_owned(),
        serde_json::json!(writes.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(crate) fn run_index_hammer(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    seed_rows(
        engine,
        "CREATE TABLE IF NOT EXISTS dick_head_choas_index_hammer(pk INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
        "DELETE FROM dick_head_choas_index_hammer",
        "INSERT INTO dick_head_choas_index_hammer(pk, tenant, v, version) VALUES (?1, ?2, ?3, 1)",
        spec.rows.max(1),
        |idx| {
            vec![
                CellValue::Integer(idx as i64),
                CellValue::Integer((idx % 32) as i64),
                CellValue::Blob(blob_for(idx)),
            ]
        },
    )?;
    let commits = AtomicU64::new(0);
    let rollbacks = AtomicU64::new(0);
    let reads = AtomicU64::new(0);
    let writes = AtomicU64::new(0);
    let deletes = AtomicU64::new(0);
    let measured = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
        conn.set_busy_timeout(Duration::from_millis(20))?;
        let choice = rng.random_range(0..100);
        if choice < 35 {
            reads.fetch_add(1, Ordering::Relaxed);
            let low = rng.random_range(0..32) as i64;
            let high = (low + 4).min(31);
            let _ = conn.query_row(
                "SELECT COUNT(*) FROM dick_head_choas_index_hammer WHERE tenant BETWEEN ?1 AND ?2",
                &[CellValue::Integer(low), CellValue::Integer(high)],
            )?;
        } else if choice < 70 {
            writes.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            conn.begin_immediate()?;
            conn.execute(
                "UPDATE dick_head_choas_index_hammer SET tenant = ?1, v = ?2, version = version + 1 WHERE pk = ?3",
                &[
                    CellValue::Integer(rng.random_range(0..32) as i64),
                    CellValue::Blob(blob_for(
                        (worker << 18) ^ key as usize ^ rng.random::<u32>() as usize,
                    )),
                    CellValue::Integer(key),
                ],
            )?;
            if rng.random_range(0..7) == 0 {
                rollbacks.fetch_add(1, Ordering::Relaxed);
                conn.rollback()?;
            } else {
                commits.fetch_add(1, Ordering::Relaxed);
                conn.commit()?;
            }
        } else {
            deletes.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            conn.begin_immediate()?;
            conn.execute(
                "DELETE FROM dick_head_choas_index_hammer WHERE pk = ?1",
                &[CellValue::Integer(key)],
            )?;
            if rng.random_range(0..6) == 0 {
                rollbacks.fetch_add(1, Ordering::Relaxed);
                conn.rollback()?;
            } else {
                commits.fetch_add(1, Ordering::Relaxed);
                conn.commit()?;
            }
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "dick-head-choas-index-hammer",
        "SELECT pk, tenant, v, version FROM dick_head_choas_index_hammer ORDER BY pk",
    )?;
    let mut stats = chaos_stats("bounded", "index-hammer", spec.rows.max(1), 20);
    stats.insert(
        "chaos_reads".to_owned(),
        serde_json::json!(reads.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_writes".to_owned(),
        serde_json::json!(writes.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_deletes".to_owned(),
        serde_json::json!(deletes.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_commits".to_owned(),
        serde_json::json!(commits.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_rollbacks".to_owned(),
        serde_json::json!(rollbacks.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(crate) fn run_temp_spill_convoy(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    seed_rows(
        engine,
        "CREATE TABLE IF NOT EXISTS dick_head_choas_temp_spill_convoy(pk INTEGER PRIMARY KEY, score INTEGER, payload BLOB, version INTEGER)",
        "DELETE FROM dick_head_choas_temp_spill_convoy",
        "INSERT INTO dick_head_choas_temp_spill_convoy(pk, score, payload, version) VALUES (?1, ?2, ?3, 1)",
        spec.rows.max(1),
        |idx| {
            vec![
                CellValue::Integer(idx as i64),
                CellValue::Integer((idx % 1024) as i64),
                CellValue::Blob(large_blob(idx)),
            ]
        },
    )?;
    let spill_queries = AtomicU64::new(0);
    let writes = AtomicU64::new(0);
    let measured = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
        conn.set_busy_timeout(Duration::from_millis(20))?;
        if rng.random_range(0..100) < 60 {
            spill_queries.fetch_add(1, Ordering::Relaxed);
            let threshold = rng.random_range(0..1024) as i64;
            let _ = conn.query_all(
                "SELECT pk, score FROM dick_head_choas_temp_spill_convoy WHERE score >= ?1 ORDER BY payload DESC LIMIT 128",
                &[CellValue::Integer(threshold)],
            )?;
        } else {
            writes.fetch_add(1, Ordering::Relaxed);
            let key = rng.random_range(0..spec.rows.max(1)) as i64;
            conn.begin_immediate()?;
            conn.execute(
                "UPDATE dick_head_choas_temp_spill_convoy SET score = score + 1, payload = ?1, version = version + 1 WHERE pk = ?2",
                &[
                    CellValue::Blob(large_blob(
                        (worker << 14) ^ key as usize ^ rng.random::<u32>() as usize,
                    )),
                    CellValue::Integer(key),
                ],
            )?;
            if rng.random_range(0..4) == 0 {
                conn.rollback()?;
            } else {
                conn.commit()?;
            }
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "dick-head-choas-temp-spill-convoy",
        "SELECT pk, score, payload, version FROM dick_head_choas_temp_spill_convoy ORDER BY pk",
    )?;
    let mut stats = chaos_stats("bounded", "temp-spill-convoy", spec.rows.max(1), 20);
    stats.insert(
        "chaos_spill_queries".to_owned(),
        serde_json::json!(spill_queries.load(Ordering::Relaxed)),
    );
    stats.insert(
        "chaos_writes".to_owned(),
        serde_json::json!(writes.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(crate) fn run_schema_storm(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    let ddl_count = AtomicU64::new(0);
    let measured = run_threaded_engine_feature(engine, spec, |engine, worker, rng| {
        let mut conn = engine.connect(worker)?;
        conn.set_busy_timeout(Duration::from_millis(5))?;
        let suffix = (worker * 1024 + rng.random_range(0..256)) as u64;
        ddl_count.fetch_add(1, Ordering::Relaxed);
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS dick_head_choas_schema_{suffix}(id INTEGER PRIMARY KEY, v BLOB)"
            ),
            &[],
        )?;
        if rng.random_range(0..2) == 0 {
            conn.execute(
                &format!("CREATE INDEX IF NOT EXISTS dick_head_choas_schema_idx_{suffix} ON dick_head_choas_schema_{suffix}(id)"),
                &[],
            )?;
        }
        Ok(())
    })?;
    let checksum = checksum_from_rows(
        "dick-head-choas-schema-storm",
        &[vec![
            CellValue::Integer(0),
            CellValue::Integer(ddl_count.load(Ordering::Relaxed) as i64),
        ]],
    );
    let mut stats = chaos_stats("extreme", "schema-storm", 0, 5);
    stats.insert(
        "chaos_ddl_ops".to_owned(),
        serde_json::json!(ddl_count.load(Ordering::Relaxed)),
    );
    finish_record(engine, spec, measured, checksum, stats, wall_started)
}

fn run_threaded_conn_feature<F>(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    op: F,
) -> Result<MeasuredMetrics>
where
    F: Fn(&mut dyn BenchConn, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
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
            handles.push(scope.spawn(move |_| {
                let mut conn = engine.connect(worker)?;
                let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ worker as u64);
                barrier.wait();
                let mut metrics = Metrics::new();
                while Instant::now() < deadline {
                    let start = Instant::now();
                    match op(&mut *conn, worker, &mut rng) {
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

fn run_threaded_engine_feature<F>(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    op: F,
) -> Result<MeasuredMetrics>
where
    F: Fn(&dyn BenchEngine, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
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
            handles.push(scope.spawn(move |_| {
                let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ worker as u64);
                barrier.wait();
                let mut metrics = Metrics::new();
                while Instant::now() < deadline {
                    let start = Instant::now();
                    match op(engine, worker, &mut rng) {
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

fn checksum_query(engine: &dyn BenchEngine, label: &str, sql: &str) -> Result<Checksum> {
    let mut conn = engine.connect(0)?;
    let rows = conn.query_all(sql, &[])?;
    Ok(checksum_from_rows(label, &rows))
}

fn checksum_from_rows(label: &str, rows: &[Vec<CellValue>]) -> Checksum {
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

fn seed_rows<F>(
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

fn chaos_stats(
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
        serde_json::json!("crates/bench/src/chaos.rs"),
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

fn blob_for(seed: usize) -> Vec<u8> {
    format!("chaos-{seed:08}").into_bytes()
}

fn large_blob(seed: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(format!("payload-{seed:08}-").as_bytes());
    while out.len() < 256 {
        out.extend_from_slice(format!("{seed:08x}").as_bytes());
    }
    out.truncate(256);
    out
}
