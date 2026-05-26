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

/// Phase 5 WS-A2f: shared candidate-row reducer for DML with ORDER BY /
/// LIMIT / OFFSET. Applies the WHERE predicate, sorts by the ORDER BY
/// keys (NULLs ordered the same way SELECT does — see [`vec::SortDirection`]),
/// and takes the requested window. When `order_by` is empty and `limit`
/// is `None` the original `rows` are returned unchanged so the legacy
/// fast paths are preserved bit-for-bit.
pub(crate) fn restrict_dml_rows(
    rows: Vec<TableRow>,
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    limit: Option<&Expr>,
    offset: Option<&Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<TableRow>> {
    if order_by.is_empty() && limit.is_none() && offset.is_none() {
        return Ok(rows);
    }
    let mut filtered: Vec<TableRow> = Vec::with_capacity(rows.len());
    for row in rows {
        if selection_passes(selection, &SqlRow::Table(row.clone()), bindings)? {
            filtered.push(row);
        }
    }
    if !order_by.is_empty() {
        let directions: Vec<crate::exec::vec::SortDirection> = order_by
            .iter()
            .map(|order| {
                crate::exec::vec::SortDirection::from_order_options(
                    matches!(order.options.asc, Some(false)),
                    order.options.nulls_first,
                )
            })
            .collect();
        let mut keyed: Vec<(Vec<SqlValue>, TableRow)> = Vec::with_capacity(filtered.len());
        for row in filtered {
            let row_ctx = SqlRow::Table(row.clone());
            let mut keys = Vec::with_capacity(order_by.len());
            for order in order_by {
                keys.push(eval_scalar(&order.expr, &row_ctx.context(), bindings)?);
            }
            keyed.push((keys, row));
        }
        keyed.sort_by(|a, b| {
            for (idx, dir) in directions.iter().enumerate() {
                let cmp = dir.compare_values(&a.0[idx], &b.0[idx]);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
        filtered = keyed.into_iter().map(|(_, row)| row).collect();
    }
    let offset_n = match offset {
        Some(expr) => scalar_to_usize(&eval_scalar(expr, &RowContext::Empty, bindings)?)?,
        None => 0,
    };
    let limit_n = match limit {
        Some(expr) => scalar_to_usize(&eval_scalar(expr, &RowContext::Empty, bindings)?)?,
        None => usize::MAX,
    };
    Ok(filtered.into_iter().skip(offset_n).take(limit_n).collect())
}

pub(crate) fn execute_update(
    conn: &Connection,
    plan: &crate::statement::UpdatePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    match crate::udf::authorize_table_access(crate::udf::AUTH_UPDATE, &plan.table.name) {
        crate::udf::AuthorizerDecision::Allow => {}
        crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
        crate::udf::AuthorizerDecision::Ignore => {
            return Ok(build_dml_execution_result(
                0,
                Vec::new(),
                plan.returning.is_some(),
            ));
        }
    }
    // Phase 5 WS-A6 fast path: pre-classify the SET clause so we can
    // skip the per-row `eval_scalar` walk when every assignment is a
    // pure literal/binding replacement or an integer-delta of the
    // assigned column. Structural eligibility (no RETURNING, no
    // generated cols, no FK/CHECK, no indexed write column, no rowid
    // alias move) is checked separately; trigger lookup needs the
    // schema snapshot so it happens inside `with_write_tx` below.
    //
    // Phase 5 WS-A2f: also disable the fast path when ORDER BY / LIMIT /
    // OFFSET are present — the SQLite contract requires evaluating the
    // ORDER BY against the pre-image and applying writes only to the
    // selected window. The fast path skips that step.
    let order_or_limit =
        !plan.order_by.is_empty() || plan.limit.is_some() || plan.offset.is_some();
    let fast_plans = if !order_or_limit && crate::exec::hot_row::structurally_eligible(plan) {
        match crate::exec::hot_row::classify_assignments(plan, bindings)? {
            crate::exec::hot_row::ClassifyResult::Supported(plans) => Some(plans),
            crate::exec::hot_row::ClassifyResult::Unsupported => None,
        }
    } else {
        None
    };
    with_write_tx(conn, |session, tx| {
        let target_rowids = if order_or_limit {
            // ORDER BY / LIMIT mode: scan + WHERE + sort + window, then
            // hand off rowids to the per-row writer loop below.
            let rows = dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?;
            restrict_dml_rows(
                rows,
                &plan.selection,
                &plan.order_by,
                plan.limit.as_ref(),
                plan.offset.as_ref(),
                bindings,
            )?
            .into_iter()
            .map(|row| row.rowid)
            .collect()
        } else if let Some(rowid) = selection_rowid_eq(&plan.table, &plan.selection, bindings)? {
            vec![rowid]
        } else {
            dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?
                .into_iter()
                .map(|row| row.rowid)
                .collect()
        };
        // Final trigger check: only safe to fast-path when no
        // BEFORE/AFTER UPDATE triggers are attached to the table.
        let fast_plans = fast_plans.as_deref().filter(|_| {
            use redlinedb_kernel::catalog::{TriggerEventKind, TriggerTimeKind, triggers_for};
            let schema = conn.engine().schema_snapshot();
            triggers_for(
                &schema,
                plan.table.schema_id,
                &plan.table.folded,
                TriggerEventKind::Update,
                TriggerTimeKind::Before,
            )
            .is_empty()
                && triggers_for(
                    &schema,
                    plan.table.schema_id,
                    &plan.table.folded,
                    TriggerEventKind::Update,
                    TriggerTimeKind::After,
                )
                .is_empty()
        });
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
            if let Some(plans) = fast_plans {
                // WS-A6 fast path: every assignment is a Replacement or
                // IntegerDelta. Apply directly without AST eval. On any
                // runtime mismatch (e.g. delta on Text), fall back per
                // row to the slow path so semantics remain identical.
                if crate::exec::hot_row::apply_plans(plans, &mut values).is_err() {
                    values = fresh.values.clone();
                    let mut scratch = EvalScratch::default();
                    for (ordinal, expr) in &plan.assignments {
                        if *ordinal >= values.len() {
                            return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
                        }
                        values[*ordinal] = evaluate_dml_value(
                            &plan.table,
                            *ordinal,
                            expr,
                            &RowContext::Table(&fresh),
                            bindings,
                            &mut scratch,
                        )?;
                    }
                }
            } else {
                let mut scratch = EvalScratch::default();
                for (ordinal, expr) in &plan.assignments {
                    if *ordinal >= values.len() {
                        return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
                    }
                    values[*ordinal] = evaluate_dml_value(
                        &plan.table,
                        *ordinal,
                        expr,
                        &RowContext::Table(&fresh),
                        bindings,
                        &mut scratch,
                    )?;
                }
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
            fire_update_triggers(
                conn,
                tx,
                &plan.table,
                redlinedb_kernel::catalog::TriggerTimeKind::Before,
                fresh.rowid,
                new_rowid,
                &old_values,
                &values,
                &plan.assignments,
            )?;
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
                redlinedb_kernel::catalog::TriggerTimeKind::After,
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
    time: redlinedb_kernel::catalog::TriggerTimeKind,
    old_rowid: redlinedb_kernel::format::RowId,
    new_rowid: redlinedb_kernel::format::RowId,
    old_values: &[SqlValue],
    new_values: &[SqlValue],
    assignments: &[(usize, DmlValue)],
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
        time,
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
            return Ok(build_dml_execution_result(
                0,
                Vec::new(),
                plan.returning.is_some(),
            ));
        }
    }
    with_write_tx(conn, |session, tx| {
        let rows = dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?;
        // Phase 5 WS-A2f: when ORDER BY / LIMIT / OFFSET are present we
        // pre-filter via WHERE, sort by the ORDER BY keys, then take the
        // requested window. Naive scan+sort — performance is correct but
        // not optimal; routing through index ordered-limit is a follow-up.
        let rows = restrict_dml_rows(
            rows,
            &plan.selection,
            &plan.order_by,
            plan.limit.as_ref(),
            plan.offset.as_ref(),
            bindings,
        )?;
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        let order_or_limit = !plan.order_by.is_empty() || plan.limit.is_some();
        for row in rows {
            // Already filtered above when ORDER BY / LIMIT is set; in the
            // legacy path keep the per-row selection check for parity.
            if !order_or_limit
                && !selection_passes(&plan.selection, &SqlRow::Table(row.clone()), bindings)?
            {
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
            crate::exec::fk::enforce_fk_on_parent_delete(conn, session, tx, &plan.table, &live)?;
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
