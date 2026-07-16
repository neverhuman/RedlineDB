use postgres::{
    types::{ToSql, Type},
    Client, NoTls, Row,
};

use crate::{
    Backend, Capabilities, Error, PlaceholderStyle, Result, Statement, TransactionMode, Value,
    GOVERNED_CAPABILITIES,
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

    fn execute(&mut self, statement: &Statement) -> Result<u64> {
        let sql = statement.render(PlaceholderStyle::DollarNumbered);
        let params = postgres_params(statement.params());
        let refs = postgres_param_refs(&params);
        self.client
            .execute(&sql, &refs)
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
            .map(row_values)
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
            Value::Null => Box::new(None::<String>) as Box<dyn ToSql + Sync>,
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

fn row_values(row: &Row) -> Result<Vec<Value>> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| postgres_value(row, index, column.type_()))
        .collect()
}

fn postgres_value(row: &Row, index: usize, value_type: &Type) -> Result<Value> {
    macro_rules! optional {
        ($rust_type:ty, $map:expr) => {
            row.try_get::<_, Option<$rust_type>>(index)
                .map(|value| value.map($map).unwrap_or(Value::Null))
                .map_err(|error| Error::Query(error.to_string()))
        };
    }
    match *value_type {
        Type::INT2 => optional!(i16, |value| Value::Integer(i64::from(value))),
        Type::INT4 => optional!(i32, |value| Value::Integer(i64::from(value))),
        Type::INT8 => optional!(i64, Value::Integer),
        Type::FLOAT4 => optional!(f32, |value| Value::Real(f64::from(value))),
        Type::FLOAT8 => optional!(f64, Value::Real),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => optional!(String, Value::Text),
        Type::BYTEA => optional!(Vec<u8>, Value::Blob),
        _ => Err(Error::Unsupported(format!(
            "Postgres result type {value_type} is outside the governed value contract"
        ))),
    }
}
