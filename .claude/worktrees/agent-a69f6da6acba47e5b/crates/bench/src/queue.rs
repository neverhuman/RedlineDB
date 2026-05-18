use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

use anyhow::Result;
use rand::Rng;

use super::feature_workloads::{finish_self_managed_record, run_threaded_conn_feature};
use super::*;

pub(super) fn run_queue_mixed(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let wall_started = Instant::now();
    {
        let mut conn = engine.connect(0)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS queue_jobs(id INTEGER PRIMARY KEY, state INTEGER, priority INTEGER, created_at INTEGER, payload BLOB, attempts INTEGER)",
            &[],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS queue_state_idx ON queue_jobs(state)",
            &[],
        )?;
        conn.execute("DELETE FROM queue_jobs", &[])?;
        conn.begin_immediate()?;
        for id in 0..spec.rows.max(1) {
            let priority = (id % 11) as i64;
            let created_at = id as i64;
            conn.execute(
                "INSERT INTO queue_jobs(id, state, priority, created_at, payload, attempts) VALUES (?1, 0, ?2, ?3, ?4, 0)",
                &[
                    CellValue::Integer(id as i64),
                    CellValue::Integer(priority),
                    CellValue::Integer(created_at),
                    CellValue::Blob(blob_for(id)),
                ],
            )?;
        }
        conn.commit()?;
    }

    let next_job_id = AtomicI64::new(1_000_000_000);
    let measured = run_threaded_conn_feature(engine, spec, |conn, _worker, rng| {
        let choice = rng.random_range(0..100);
        if choice < 35 {
            let id = next_job_id.fetch_add(1, Ordering::Relaxed);
            let priority = id % 11;
            let created_at = id;
            let _ = conn.execute(
                "INSERT INTO queue_jobs(id, state, priority, created_at, payload, attempts) VALUES (?1, 0, ?2, ?3, ?4, 0)",
                &[
                    CellValue::Integer(id),
                    CellValue::Integer(priority),
                    CellValue::Integer(created_at),
                    CellValue::Blob(blob_for(id as usize)),
                ],
            )?;
        } else if choice < 85 {
            if let Some(id) = queue_claim_one(conn)? {
                let _ = conn.execute(
                    "UPDATE queue_jobs SET state = 2 WHERE id = ?1 AND state = 1",
                    &[CellValue::Integer(id)],
                )?;
            }
        } else {
            let _ = conn.query_row("SELECT COUNT(*) FROM queue_jobs WHERE state = 0", &[])?;
        }
        Ok(())
    })?;
    let checksum = checksum_query(
        engine,
        "queue-mixed",
        "SELECT id, state, priority, created_at, attempts FROM queue_jobs ORDER BY id",
    )?;
    let mut stats = BTreeMap::new();
    stats.insert(
        "queue_mixed_ops".to_owned(),
        serde_json::json!(measured.metrics.operations()),
    );
    stats.insert(
        "queue_seed_rows".to_owned(),
        serde_json::json!(spec.rows.max(1)),
    );
    stats.insert("queue_producer_pct".to_owned(), serde_json::json!(35));
    stats.insert("queue_consumer_pct".to_owned(), serde_json::json!(50));
    stats.insert("queue_reader_pct".to_owned(), serde_json::json!(15));
    finish_self_managed_record(engine, spec, measured, checksum, stats, wall_started)
}

fn queue_claim_one(conn: &mut dyn BenchConn) -> Result<Option<i64>> {
    loop {
        match queue_claim_one_once(conn) {
            Ok(result) => return Ok(result),
            Err(err) if is_retryable_queue_claim_error(&err) => {
                let _ = conn.rollback();
                std::thread::yield_now();
                continue;
            }
            Err(err) => return Err(err),
        }
    }
}

fn queue_claim_one_once(conn: &mut dyn BenchConn) -> Result<Option<i64>> {
    conn.begin_immediate()?;
    let outcome = (|| -> Result<Option<i64>> {
        let row = conn.query_row(
            "SELECT id \
             FROM queue_jobs \
             WHERE state = 0 \
             ORDER BY priority DESC, created_at ASC, id ASC \
             LIMIT 1",
            &[],
        )?;
        let Some(CellValue::Integer(id)) = row.first() else {
            return Ok(None);
        };
        let _ = conn.execute(
            "UPDATE queue_jobs SET state = 1, attempts = attempts + 1 WHERE id = ?1 AND state = 0",
            &[CellValue::Integer(*id)],
        )?;
        Ok(Some(*id))
    })();
    match outcome {
        Ok(result) => {
            conn.commit()?;
            Ok(result)
        }
        Err(err) => {
            let _ = conn.rollback();
            Err(err)
        }
    }
}

fn is_retryable_queue_claim_error(err: &anyhow::Error) -> bool {
    err.to_string()
        .to_ascii_lowercase()
        .contains("serialization failure")
}
