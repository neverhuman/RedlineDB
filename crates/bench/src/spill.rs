use std::time::Instant;

use anyhow::Result;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::*;

/// Lane VE: drive the spillable-sort path.
///
/// Seeds `spec.rows` rows (default 200_000 if smaller) into a separate
/// `sortable` table with a 64-byte payload, then issues
/// `SELECT * FROM sortable ORDER BY payload` repeatedly until the deadline.
/// Reports the spill-bytes / total-bytes ratio under
/// `engine_stats.spill_bytes_ratio`.
pub(super) fn run_large_sort_spill(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    {
        let mut conn = engine.connect(0)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sortable(id INTEGER PRIMARY KEY, payload TEXT)",
            &[],
        )?;
        conn.execute("DELETE FROM sortable", &[])?;
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

    let measured_started = Instant::now();
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
    let measured_elapsed = measured_started.elapsed();

    engine.checkpoint()?;
    let snapshot = engine.snapshot()?;
    let checksum = engine.checksum()?;
    let wall_elapsed = wall_started.elapsed();
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
    let total_bytes = total_rows.saturating_mul(80);
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
        metrics: metrics_summary(&metrics, measured_elapsed),
        checksum,
        data_bytes: snapshot.data_bytes,
        wal_bytes: snapshot.wal_bytes,
        engine_stats: serde_json::Value::Object({
            engine_stats.insert(
                "wall_elapsed_ms".to_owned(),
                serde_json::json!(wall_elapsed.as_millis() as u64),
            );
            engine_stats
        }),
        process_metrics: Some(process),
    })
}
