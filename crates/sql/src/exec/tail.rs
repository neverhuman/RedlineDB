#[path = "tail_conflict.rs"]
mod conflict;
#[path = "tail_rows.rs"]
mod rows;
#[path = "tail_stats.rs"]
mod stats;

pub(crate) use conflict::*;
pub(crate) use rows::*;
pub(crate) use stats::*;

use super::*;

pub(crate) fn execute_update(
    conn: &Connection,
    plan: &crate::statement::UpdatePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    with_write_tx(conn, |session, tx| {
        let target_rowids =
            if let Some(rowid) = selection_rowid_eq(&plan.table, &plan.selection, bindings)? {
                vec![rowid]
            } else {
                dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?
                    .into_iter()
                    .map(|row| row.rowid)
                    .collect()
            };
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        for rowid in target_rowids {
            // Lock before reloading/evaluating assignments. Autocommit
            // writes run at read-committed isolation, so expressions like
            // `version = version + 1` must be based on the latest tuple
            // after the hot-row handoff, not on the pre-lock probe row.
            conn.engine()
                .lock_row_for_relation(tx, plan.table.relation_id, rowid)?;
            let Some(fresh) = load_table_row_by_rowid(conn.engine(), tx, &plan.table, rowid)?
            else {
                continue;
            };
            if !selection_passes(&plan.selection, &SqlRow::Table(fresh.clone()), bindings)? {
                continue;
            }
            let old_values = fresh.values.clone();
            let mut values = fresh.values.clone();
            for (ordinal, expr) in &plan.assignments {
                if *ordinal >= values.len() {
                    return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
                }
                values[*ordinal] = eval_scalar(expr, &RowContext::Table(&fresh), bindings)?;
            }
            values = apply_row_affinity(&plan.table, values)?;
            let new_rowid =
                choose_rowid_for_update(conn.engine(), &plan.table, &values, fresh.rowid)?;
            if let Some(alias) = plan.table.rowid_alias_column
                && let Some(slot) = values.get_mut(alias as usize)
                && matches!(slot, SqlValue::Null)
            {
                *slot = SqlValue::Integer(new_rowid.0 as i64);
            }
            apply_constraints(&plan.table, &values)?;
            ensure_unique_constraints(conn, session, tx, &plan.table, &values, Some(fresh.rowid))?;
            let payload = encode_sql_row(plan.table.table_id.0, &values)?;
            if new_rowid == fresh.rowid {
                conn.engine().update_for_relation(
                    tx,
                    plan.table.relation_id,
                    fresh.rowid,
                    payload,
                )?;
            } else {
                conn.engine()
                    .delete_for_relation(tx, plan.table.relation_id, fresh.rowid)?;
                conn.engine().insert_for_relation(
                    tx,
                    plan.table.relation_id,
                    new_rowid,
                    payload,
                )?;
            }
            crate::exec::index_dml::maintain_indexes_on_update(
                conn.engine(),
                tx,
                &plan.table,
                &old_values,
                &values,
                fresh.rowid,
                new_rowid,
            )?;
            if let Some(returning) = &plan.returning {
                returning_rows.push(project_returning_row(
                    &plan.table,
                    &values,
                    new_rowid,
                    returning,
                    bindings,
                )?);
            }
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

pub(crate) fn execute_delete(
    conn: &Connection,
    plan: &crate::statement::DeletePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    with_write_tx(conn, |_session, tx| {
        let rows = dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?;
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        for row in rows {
            if !selection_passes(&plan.selection, &SqlRow::Table(row.clone()), bindings)? {
                continue;
            }
            if let Some(returning) = &plan.returning {
                returning_rows.push(project_returning_row(
                    &plan.table,
                    &row.values,
                    row.rowid,
                    returning,
                    bindings,
                )?);
            }
            // Reload the row to make sure we delete-mark the right index
            // entries; the heap state may have moved since plan time.
            let live = match load_table_row_by_rowid(conn.engine(), tx, &plan.table, row.rowid)?
                .map(|fresh| fresh.values)
            {
                Some(v) => v,
                None => row.values.clone(),
            };
            conn.engine()
                .delete_for_relation(tx, plan.table.relation_id, row.rowid)?;
            crate::exec::index_dml::maintain_indexes_on_delete(
                conn.engine(),
                tx,
                &plan.table,
                &live,
                row.rowid,
            )?;
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

fn dml_target_rows(
    conn: &Connection,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<TableRow>> {
    if let Some(rowid) = selection_rowid_eq(table, selection, bindings)?
        && let Some(row) = load_table_row_by_rowid(conn.engine(), tx, table, rowid)?
    {
        return Ok(vec![row]);
    }

    if let Some(matched) =
        crate::exec::index_access::try_match_index_access(conn.engine(), table, selection, bindings)
        && crate::exec::index_access::open_handle(conn.engine(), &matched.index).is_some()
    {
        let rowids = crate::exec::index_access::execute_index_probe(
            conn.engine(),
            tx,
            table,
            &matched.index,
            &matched.probe,
        )?;
        let mut rows = Vec::with_capacity(rowids.len());
        for rowid in rowids {
            if let Some(row) = load_table_row_by_rowid(conn.engine(), tx, table, rowid)? {
                rows.push(row);
            }
        }
        return Ok(rows);
    }

    collect_table_rows(conn.engine(), tx, table)
}

pub(super) fn project_returning_row(
    table: &Arc<TableDef>,
    values: &[SqlValue],
    rowid: RowId,
    returning: &[SelectItem],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let row = TableRow {
        rowid,
        values: values.to_vec(),
        table: Arc::clone(table),
        alias: None,
    };
    project_row(returning, &SqlRow::Table(row), bindings)
}

pub(super) fn build_dml_execution_result(
    affected_rows: usize,
    returning_rows: Vec<Vec<SqlValue>>,
    has_returning: bool,
) -> ExecutionResult {
    if has_returning {
        ExecutionResult {
            runtime: returning_rows.into_returning_runtime(),
            affected_rows,
        }
    } else {
        ExecutionResult {
            runtime: RuntimeState::Done,
            affected_rows,
        }
    }
}

trait ReturningRuntimeExt {
    fn into_returning_runtime(self) -> RuntimeState;
}

impl ReturningRuntimeExt for Vec<Vec<SqlValue>> {
    fn into_returning_runtime(self) -> RuntimeState {
        RuntimeState::Select(SelectRuntime {
            tx: SelectRuntimeTx::Empty,
            restore_tx: false,
            source: SelectRuntimeSource::StaticRows {
                rows: Arc::from(self),
                cursor: 0,
            },
            selection: None,
            projection: Vec::new(),
            limit: usize::MAX,
            offset: 0,
            seen: 0,
            yielded: 0,
            memory: QueryMemoryBroker::new(0, 0, None),
        })
    }
}

pub(crate) fn build_row(
    table: &Arc<TableDef>,
    row: &[Expr],
    columns: &[usize],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let mut values = vec![SqlValue::Null; table.columns.len()];
    let mut provided = vec![false; table.columns.len()];
    for (ordinal, expr) in columns.iter().copied().zip(row.iter()) {
        values[ordinal] = eval_scalar(expr, &RowContext::Empty, bindings)?;
        provided[ordinal] = true;
    }
    build_default_values_for_omitted(table, values, &provided)
}

pub(crate) fn build_row_from_values(
    table: &Arc<TableDef>,
    row: &[SqlValue],
    columns: &[usize],
) -> Result<Vec<SqlValue>> {
    let mut values = vec![SqlValue::Null; table.columns.len()];
    let mut provided = vec![false; table.columns.len()];
    for (ordinal, value) in columns.iter().copied().zip(row.iter()) {
        values[ordinal] = value.clone();
        provided[ordinal] = true;
    }
    build_default_values_for_omitted(table, values, &provided)
}

pub(crate) fn build_default_row(table: &Arc<TableDef>) -> Result<Vec<SqlValue>> {
    build_default_values(table, vec![SqlValue::Null; table.columns.len()])
}

pub(crate) fn build_default_values(
    table: &Arc<TableDef>,
    mut values: Vec<SqlValue>,
) -> Result<Vec<SqlValue>> {
    for (idx, column) in table.columns.iter().enumerate() {
        if matches!(values[idx], SqlValue::Null)
            && let Some(default) = &column.default_value
        {
            values[idx] = default.clone();
        }
    }
    apply_row_affinity(table, values)
}

fn build_default_values_for_omitted(
    table: &Arc<TableDef>,
    mut values: Vec<SqlValue>,
    provided: &[bool],
) -> Result<Vec<SqlValue>> {
    for (idx, column) in table.columns.iter().enumerate() {
        if !provided.get(idx).copied().unwrap_or(false)
            && matches!(values[idx], SqlValue::Null)
            && let Some(default) = &column.default_value
        {
            values[idx] = default.clone();
        }
    }
    apply_row_affinity(table, values)
}

pub(crate) fn apply_row_affinity(table: &TableDef, values: Vec<SqlValue>) -> Result<Vec<SqlValue>> {
    let mut out = values;
    for (idx, column) in table.columns.iter().enumerate() {
        out[idx] = apply_affinity(out[idx].clone(), column.affinity)
            .map_err(|_| Error::DatatypeMismatch)?;
    }
    Ok(out)
}

pub(crate) fn apply_constraints(table: &TableDef, values: &[SqlValue]) -> Result<()> {
    let mut scratch = EvalScratch::default();
    for (idx, column) in table.columns.iter().enumerate() {
        let value = match values.get(idx) {
            Some(v) => v,
            None => return Err(Error::UnknownColumn(column.name.to_string())),
        };
        if column.not_null && matches!(value, SqlValue::Null) {
            return Err(Error::ConstraintViolation(format!(
                "NOT NULL constraint failed: {}.{}",
                table.name, column.name
            )));
        }
    }

    for check in &table.checks {
        let row = TableRowSource { values };
        let result = eval_expr(&check.expr, &row, &mut scratch).map_err(|_| {
            Error::ConstraintViolation(format!("CHECK constraint failed: {}", table.name))
        })?;
        if matches!(result, SqlValue::Null) || is_truthy(&result) {
            continue;
        }
        return Err(Error::ConstraintViolation(format!(
            "CHECK constraint failed: {}",
            table.name
        )));
    }
    Ok(())
}

pub(crate) fn choose_rowid_for_insert(
    engine: &Engine,
    table: &TableDef,
    values: &mut [SqlValue],
) -> Result<RowId> {
    if let Some(alias) = table.rowid_alias_column {
        let slot = alias as usize;
        match values.get(slot).cloned().unwrap_or(SqlValue::Null) {
            SqlValue::Null => {
                let rowid = engine.reserve_row_id();
                values[slot] = SqlValue::Integer(rowid.0 as i64);
                Ok(rowid)
            }
            SqlValue::Integer(v) if v >= 0 => Ok(RowId::new(v as u64)),
            SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(RowId::new(v as u64)),
            SqlValue::Integer(_) | SqlValue::Real(_) => Err(Error::DatatypeMismatch),
            _ => Err(Error::DatatypeMismatch),
        }
    } else {
        Ok(engine.reserve_row_id())
    }
}

pub(crate) fn choose_rowid_for_update(
    engine: &Engine,
    table: &TableDef,
    values: &[SqlValue],
    current_rowid: RowId,
) -> Result<RowId> {
    if let Some(alias) = table.rowid_alias_column {
        match values
            .get(alias as usize)
            .cloned()
            .unwrap_or(SqlValue::Null)
        {
            SqlValue::Null => Ok(engine.reserve_row_id()),
            SqlValue::Integer(v) if v >= 0 => Ok(RowId::new(v as u64)),
            SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(RowId::new(v as u64)),
            SqlValue::Integer(_) | SqlValue::Real(_) => Err(Error::DatatypeMismatch),
            _ => Err(Error::DatatypeMismatch),
        }
    } else {
        Ok(current_rowid)
    }
}
