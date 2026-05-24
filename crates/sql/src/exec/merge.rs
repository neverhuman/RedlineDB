//! Track K — SQL:2003 MERGE statement dispatch.
//!
//! Lowers `MERGE INTO target USING source ON ... WHEN ...` (see
//! `crate::statement::MergePlan`) into per-source-row scans of the target
//! table. For each source row, we evaluate the ON predicate against the
//! candidate target rows; matched and not-matched arms then fire the
//! first WHEN-clause whose AND-predicate holds.
//!
//! The MERGE plan body is intentionally minimal: it uses the kernel
//! heap and index helpers directly (`insert_for_relation`,
//! `update_for_relation`, `delete_for_relation`, plus
//! `maintain_indexes_on_*`) so basic tables — which is what the
//! beyond-SQLite portability cases exercise — get correct semantics
//! without re-implementing the full trigger / FK / hot-row plumbing
//! that `execute_update` / `execute_delete` / `execute_insert` provide.

use std::sync::Arc;

use redlinedb_kernel::catalog::TableDef;
use redlinedb_kernel::format::RowId;

use super::expr::scalar::row::{JoinedRow, RowContext, TableRow};
use super::*;
use crate::error::{Error, Result};
use crate::statement::{DmlValue, MergeClausePlan, MergePlan};
use crate::value::SqlValue;

pub(crate) fn execute_merge(
    conn: &Connection,
    plan: &MergePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    with_write_tx(conn, |_session, tx| {
        // Materialise both sides up front. The target is mutated in
        // place; we re-read freshly per source row to avoid acting on
        // stale tuples after earlier clauses delete/update them.
        let source_rows = super::collect_table_rows_with_alias(
            conn.engine(),
            tx,
            &plan.source,
            plan.source_alias.clone(),
        )?;

        let mut affected = 0usize;

        for source_row in &source_rows {
            // Re-scan the target each iteration so an earlier MATCHED DELETE
            // is not seen by a later source row's match probe.
            let target_rows = super::collect_table_rows_with_alias(
                conn.engine(),
                tx,
                &plan.target,
                plan.target_alias.clone(),
            )?;

            let mut matched_any = false;
            for target_row in &target_rows {
                let joined = vec![
                    joined_from(&plan.target, plan.target_alias.clone(), Some(target_row.clone())),
                    joined_from(&plan.source, plan.source_alias.clone(), Some(source_row.clone())),
                ];
                let ctx = RowContext::Joined(&joined);
                let on_ok = is_truthy(&eval_scalar(&plan.on, &ctx, bindings)?);
                if !on_ok {
                    continue;
                }
                matched_any = true;

                // First matching MATCHED clause wins.
                for clause in &plan.clauses {
                    let (predicate, kind_matched) = match clause {
                        MergeClausePlan::MatchedUpdate { predicate, .. }
                        | MergeClausePlan::MatchedDelete { predicate } => (predicate.as_ref(), true),
                        MergeClausePlan::NotMatchedInsert { .. } => (None, false),
                    };
                    if !kind_matched {
                        continue;
                    }
                    if let Some(pred) = predicate {
                        if !is_truthy(&eval_scalar(pred, &ctx, bindings)?) {
                            continue;
                        }
                    }
                    match clause {
                        MergeClausePlan::MatchedUpdate { assignments, .. } => {
                            apply_matched_update(
                                conn, tx, plan, target_row, &ctx, assignments, bindings,
                            )?;
                            affected += 1;
                        }
                        MergeClausePlan::MatchedDelete { .. } => {
                            apply_matched_delete(conn, tx, plan, target_row)?;
                            affected += 1;
                        }
                        MergeClausePlan::NotMatchedInsert { .. } => unreachable!(),
                    }
                    break;
                }
            }

            if !matched_any {
                // First matching NOT MATCHED clause wins. The row
                // context for predicates / values is source-only.
                let joined = vec![joined_from(
                    &plan.source,
                    plan.source_alias.clone(),
                    Some(source_row.clone()),
                )];
                let ctx = RowContext::Joined(&joined);
                for clause in &plan.clauses {
                    let MergeClausePlan::NotMatchedInsert {
                        predicate,
                        columns,
                        values,
                    } = clause
                    else {
                        continue;
                    };
                    if let Some(pred) = predicate {
                        if !is_truthy(&eval_scalar(pred, &ctx, bindings)?) {
                            continue;
                        }
                    }
                    apply_not_matched_insert(
                        conn, tx, plan, columns, values, &ctx, bindings,
                    )?;
                    affected += 1;
                    break;
                }
            }
        }

        Ok(ExecutionResult {
            runtime: RuntimeState::Done,
            affected_rows: affected,
        })
    })
}

