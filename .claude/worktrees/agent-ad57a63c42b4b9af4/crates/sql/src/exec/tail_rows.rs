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

pub(crate) fn selection_rowid_eq(
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Option<RowId>> {
    let Some(expr) = selection else {
        return Ok(None);
    };
    let rowid_col = |name: &str| {
        name.eq_ignore_ascii_case("rowid")
            || name.eq_ignore_ascii_case("_rowid_")
            || name.eq_ignore_ascii_case("oid")
            || table
                .rowid_alias_column
                .and_then(|alias| table.columns.get(alias as usize))
                .is_some_and(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
    };
    let Expr::BinaryOp { left, op, right } = expr else {
        return Ok(None);
    };
    if !matches!(op, BinaryOperator::Eq) {
        return Ok(None);
    }
    let expr_rowid = if let Some(value) = rowid_eq_side(table, left, right, bindings, &rowid_col)? {
        value
    } else if let Some(value) = rowid_eq_side(table, right, left, bindings, &rowid_col)? {
        value
    } else {
        return Ok(None);
    };
    Ok(Some(expr_rowid))
}

pub(crate) fn rowid_eq_side(
    _table: &Arc<TableDef>,
    ident_side: &Expr,
    value_side: &Expr,
    bindings: &[Option<SqlValue>],
    rowid_col: &impl Fn(&str) -> bool,
) -> Result<Option<RowId>> {
    let name = match ident_side {
        Expr::Identifier(ident) if rowid_col(&ident.value) => Some(ident.value.as_str()),
        Expr::CompoundIdentifier(parts) => parts.last().and_then(|ident| {
            if rowid_col(&ident.value) {
                Some(ident.value.as_str())
            } else {
                None
            }
        }),
        _ => None,
    };
    if name.is_none() {
        return Ok(None);
    }
    let value = eval_scalar(value_side, &RowContext::Empty, bindings)?;
    match value {
        SqlValue::Integer(v) if v >= 0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Null => Ok(None),
        _ => Err(Error::DatatypeMismatch),
    }
}
