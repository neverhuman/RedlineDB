use std::sync::Arc;
use std::time::Instant;

use super::*;

#[path = "tail_stats/helpers.rs"]
mod helpers;

use helpers::*;

pub(crate) fn analyze_database(conn: &Connection, plan: &AnalyzePlan) -> Result<()> {
    let schema = conn.engine().schema_snapshot();
    let mut tx = conn.engine().begin(Isolation::Snapshot)?;
    let result = (|| -> Result<()> {
        let current = conn.stats_snapshot();
        let mut next = StatsSnapshot::empty(StatsEpoch(current.epoch.0.saturating_add(1)));
        next.tables = current.tables.clone();
        next.columns = current.columns.clone();
        next.indexes = current.indexes.clone();

        let tables = match &plan.table {
            Some(table) => vec![Arc::clone(table)],
            None => schema.tables.to_vec(),
        };

        for table in tables {
            let rows = super::collect_table_rows(conn.engine(), &mut tx, &table)?
                .into_iter()
                .map(|row| row.values)
                .collect::<Vec<_>>();
            let table_stats = build_table_stats(conn, &table, &rows)?;
            next.tables.insert(table.table_id, table_stats);

            let sample = sample_rows(conn.stats_config(), &rows);
            for (ordinal, column) in table.columns.iter().enumerate() {
                let stats = build_column_stats(conn.stats_config(), &sample, ordinal);
                next.columns
                    .insert((table.table_id, column.column_id), stats);
            }
            for index in &table.indexes {
                let stats = build_index_stats(conn.stats_config(), &sample, index);
                next.indexes.insert(index.index_id, stats);
            }
        }

        conn.publish_stats(Arc::new(next))
    })();
    let _ = conn.engine().rollback(tx);
    result
}

pub(crate) fn execute_explain(
    conn: &Connection,
    plan: &ExplainPlan,
    bindings: &[Option<SqlValue>],
) -> Result<SelectRuntime> {
    let temp_dir = conn.temp_dir().map(|path| path.to_path_buf());
    let rows = if plan.analyze {
        let start = Instant::now();
        let mut result = execute_prepared(conn, &plan.inner, bindings)?;
        let mut actual_rows = result.affected_rows;
        let mut loops = 0usize;
        if let RuntimeState::Select(runtime) = &mut result.runtime {
            let mut current_row = None;
            loop {
                loops += 1;
                if step_select_runtime(conn, runtime, bindings, &mut current_row)? {
                    break;
                }
                actual_rows += 1;
            }
        }
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (peak_memory_bytes, spill_bytes) = match &result.runtime {
            RuntimeState::Select(runtime) => {
                (runtime.memory.used_bytes, runtime.memory.spilled_bytes)
            }
            RuntimeState::Done | RuntimeState::Idle => (0, 0),
        };
        planner::explain_rows(
            conn,
            &plan.inner.kind,
            bindings,
            Some(ExplainMetrics {
                actual_rows: Some(actual_rows),
                loops: Some(loops),
                elapsed_ms: Some(elapsed_ms),
                peak_memory_bytes: Some(peak_memory_bytes),
                spill_bytes: Some(spill_bytes),
            }),
            plan.format,
        )
    } else {
        planner::explain_rows(conn, &plan.inner.kind, bindings, None, plan.format)
    };

    Ok(SelectRuntime {
        tx: SelectRuntimeTx::Empty,
        restore_tx: false,
        source: SelectRuntimeSource::Batched {
            node: MaterializeNode::new(rows),
            ctx: ExecContext::new(
                conn.query_memory().work_mem_bytes,
                conn.query_memory().max_spill_bytes,
                temp_dir.clone(),
            ),
            batch: RowBatch::new(Arc::new(RowLayout {
                columns: Arc::from([]),
            })),
            cursor: 0,
        },
        selection: None,
        projection: Vec::new(),
        limit: usize::MAX,
        offset: 0,
        seen: 0,
        yielded: 0,
        memory: QueryMemoryBroker::new(
            conn.query_memory().work_mem_bytes,
            conn.query_memory().max_spill_bytes,
            temp_dir.clone(),
        ),
    })
}
