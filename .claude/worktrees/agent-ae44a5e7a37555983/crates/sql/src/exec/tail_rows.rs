use std::sync::Arc;

use super::*;

pub(crate) fn collect_table_rows(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
) -> Result<Vec<TableRow>> {
    collect_table_rows_with_alias(engine, tx, table, None)
}

pub(crate) fn collect_table_rows_with_alias(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    alias: Option<Arc<str>>,
) -> Result<Vec<TableRow>> {
    // CTE-backed table: short-circuit to the pre-materialized row store.
    if crate::exec::cte::is_cte_table_def(table) {
        if let Some(rows) = crate::exec::cte::rows_for_relation(table.relation_id) {
            return Ok(rows
                .iter()
                .enumerate()
                .map(|(idx, values)| TableRow {
                    rowid: redlinedb_kernel::format::RowId(idx as u64 + 1),
                    values: values.clone(),
                    table: Arc::clone(table),
                    alias: alias.clone(),
                })
                .collect());
        }
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    let rowids = engine.relation_rowids(table.relation_id)?;
    for rowid in rowids {
        if let Some(row) = load_table_row_by_rowid(engine, tx, table, rowid)? {
            let mut row = row;
            row.alias = alias.clone();
            rows.push(row);
        }
    }
    Ok(rows)
}

pub(crate) fn collect_join_rows(
    engine: &Engine,
    tx: &mut Txn,
    tables: &[crate::statement::BoundTable],
) -> Result<Vec<SqlRow>> {
    let mut joined: Vec<Vec<JoinedRow>> = vec![Vec::new()];
    for table in tables {
        let rows = collect_table_rows_with_alias(engine, tx, &table.table, table.alias.clone())?;
        let mut next = Vec::new();
        for prefix in &joined {
            for row in &rows {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(table, Some(row.clone())));
                next.push(combined);
            }
        }
        joined = next;
    }
    Ok(joined.into_iter().map(SqlRow::Joined).collect())
}

pub(crate) fn collect_join_source_rows(
    engine: &Engine,
    tx: &mut Txn,
    source: &crate::statement::JoinSource,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlRow>> {
    let base_rows =
        collect_table_rows_with_alias(engine, tx, &source.base.table, source.base.alias.clone())?;
    let mut joined: Vec<Vec<JoinedRow>> = base_rows
        .into_iter()
        .map(|row| vec![joined_row_from_table_row(&source.base, Some(row))])
        .collect();

    for step in &source.joins {
        let right_rows =
            collect_table_rows_with_alias(engine, tx, &step.right.table, step.right.alias.clone())?;
        let mut next = Vec::new();
        for prefix in &joined {
            let mut matched = false;
            for row in &right_rows {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(&step.right, Some(row.clone())));
                if selection_passes(&step.selection, &SqlRow::Joined(combined.clone()), bindings)? {
                    matched = true;
                    next.push(combined);
                }
            }
            if !matched && matches!(step.kind, crate::statement::JoinKind::Left) {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(&step.right, None));
                next.push(combined);
            }
        }
        joined = next;
    }

    Ok(joined.into_iter().map(SqlRow::Joined).collect())
}

fn joined_row_from_table_row(
    table: &crate::statement::BoundTable,
    row: Option<TableRow>,
) -> JoinedRow {
    let alias = table.alias.clone();
    let row = row.map(|mut row| {
        row.alias = alias.clone();
        row
    });
    JoinedRow {
        table: Arc::clone(&table.table),
        alias,
        row,
    }
}

pub(crate) fn collect_table_rowids(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
) -> Result<Vec<RowId>> {
    if crate::exec::cte::is_cte_table_def(table) {
        if let Some(rows) = crate::exec::cte::rows_for_relation(table.relation_id) {
            return Ok((1..=rows.len()).map(|i| RowId(i as u64)).collect());
        }
        return Ok(Vec::new());
    }
    let mut rowids = Vec::new();
    let scan = engine.relation_rowids(table.relation_id)?;
    for rowid in scan {
        if load_table_row_by_rowid(engine, tx, table, rowid)?.is_some() {
            rowids.push(rowid);
        }
    }
    Ok(rowids)
}

pub(crate) fn load_table_row_by_rowid(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    rowid: RowId,
) -> Result<Option<TableRow>> {
    if crate::exec::cte::is_cte_table_def(table) {
        if let Some(rows) = crate::exec::cte::rows_for_relation(table.relation_id) {
            let idx = match (rowid.0 as usize).checked_sub(1) {
                Some(idx) => idx,
                None => return Ok(None),
            };
            if let Some(values) = rows.get(idx) {
                return Ok(Some(TableRow {
                    rowid,
                    values: values.clone(),
                    table: Arc::clone(table),
                    alias: None,
                }));
            }
        }
        return Ok(None);
    }
    if let Some(payload) = engine.get_for_relation(tx, table.relation_id, rowid)?
        && let Some((table_id, values)) = decode_sql_row(&payload)?
        && table_id == table.table_id.0
    {
        let mut values = values;
        if values.len() < table.columns.len() {
            values.resize(table.columns.len(), SqlValue::Null);
            values = build_default_values(table, values)?;
        }
        return Ok(Some(TableRow {
            rowid,
            values,
            table: Arc::clone(table),
            alias: None,
        }));
    }
    Ok(None)
}

/// Executor-side wrapper around the planner's shared `selection_rowid_eq`
/// detector. Uses `eval_scalar` so runtime evaluation errors are
/// surfaced instead of being silently treated as `NULL` (which is what
/// the planner's conservative path does at planning time).
pub(crate) fn selection_rowid_eq(
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Option<RowId>> {
    crate::planner::helpers::selection_rowid_eq_with(
        table,
        selection,
        bindings,
        |expr, bindings| eval_scalar(expr, &RowContext::Empty, bindings),
    )
}
