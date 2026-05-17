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
    match crate::udf::authorize_table_access(crate::udf::AUTH_UPDATE, &plan.table.name) {
        crate::udf::AuthorizerDecision::Allow => {}
        crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
        crate::udf::AuthorizerDecision::Ignore => {
            return Ok(build_dml_execution_result(0, Vec::new(), plan.returning.is_some()));
        }
    }
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
            // Phase-11 SQL-D A6: an UPDATE may have touched an input to
            // a STORED generated column. Recompute every STORED column
            // here so the persisted row stays consistent with the
            // declared expression.
            values = compute_stored_generated_columns(&plan.table, values)?;
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
            fire_update_triggers(
                conn,
                tx,
                &plan.table,
                fresh.rowid,
                new_rowid,
                &old_values,
                &values,
                &plan.assignments,
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
            // Fire update hook AFTER the heap+indexes are in sync. When
            // the rowid alias changed the row was implemented as a
            // delete+insert under the hood, but SQLite's contract is to
            // surface this as a single UPDATE event with the new rowid.
            crate::udf::fire_mutation(
                crate::udf::MUTATION_UPDATE,
                &plan.table.name,
                new_rowid.0 as i64,
            );
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

/// Fire AFTER UPDATE triggers attached to `table`. The before-image is the
/// row before the update; the after-image is the row after. The `assignments` list
/// drives the `UPDATE OF cols` filter so triggers declared to fire on a
/// specific column set are skipped when none of those columns appear in
/// the SET list.
fn fire_update_triggers(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    old_rowid: redlinedb_kernel::format::RowId,
    new_rowid: redlinedb_kernel::format::RowId,
    old_values: &[SqlValue],
    new_values: &[SqlValue],
    assignments: &[(usize, sqlparser::ast::Expr)],
) -> Result<()> {
    let schema = conn.engine().schema_snapshot();
    let changed_cols: Vec<String> = assignments
        .iter()
        .filter_map(|(ordinal, _)| {
            table
                .columns
                .get(*ordinal)
                .map(|col| col.name.as_ref().to_owned())
        })
        .collect();
    crate::exec::trigger::fire_triggers(
        conn,
        tx,
        &schema,
        table,
        redlinedb_kernel::catalog::TriggerEventKind::Update,
        redlinedb_kernel::catalog::TriggerTimeKind::After,
        Some(crate::exec::trigger::TriggerRowValues {
            rowid: old_rowid,
            values: old_values.to_vec(),
        }),
        Some(crate::exec::trigger::TriggerRowValues {
            rowid: new_rowid,
            values: new_values.to_vec(),
        }),
        Some(&changed_cols),
    )
}

/// Fire AFTER DELETE triggers attached to `table`. The before-image is the
/// row just removed; after-image is absent for DELETE.
fn fire_delete_triggers(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    rowid: redlinedb_kernel::format::RowId,
    values: &[SqlValue],
) -> Result<()> {
    let schema = conn.engine().schema_snapshot();
    crate::exec::trigger::fire_triggers(
        conn,
        tx,
        &schema,
        table,
        redlinedb_kernel::catalog::TriggerEventKind::Delete,
        redlinedb_kernel::catalog::TriggerTimeKind::After,
        Some(crate::exec::trigger::TriggerRowValues {
            rowid,
            values: values.to_vec(),
        }),
        None,
        None,
    )
}

fn fire_before_delete_triggers(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    rowid: redlinedb_kernel::format::RowId,
    values: &[SqlValue],
) -> Result<()> {
    let schema = conn.engine().schema_snapshot();
    crate::exec::trigger::fire_triggers(
        conn,
        tx,
        &schema,
        table,
        redlinedb_kernel::catalog::TriggerEventKind::Delete,
        redlinedb_kernel::catalog::TriggerTimeKind::Before,
        Some(crate::exec::trigger::TriggerRowValues {
            rowid,
            values: values.to_vec(),
        }),
        None,
        None,
    )
}

pub(crate) fn execute_delete(
    conn: &Connection,
    plan: &crate::statement::DeletePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    match crate::udf::authorize_table_access(crate::udf::AUTH_DELETE, &plan.table.name) {
        crate::udf::AuthorizerDecision::Allow => {}
        crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
        crate::udf::AuthorizerDecision::Ignore => {
            return Ok(build_dml_execution_result(0, Vec::new(), plan.returning.is_some()));
        }
    }
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
            // BEFORE DELETE triggers fire while the before-image row still exists.
            fire_before_delete_triggers(conn, tx, &plan.table, row.rowid, &live)?;
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
            fire_delete_triggers(conn, tx, &plan.table, row.rowid, &live)?;
            crate::udf::fire_mutation(
                crate::udf::MUTATION_DELETE,
                &plan.table.name,
                row.rowid.0 as i64,
            );
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}
