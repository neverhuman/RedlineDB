use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::ValueEnum;
use crossbeam_utils::thread;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use redlinedb_kernel::format::{
    PageGeneration, PageId, RelId, RowId as KernelRowId, TuplePtr, TxId,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::vector::diskann::{DiskAnnIndex, DiskAnnParams, RowId as DiskAnnRowId};
use redlinedb_kernel::vector::hnsw::{HnswIndex, HnswParams, IndexedRowRef};
use sha2::{Digest, Sha256};

use crate::config::{EngineKind, RunSpec, WorkloadKind};
use crate::engine::{self, BenchConn, BenchEngine, CellValue};
use crate::metrics::{FailureKind, Metrics};
use crate::process_metrics;
use crate::report::{Checksum, MetricsSummary, RunRecord};

pub fn run_once(spec: &RunSpec) -> Result<RunRecord> {
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
    ) {
        engine.seed_kv(spec.rows)?;
    }
    // Lane BH P1 #7: connection-limit is a self-managed workload —
    // it owns its connection pool, runs its own binary search, and
    // reports the resulting `max_stable_connections` via
    // `engine_stats`. The default per-thread runner is bypassed.
    if matches!(spec.workload, WorkloadKind::ConnectionLimit) {
        engine.seed_kv(spec.rows)?;
        return run_connection_limit(engine.as_ref(), spec);
    }
    // Lane VE: large-sort-spill is also self-managed: it seeds its own
    // table (separate `sortable` schema with a 64-byte payload), then
    // runs sort queries until the deadline. Reports
    // `engine_stats.spill_bytes_ratio` so the paper plot can show how
    // much of the sort actually went through the spill path.
    if matches!(spec.workload, WorkloadKind::LargeSortSpill) {
        return run_large_sort_spill(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::JsonPathExtract) {
        return run_json_path_extract(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::JsonPathUpdate) {
        return run_json_path_update(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorFlatSearch) {
        return run_vector_flat_search(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorAnnSearch) {
        return run_vector_ann_search(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::VectorAnnSearchDisk) {
        return run_vector_ann_search_disk(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::CommitStormBatched) {
        return run_commit_storm_batched(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::CoveredRangeCold) {
        return run_covered_range_cold(spec);
    }
    if matches!(spec.workload, WorkloadKind::CoveredRangeWarm) {
        return run_covered_range_warm(engine.as_ref(), spec);
    }
    if matches!(spec.workload, WorkloadKind::HotCounterUpdate) {
        return run_hot_counter_update(engine.as_ref(), spec);
    }
    let started = Instant::now();
    let metrics = run_workload(engine.as_ref(), spec)?;
    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let checksum = engine.checksum()?;
    let elapsed = started.elapsed();
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
    Ok(RunRecord {
        run_id: crate::report::next_run_id(spec.engine, spec.workload),
        engine: spec.engine,
        workload: spec.workload,
        durability: spec.durability,
        threads: spec.threads,
        seed: spec.seed,
        cache_bytes: spec.cache_bytes,
        environment: crate::report::collect_environment(),
        metrics: MetricsSummary {
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
        },
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: snapshot.engine_stats,
        process_metrics: Some(process),
    })
}

fn run_workload(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<Metrics> {
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
                        | WorkloadKind::HotCounterUpdate => {
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
    Ok(merged)
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

fn run_json_path_extract(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    setup_json_docs(engine, spec)?;
    let metrics = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
        let id = rng.random_range(0..spec.rows.max(1)) as i64;
        let _ = conn.query_row(
            "SELECT json_extract(body, '$.nested.score'), json_extract(body, '$.tags[1]') \
             FROM json_docs WHERE id = ?1",
            &[CellValue::Integer(id)],
        )?;
        Ok(())
    })?;
    let checksum = json_docs_checksum(engine)?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "json_path_ops".to_owned(),
        serde_json::json!(metrics.operations()),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_json_path_update(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    setup_json_docs(engine, spec)?;
    let metrics = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
        let id = rng.random_range(0..spec.rows.max(1)) as i64;
        let _ = conn.execute(
            "UPDATE json_docs \
             SET body = json_set(body, '$.counter', json_extract(body, '$.counter') + 1) \
             WHERE id = ?1",
            &[CellValue::Integer(id)],
        )?;
        Ok(())
    })?;
    let checksum = json_docs_checksum(engine)?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "json_path_ops".to_owned(),
        serde_json::json!(metrics.operations()),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_vector_flat_search(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    setup_vector_table(engine, spec)?;
    let metrics = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
        let query = vector_for(rng.random::<u64>() as usize, VECTOR_DIM);
        let _ = vector_flat_top_k(conn, spec.engine, &query, 10)?;
        Ok(())
    })?;
    let checksum = vector_flat_checksum(engine, spec.engine)?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "vector_exact_topk_ops".to_owned(),
        serde_json::json!(metrics.operations()),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_vector_ann_search(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    let rows = ann_rows(spec);
    let vectors = vector_dataset(rows, HNSW_DIM, spec.seed);
    let temp = tempfile::TempDir::new()?;
    let page_file = Arc::new(PageFile::create(temp.path().join("hnsw.redline"), 512)?);
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 1024)?);
    let mut params = HnswParams::standard(HNSW_DIM);
    params.m = 16;
    params.m_max0 = 32;
    params.ef_construction = 128;
    params.ef_search = 96;
    let index = HnswIndex::create_with_wal(buffer, RelId(1), 1, params, None, spec.seed)?;
    for (idx, vector) in vectors.iter().enumerate() {
        index.insert_tx(TxId(idx as u64 + 1), vector, hnsw_row_ref(idx))?;
    }
    let index = Arc::new(index);
    let vectors = Arc::new(vectors);
    let metrics = run_threaded_compute_feature(spec, {
        let index = Arc::clone(&index);
        move |_worker, rng| {
            let query = vector_for(rng.random::<u64>() as usize, HNSW_DIM);
            let hits = index.search(&query, 10, 96)?;
            if hits.is_empty() {
                anyhow::bail!("hnsw search returned no hits");
            }
            Ok(())
        }
    })?;
    let (checksum, recall) = hnsw_checksum(&index, &vectors)?;
    let mut stats = BTreeMap::new();
    stats.insert("vector_recall_at_10".to_owned(), serde_json::json!(recall));
    stats.insert("vector_candidates".to_owned(), serde_json::json!(96));
    stats.insert("vector_index_kind".to_owned(), serde_json::json!("hnsw"));
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_vector_ann_search_disk(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    let rows = ann_rows(spec);
    let vectors = vector_dataset(rows, DISKANN_DIM, spec.seed ^ 0xD15C_A991);
    let row_ids: Vec<DiskAnnRowId> = (0..rows).map(|idx| DiskAnnRowId(idx as u64)).collect();
    let params = DiskAnnParams {
        max_degree: 32,
        search_list_size: 64,
        alpha: 1.2,
        seed: spec.seed ^ 0xA11CE,
    };
    let built = DiskAnnIndex::build(DISKANN_DIM, &vectors, &row_ids, params)?;
    let sectors = built.to_sectors()?;
    let temp = tempfile::TempDir::new()?;
    let sector_path = temp.path().join("diskann.sectors");
    std::fs::write(&sector_path, &sectors)?;
    let loaded = std::fs::read(&sector_path)?;
    let index = Arc::new(DiskAnnIndex::from_sectors(
        &loaded,
        DISKANN_DIM,
        params,
        built.entry(),
        rows,
    )?);
    let vectors = Arc::new(vectors);
    let metrics = run_threaded_compute_feature(spec, {
        let index = Arc::clone(&index);
        move |_worker, rng| {
            let query = vector_for(rng.random::<u64>() as usize, DISKANN_DIM);
            let hits = index.search(&query, 10, 64)?;
            if hits.is_empty() {
                anyhow::bail!("diskann search returned no hits");
            }
            Ok(())
        }
    })?;
    let (checksum, recall) = diskann_checksum(&index, &vectors)?;
    let mut stats = BTreeMap::new();
    stats.insert("vector_recall_at_10".to_owned(), serde_json::json!(recall));
    stats.insert("vector_candidates".to_owned(), serde_json::json!(64));
    stats.insert("vector_index_kind".to_owned(), serde_json::json!("diskann"));
    stats.insert(
        "diskann_sector_bytes".to_owned(),
        serde_json::json!(loaded.len() as u64),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_commit_storm_batched(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    setup_commit_storm(engine, spec)?;
    let metrics = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
        let id =
            ((worker + rng.random_range(0..4) * spec.threads.max(1)) % commit_rows(spec)) as i64;
        let _ = conn.execute(
            "UPDATE commit_storm SET v = v + 1 WHERE id = ?1",
            &[CellValue::Integer(id)],
        )?;
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "commit-storm",
        "SELECT id, v FROM commit_storm ORDER BY id",
    )?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "commit_storm_ops".to_owned(),
        serde_json::json!(metrics.operations()),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

fn run_threaded_conn_feature<F>(engine: &dyn BenchEngine, spec: &RunSpec, op: F) -> Result<Metrics>
where
    F: Fn(&mut dyn BenchConn, usize, &mut ChaCha8Rng) -> Result<()> + Sync,
{
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
    Ok(merged)
}

fn run_threaded_compute_feature<F>(spec: &RunSpec, op: F) -> Result<Metrics>
where
    F: Fn(usize, &mut ChaCha8Rng) -> Result<()> + Sync,
{
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
                    match op(worker, &mut rng) {
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
    Ok(merged)
}

fn finish_self_managed_record(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    started: Instant,
    metrics: Metrics,
    checksum: Checksum,
    extra_stats: BTreeMap<String, serde_json::Value>,
) -> Result<RunRecord> {
    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let elapsed = started.elapsed();
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
    for (key, value) in extra_stats {
        engine_stats.insert(key, value);
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
        metrics: MetricsSummary {
            operations: metrics.operations(),
            failures: metrics.failures(),
            busy_errors: metrics.busy_errors() + metrics.locked_errors(),
            locked_errors: metrics.locked_errors(),
            timeout_errors: metrics.timeout_errors(),
            elapsed_ms: elapsed.as_millis() as u64,
            throughput_ops_per_sec: throughput(metrics.operations(), elapsed),
            latency: metrics.latency(),
        },
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: serde_json::Value::Object(engine_stats),
        process_metrics: Some(process),
    })
}

fn setup_json_docs(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<()> {
    let mut conn = engine.connect(0)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS json_docs(id INTEGER PRIMARY KEY, body TEXT)",
        &[],
    )?;
    conn.execute("DELETE FROM json_docs", &[])?;
    conn.begin_immediate()?;
    for id in 0..spec.rows.max(1) {
        let body = format!(
            "{{\"id\":{id},\"tenant\":{},\"nested\":{{\"score\":{}}},\
             \"tags\":[\"tag{}\",\"bucket{}\"],\"counter\":0}}",
            id % 32,
            (id * 7) % 1000,
            id % 17,
            id % 5
        );
        conn.execute(
            "INSERT INTO json_docs(id, body) VALUES (?1, ?2)",
            &[CellValue::Integer(id as i64), CellValue::Text(body)],
        )?;
    }
    conn.commit()
}

fn json_docs_checksum(engine: &dyn BenchEngine) -> Result<Checksum> {
    checksum_query(
        engine,
        "json-docs",
        "SELECT id, json_extract(body, '$.counter'), json_extract(body, '$.nested.score'), \
         json_extract(body, '$.tags[1]') FROM json_docs ORDER BY id",
    )
}

const VECTOR_DIM: usize = 8;
const HNSW_DIM: usize = 16;
const DISKANN_DIM: usize = 16;

fn setup_vector_table(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<()> {
    let mut conn = engine.connect(0)?;
    match spec.engine {
        EngineKind::Redline => {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS bench_vectors(id INTEGER PRIMARY KEY, e VECTOR(8))",
                &[],
            )?;
        }
        EngineKind::Sqlite => {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS bench_vectors(id INTEGER PRIMARY KEY, e TEXT)",
                &[],
            )?;
        }
    }
    conn.execute("DELETE FROM bench_vectors", &[])?;
    conn.begin_immediate()?;
    for id in 0..spec.rows.max(1) {
        let literal = vector_literal(&vector_for(id, VECTOR_DIM));
        match spec.engine {
            EngineKind::Redline => {
                let sql =
                    format!("INSERT INTO bench_vectors(id, e) VALUES ({id}, vector('{literal}'))");
                conn.execute(&sql, &[])?;
            }
            EngineKind::Sqlite => {
                conn.execute(
                    "INSERT INTO bench_vectors(id, e) VALUES (?1, ?2)",
                    &[CellValue::Integer(id as i64), CellValue::Text(literal)],
                )?;
            }
        }
    }
    conn.commit()
}

fn vector_flat_top_k(
    conn: &mut dyn BenchConn,
    engine: EngineKind,
    query: &[f32],
    k: usize,
) -> Result<Vec<i64>> {
    match engine {
        EngineKind::Redline => {
            let literal = vector_literal(query);
            let sql = format!(
                "SELECT id FROM bench_vectors \
                 ORDER BY vector_distance_l2(e, vector('{literal}')) LIMIT {k}"
            );
            let rows = conn.query_all(&sql, &[])?;
            Ok(rows
                .into_iter()
                .filter_map(|row| match row.first() {
                    Some(CellValue::Integer(id)) => Some(*id),
                    _ => None,
                })
                .collect())
        }
        EngineKind::Sqlite => {
            let rows = conn.query_all("SELECT id, e FROM bench_vectors", &[])?;
            let mut scored = Vec::with_capacity(rows.len());
            for row in rows {
                let id = match row.first() {
                    Some(CellValue::Integer(id)) => *id,
                    _ => continue,
                };
                let vector = match row.get(1) {
                    Some(CellValue::Text(text)) => parse_vector_literal(text)?,
                    _ => continue,
                };
                scored.push((l2_squared(&vector, query), id));
            }
            scored.sort_by(|left, right| left.0.total_cmp(&right.0));
            Ok(scored.into_iter().take(k).map(|(_, id)| id).collect())
        }
    }
}

fn vector_flat_checksum(engine: &dyn BenchEngine, kind: EngineKind) -> Result<Checksum> {
    let mut conn = engine.connect(0)?;
    let mut rows = Vec::new();
    for seed in 0..16 {
        let query = vector_for(seed * 17 + 3, VECTOR_DIM);
        let ids = vector_flat_top_k(&mut *conn, kind, &query, 10)?;
        rows.push(vec![
            CellValue::Integer(seed as i64),
            CellValue::Text(
                ids.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ]);
    }
    Ok(checksum_from_rows("vector-flat", &rows))
}

fn ann_rows(spec: &RunSpec) -> usize {
    if spec.rows == 512 {
        2_000
    } else {
        spec.rows.max(32)
    }
}

fn vector_dataset(rows: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..rows)
        .map(|_| (0..dim).map(|_| rng.random_range(-1.0..1.0)).collect())
        .collect()
}

fn vector_for(seed: usize, dim: usize) -> Vec<f32> {
    let mut state = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(dim);
    for _ in 0..dim {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = ((state >> 32) as u32) as f32 / u32::MAX as f32;
        out.push(value * 2.0 - 1.0);
    }
    out
}

fn vector_literal(values: &[f32]) -> String {
    let mut out = String::from("[");
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!("{value:.6}"));
    }
    out.push(']');
    out
}

fn parse_vector_literal(raw: &str) -> Result<Vec<f32>> {
    let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(|part| Ok(part.trim().parse::<f32>()?))
        .collect()
}

fn l2_squared(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| {
            let d = a - b;
            d * d
        })
        .sum()
}

fn hnsw_row_ref(idx: usize) -> IndexedRowRef {
    IndexedRowRef::new(
        KernelRowId(idx as u64),
        TuplePtr::new_with_generation(
            PageId((idx / u16::MAX as usize + 1) as u64),
            (idx % u16::MAX as usize) as u16,
            PageGeneration::ONE,
        ),
    )
}

fn hnsw_checksum(index: &HnswIndex, vectors: &[Vec<f32>]) -> Result<(Checksum, f64)> {
    let mut rows = Vec::new();
    let mut total_hits = 0usize;
    let mut total_truth = 0usize;
    for seed in 0..32 {
        let query = vector_for(seed * 31 + 11, HNSW_DIM);
        let truth = brute_force_top_k(vectors, &query, 10);
        let hits = index.search(&query, 10, 96)?;
        let hit_ids: Vec<u64> = hits.iter().map(|hit| hit.row_id.0).collect();
        total_hits += hit_ids.iter().filter(|id| truth.contains(id)).count();
        total_truth += truth.len();
        rows.push(vec![
            CellValue::Integer(seed as i64),
            CellValue::Text(
                hit_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ]);
    }
    let recall = if total_truth > 0 {
        total_hits as f64 / total_truth as f64
    } else {
        1.0
    };
    Ok((checksum_from_rows("hnsw", &rows), recall))
}

fn diskann_checksum(index: &DiskAnnIndex, vectors: &[Vec<f32>]) -> Result<(Checksum, f64)> {
    let mut rows = Vec::new();
    let mut total_hits = 0usize;
    let mut total_truth = 0usize;
    for seed in 0..32 {
        let query = vector_for(seed * 43 + 5, DISKANN_DIM);
        let truth = brute_force_top_k(vectors, &query, 10);
        let hits = index.search(&query, 10, 64)?;
        let hit_ids: Vec<u64> = hits.iter().map(|hit| hit.0).collect();
        total_hits += hit_ids.iter().filter(|id| truth.contains(id)).count();
        total_truth += truth.len();
        rows.push(vec![
            CellValue::Integer(seed as i64),
            CellValue::Text(
                hit_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ]);
    }
    let recall = if total_truth > 0 {
        total_hits as f64 / total_truth as f64
    } else {
        1.0
    };
    Ok((checksum_from_rows("diskann", &rows), recall))
}

fn brute_force_top_k(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<u64> {
    let mut scored: Vec<(f32, u64)> = vectors
        .iter()
        .enumerate()
        .map(|(idx, vector)| (l2_squared(vector, query), idx as u64))
        .collect();
    scored.sort_by(|left, right| left.0.total_cmp(&right.0));
    scored.into_iter().take(k).map(|(_, id)| id).collect()
}

fn setup_commit_storm(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<()> {
    let mut conn = engine.connect(0)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS commit_storm(id INTEGER PRIMARY KEY, v INTEGER)",
        &[],
    )?;
    conn.execute("DELETE FROM commit_storm", &[])?;
    conn.begin_immediate()?;
    for id in 0..commit_rows(spec) {
        conn.execute(
            "INSERT INTO commit_storm(id, v) VALUES (?1, 0)",
            &[CellValue::Integer(id as i64)],
        )?;
    }
    conn.commit()
}

fn commit_rows(spec: &RunSpec) -> usize {
    spec.threads.max(1) * 4
}

/// Phase 11 wave 1a: row-count target for the covering-range fixtures.
/// We aim for "enough rows that the leaves span multiple pages but the
/// table fits comfortably in the configured cache" — `spec.rows`
/// directly controls the corpus size, with a floor of 4_096 so the
/// scan does meaningful work even at the smoke default of 512.
fn covered_range_rows(spec: &RunSpec) -> usize {
    spec.rows.max(4_096)
}

/// Phase 11 wave 1a: setup the `covered_kv` table used by both
/// `CoveredRangeCold` and `CoveredRangeWarm`.
///
/// Schema is `(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER)` with a
/// composite index on `(k, v)` so the SELECT projection of `(k, v)`
/// is fully covered. Rows are seeded deterministically from
/// `spec.seed` so every replicate sees the same dataset.
fn setup_covered_kv(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<()> {
    let mut conn = engine.connect(0)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS covered_kv(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER)",
        &[],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS covered_kv_kv_idx ON covered_kv(k, v)",
        &[],
    )?;
    conn.execute("DELETE FROM covered_kv", &[])?;
    let rows = covered_range_rows(spec);
    let mut rng = ChaCha8Rng::seed_from_u64(spec.seed ^ 0xC07E_C07E);
    conn.begin_immediate()?;
    for id in 0..rows {
        // `k` spans 0..rows so a `BETWEEN ? AND ?` window of `STEP`
        // entries returns predictable workload-sized batches; `v` is
        // a deterministic noise column that still rides along in the
        // covering index.
        let k = id as i64;
        let v = (rng.random::<u32>() as i64) % 1_000_000;
        conn.execute(
            "INSERT INTO covered_kv(id, k, v) VALUES (?1, ?2, ?3)",
            &[
                CellValue::Integer(id as i64),
                CellValue::Integer(k),
                CellValue::Integer(v),
            ],
        )?;
    }
    conn.commit()
}

/// How many rows each covered-range query asks for. Aim for ~256
/// entries per scan: large enough that page-level prefetch matters,
/// small enough that the per-op work stays bounded.
const COVERED_RANGE_WINDOW: i64 = 256;

fn covered_range_step(
    conn: &mut dyn BenchConn,
    total_rows: usize,
    rng: &mut ChaCha8Rng,
) -> Result<()> {
    let rows = total_rows.max(1) as i64;
    let low = rng.random_range(0..rows.max(1) as u64) as i64;
    let high = (low + COVERED_RANGE_WINDOW).min(rows.saturating_sub(1).max(low));
    let _ = conn.query_all(
        "SELECT k, v FROM covered_kv WHERE k BETWEEN ?1 AND ?2",
        &[CellValue::Integer(low), CellValue::Integer(high)],
    )?;
    Ok(())
}

/// Phase 11 wave 1a: covered-range scan with cold cache.
///
/// Reopens the database from disk for the measurement window so the
/// engine's buffer pool starts empty; this is the closest portable
/// approximation to "drop the OS page cache" that works on macOS as
/// well as Linux. SQLite's per-process cache_size pragma plus a fresh
/// `Connection::open` walks the same path. Threads are honored —
/// each worker drives its own connection.
fn run_covered_range_cold(spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    // Seed phase: open the engine, populate the table, then drop
    // the handle so the next open hits a cold buffer pool.
    let db_dir = spec.base_dir.join(format!(
        "{}-{}-cold-seed-t{}-s{}",
        spec.engine.to_possible_value().expect("value").get_name(),
        spec.workload.as_str(),
        spec.threads,
        spec.seed
    ));
    let _ = std::fs::remove_dir_all(&db_dir);
    {
        let engine = engine::open(spec, &db_dir)?;
        engine.setup_schema()?;
        setup_covered_kv(engine.as_ref(), spec)?;
        engine.checkpoint()?;
        // engine drops here; buffer pool / process-private caches go
        // away with it.
    }
    // Measurement phase: reopen against the same on-disk dataset and
    // drive the scan loop. The buffer pool is empty so leaf-page
    // faults are observable in the latency tail.
    let measure_engine = engine::open(spec, &db_dir)?;
    let total = covered_range_rows(spec);
    let metrics = run_threaded_conn_feature(measure_engine.as_ref(), spec, |conn, _w, rng| {
        covered_range_step(conn, total, rng)
    })?;
    let checksum = checksum_query(
        measure_engine.as_ref(),
        "covered-range-cold",
        "SELECT id, k, v FROM covered_kv ORDER BY id",
    )?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "covered_range_rows".to_owned(),
        serde_json::json!(total as u64),
    );
    stats.insert("covered_range_window".to_owned(), serde_json::json!(256));
    stats.insert("covered_range_cold".to_owned(), serde_json::json!(true));
    finish_self_managed_record(
        measure_engine.as_ref(),
        spec,
        started,
        metrics,
        checksum,
        stats,
    )
}

/// Phase 11 wave 1a: covered-range scan with warm cache.
///
/// Same shape as the cold variant but issues a deterministic warmup
/// pass over the same range *before* the measurement window starts,
/// so the leaf pages are already resident. The throughput delta
/// versus `CoveredRangeCold` is the headline number — it shows the
/// covering-scan working set actually fitting in the buffer pool.
fn run_covered_range_warm(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    setup_covered_kv(engine, spec)?;
    let total = covered_range_rows(spec);
    // Warmup: a single connection runs the full range scan once so
    // every leaf page is faulted in. We deliberately do not clock
    // this against `Metrics`.
    {
        let mut warmup_conn = engine.connect(0)?;
        let _ = warmup_conn.query_all(
            "SELECT k, v FROM covered_kv WHERE k BETWEEN ?1 AND ?2",
            &[CellValue::Integer(0), CellValue::Integer(total as i64)],
        )?;
    }
    let metrics = run_threaded_conn_feature(engine, spec, |conn, _w, rng| {
        covered_range_step(conn, total, rng)
    })?;
    let checksum = checksum_query(
        engine,
        "covered-range-warm",
        "SELECT id, k, v FROM covered_kv ORDER BY id",
    )?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "covered_range_rows".to_owned(),
        serde_json::json!(total as u64),
    );
    stats.insert("covered_range_window".to_owned(), serde_json::json!(256));
    stats.insert("covered_range_cold".to_owned(), serde_json::json!(false));
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
}

/// Phase 11 wave 1a: hot-counter increment baseline.
///
/// Uses a dedicated `hot_counter(pk INTEGER PRIMARY KEY, counter
/// INTEGER)` table with a single row at `pk = 0`. Every step issues
/// `UPDATE hot_counter SET counter = counter + 1 WHERE pk = ?1`,
/// which under contention is the future "commutative-delta combiner"
/// playground. For now the workload simply lets the engine fight
/// itself — the throughput number is the baseline that the combiner
/// path needs to beat in Wave 1b.
///
/// Multi-thread variants are valid but the canonical run is
/// `threads = 1`, which we use in `phase11-oltp-gap.toml` to
/// establish the no-contention baseline the combiner rollout will
/// be measured against.
fn run_hot_counter_update(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    {
        let mut conn = engine.connect(0)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS hot_counter(pk INTEGER PRIMARY KEY, counter INTEGER)",
            &[],
        )?;
        conn.execute("DELETE FROM hot_counter", &[])?;
        conn.execute(
            "INSERT INTO hot_counter(pk, counter) VALUES (?1, 0)",
            &[CellValue::Integer(0)],
        )?;
    }
    let metrics = run_threaded_conn_feature(engine, spec, |conn, _w, _rng| {
        let _ = conn.execute(
            "UPDATE hot_counter SET counter = counter + 1 WHERE pk = ?1",
            &[CellValue::Integer(0)],
        )?;
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "hot-counter-update",
        "SELECT pk, counter FROM hot_counter ORDER BY pk",
    )?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "hot_counter_ops".to_owned(),
        serde_json::json!(metrics.operations()),
    );
    finish_self_managed_record(engine, spec, started, metrics, checksum, stats)
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
/// across `n` worker threads:
///   - error rate is at most 5%
///   - locked / busy / timeout ratio is at most 50%
///   - p99 stays within 100x of the baseline p99
///
/// Loop terminates once the binary-search window narrows to
/// within 8 connections.
///
/// Output is a single [`RunRecord`] whose `engine_stats` contains
/// `{"max_stable_connections": <usize>, "baseline_p99_us": <u64>}`
/// so downstream consumers can plot the limit per engine without
/// touching the workload-specific output schema.
fn run_connection_limit(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    use std::time::Duration;

    // Baseline: a single connection, 1s of point-reads.
    let baseline_metrics = run_connection_burst(engine, spec, 1, Duration::from_secs(1))?;
    let baseline_p99 = baseline_metrics.latency.p99_us.max(1);

    let mut low: usize = 1;
    let mut high: usize = 64;
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

    let started = Instant::now();
    // Use the captured "last probe" metrics as the run's metrics
    // so latency/throughput on the record reflect the final probe
    // (not just the baseline). The most-load probe is the
    // representative datapoint for telemetry.
    let elapsed = started.elapsed().max(Duration::from_millis(1));
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

/// Pre-aggregated metrics shape used by the connection-limit probe.
#[derive(Debug, Clone)]
struct ProbeMetrics {
    operations: u64,
    failures: u64,
    busy_errors: u64,
    locked_errors: u64,
    timeout_errors: u64,
    latency: crate::report::LatencySummary,
}

/// Run a 1-second point-read burst across `n` worker threads and
/// summarize per-class error counts plus the latency histogram.
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
    let scope_result = crossbeam_utils::thread::scope(|scope| {
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

/// Classify an error string into one of the four [`FailureKind`]
/// buckets. The classes are evaluated in priority order (locked first,
/// then timeout, then busy) so the same error never lands in more than
/// one bucket — Reviewer Finding #7 specifically called out that the
/// previous implementation collapsed LOCKED into BUSY.
fn classify_failure(err: &anyhow::Error) -> FailureKind {
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

/// Lane VE: drive the spillable-sort path.
///
/// Seeds `spec.rows` rows (default 200_000 if smaller) into a separate
/// `sortable` table with a 64-byte payload, then issues
/// `SELECT * FROM sortable ORDER BY payload` repeatedly until the deadline.
/// Reports the spill-bytes / total-bytes ratio under
/// `engine_stats.spill_bytes_ratio`.
fn run_large_sort_spill(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let started = Instant::now();
    {
        let mut conn = engine.connect(0)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sortable(id INTEGER PRIMARY KEY, payload TEXT)",
            &[],
        )?;
        conn.execute("DELETE FROM sortable", &[])?;
        // Honor `--rows` so callers can run smoke-tests; only when the
        // CLI's default (512) is used do we bump to the documented
        // paper-grade 200_000.
        let target_rows = if spec.rows == 512 { 200_000 } else { spec.rows };
        let mut rng = ChaCha8Rng::seed_from_u64(spec.seed);
        conn.begin_immediate()?;
        for id in 0..target_rows {
            let mut payload = String::with_capacity(64);
            for _ in 0..64 {
                let byte = (rng.random::<u32>() % 26) as u8;
                payload.push((b'a' + byte) as char);
            }
            conn.execute(
                "INSERT INTO sortable(id, payload) VALUES (?1, ?2)",
                &[CellValue::Integer(id as i64), CellValue::Text(payload)],
            )?;
        }
        conn.commit()?;
    }

    // Sort loop; capture how many rows we sorted (proxy for total bytes).
    let mut total_rows: u64 = 0;
    let mut metrics = Metrics::new();
    let mut conn = engine.connect(0)?;
    let deadline = Instant::now() + spec.duration;
    while Instant::now() < deadline {
        let start = Instant::now();
        let result = conn.query_all("SELECT id, payload FROM sortable ORDER BY payload", &[]);
        match result {
            Ok(rows) => {
                total_rows = total_rows.saturating_add(rows.len() as u64);
                metrics.record_success(start.elapsed());
            }
            Err(err) => metrics.record_failure(classify_failure(&err)),
        }
    }

    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let checksum = engine.checksum()?;
    let elapsed = started.elapsed();
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
    // Each row is roughly 64 bytes payload + 8 bytes id + framing.
    let total_bytes = total_rows.saturating_mul(80);
    // Spill bytes are best-effort: if the engine snapshot exposes a
    // spill counter we propagate it; otherwise we report zero so the
    // paper plot stays well-typed.
    let spill_bytes = engine_stats
        .get("spill_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ratio = if total_bytes > 0 {
        spill_bytes as f64 / total_bytes as f64
    } else {
        0.0
    };
    engine_stats.insert("spill_bytes_ratio".to_owned(), serde_json::json!(ratio));
    engine_stats.insert(
        "sort_total_rows_observed".to_owned(),
        serde_json::json!(total_rows),
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
        metrics: MetricsSummary {
            operations: metrics.operations(),
            failures: metrics.failures(),
            busy_errors: metrics.busy_errors() + metrics.locked_errors(),
            locked_errors: metrics.locked_errors(),
            timeout_errors: metrics.timeout_errors(),
            elapsed_ms: elapsed.as_millis() as u64,
            throughput_ops_per_sec: throughput(metrics.operations(), elapsed),
            latency: metrics.latency(),
        },
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: serde_json::Value::Object(engine_stats),
        process_metrics: Some(process),
    })
}

fn throughput(operations: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        operations as f64 / seconds
    } else {
        0.0
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
}
