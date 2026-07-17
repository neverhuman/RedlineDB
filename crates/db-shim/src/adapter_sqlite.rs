use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    ValueType, GOVERNED_CAPABILITIES,
};

pub(super) fn open(dsn: &str) -> Result<Box<dyn Backend>> {
    let connection = if dsn.is_empty() || dsn == ":memory:" {
        rusqlite::Connection::open_in_memory()
    } else {
        rusqlite::Connection::open(dsn)
    }
    .map_err(|error| Error::Connection(error.to_string()))?;
    connection
        .execute_batch("PRAGMA foreign_keys=ON")
        .map_err(map_query_error)?;
    Ok(Box::new(SqliteBackend { connection }))
}

struct SqliteBackend {
    connection: rusqlite::Connection,
}

impl Backend for SqliteBackend {
    fn capabilities(&self) -> Capabilities {
        GOVERNED_CAPABILITIES
    }

    fn execute(&mut self, statement: &Statement) -> Result<()> {
        let sql = statement.render(PlaceholderStyle::Question);
        self.connection
            .execute(
                &sql,
                rusqlite::params_from_iter(statement.params().iter().map(to_sqlite)),
            )
            .map(|_| ())
            .map_err(map_query_error)
    }

    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        let sql = statement.render(PlaceholderStyle::Question);
        let mut prepared = self.connection.prepare(&sql).map_err(map_query_error)?;
        let columns = prepared.column_count();
        let result_types = statement
            .result_types_for_columns(columns)?
            .map(<[ValueType]>::to_vec);
        let mut rows = prepared
            .query(rusqlite::params_from_iter(
                statement.params().iter().map(to_sqlite),
            ))
            .map_err(map_query_error)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(map_query_error)? {
            let values = (0..columns)
                .map(|index| {
                    let value = row.get_ref(index).map_err(map_query_error)?;
                    from_sqlite(value, result_types.as_ref().map(|types| types[index]))
                })
                .collect::<Result<Vec<_>>>()?;
            result.push(values);
        }
        Ok(result)
    }

    fn begin(&mut self, mode: TransactionMode) -> Result<()> {
        match mode {
            TransactionMode::Atomic => self
                .connection
                .execute_batch("BEGIN IMMEDIATE")
                .map_err(map_query_error),
        }
    }

    fn commit(&mut self) -> Result<()> {
        self.connection
            .execute_batch("COMMIT")
            .map_err(map_query_error)
    }

    fn rollback(&mut self) -> Result<()> {
        self.connection
            .execute_batch("ROLLBACK")
            .map_err(map_query_error)
    }
}

fn to_sqlite(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null(_) => rusqlite::types::Value::Null,
        Value::Integer(value) => rusqlite::types::Value::Integer(*value),
        Value::Real(value) => rusqlite::types::Value::Real(*value),
        Value::Text(value) => rusqlite::types::Value::Text(value.clone()),
        Value::Blob(value) => rusqlite::types::Value::Blob(value.clone()),
    }
}

fn from_sqlite(value: rusqlite::types::ValueRef<'_>, expected: Option<ValueType>) -> Result<Value> {
    use rusqlite::types::ValueRef;
    let value = match value {
        ValueRef::Null => Value::Null(expected.ok_or_else(|| {
            Error::Contract("SQLite null result requires a declared portable type".to_owned())
        })?),
        ValueRef::Integer(value) => Value::Integer(value),
        ValueRef::Real(value) => Value::Real(value),
        ValueRef::Text(value) => Value::Text(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::Blob(value.to_vec()),
    };
    if expected.is_some_and(|expected| expected != value.value_type()) {
        return Err(Error::Contract(format!(
            "query result type {:?} differs from declared {expected:?}",
            value.value_type()
        )));
    }
    Ok(value)
}

fn map_query_error(error: rusqlite::Error) -> Error {
    Error::Query(error.to_string())
}
