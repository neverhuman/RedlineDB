use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    GOVERNED_CAPABILITIES,
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

    fn execute(&mut self, statement: &Statement) -> Result<u64> {
        let sql = statement.render(PlaceholderStyle::Question);
        self.connection
            .execute(
                &sql,
                rusqlite::params_from_iter(statement.params().iter().map(to_sqlite)),
            )
            .map(|rows| rows as u64)
            .map_err(map_query_error)
    }

    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        let sql = statement.render(PlaceholderStyle::Question);
        let mut prepared = self.connection.prepare(&sql).map_err(map_query_error)?;
        let columns = prepared.column_count();
        let rows = prepared
            .query_map(
                rusqlite::params_from_iter(statement.params().iter().map(to_sqlite)),
                |row| {
                    (0..columns)
                        .map(|index| row.get_ref(index).map(from_sqlite))
                        .collect::<std::result::Result<Vec<_>, _>>()
                },
            )
            .map_err(map_query_error)?;
        rows.map(|row| row.map_err(map_query_error)).collect()
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
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(value) => rusqlite::types::Value::Integer(*value),
        Value::Real(value) => rusqlite::types::Value::Real(*value),
        Value::Text(value) => rusqlite::types::Value::Text(value.clone()),
        Value::Blob(value) => rusqlite::types::Value::Blob(value.clone()),
    }
}

fn from_sqlite(value: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::Integer(value),
        ValueRef::Real(value) => Value::Real(value),
        ValueRef::Text(value) => Value::Text(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::Blob(value.to_vec()),
    }
}

fn map_query_error(error: rusqlite::Error) -> Error {
    Error::Query(error.to_string())
}
