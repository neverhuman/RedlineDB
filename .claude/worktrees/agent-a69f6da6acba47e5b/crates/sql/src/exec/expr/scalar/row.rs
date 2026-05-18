//! Row-context plumbing and per-row helpers.
//!
//! This file owns the SQL row data model (`SqlRow`, `TableRow`,
//! `JoinedRow`, `RowContext`, `TableRowSource`) plus the helpers that
//! operate on it:
//!
//!   * column lookup (`lookup_column`, `lookup_qualified_column`, and
//!     the lower-level `lookup_*` helpers)
//!   * row encode / decode (`unique_key_bytes`, `encode_sql_row`,
//!     `decode_sql_row`, `key_values_equal`)
//!   * row-shape utilities (`row_width`, `row_width_value`,
//!     `compare_row_ordering`, `scalar_to_usize`)

use super::*;

pub(crate) fn row_width(row: &[SqlValue]) -> usize {
    row.iter().map(row_width_value).sum()
}

pub(crate) fn row_width_value(value: &SqlValue) -> usize {
    match value {
        SqlValue::Null => 0,
        SqlValue::Integer(_) | SqlValue::Real(_) => 8,
        SqlValue::Text(value) => value.len(),
        SqlValue::Blob(value) => value.len(),
    }
}

// Lane VE: the pre-VE SQL-A in-place row-sort path was replaced by
// `vec::SpillSort` + top-K heap, so this helper has no current
// caller outside tests. Marked allow(dead_code) instead of removed
// to keep the SQL-A surface intact.
#[allow(dead_code)]
pub(crate) fn compare_row_ordering(
    left: &SqlRow,
    right: &SqlRow,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<Ordering> {
    for order in order_by {
        let collation = collation_from_expr(&order.expr);
        let left_value = eval_scalar(&order.expr, &left.context(), bindings)?;
        let right_value = eval_scalar(&order.expr, &right.context(), bindings)?;
        let mut ord = collation
            .and_then(|c| c.compare_values(&left_value, &right_value))
            .unwrap_or_else(|| compare_values(&left_value, &right_value));
        if matches!(order.options.asc, Some(false)) {
            ord = ord.reverse();
        }
        if ord != Ordering::Equal {
            return Ok(ord);
        }
    }
    Ok(Ordering::Equal)
}

pub(crate) fn lookup_column(row: &RowContext<'_>, name: &str) -> Result<SqlValue> {
    match row {
        RowContext::Table(row) => lookup_table_column(row, name),
        RowContext::Upsert { current, .. } => lookup_table_column(current, name),
        RowContext::Joined(rows) => {
            let mut found = None;
            for row in rows.iter() {
                if row.row.is_none() {
                    continue;
                }
                if let Ok(value) = lookup_joined_row_column(row, name) {
                    if found.is_some() {
                        return Err(Error::UnsupportedSql(format!(
                            "ambiguous column name: {name}"
                        )));
                    }
                    found = Some(value);
                }
            }
            found.ok_or_else(|| Error::UnknownColumn(name.to_owned()))
        }
        RowContext::SqliteSchema(row) => match name.to_ascii_lowercase().as_str() {
            "type" => Ok(SqlValue::Text(Arc::from(row.type_name.as_ref()))),
            "name" => Ok(SqlValue::Text(Arc::from(row.name.as_ref()))),
            "tbl_name" => Ok(SqlValue::Text(Arc::from(row.tbl_name.as_ref()))),
            "rootpage" => Ok(SqlValue::Integer(row.rootpage as i64)),
            "sql" => Ok(SqlValue::Text(Arc::from(row.sql.as_ref()))),
            _ => Err(Error::UnknownColumn(name.to_owned())),
        },
        RowContext::Empty => Err(Error::UnknownColumn(name.to_owned())),
    }
}

pub(crate) fn lookup_qualified_column(
    row: &RowContext<'_>,
    qualifier: &str,
    name: &str,
) -> Result<SqlValue> {
    match row {
        RowContext::Table(row) => {
            if row_matches_qualifier(row, qualifier) {
                lookup_table_column(row, name)
            } else {
                Err(Error::UnknownColumn(format!("{qualifier}.{name}")))
            }
        }
        RowContext::Joined(rows) => {
            let mut found = None;
            for row in rows.iter() {
                if row_matches_joined_qualifier(row, qualifier) {
                    let value = lookup_joined_row_column(row, name)?;
                    if found.is_some() {
                        return Err(Error::UnsupportedSql(format!(
                            "ambiguous column reference: {qualifier}.{name}"
                        )));
                    }
                    found = Some(value);
                }
            }
            found.ok_or_else(|| Error::UnknownColumn(format!("{qualifier}.{name}")))
        }
        RowContext::Upsert { current, excluded } => {
            if row_matches_qualifier(current, qualifier) {
                lookup_table_column(current, name)
            } else if qualifier.eq_ignore_ascii_case("excluded") {
                lookup_excluded_column(current.table.as_ref(), excluded, name)
            } else {
                Err(Error::UnknownColumn(format!("{qualifier}.{name}")))
            }
        }
        RowContext::SqliteSchema(row) => match qualifier.to_ascii_lowercase().as_str() {
            "sqlite_schema" | "sqlite_master" => lookup_schema_column(row, name),
            _ => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
        },
        RowContext::Empty => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
    }
}

fn row_matches_qualifier(row: &TableRow, qualifier: &str) -> bool {
    if let Some(alias) = &row.alias
        && alias.as_ref().eq_ignore_ascii_case(qualifier)
    {
        return true;
    }
    row.table.name.to_string().eq_ignore_ascii_case(qualifier)
}

fn row_matches_joined_qualifier(row: &JoinedRow, qualifier: &str) -> bool {
    if let Some(alias) = &row.alias
        && alias.as_ref().eq_ignore_ascii_case(qualifier)
    {
        return true;
    }
    row.table.name.to_string().eq_ignore_ascii_case(qualifier)
}

fn lookup_schema_column(row: &SqliteSchemaRow, name: &str) -> Result<SqlValue> {
    match name.to_ascii_lowercase().as_str() {
        "type" => Ok(SqlValue::Text(Arc::from(row.type_name.as_ref()))),
        "name" => Ok(SqlValue::Text(Arc::from(row.name.as_ref()))),
        "tbl_name" => Ok(SqlValue::Text(Arc::from(row.tbl_name.as_ref()))),
        "rootpage" => Ok(SqlValue::Integer(row.rootpage as i64)),
        "sql" => Ok(SqlValue::Text(Arc::from(row.sql.as_ref()))),
        _ => Err(Error::UnknownColumn(name.to_owned())),
    }
}

fn lookup_table_column(row: &TableRow, name: &str) -> Result<SqlValue> {
    if name.eq_ignore_ascii_case("rowid")
        || name.eq_ignore_ascii_case("_rowid_")
        || name.eq_ignore_ascii_case("oid")
    {
        return Ok(SqlValue::Integer(row.rowid.0 as i64));
    }
    let idx = row
        .table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
    Ok(row.values[idx].clone())
}

fn lookup_joined_row_column(row: &JoinedRow, name: &str) -> Result<SqlValue> {
    match &row.row {
        Some(present) => lookup_table_column(present, name),
        None => {
            if name.eq_ignore_ascii_case("rowid")
                || name.eq_ignore_ascii_case("_rowid_")
                || name.eq_ignore_ascii_case("oid")
            {
                return Ok(SqlValue::Null);
            }
            row.table
                .columns
                .iter()
                .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
                .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
            Ok(SqlValue::Null)
        }
    }
}

fn lookup_excluded_column(table: &TableDef, excluded: &[SqlValue], name: &str) -> Result<SqlValue> {
    if name.eq_ignore_ascii_case("rowid")
        || name.eq_ignore_ascii_case("_rowid_")
        || name.eq_ignore_ascii_case("oid")
    {
        if let Some(alias) = table.rowid_alias_column
            && let Some(value) = excluded.get(alias as usize)
        {
            return Ok(value.clone());
        }
        return Err(Error::UnknownColumn(name.to_owned()));
    }
    let idx = table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
    Ok(excluded.get(idx).cloned().unwrap_or(SqlValue::Null))
}

pub(crate) fn unique_key_bytes(
    table_id: u64,
    constraint_id: u64,
    values: &[SqlValue],
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(&table_id.to_le_bytes());
    out.extend_from_slice(&constraint_id.to_le_bytes());
    let refs = values.iter().map(|v| v.as_ref()).collect::<Vec<_>>();
    encode_record(&refs, &mut out).map_err(|_| Error::DatatypeMismatch)?;
    Ok(out)
}

pub(crate) fn key_values_equal(left: &[SqlValue], right: &[SqlValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(a, b)| compare_values(a, b) == Ordering::Equal)
}

pub(crate) fn encode_sql_row(table_id: u64, values: &[SqlValue]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut refs = Vec::with_capacity(values.len() + 1);
    refs.push(ValueRef::Integer(table_id as i64));
    refs.extend(values.iter().map(|value| value.as_ref()));
    encode_record(&refs, &mut out).map_err(|_| Error::DatatypeMismatch)?;
    Ok(out)
}

pub(crate) fn decode_sql_row(bytes: &[u8]) -> Result<Option<(u64, Vec<SqlValue>)>> {
    let record = RecordRef::new(bytes).map_err(|_| Error::DatatypeMismatch)?;
    let mut scratch = RecordScratch::default();
    record
        .decode_into(&mut scratch)
        .map_err(|_| Error::DatatypeMismatch)?;
    let mut values = Vec::new();
    let table_id = match record
        .value_at(&scratch, 0)
        .map_err(|_| Error::DatatypeMismatch)?
    {
        ValueRef::Integer(v) => v as u64,
        _ => return Err(Error::DatatypeMismatch),
    };
    for idx in 1..record.column_count().map_err(|_| Error::DatatypeMismatch)? {
        let value = record
            .value_at(&scratch, idx)
            .map_err(|_| Error::DatatypeMismatch)?;
        values.push(value.to_owned());
    }
    Ok(Some((table_id, values)))
}

pub(crate) fn scalar_to_usize(value: &SqlValue) -> Result<usize> {
    match value {
        SqlValue::Integer(v) => Ok((*v).max(0) as usize),
        SqlValue::Real(v) => Ok((*v).max(0.0) as usize),
        SqlValue::Null => Ok(0),
        _ => Err(Error::DatatypeMismatch),
    }
}

#[derive(Clone)]
pub(crate) struct TableRow {
    pub(crate) rowid: RowId,
    pub(crate) values: Vec<SqlValue>,
    pub(crate) table: Arc<TableDef>,
    pub(crate) alias: Option<Arc<str>>,
}

#[derive(Clone)]
pub(crate) struct JoinedRow {
    pub(crate) table: Arc<TableDef>,
    pub(crate) alias: Option<Arc<str>>,
    pub(crate) row: Option<TableRow>,
}

pub(crate) struct TableRowSource<'a> {
    pub(crate) values: &'a [SqlValue],
}

