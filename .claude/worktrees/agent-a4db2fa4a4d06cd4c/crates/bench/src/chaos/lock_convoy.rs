//! `chaos-lock-convoy` workload: a tight write loop on a single hot row.
//!
//! Every worker thread targets `pk = 0` under a short busy-timeout, so the
//! engine has to fall back on lock/queue-fairness behaviour. The workload
//! reports commit/rollback splits as part of the run record.

use std::sync::atomic::AtomicU64;
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

pub(crate) fn run_lock_convoy(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    struct Counters {
        commits: AtomicU64,
        rollbacks: AtomicU64,
    }
    let busy_timeout_ms = 25_u64;
    run_chaos_workload(
        engine,
        spec,
        |engine, spec| {
            seed_rows(
                engine,
                "CREATE TABLE IF NOT EXISTS dick_head_choas_lock_convoy(pk INTEGER PRIMARY KEY, v INTEGER, payload BLOB)",
                "DELETE FROM dick_head_choas_lock_convoy",
                "INSERT INTO dick_head_choas_lock_convoy(pk, v, payload) VALUES (?1, 0, ?2)",
                spec.rows.max(1),
                pk_payload_seed_row,
            )?;
            Ok(Counters {
                commits: AtomicU64::new(0),
                rollbacks: AtomicU64::new(0),
            })
        },
        |engine, spec, counters| {
            run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
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
                finish_chaos_txn(
                    conn,
                    rng,
                    8,
                    Some(&counters.commits),
                    Some(&counters.rollbacks),
                )?;
                Ok(())
            })
        },
        |engine, _| {
            checksum_query(
                engine,
                "dick-head-choas-lock-convoy",
                "SELECT pk, v, payload FROM dick_head_choas_lock_convoy ORDER BY pk",
            )
        },
        |counters| {
            let mut stats = chaos_stats("bounded", "lock-convoy", spec.rows.max(1), busy_timeout_ms);
            record_chaos_counters(
                &mut stats,
                &[
                    ("commits", &counters.commits),
                    ("rollbacks", &counters.rollbacks),
                ],
            );
            stats
        },
    )
}
