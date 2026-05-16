//! `chaos-schema-storm` workload: hammer the catalog with `CREATE TABLE` /
//! `CREATE INDEX` calls across all workers. Reports a single DDL counter.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use rand::Rng;

use crate::config::RunSpec;
use crate::engine::{BenchEngine, CellValue};
use crate::report::RunRecord;

use super::helpers::{
    chaos_stats, checksum_from_rows, record_chaos_counters, run_chaos_workload,
    run_threaded_engine_feature,
};

pub(crate) fn run_schema_storm(engine: &dyn BenchEngine, spec: &RunSpec) -> Result<RunRecord> {
    let busy_timeout_ms = 5_u64;
    run_chaos_workload(
        engine,
        spec,
        |_engine, _spec| Ok(AtomicU64::new(0)),
        |engine, spec, ddl_count| {
            run_threaded_engine_feature(engine, spec, |engine, worker, rng| {
                let mut conn = engine.connect(worker)?;
                conn.set_busy_timeout(Duration::from_millis(busy_timeout_ms))?;
                let suffix = (worker * 1024 + rng.random_range(0..256)) as u64;
                ddl_count.fetch_add(1, Ordering::Relaxed);
                conn.execute(
                    &format!(
                        "CREATE TABLE IF NOT EXISTS dick_head_choas_schema_{suffix}(id INTEGER PRIMARY KEY, v BLOB)"
                    ),
                    &[],
                )?;
                if rng.random_range(0..2) == 0 {
                    conn.execute(
                        &format!("CREATE INDEX IF NOT EXISTS dick_head_choas_schema_idx_{suffix} ON dick_head_choas_schema_{suffix}(id)"),
                        &[],
                    )?;
                }
                Ok(())
            })
        },
        |_engine, ddl_count| {
            Ok(checksum_from_rows(
                "dick-head-choas-schema-storm",
                &[vec![
                    CellValue::Integer(0),
                    CellValue::Integer(ddl_count.load(Ordering::Relaxed) as i64),
                ]],
            ))
        },
        |ddl_count| {
            let mut stats = chaos_stats("extreme", "schema-storm", 0, busy_timeout_ms);
            record_chaos_counters(&mut stats, &[("ddl_ops", ddl_count)]);
            stats
        },
    )
}
