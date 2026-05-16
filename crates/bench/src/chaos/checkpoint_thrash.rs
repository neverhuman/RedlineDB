//! `chaos-checkpoint-thrash` workload: interleave reads and writes with
//! aggressive checkpoint calls from worker zero. Stresses the WAL+checkpoint
//! interaction and surfaces the per-loop counters as part of the run record.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use rand::Rng;

use crate::config::RunSpec;
use crate::engine::{BenchEngine, CellValue};
use crate::report::RunRecord;

use super::helpers::{
    blob_for, chaos_stats, checksum_query, finish_chaos_txn, pk_payload_seed_row,
    record_chaos_counters, run_chaos_workload, run_threaded_conn_feature, seed_rows,
};

pub(crate) fn run_checkpoint_thrash(
    engine: &dyn BenchEngine,
    spec: &RunSpec,
) -> Result<RunRecord> {
    struct Counters {
        checkpoint_calls: AtomicU64,
        reads: AtomicU64,
        writes: AtomicU64,
    }
    let busy_timeout_ms = 15_u64;
    run_chaos_workload(
        engine,
        spec,
        |engine, spec| {
            seed_rows(
                engine,
                "CREATE TABLE IF NOT EXISTS dick_head_choas_checkpoint_thrash(pk INTEGER PRIMARY KEY, v INTEGER, payload BLOB)",
                "DELETE FROM dick_head_choas_checkpoint_thrash",
                "INSERT INTO dick_head_choas_checkpoint_thrash(pk, v, payload) VALUES (?1, 0, ?2)",
                spec.rows.max(1),
                pk_payload_seed_row,
            )?;
            Ok(Counters {
                checkpoint_calls: AtomicU64::new(0),
                reads: AtomicU64::new(0),
                writes: AtomicU64::new(0),
            })
        },
        |engine, spec, counters| {
            run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
                conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
                if worker == 0 && rng.random_range(0..4) != 0 {
                    counters.checkpoint_calls.fetch_add(1, Ordering::Relaxed);
                    engine.checkpoint()?;
                    return Ok(());
                }
                if rng.random_range(0..100) < 55 {
                    counters.reads.fetch_add(1, Ordering::Relaxed);
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    let _ = conn.query_row(
                        "SELECT COUNT(*) FROM dick_head_choas_checkpoint_thrash WHERE pk BETWEEN ?1 AND ?2",
                        &[
                            CellValue::Integer(key),
                            CellValue::Integer((key + 32).min(spec.rows.max(1) as i64)),
                        ],
                    )?;
                } else {
                    counters.writes.fetch_add(1, Ordering::Relaxed);
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
                    finish_chaos_txn(conn, rng, 5, None, None)?;
                }
                Ok(())
            })
        },
        |engine, _| {
            checksum_query(
                engine,
                "dick-head-choas-checkpoint-thrash",
                "SELECT pk, v, payload FROM dick_head_choas_checkpoint_thrash ORDER BY pk",
            )
        },
        |counters| {
            let mut stats = chaos_stats(
                "bounded",
                "checkpoint-thrash",
                spec.rows.max(1),
                busy_timeout_ms,
            );
            record_chaos_counters(
                &mut stats,
                &[
                    ("checkpoint_calls", &counters.checkpoint_calls),
                    ("reads", &counters.reads),
                    ("writes", &counters.writes),
                ],
            );
            stats
        },
    )
}
