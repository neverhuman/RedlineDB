use super::{Value, ValueRef};
use crate::error::{Error, ErrorCode, Result};

impl TryFrom<&Value> for i64 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        value.as_integer()
    }
}

impl TryFrom<&Value> for i32 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        i32::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i32"))
    }
}

impl TryFrom<&Value> for i16 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        i16::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i16"))
    }
}

impl TryFrom<&Value> for i8 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        i8::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i8"))
    }
}

impl TryFrom<&Value> for u64 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        u64::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u64"))
    }
}

impl TryFrom<&Value> for u32 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        u32::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u32"))
    }
}

impl TryFrom<&Value> for u16 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        u16::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u16"))
    }
}

impl TryFrom<&Value> for u8 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        u8::try_from(value.as_integer()?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u8"))
    }
}

impl TryFrom<&Value> for bool {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        value.as_integer().map(|v| v != 0)
    }
}

impl TryFrom<&Value> for f64 {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        value.as_real()
    }
}

impl TryFrom<&Value> for String {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        value.as_text().map(str::to_owned)
    }
}

impl TryFrom<&Value> for Vec<u8> {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        value.as_blob().map(<[u8]>::to_vec)
    }
}

impl From<redlinedb_sql::SqlValue> for Value {
    fn from(value: redlinedb_sql::SqlValue) -> Self {
        match value {
            redlinedb_sql::SqlValue::Null => Self::Null,
            redlinedb_sql::SqlValue::Integer(value) => Self::Integer(value),
            redlinedb_sql::SqlValue::Real(value) => Self::Real(value),
            redlinedb_sql::SqlValue::Text(value) => Self::Text(value),
            redlinedb_sql::SqlValue::Blob(value) => Self::Blob(value),
        }
    }
}

impl From<Value> for redlinedb_sql::SqlValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Integer(value) => Self::Integer(value),
            Value::Real(value) => Self::Real(value),
            Value::Text(value) => Self::Text(value),
            Value::Blob(value) => Self::Blob(value),
        }
    }
}

impl<'a> From<redlinedb_sql::SqlValueRef<'a>> for ValueRef<'a> {
    fn from(value: redlinedb_sql::SqlValueRef<'a>) -> Self {
        match value {
            redlinedb_sql::SqlValueRef::Null => Self::Null,
            redlinedb_sql::SqlValueRef::Integer(value) => Self::Integer(value),
            redlinedb_sql::SqlValueRef::Real(value) => Self::Real(value),
            redlinedb_sql::SqlValueRef::Text(value) => Self::Text(value),
            redlinedb_sql::SqlValueRef::Blob(value) => Self::Blob(value),
        }
    }
}

#[path = "value_conv/postgres.rs"]
mod postgres;
