use super::*;

pub(super) fn execute_insert(
    conn: &Connection,
    plan: &crate::statement::InsertPlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
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
                InsertOutcome::Inserted { rowid, values }
                | InsertOutcome::Updated { rowid, values } => {
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
                    InsertOutcome::Inserted { rowid, values }
                    | InsertOutcome::Updated { rowid, values } => {
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
                InsertOutcome::Inserted { rowid, values }
                | InsertOutcome::Updated { rowid, values } => {
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
