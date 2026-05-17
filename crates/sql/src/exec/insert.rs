use super::*;
use redlinedb_kernel::catalog::{TriggerEventKind, TriggerTimeKind};

pub(super) fn execute_insert(
    conn: &Connection,
    plan: &crate::statement::InsertPlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    // Authorizer veto on INSERT — DENY surfaces the standard "not
    // authorized" error; IGNORE silently drops the entire statement.
    match crate::udf::authorize_table_access(crate::udf::AUTH_INSERT, &plan.table.name) {
        crate::udf::AuthorizerDecision::Allow => {}
        crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
        crate::udf::AuthorizerDecision::Ignore => {
            return Ok(build_dml_execution_result(0, Vec::new(), plan.returning.is_some()));
        }
    }
    let source_rows = plan
        .source_select
        .as_ref()
        .map(|source_select| materialize_select_plan_rows(conn, source_select, bindings))
        .transpose()?;
    with_write_tx(conn, |session, tx| {
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        if plan.default_values {
            let mut values = build_default_row(&plan.table)?;
            match insert_row_with_resolution(
                conn,
                session,
                tx,
                &plan.table,
                &mut values,
                plan.conflict.as_ref(),
                bindings,
            )? {
                InsertOutcome::Inserted { rowid, values } => {
                    fire_insert_triggers(conn, tx, &plan.table, rowid, &values)?;
                    fire_insert_hook(&plan.table, rowid);
                    if let Some(returning) = &plan.returning {
                        returning_rows.push(project_returning_row(
                            &plan.table,
                            &values,
                            rowid,
                            returning,
                            bindings,
                        )?);
                    }
                    return Ok(build_dml_execution_result(
                        1,
                        returning_rows,
                        plan.returning.is_some(),
                    ));
                }
                InsertOutcome::Updated { rowid, values } => {
                    fire_insert_triggers(conn, tx, &plan.table, rowid, &values)?;
                    fire_update_hook(&plan.table, rowid);
                    if let Some(returning) = &plan.returning {
                        returning_rows.push(project_returning_row(
                            &plan.table,
                            &values,
                            rowid,
                            returning,
                            bindings,
                        )?);
                    }
                    return Ok(build_dml_execution_result(
                        1,
                        returning_rows,
                        plan.returning.is_some(),
                    ));
                }
                InsertOutcome::Ignored => {
                    return Ok(build_dml_execution_result(
                        0,
                        returning_rows,
                        plan.returning.is_some(),
                    ));
                }
            }
        }
        if let Some(source_rows) = &source_rows {
            for row in source_rows {
                if row.len() != plan.columns.len() {
                    return Err(Error::Bind(
                        "INSERT SELECT row arity does not match column list".to_owned(),
                    ));
                }
                let mut values = build_row_from_values(&plan.table, row, &plan.columns)?;
                match insert_row_with_resolution(
                    conn,
                    session,
                    tx,
                    &plan.table,
                    &mut values,
                    plan.conflict.as_ref(),
                    bindings,
                )? {
                    InsertOutcome::Inserted { rowid, values } => {
                        fire_insert_hook(&plan.table, rowid);
                        if let Some(returning) = &plan.returning {
                            returning_rows.push(project_returning_row(
                                &plan.table,
                                &values,
                                rowid,
                                returning,
                                bindings,
                            )?);
                        }
                        count += 1;
                    }
                    InsertOutcome::Updated { rowid, values } => {
                        fire_update_hook(&plan.table, rowid);
                        if let Some(returning) = &plan.returning {
                            returning_rows.push(project_returning_row(
                                &plan.table,
                                &values,
                                rowid,
                                returning,
                                bindings,
                            )?);
                        }
                        count += 1;
                    }
                    InsertOutcome::Ignored => {}
                }
            }
            return Ok(build_dml_execution_result(
                count,
                returning_rows,
                plan.returning.is_some(),
            ));
        }
        for row in &plan.rows {
            if row.len() != plan.columns.len() {
                return Err(Error::Bind(
                    "INSERT row arity does not match column list".to_owned(),
                ));
            }
            let mut values = build_row(&plan.table, row, &plan.columns, bindings)?;
            match insert_row_with_resolution(
                conn,
                session,
                tx,
                &plan.table,
                &mut values,
                plan.conflict.as_ref(),
                bindings,
            )? {
                InsertOutcome::Inserted { rowid, values } => {
                    fire_insert_triggers(conn, tx, &plan.table, rowid, &values)?;
                    fire_insert_hook(&plan.table, rowid);
                    if let Some(returning) = &plan.returning {
                        returning_rows.push(project_returning_row(
                            &plan.table,
                            &values,
                            rowid,
                            returning,
                            bindings,
                        )?);
                    }
                    count += 1;
                }
                InsertOutcome::Updated { rowid, values } => {
                    fire_insert_triggers(conn, tx, &plan.table, rowid, &values)?;
                    fire_update_hook(&plan.table, rowid);
                    if let Some(returning) = &plan.returning {
                        returning_rows.push(project_returning_row(
                            &plan.table,
                            &values,
                            rowid,
                            returning,
                            bindings,
                        )?);
                    }
                    count += 1;
                }
                InsertOutcome::Ignored => {}
            }
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

/// Fire AFTER INSERT triggers attached to `table`. NEW is the row just
/// inserted; before-image is absent for INSERT. The schema snapshot is loaded
/// from the live engine so triggers created earlier in the transaction
/// are visible.
fn fire_insert_triggers(
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
        TriggerEventKind::Insert,
        TriggerTimeKind::After,
        None,
        Some(crate::exec::trigger::TriggerRowValues {
            rowid,
            values: values.to_vec(),
        }),
        None,
    )
}

fn fire_insert_hook(table: &Arc<TableDef>, rowid: RowId) {
    crate::udf::fire_mutation(crate::udf::MUTATION_INSERT, &table.name, rowid.0 as i64);
}

fn fire_update_hook(table: &Arc<TableDef>, rowid: RowId) {
    crate::udf::fire_mutation(crate::udf::MUTATION_UPDATE, &table.name, rowid.0 as i64);
}
