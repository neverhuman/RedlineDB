use super::super::*;

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

/// A row drawn from a CTE / named-subquery: its values are paired with
/// the CTE's column names so qualified and bare identifier lookups work.
#[derive(Clone)]
pub(crate) struct CteRow {
    pub(crate) name: Arc<str>,
    pub(crate) alias: Option<Arc<str>>,
    pub(crate) columns: Arc<[String]>,
    pub(crate) values: Vec<SqlValue>,
}

#[derive(Clone)]
pub(crate) enum SqlRow {
    Table(TableRow),
    Joined(Vec<JoinedRow>),
    SqliteSchema(SqliteSchemaRow),
    Static(Vec<SqlValue>),
    Cte(CteRow),
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
    Cte(&'a CteRow),
    Empty,
}

impl<'a> RowContext<'a> {
    /// Materialise an owned `SqlRow` snapshot of this context so it can be
    /// stashed on the correlated-subquery stack (where the borrow can't
    /// be carried across thread-local storage).
    pub(crate) fn to_owned_row(&self) -> SqlRow {
        match self {
            RowContext::Table(row) => SqlRow::Table((*row).clone()),
            RowContext::Joined(rows) => SqlRow::Joined(rows.to_vec()),
            RowContext::Upsert { current, .. } => SqlRow::Table((*current).clone()),
            RowContext::SqliteSchema(row) => SqlRow::SqliteSchema((*row).clone()),
            RowContext::Cte(row) => SqlRow::Cte((*row).clone()),
            RowContext::Empty => SqlRow::Empty,
        }
    }
}

impl SqlRow {
    pub(crate) fn context(&self) -> RowContext<'_> {
        match self {
            SqlRow::Table(row) => RowContext::Table(row),
            SqlRow::Joined(rows) => RowContext::Joined(rows),
            SqlRow::SqliteSchema(row) => RowContext::SqliteSchema(row),
            SqlRow::Cte(row) => RowContext::Cte(row),
            SqlRow::Static(_) | SqlRow::Empty => RowContext::Empty,
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
            SqlRow::Cte(row) => Ok(row.values.clone()),
            SqlRow::Empty => Ok(Vec::new()),
        }
    }
}
