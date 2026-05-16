//! `chaos-connection-churn` workload: open/close connections under load.
//!
//! Each worker iteration opens a fresh per-worker connection, performs a
//! read or write, and drops it. We report the open count along with the
//! standard chaos counter shape so the certification consumer can see how
//! often the engine paid for connection setup.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use rand::Rng;

use crate::config::RunSpec;
use crate::engine::{BenchEngine, CellValue};
use crate::report::RunRecord;

use super::helpers::{
    ChaosCounters, ChaosOp, blob_for, chaos_stats, checksum_query, record_chaos_counters,
    run_chaos_workload, run_chaos_write, run_threaded_engine_feature, seed_rows, tenant_seed_row,
};

pub(crate) fn run_connection_churn(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    struct Counters {
        connection_opens: AtomicU64,
        ops: ChaosCounters,
    }
    let busy_timeout_ms = 10_u64;
    run_chaos_workload(
        engine,
        spec,
        |engine, spec| {
            seed_rows(
                engine,
                "CREATE TABLE IF NOT EXISTS dick_head_choas_connection_churn(pk INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER, payload BLOB)",
                "DELETE FROM dick_head_choas_connection_churn",
                "INSERT INTO dick_head_choas_connection_churn(pk, tenant, v, payload) VALUES (?1, ?2, 0, ?3)",
                spec.rows.max(1),
                tenant_seed_row,
            )?;
            Ok(Counters {
                connection_opens: AtomicU64::new(0),
                ops: ChaosCounters::default(),
            })
        },
        |engine, spec, counters| {
            run_threaded_engine_feature(engine, spec, |engine, worker, rng| {
                counters.connection_opens.fetch_add(1, Ordering::Relaxed);
                let mut conn = engine.connect(worker)?;
                conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
                if rng.random_range(0..100) < 70 {
                    counters.ops.reads.fetch_add(1, Ordering::Relaxed);
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    let _ = conn.query_row(
                        "SELECT v FROM dick_head_choas_connection_churn WHERE pk = ?1",
                        &[CellValue::Integer(key)],
                    )?;
                } else {
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    let payload_seed = (worker << 20) ^ key as usize ^ rng.random::<u32>() as usize;
                    run_chaos_write(
                        &mut *conn,
                        rng,
                        &counters.ops,
                        ChaosOp::Write,
                        "UPDATE dick_head_choas_connection_churn SET v = v + 1, payload = ?1 WHERE pk = ?2",
                        &[
                            CellValue::Blob(blob_for(payload_seed)),
                            CellValue::Integer(key),
                        ],
                        10,
                    )?;
                }
                Ok(())
            })
        },
        |engine, _| {
            checksum_query(
                engine,
                "dick-head-choas-connection-churn",
                "SELECT pk, tenant, v, payload FROM dick_head_choas_connection_churn ORDER BY pk",
            )
        },
        |counters| {
            let mut stats = chaos_stats(
                "bounded",
                "connection-churn",
                spec.rows.max(1),
                busy_timeout_ms,
            );
            record_chaos_counters(
                &mut stats,
                &[
                    ("connection_opens", &counters.connection_opens),
                    ("reads", &counters.ops.reads),
                    ("writes", &counters.ops.writes),
                    ("commits", &counters.ops.commits),
                    ("rollbacks", &counters.ops.rollbacks),
                ],
            );
            stats
        },
    )
}
