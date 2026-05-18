#[path = "tail_build.rs"]
mod build;
#[path = "tail_conflict.rs"]
mod conflict;
#[path = "tail_rows.rs"]
mod rows;
#[path = "tail_stats.rs"]
mod stats;

pub(crate) use build::*;
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
            // A6 SQLite parity: an UPDATE both re-validates the row's own
            // FK columns (if they changed) and propagates the change to
            // children that reference the parent key.
            crate::exec::fk::enforce_fk_on_insert(
                conn,
                session,
                tx,
                &plan.table,
                &values,
                new_rowid,
            )?;
            crate::exec::fk::enforce_fk_on_parent_update(
                conn,
                session,
                tx,
                &plan.table,
                &old_values,
                &values,
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
    with_write_tx(conn, |session, tx| {
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
            // A6 SQLite parity: propagate the parent deletion to every
            // referencing child via the declared `ON DELETE` action.
            crate::exec::fk::enforce_fk_on_parent_delete(
                conn,
                session,
                tx,
                &plan.table,
                &live,
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
