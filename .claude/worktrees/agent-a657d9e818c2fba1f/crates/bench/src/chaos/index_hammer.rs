//! `chaos-index-hammer` workload: mixed read/update/delete traffic on a
//! tenant-indexed table. Surfaces read/write/delete/commit/rollback counters
//! so the certification consumer can tell which arm of the workload is hot.

use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use rand::Rng;

use crate::config::RunSpec;
use crate::engine::{BenchEngine, CellValue};
use crate::report::RunRecord;

use super::helpers::{
    ChaosCounters, ChaosOp, blob_for, chaos_stats, checksum_query, record_chaos_counters,
    run_chaos_workload, run_chaos_write, run_threaded_conn_feature, seed_rows, tenant_seed_row,
};

pub(crate) fn run_index_hammer(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let busy_timeout_ms = 20_u64;
    run_chaos_workload(
        engine,
        spec,
        |engine, spec| {
            seed_rows(
                engine,
                "CREATE TABLE IF NOT EXISTS dick_head_choas_index_hammer(pk INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
                "DELETE FROM dick_head_choas_index_hammer",
                "INSERT INTO dick_head_choas_index_hammer(pk, tenant, v, version) VALUES (?1, ?2, ?3, 1)",
                spec.rows.max(1),
                tenant_seed_row,
            )?;
            Ok(ChaosCounters::default())
        },
        |engine, spec, counters| {
            run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
                conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
                let choice = rng.random_range(0..100);
                if choice < 35 {
                    counters.reads.fetch_add(1, Ordering::Relaxed);
                    let low = rng.random_range(0..32) as i64;
                    let high = (low + 4).min(31);
                    let _ = conn.query_row(
                        "SELECT COUNT(*) FROM dick_head_choas_index_hammer WHERE tenant BETWEEN ?1 AND ?2",
                        &[CellValue::Integer(low), CellValue::Integer(high)],
                    )?;
                } else if choice < 70 {
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    let tenant = rng.random_range(0..32) as i64;
                    let payload_seed = (worker << 18) ^ key as usize ^ rng.random::<u32>() as usize;
                    run_chaos_write(
                        conn,
                        rng,
                        counters,
                        ChaosOp::Write,
                        "UPDATE dick_head_choas_index_hammer SET tenant = ?1, v = ?2, version = version + 1 WHERE pk = ?3",
                        &[
                            CellValue::Integer(tenant),
                            CellValue::Blob(blob_for(payload_seed)),
                            CellValue::Integer(key),
                        ],
                        7,
                    )?;
                } else {
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    run_chaos_write(
                        conn,
                        rng,
                        counters,
                        ChaosOp::Delete,
                        "DELETE FROM dick_head_choas_index_hammer WHERE pk = ?1",
                        &[CellValue::Integer(key)],
                        6,
                    )?;
                }
                Ok(())
            })
        },
        |engine, _| {
            checksum_query(
                engine,
                "dick-head-choas-index-hammer",
                "SELECT pk, tenant, v, version FROM dick_head_choas_index_hammer ORDER BY pk",
            )
        },
        |counters| {
            let mut stats =
                chaos_stats("bounded", "index-hammer", spec.rows.max(1), busy_timeout_ms);
            record_chaos_counters(
                &mut stats,
                &[
                    ("reads", &counters.reads),
                    ("writes", &counters.writes),
                    ("deletes", &counters.deletes),
                    ("commits", &counters.commits),
                    ("rollbacks", &counters.rollbacks),
                ],
            );
            stats
        },
    )
}
