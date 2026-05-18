use std::sync::Arc;

use crate::error::{Error, ErrorCode, Result};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(Arc<str>),
    Blob(Arc<[u8]>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a str),
    Blob(&'a [u8]),
}

impl ValueRef<'_> {
    pub fn as_str(&self) -> Result<&str> {
        match self {
            Self::Text(value) => Ok(value),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not text")),
        }
    }

    pub fn as_blob(&self) -> Result<&[u8]> {
        match self {
            Self::Blob(value) => Ok(value),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not blob")),
        }
    }
}

impl Value {
    pub fn as_ref(&self) -> ValueRef<'_> {
        match self {
            Self::Null => ValueRef::Null,
            Self::Integer(value) => ValueRef::Integer(*value),
            Self::Real(value) => ValueRef::Real(*value),
            Self::Text(value) => ValueRef::Text(value.as_ref()),
            Self::Blob(value) => ValueRef::Blob(value.as_ref()),
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(Arc::from(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(Arc::from(value))
    }
}

impl From<Arc<str>> for Value {
    fn from(value: Arc<str>) -> Self {
        Self::Text(value)
    }
}

impl From<&[u8]> for Value {
    fn from(value: &[u8]) -> Self {
        Self::Blob(Arc::from(value))
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(Arc::from(value.into_boxed_slice()))
    }
}

impl From<Arc<[u8]>> for Value {
    fn from(value: Arc<[u8]>) -> Self {
        Self::Blob(value)
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Self::Null
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
