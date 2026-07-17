use redlinedb_client::{Client, Error as RedlineError, Value as RedlineValue};

use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    ValueType, GOVERNED_CAPABILITIES,
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

    fn execute(&mut self, statement: &Statement) -> Result<()> {
        let sql = statement.render(PlaceholderStyle::Question);
        if statement.params().is_empty() {
            self.client
                .execute(&sql)
                .map(|_| ())
                .map_err(map_query_error)
        } else {
            let params = statement
                .params()
                .iter()
                .map(to_redline)
                .collect::<Vec<_>>();
            self.client
                .execute_params(&sql, &params)
                .map_err(map_query_error)
        }
    }

    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        let sql = statement.render(PlaceholderStyle::Question);
        let params = statement
            .params()
            .iter()
            .map(to_redline)
            .collect::<Vec<_>>();
        let result = self.client.query(&sql, &params).map_err(map_query_error)?;
        let column_count = result.columns.len();
        let result_types = statement.result_types_for_columns(column_count)?;
        result
            .rows
            .into_iter()
            .map(|row| {
                if row.len() != column_count {
                    return Err(Error::Protocol(format!(
                        "query returned {} values for {} columns",
                        row.len(),
                        column_count
                    )));
                }
                row.into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        from_redline(value, result_types.map(|types| types[index]))
                    })
                    .collect()
            })
            .collect()
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
        Value::Null(_) => RedlineValue::Null,
        Value::Integer(value) => RedlineValue::Integer(*value),
        Value::Real(value) => RedlineValue::Real(*value),
        Value::Text(value) => RedlineValue::Text(value.clone()),
        Value::Blob(value) => RedlineValue::Blob(value.clone()),
    }
}

fn from_redline(value: RedlineValue, expected: Option<ValueType>) -> Result<Value> {
    let value = match value {
        RedlineValue::Null => Value::Null(expected.ok_or_else(|| {
            Error::Contract("Redline null result requires a declared portable type".to_owned())
        })?),
        RedlineValue::Integer(value) => Value::Integer(value),
        RedlineValue::Real(value) => Value::Real(value),
        RedlineValue::Text(value) => Value::Text(value),
        RedlineValue::Blob(value) => Value::Blob(value),
    };
    if expected.is_some_and(|expected| expected != value.value_type()) {
        return Err(Error::Contract(format!(
            "query result type {:?} differs from declared {expected:?}",
            value.value_type()
        )));
    }
    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redline_nulls_require_and_preserve_the_declared_type() {
        for value_type in [
            ValueType::Integer,
            ValueType::Real,
            ValueType::Text,
            ValueType::Blob,
        ] {
            assert_eq!(
                from_redline(RedlineValue::Null, Some(value_type)).unwrap(),
                Value::Null(value_type)
            );
        }
        assert!(from_redline(RedlineValue::Null, None).is_err());
    }
}