fn joined_from(
    table: &Arc<TableDef>,
    alias: Option<Arc<str>>,
    row: Option<TableRow>,
) -> JoinedRow {
    JoinedRow {
        table: Arc::clone(table),
        alias,
        row,
    }
}

fn apply_matched_update(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    plan: &MergePlan,
    target_row: &TableRow,
    ctx: &RowContext<'_>,
    assignments: &[(usize, DmlValue)],
    bindings: &[Option<SqlValue>],
) -> Result<()> {
    // Reload the target row so we are operating on the latest tuple.
    let Some(fresh) =
        super::load_table_row_by_rowid(conn.engine(), tx, &plan.target, target_row.rowid)?
    else {
        return Ok(());
    };
    conn.engine()
        .lock_row_for_relation(tx, plan.target.relation_id, fresh.rowid)?;
    let old_values = fresh.values.clone();
    let mut values = fresh.values.clone();
    let mut scratch = EvalScratch::default();
    for (ordinal, expr) in assignments {
        if *ordinal >= values.len() {
            return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
        }
        values[*ordinal] = super::evaluate_dml_value(
            &plan.target,
            *ordinal,
            expr,
            ctx,
            bindings,
            &mut scratch,
        )?;
    }
    values = super::apply_row_affinity(&plan.target, values)?;
    values = super::compute_stored_generated_columns(&plan.target, values)?;
    super::apply_constraints(&plan.target, &values)?;

    let payload = encode_sql_row(plan.target.table_id.0, &values)?;
    conn.engine().update_for_relation(
        tx,
        plan.target.relation_id,
        fresh.rowid,
        payload,
    )?;
    super::index_dml::maintain_indexes_on_update(
        conn.engine(),
        tx,
        &plan.target,
        &old_values,
        &values,
        fresh.rowid,
        fresh.rowid,
    )?;
    Ok(())
}

fn apply_matched_delete(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    plan: &MergePlan,
    target_row: &TableRow,
) -> Result<()> {
    let live = match super::load_table_row_by_rowid(
        conn.engine(),
        tx,
        &plan.target,
        target_row.rowid,
    )? {
        Some(row) => row.values,
        None => return Ok(()),
    };
    conn.engine()
        .delete_for_relation(tx, plan.target.relation_id, target_row.rowid)?;
    super::index_dml::maintain_indexes_on_delete(
        conn.engine(),
        tx,
        &plan.target,
        &live,
        target_row.rowid,
    )?;
    Ok(())
}

fn apply_not_matched_insert(
    conn: &Connection,
    tx: &mut redlinedb_kernel::engine::Txn,
    plan: &MergePlan,
    columns: &[usize],
    values: &[DmlValue],
    ctx: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<()> {
    let mut row_values = vec![SqlValue::Null; plan.target.columns.len()];
    let mut provided = vec![false; plan.target.columns.len()];
    let mut scratch = EvalScratch::default();
    for (ord, expr) in columns.iter().zip(values.iter()) {
        if *ord >= row_values.len() {
            return Err(Error::UnknownColumn(format!("ordinal {ord}")));
        }
        row_values[*ord] = super::evaluate_dml_value(
            &plan.target,
            *ord,
            expr,
            ctx,
            bindings,
            &mut scratch,
        )?;
        provided[*ord] = true;
    }
    // Apply defaults to unprovided columns.
    row_values = super::build_default_values(&plan.target, row_values)?;
    row_values = super::apply_row_affinity(&plan.target, row_values)?;
    row_values = super::compute_stored_generated_columns(&plan.target, row_values)?;
    super::apply_constraints(&plan.target, &row_values)?;

    let new_rowid = super::choose_rowid_for_insert(conn.engine(), &plan.target, &mut row_values)?;
    let payload = encode_sql_row(plan.target.table_id.0, &row_values)?;
    conn.engine()
        .insert_for_relation(tx, plan.target.relation_id, new_rowid, payload)?;
    super::index_dml::maintain_indexes_on_insert(
        conn.engine(),
        tx,
        &plan.target,
        &row_values,
        new_rowid,
    )?;
    Ok(())
}

#[allow(dead_code)]
fn _silence_rowid(_: RowId) {}
