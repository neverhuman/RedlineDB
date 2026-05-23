use super::super::*;

/// Inner helper that consults only the immediate row context. The public
/// `lookup_column` falls back to the correlated-scope stack on miss.
fn lookup_column_local(row: &RowContext<'_>, name: &str) -> Result<SqlValue> {
    match row {
        RowContext::Table(row) => lookup_table_column(row, name),
        RowContext::Upsert { current, .. } => lookup_table_column(current, name),
        RowContext::Joined(rows) => {
            let mut found = None;
            for row in rows.iter() {
                if let Ok(value) = lookup_joined_row_column(row, name) {
                    if found.as_ref().is_some_and(|existing| {
                        crate::value::compare_values(existing, &value) != std::cmp::Ordering::Equal
                    }) {
                        return Err(Error::UnsupportedSql(format!(
                            "ambiguous column name: {name}"
                        )));
                    }
                    found = Some(value);
                }
            }
            match found {
                Some(v) => Ok(v),
                None => Err(Error::UnknownColumn(name.to_owned())),
            }
        }
        RowContext::SqliteSchema(row) => match name.to_ascii_lowercase().as_str() {
            "type" => Ok(SqlValue::Text(Arc::from(row.type_name.as_ref()))),
            "name" => Ok(SqlValue::Text(Arc::from(row.name.as_ref()))),
            "tbl_name" => Ok(SqlValue::Text(Arc::from(row.tbl_name.as_ref()))),
            "rootpage" => Ok(SqlValue::Integer(row.rootpage as i64)),
            "sql" => Ok(SqlValue::Text(Arc::from(row.sql.as_ref()))),
            _ => Err(Error::UnknownColumn(name.to_owned())),
        },
        RowContext::Cte(row) => lookup_cte_column(row, name),
        RowContext::Empty => Err(Error::UnknownColumn(name.to_owned())),
    }
}

pub(crate) fn lookup_column(row: &RowContext<'_>, name: &str) -> Result<SqlValue> {
    match lookup_column_local(row, name) {
        Ok(v) => Ok(v),
        Err(Error::UnknownColumn(_)) => {
            match crate::exec::lookup_correlated(|outer| lookup_column_local(outer, name).ok()) {
                Some(v) => Ok(v),
                None => Err(Error::UnknownColumn(name.to_owned())),
            }
        }
        Err(other) => Err(other),
    }
}

/// Inner helper that consults only the immediate row context.
fn lookup_qualified_column_local(
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
            match found {
                Some(v) => Ok(v),
                None => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
            }
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
            "sqlite_schema" | "sqlite_master" | "redline_master" => lookup_schema_column(row, name),
            _ => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
        },
        RowContext::Cte(row) => {
            let matches = row
                .alias
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case(qualifier))
                .unwrap_or(false)
                || row.name.as_ref().eq_ignore_ascii_case(qualifier);
            if matches {
                lookup_cte_column(row, name)
            } else {
                Err(Error::UnknownColumn(format!("{qualifier}.{name}")))
            }
        }
        RowContext::Empty => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
    }
}

pub(crate) fn lookup_qualified_column(
    row: &RowContext<'_>,
    qualifier: &str,
    name: &str,
) -> Result<SqlValue> {
    match lookup_qualified_column_local(row, qualifier, name) {
        Ok(v) => Ok(v),
        Err(Error::UnknownColumn(_)) => {
            // Walk the correlated-subquery stack (innermost → outermost).
            match crate::exec::lookup_correlated(|outer| {
                lookup_qualified_column_local(outer, qualifier, name).ok()
            }) {
                Some(v) => Ok(v),
                None => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
            }
        }
        Err(other) => Err(other),
    }
}

fn lookup_cte_column(row: &CteRow, name: &str) -> Result<SqlValue> {
    if let Some(idx) = row
        .columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case(name))
    {
        return Ok(row.values.get(idx).cloned().unwrap_or(SqlValue::Null));
    }
    Err(Error::UnknownColumn(name.to_owned()))
}

fn matches_table_qualifier(alias: Option<&Arc<str>>, table: &TableDef, qualifier: &str) -> bool {
    if let Some(alias) = alias {
        return alias.as_ref().eq_ignore_ascii_case(qualifier);
    }
    table.name.to_string().eq_ignore_ascii_case(qualifier)
}

fn row_matches_qualifier(row: &TableRow, qualifier: &str) -> bool {
    matches_table_qualifier(row.alias.as_ref(), &row.table, qualifier)
}

fn row_matches_joined_qualifier(row: &JoinedRow, qualifier: &str) -> bool {
    matches_table_qualifier(row.alias.as_ref(), &row.table, qualifier)
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
    if row.table.is_public_rowid_name(name) {
        return Ok(SqlValue::Integer(row.rowid.0 as i64));
    }
    let idx = match row
        .table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
    {
        Some(i) => i,
        None => return Err(Error::UnknownColumn(name.to_owned())),
    };
    Ok(row.values[idx].clone())
}

fn lookup_joined_row_column(row: &JoinedRow, name: &str) -> Result<SqlValue> {
    match &row.row {
        Some(present) => lookup_table_column(present, name),
        None => {
            if row.table.is_public_rowid_name(name) {
                return Ok(SqlValue::Null);
            }
            match row
                .table
                .columns
                .iter()
                .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
            {
                Some(_) => {}
                None => return Err(Error::UnknownColumn(name.to_owned())),
            }
            Ok(SqlValue::Null)
        }
    }
}

fn lookup_excluded_column(table: &TableDef, excluded: &[SqlValue], name: &str) -> Result<SqlValue> {
    if table.is_public_rowid_name(name) {
        if let Some(alias) = table.rowid_alias_column
            && let Some(value) = excluded.get(alias as usize)
        {
            return Ok(value.clone());
        }
        return Err(Error::UnknownColumn(name.to_owned()));
    }
    let idx = match table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
    {
        Some(i) => i,
        None => return Err(Error::UnknownColumn(name.to_owned())),
    };
    Ok(excluded.get(idx).cloned().unwrap_or(SqlValue::Null))
}
