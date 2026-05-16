//! `chaos-sort-spill-convoy` workload: sort-heavy reads with periodic writes
//! to provoke spill paths. Reports the spill-query count and the write count
//! so the certification consumer can pin both arms of the workload.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use rand::Rng;

use crate::config::RunSpec;
use crate::engine::{BenchEngine, CellValue};
use crate::report::RunRecord;

use super::helpers::{
    chaos_stats, checksum_query, finish_chaos_txn, large_blob, record_chaos_counters,
    run_chaos_workload, run_threaded_conn_feature, seed_rows,
};

pub(crate) fn run_sort_spill_convoy(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    struct Counters {
        spill_queries: AtomicU64,
        writes: AtomicU64,
    }
    let busy_timeout_ms = 20_u64;
    run_chaos_workload(
        engine,
        spec,
        |engine, spec| {
            seed_rows(
                engine,
                "CREATE TABLE IF NOT EXISTS dick_head_choas_sort_spill_convoy(pk INTEGER PRIMARY KEY, score INTEGER, payload BLOB, version INTEGER)",
                "DELETE FROM dick_head_choas_sort_spill_convoy",
                "INSERT INTO dick_head_choas_sort_spill_convoy(pk, score, payload, version) VALUES (?1, ?2, ?3, 1)",
                spec.rows.max(1),
                |idx| {
                    vec![
                        CellValue::Integer(idx as i64),
                        CellValue::Integer((idx % 1024) as i64),
                        CellValue::Blob(large_blob(idx)),
                    ]
                },
            )?;
            Ok(Counters {
                spill_queries: AtomicU64::new(0),
                writes: AtomicU64::new(0),
            })
        },
        |engine, spec, counters| {
            run_threaded_conn_feature(engine, spec, |conn, worker, rng| {
                conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
                if rng.random_range(0..100) < 60 {
                    counters.spill_queries.fetch_add(1, Ordering::Relaxed);
                    let threshold = rng.random_range(0..1024) as i64;
                    let _ = conn.query_all(
                        "SELECT pk, score FROM dick_head_choas_sort_spill_convoy WHERE score >= ?1 ORDER BY payload DESC LIMIT 128",
                        &[CellValue::Integer(threshold)],
                    )?;
                } else {
                    counters.writes.fetch_add(1, Ordering::Relaxed);
                    let key = rng.random_range(0..spec.rows.max(1)) as i64;
                    conn.begin_immediate()?;
                    conn.execute(
                        "UPDATE dick_head_choas_sort_spill_convoy SET score = score + 1, payload = ?1, version = version + 1 WHERE pk = ?2",
                        &[
                            CellValue::Blob(large_blob(
                                (worker << 14) ^ key as usize ^ rng.random::<u32>() as usize,
                            )),
                            CellValue::Integer(key),
                        ],
                    )?;
                    finish_chaos_txn(conn, rng, 4, None, None)?;
                }
                Ok(())
            })
        },
        |engine, _| {
            checksum_query(
                engine,
                "dick-head-choas-sort-spill-convoy",
                "SELECT pk, score, payload, version FROM dick_head_choas_sort_spill_convoy ORDER BY pk",
            )
        },
        |counters| {
            let mut stats = chaos_stats(
                "bounded",
                "sort-spill-convoy",
                spec.rows.max(1),
                busy_timeout_ms,
            );
            record_chaos_counters(
                &mut stats,
                &[
                    ("spill_queries", &counters.spill_queries),
                    ("writes", &counters.writes),
                ],
            );
            stats
        },
    )
}
