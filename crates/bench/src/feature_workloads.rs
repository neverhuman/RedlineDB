use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};
use std::time::Instant;

use anyhow::Result;
use crossbeam_utils::thread;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use redlinedb_kernel::format::{
    PageGeneration, PageId, RelId, RowId as KernelRowId, TuplePtr, TxId,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::vector::diskann::{DiskAnnIndex, DiskAnnParams, RowId as DiskAnnRowId};
use redlinedb_kernel::vector::hnsw::{HnswIndex, HnswParams, IndexedRowRef};

use super::*;
use crate::checksum::checksum_from_rows;

pub(super) fn run_json_path_extract(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    setup_json_docs(engine, spec)?;
    let measured = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
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
        serde_json::json!(measured.metrics.operations()),
    );
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_json_path_update(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    setup_json_docs(engine, spec)?;
    let measured = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
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
        serde_json::json!(measured.metrics.operations()),
    );
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_vector_flat_search(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    let wall_started = Instant::now();
    setup_vector_table(engine, spec)?;
    let measured = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
        let query = vector_for(rng.random::<u64>() as usize, VECTOR_DIM);
        let _ = vector_flat_top_k(conn, spec.engine, &query, 10)?;
        Ok(())
    })?;
    let checksum = vector_flat_checksum(engine, spec.engine)?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "vector_exact_topk_ops".to_owned(),
        serde_json::json!(measured.metrics.operations()),
    );
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_vector_ann_search(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    let rows = ann_rows(spec);
    let vectors = vector_dataset(rows, HNSW_DIM, spec.seed);
    let staging_dir = tempfile::TempDir::new()?;
    let page_file = Arc::new(PageFile::create(
        staging_dir.path().join("hnsw.redline"),
        512,
    )?);
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
    let measured = run_threaded_compute_feature(spec, {
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
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_vector_ann_search_disk(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    let wall_started = Instant::now();
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
    let staging_dir = tempfile::TempDir::new()?;
    let sector_path = staging_dir.path().join("diskann.sectors");
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
    let measured = run_threaded_compute_feature(spec, {
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
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_commit_storm_batched(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    let wall_started = Instant::now();
    setup_commit_storm(engine, spec)?;
    let measured = run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
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
        serde_json::json!(measured.metrics.operations()),
    );
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

pub(super) fn run_threaded_conn_feature<F>(
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

pub(super) fn run_threaded_compute_feature<F>(spec: &RunSpec, op: F) -> Result<MeasuredMetrics>
where
    F: Fn(usize, &mut ChaCha8Rng) -> Result<()> + Sync,
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
    Ok(MeasuredMetrics {
        metrics: merged,
        elapsed: started.elapsed(),
    })
}

pub(super) fn finish_self_managed_record(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
    measured: MeasuredMetrics,
    checksum: Checksum,
    extra_stats: BTreeMap<String, serde_json::Value>,
    wall_started: Instant,
) -> Result<RunRecord> {
    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
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
    let wall_elapsed = wall_started.elapsed();
    engine_stats.insert(
        "wall_elapsed_ms".to_owned(),
        serde_json::json!(wall_elapsed.as_millis() as u64),
    );
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
        metrics: metrics_summary(&measured.metrics, measured.elapsed),
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
