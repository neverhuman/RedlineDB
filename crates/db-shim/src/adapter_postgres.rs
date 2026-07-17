use postgres::{
    types::{ToSql, Type},
    Client, NoTls, Row,
};

use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    ValueType, GOVERNED_CAPABILITIES,
};

pub(super) fn open(dsn: &str) -> Result<Box<dyn Backend>> {
    let client =
        Client::connect(dsn, NoTls).map_err(|error| Error::Connection(error.to_string()))?;
    Ok(Box::new(PostgresBackend { client }))
}

struct PostgresBackend {
    client: Client,
}

impl Backend for PostgresBackend {
    fn capabilities(&self) -> Capabilities {
        GOVERNED_CAPABILITIES
    }

    fn execute(&mut self, statement: &Statement) -> Result<()> {
        let sql = statement.render(PlaceholderStyle::DollarNumbered);
        let params = postgres_params(statement.params());
        let refs = postgres_param_refs(&params);
        self.client
            .execute(&sql, &refs)
            .map(|_| ())
            .map_err(|error| Error::Query(error.to_string()))
    }

    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        let sql = statement.render(PlaceholderStyle::DollarNumbered);
        let params = postgres_params(statement.params());
        let refs = postgres_param_refs(&params);
        self.client
            .query(&sql, &refs)
            .map_err(|error| Error::Query(error.to_string()))?
            .iter()
            .map(|row| row_values(row, statement))
            .collect()
    }

    fn begin(&mut self, mode: TransactionMode) -> Result<()> {
        match mode {
            TransactionMode::Atomic => self
                .client
                .batch_execute("BEGIN")
                .map_err(|error| Error::Query(error.to_string())),
        }
    }

    fn commit(&mut self) -> Result<()> {
        self.client
            .batch_execute("COMMIT")
            .map_err(|error| Error::Query(error.to_string()))
    }

    fn rollback(&mut self) -> Result<()> {
        self.client
            .batch_execute("ROLLBACK")
            .map_err(|error| Error::Query(error.to_string()))
    }
}

fn postgres_params(values: &[Value]) -> Vec<Box<dyn ToSql + Sync>> {
    values
        .iter()
        .map(|value| match value {
            Value::Null(ValueType::Integer) => Box::new(None::<i64>) as Box<dyn ToSql + Sync>,
            Value::Null(ValueType::Real) => Box::new(None::<f64>),
            Value::Null(ValueType::Text) => Box::new(None::<String>),
            Value::Null(ValueType::Blob) => Box::new(None::<Vec<u8>>),
            Value::Integer(value) => Box::new(*value),
            Value::Real(value) => Box::new(*value),
            Value::Text(value) => Box::new(value.clone()),
            Value::Blob(value) => Box::new(value.clone()),
        })
        .collect()
}

fn postgres_param_refs(params: &[Box<dyn ToSql + Sync>]) -> Vec<&(dyn ToSql + Sync)> {
    params.iter().map(|param| param.as_ref()).collect()
}

fn row_values(row: &Row, statement: &Statement) -> Result<Vec<Value>> {
    let expected = statement.result_types_for_columns(row.columns().len())?;
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| {
            postgres_value(
                row,
                index,
                column.type_(),
                expected.map(|types| types[index]),
            )
        })
        .collect()
}

fn postgres_value(
    row: &Row,
    index: usize,
    postgres_type: &Type,
    expected: Option<ValueType>,
) -> Result<Value> {
    let value_type = postgres_value_type(postgres_type)?;
    if expected.is_some_and(|expected| expected != value_type) {
        return Err(Error::Contract(format!(
            "query result type {value_type:?} differs from declared {expected:?}"
        )));
    }
    macro_rules! optional {
        ($rust_type:ty, $map:expr) => {
            row.try_get::<_, Option<$rust_type>>(index)
                .map(|value| value.map($map).unwrap_or(Value::Null(value_type)))
                .map_err(|error| Error::Query(error.to_string()))
        };
    }
    match *postgres_type {
        Type::INT2 => optional!(i16, |value| Value::Integer(i64::from(value))),
        Type::INT4 => optional!(i32, |value| Value::Integer(i64::from(value))),
        Type::INT8 => optional!(i64, Value::Integer),
        Type::FLOAT4 => optional!(f32, |value| Value::Real(f64::from(value))),
        Type::FLOAT8 => optional!(f64, Value::Real),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => optional!(String, Value::Text),
        Type::BYTEA => optional!(Vec<u8>, Value::Blob),
        _ => Err(Error::Unsupported(format!(
            "Postgres result type {postgres_type} is outside the governed value contract"
        ))),
    }
}

fn postgres_value_type(value_type: &Type) -> Result<ValueType> {
    match *value_type {
        Type::INT2 | Type::INT4 | Type::INT8 => Ok(ValueType::Integer),
        Type::FLOAT4 | Type::FLOAT8 => Ok(ValueType::Real),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => Ok(ValueType::Text),
        Type::BYTEA => Ok(ValueType::Blob),
        _ => Err(Error::Unsupported(format!(
            "Postgres result type {value_type} is outside the governed value contract"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use postgres::types::{private::BytesMut, IsNull};

    use super::*;

    #[test]
    fn typed_null_parameters_accept_only_their_postgres_targets() {
        for (value, postgres_type) in [
            (Value::Null(ValueType::Integer), Type::INT8),
            (Value::Null(ValueType::Real), Type::FLOAT8),
            (Value::Null(ValueType::Text), Type::TEXT),
            (Value::Null(ValueType::Blob), Type::BYTEA),
        ] {
            let params = postgres_params(&[value]);
            let mut output = BytesMut::new();
            assert!(matches!(
                params[0].to_sql_checked(&postgres_type, &mut output),
                Ok(IsNull::Yes)
            ));
            assert!(output.is_empty());
        }

        let params = postgres_params(&[Value::Null(ValueType::Blob)]);
        assert!(params[0]
            .to_sql_checked(&Type::INT8, &mut BytesMut::new())
            .is_err());
    }

    #[test]
    fn postgres_result_types_map_to_the_portable_contract() {
        for (postgres_type, expected) in [
            (Type::INT2, ValueType::Integer),
            (Type::INT4, ValueType::Integer),
            (Type::INT8, ValueType::Integer),
            (Type::FLOAT4, ValueType::Real),
            (Type::FLOAT8, ValueType::Real),
            (Type::TEXT, ValueType::Text),
            (Type::BYTEA, ValueType::Blob),
        ] {
            assert_eq!(postgres_value_type(&postgres_type).unwrap(), expected);
        }
        assert!(postgres_value_type(&Type::BOOL).is_err());
    }
}
