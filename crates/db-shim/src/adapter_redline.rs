use redlinedb_client::{Client, Error as RedlineError, Value as RedlineValue};

use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    GOVERNED_CAPABILITIES,
};

pub(super) fn open(dsn: &str) -> Result<Box<dyn Backend>> {
    let address = dsn
        .strip_prefix("redline://")
        .or_else(|| dsn.strip_prefix("redlinedb://"))
        .unwrap_or(dsn);
    let client = Client::connect(address).map_err(map_connection_error)?;
    Ok(Box::new(RedlineBackend { client }))
}

struct RedlineBackend {
    client: Client,
}

impl Backend for RedlineBackend {
    fn capabilities(&self) -> Capabilities {
        GOVERNED_CAPABILITIES
    }

    fn execute(&mut self, statement: &Statement) -> Result<u64> {
        let sql = statement.render(PlaceholderStyle::Question);
        if statement.params().is_empty() {
            self.client.execute(&sql).map_err(map_query_error)
        } else {
            let params = statement
                .params()
                .iter()
                .map(to_redline)
                .collect::<Vec<_>>();
            self.client
                .execute_params(&sql, &params)
                .map_err(map_query_error)?;
            Ok(0)
        }
    }

    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        let sql = statement.render(PlaceholderStyle::Question);
        let params = statement
            .params()
            .iter()
            .map(to_redline)
            .collect::<Vec<_>>();
        self.client
            .query(&sql, &params)
            .map(|result| {
                result
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(from_redline).collect())
                    .collect()
            })
            .map_err(map_query_error)
    }

    fn begin(&mut self, mode: TransactionMode) -> Result<()> {
        match mode {
            TransactionMode::Atomic => self
                .client
                .begin(Some("immediate"))
                .map_err(map_query_error),
        }
    }

    fn commit(&mut self) -> Result<()> {
        self.client.commit().map_err(map_query_error)
    }

    fn rollback(&mut self) -> Result<()> {
        self.client.rollback().map_err(map_query_error)
    }
}

fn to_redline(value: &Value) -> RedlineValue {
    match value {
        Value::Null => RedlineValue::Null,
        Value::Integer(value) => RedlineValue::Integer(*value),
        Value::Real(value) => RedlineValue::Real(*value),
        Value::Text(value) => RedlineValue::Text(value.clone()),
        Value::Blob(value) => RedlineValue::Blob(value.clone()),
    }
}

fn from_redline(value: RedlineValue) -> Value {
    match value {
        RedlineValue::Null => Value::Null,
        RedlineValue::Integer(value) => Value::Integer(value),
        RedlineValue::Real(value) => Value::Real(value),
        RedlineValue::Text(value) => Value::Text(value),
        RedlineValue::Blob(value) => Value::Blob(value),
    }
}

fn map_connection_error(error: RedlineError) -> Error {
    match error {
        RedlineError::Io(error) => Error::Connection(error.to_string()),
        RedlineError::Protocol(message) => Error::Protocol(message),
        RedlineError::Server { code, message } => {
            Error::Connection(format!("server[{code}]: {message}"))
        }
    }
}

fn map_query_error(error: RedlineError) -> Error {
    match error {
        RedlineError::Io(error) => Error::Connection(error.to_string()),
        RedlineError::Protocol(message) => Error::Protocol(message),
        RedlineError::Server { code, message } => {
            Error::Query(format!("server[{code}]: {message}"))
        }
    }
}
