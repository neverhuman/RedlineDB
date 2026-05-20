use std::sync::Arc;

use super::*;

pub(crate) fn dml_target_rows(
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

pub(crate) fn project_returning_row(
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

pub(crate) fn build_dml_execution_result(
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
    let mut scratch = EvalScratch::default();
    for (idx, column) in table.columns.iter().enumerate() {
        if matches!(values[idx], SqlValue::Null)
            && let Some(default) = column_default_value(column, &mut scratch)?
        {
            values[idx] = default;
        }
    }
    let values = apply_row_affinity(table, values)?;
    compute_stored_generated_columns(table, values)
}

fn build_default_values_for_omitted(
    table: &Arc<TableDef>,
    mut values: Vec<SqlValue>,
    provided: &[bool],
) -> Result<Vec<SqlValue>> {
    let mut scratch = EvalScratch::default();
    for (idx, column) in table.columns.iter().enumerate() {
        if !provided.get(idx).copied().unwrap_or(false)
            && matches!(values[idx], SqlValue::Null)
            && let Some(default) = column_default_value(column, &mut scratch)?
        {
            values[idx] = default;
        }
    }
    let values = apply_row_affinity(table, values)?;
    compute_stored_generated_columns(table, values)
}

fn column_default_value(
    column: &redlinedb_kernel::catalog::ColumnDef,
    scratch: &mut EvalScratch,
) -> Result<Option<SqlValue>> {
    if let Some(default) = &column.default_value {
        return Ok(Some(default.clone()));
    }
    let Some(default_expr) = &column.default_expr else {
        return Ok(None);
    };
    eval_expr(default_expr, &EmptyDefaultRow, scratch)
        .map(Some)
        .map_err(|err| Error::UnsupportedSql(format!("invalid default expression: {err}")))
}

struct EmptyDefaultRow;

impl RowValueSource for EmptyDefaultRow {
    fn value_at(&self, _col: u16) -> Option<OwnedValue> {
        None
    }
}

/// Phase-11 SQL-D A6: compute every STORED generated column from the
/// already-populated row. VIRTUAL columns are evaluated at SELECT time
/// (in `tail_rows::materialize_virtual_generated_columns`) and are left
/// as NULL in the persisted heap row.
pub(crate) fn compute_stored_generated_columns(
    table: &TableDef,
    mut values: Vec<SqlValue>,
) -> Result<Vec<SqlValue>> {
    use redlinedb_kernel::catalog::GeneratedColumnKind;

    let has_generated = table.columns.iter().any(|c| c.generated.is_some());
    if !has_generated {
        return Ok(values);
    }
    // Snapshot inputs BEFORE we start overwriting generated columns so
    // expressions like `c = a + b` always observe the original
    // user-provided / defaulted inputs regardless of evaluation order.
    let input_snapshot = values.clone();
    for (idx, column) in table.columns.iter().enumerate() {
        let Some(gen_def) = &column.generated else {
            continue;
        };
        if !matches!(gen_def.kind, GeneratedColumnKind::Stored) {
            // VIRTUAL columns leave the heap slot as NULL; reads
            // compute on demand.
            values[idx] = SqlValue::Null;
            continue;
        }
        values[idx] = crate::exec::index_predicate::eval_generated_expr(
            table,
            gen_def.expr_sql.as_ref(),
            &input_snapshot,
        )?;
    }
    Ok(values)
}

pub(crate) fn apply_row_affinity(table: &TableDef, values: Vec<SqlValue>) -> Result<Vec<SqlValue>> {
    let mut out = values;
    for (idx, column) in table.columns.iter().enumerate() {
        out[idx] = apply_affinity(out[idx].clone(), column.affinity)
            .map_err(|_| Error::DatatypeMismatch)?;
        validate_strict_storage(table, column, &out[idx])?;
    }
    Ok(out)
}

fn validate_strict_storage(
    table: &TableDef,
    column: &redlinedb_kernel::catalog::ColumnDef,
    value: &SqlValue,
) -> Result<()> {
    if !table.is_strict() || matches!(value, SqlValue::Null) || strict_declared_any(column) {
        return Ok(());
    }
    let allowed = match column.affinity {
        redlinedb_kernel::catalog::Affinity::Integer => matches!(value, SqlValue::Integer(_)),
        redlinedb_kernel::catalog::Affinity::Real => matches!(value, SqlValue::Real(_)),
        redlinedb_kernel::catalog::Affinity::Text => matches!(value, SqlValue::Text(_)),
        redlinedb_kernel::catalog::Affinity::Blob => matches!(value, SqlValue::Blob(_)),
        redlinedb_kernel::catalog::Affinity::Numeric => {
            matches!(value, SqlValue::Integer(_) | SqlValue::Real(_))
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(Error::ConstraintViolation(format!(
            "cannot store {} value in {} column {}.{}",
            storage_class_name(value),
            strict_type_name(column),
            table.name,
            column.name
        )))
    }
}

fn strict_declared_any(column: &redlinedb_kernel::catalog::ColumnDef) -> bool {
    column
        .declared_type
        .as_deref()
        .is_some_and(|declared| declared.eq_ignore_ascii_case("ANY"))
}

fn strict_type_name(column: &redlinedb_kernel::catalog::ColumnDef) -> &str {
    column
        .declared_type
        .as_deref()
        .unwrap_or(match column.affinity {
            redlinedb_kernel::catalog::Affinity::Integer => "INTEGER",
            redlinedb_kernel::catalog::Affinity::Real => "REAL",
            redlinedb_kernel::catalog::Affinity::Text => "TEXT",
            redlinedb_kernel::catalog::Affinity::Blob => "BLOB",
            redlinedb_kernel::catalog::Affinity::Numeric => "NUMERIC",
        })
}

fn storage_class_name(value: &SqlValue) -> &'static str {
    match value {
        SqlValue::Null => "NULL",
        SqlValue::Integer(_) => "INTEGER",
        SqlValue::Real(_) => "REAL",
        SqlValue::Text(_) => "TEXT",
        SqlValue::Blob(_) => "BLOB",
    }
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