impl RowValueSource for TableRowSource<'_> {
    fn value_at(&self, col: u16) -> Option<OwnedValue> {
        self.values.get(col as usize).cloned()
    }
}

#[derive(Clone)]
pub(crate) enum SqlRow {
    Table(TableRow),
    Joined(Vec<JoinedRow>),
    SqliteSchema(SqliteSchemaRow),
    Static(Vec<SqlValue>),
    Empty,
}

pub(crate) enum RowContext<'a> {
    Table(&'a TableRow),
    Joined(&'a [JoinedRow]),
    Upsert {
        current: &'a TableRow,
        excluded: &'a [SqlValue],
    },
    SqliteSchema(&'a SqliteSchemaRow),
    Empty,
}

impl SqlRow {
    pub(crate) fn context(&self) -> RowContext<'_> {
        match self {
            SqlRow::Table(row) => RowContext::Table(row),
            SqlRow::Joined(rows) => RowContext::Joined(rows),
            SqlRow::SqliteSchema(row) => RowContext::SqliteSchema(row),
            SqlRow::Static(_) => RowContext::Empty,
            SqlRow::Empty => RowContext::Empty,
        }
    }

    pub(crate) fn values(&self) -> Result<Vec<SqlValue>> {
        match self {
            SqlRow::Table(row) => Ok(row.values.clone()),
            SqlRow::Joined(rows) => Ok(rows
                .iter()
                .flat_map(|row| match &row.row {
                    Some(present) => present.values.clone(),
                    None => vec![SqlValue::Null; row.table.columns.len()],
                })
                .collect::<Vec<_>>()),
            SqlRow::SqliteSchema(row) => Ok(vec![
                SqlValue::Text(Arc::from(row.type_name.as_ref())),
                SqlValue::Text(Arc::from(row.name.as_ref())),
                SqlValue::Text(Arc::from(row.tbl_name.as_ref())),
                SqlValue::Integer(row.rootpage as i64),
                SqlValue::Text(Arc::from(row.sql.as_ref())),
            ]),
            SqlRow::Static(values) => Ok(values.clone()),
            SqlRow::Empty => Ok(Vec::new()),
        }
    }
}
