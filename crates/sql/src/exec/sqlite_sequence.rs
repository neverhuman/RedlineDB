use std::path::PathBuf;
use std::sync::Arc;

use super::*;

pub(super) fn rows(
    conn: &Connection,
    alias: Option<&Arc<str>>,
) -> Vec<crate::statement::SqliteSequenceRow> {
    super::with_session_reentrant(conn, |session| {
        let sequences = if session.tx.is_some() || session.sqlite_sequences_tx_snapshot.is_some() {
            session.sqlite_sequences.clone()
        } else {
            conn.committed_sqlite_sequences()
        };
        Ok(sequences
            .iter()
            .map(|(name, seq)| crate::statement::SqliteSequenceRow {
                name: Arc::from(name.as_str()),
                seq: *seq,
                alias: alias.cloned(),
            })
            .collect())
    })
    .unwrap_or_default()
}

pub(super) fn build_runtime(
    conn: &Connection,
    alias: Option<&Arc<str>>,
    plan: &SelectPlan,
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    temp_dir: Option<PathBuf>,
    memory: &mut QueryMemoryBroker,
) -> Result<SelectRuntimeSource> {
    let rows = rows(conn, alias);
    if plan.order_by.is_empty() {
        return Ok(SelectRuntimeSource::SqliteSequence { rows, cursor: 0 });
    }

    let sqlite_rows = rows
        .into_iter()
        .map(SqlRow::SqliteSequence)
        .collect::<Vec<_>>();
    Ok(SelectRuntimeSource::Batched {
        node: MaterializeNode::new(super::select_top::order_and_project_rows_with_distinct_on(
            sqlite_rows,
            &plan.selection,
            &plan.order_by,
            &plan.distinct_on,
            bindings,
            &plan.projection,
            limit,
            offset,
            memory,
        )?),
        ctx: ExecContext::new(
            conn.query_memory().work_mem_bytes,
            conn.query_memory().max_spill_bytes,
            temp_dir,
        ),
        batch: RowBatch::new(Arc::new(RowLayout {
            columns: Arc::from([]),
        })),
        cursor: 0,
    })
}

pub(super) fn collect_rows(conn: &Connection, alias: Option<&Arc<str>>) -> Vec<SqlRow> {
    rows(conn, alias)
        .into_iter()
        .map(SqlRow::SqliteSequence)
        .collect()
}
