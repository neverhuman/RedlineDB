use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::Result;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::*;

/// Phase 11 wave 1a: row-count target for the covering-range fixtures.
fn covered_range_rows(spec: &RunSpec) -> usize {
    spec.rows.max(4_096)
}

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

pub(super) fn run_covered_range_cold(spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
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
    }
    let measure_engine = engine::open(spec, &db_dir)?;
    let total = covered_range_rows(spec);
    let measured = super::feature_workloads::run_threaded_conn_feature(
        measure_engine.as_ref(),
        spec,
        |conn, _w, rng| covered_range_step(conn, total, rng),
    )?;
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
    super::feature_workloads::finish_self_managed_record(
        measure_engine.as_ref(),
        spec,
        measured,
        checksum,
        stats,
        wall_started,
    )
}

pub(super) fn run_covered_range_warm(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    let wall_started = Instant::now();
    setup_covered_kv(engine, spec)?;
    let total = covered_range_rows(spec);
    {
        let mut warmup_conn = engine.connect(0)?;
        let _ = warmup_conn.query_all(
            "SELECT k, v FROM covered_kv WHERE k BETWEEN ?1 AND ?2",
            &[CellValue::Integer(0), CellValue::Integer(total as i64)],
        )?;
    }
    let measured =
        super::feature_workloads::run_threaded_conn_feature(engine, spec, |conn, _w, rng| {
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
    super::feature_workloads::finish_self_managed_record(
        engine,
        spec,
        measured,
        checksum,
        stats,
        wall_started,
    )
}

pub(super) fn run_hot_counter_update(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    let wall_started = Instant::now();
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
    let measured =
        super::feature_workloads::run_threaded_conn_feature(engine, spec, |conn, _w, _rng| {
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
        serde_json::json!(measured.metrics.operations()),
    );
    super::feature_workloads::finish_self_managed_record(
        engine,
        spec,
        measured,
        checksum,
        stats,
        wall_started,
    )
}
