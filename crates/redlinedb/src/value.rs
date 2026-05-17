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

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Integer(i64::from(value))
    }
}

/// `u64` may exceed `i64::MAX`; conversion fails for values above `2^63 - 1`.
impl TryFrom<u64> for Value {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self> {
        i64::try_from(value)
            .map(Self::Integer)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "u64 value exceeds i64 range"))
    }
}

/// `usize` may exceed `i64::MAX` on 64-bit targets when above `2^63 - 1`.
impl TryFrom<usize> for Value {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        i64::try_from(value)
            .map(Self::Integer)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "usize value exceeds i64 range"))
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Real(f64::from(value))
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

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        Self::Text(Arc::from(value.as_str()))
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

impl From<&Vec<u8>> for Value {
    fn from(value: &Vec<u8>) -> Self {
        Self::Blob(Arc::from(value.as_slice()))
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

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => v.into(),
            None => Self::Null,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_to_integer() {
        assert_eq!(Value::from(true), Value::Integer(1));
        assert_eq!(Value::from(false), Value::Integer(0));
    }

    #[test]
    fn signed_widening() {
        assert_eq!(Value::from(7_i8), Value::Integer(7));
        assert_eq!(Value::from(-1_i16), Value::Integer(-1));
        assert_eq!(Value::from(i32::MAX), Value::Integer(i64::from(i32::MAX)));
        assert_eq!(Value::from(i64::MIN), Value::Integer(i64::MIN));
    }

    #[test]
    fn unsigned_widening() {
        assert_eq!(Value::from(u8::MAX), Value::Integer(255));
        assert_eq!(Value::from(u16::MAX), Value::Integer(65_535));
        assert_eq!(Value::from(u32::MAX), Value::Integer(i64::from(u32::MAX)));
    }

    #[test]
    fn try_from_u64_ok() {
        let v = Value::try_from(42_u64).unwrap();
        assert_eq!(v, Value::Integer(42));
    }

    #[test]
    fn try_from_u64_overflow() {
        let v = Value::try_from(u64::MAX);
        assert!(v.is_err());
    }

    #[test]
    fn try_from_usize_ok() {
        let v = Value::try_from(123_usize).unwrap();
        assert_eq!(v, Value::Integer(123));
    }

    #[test]
    fn f32_widens_to_real() {
        assert_eq!(Value::from(1.5_f32), Value::Real(1.5));
    }

    #[test]
    fn string_ref_to_text() {
        let s = String::from("hello");
        let v = Value::from(&s);
        assert_eq!(v, Value::Text(Arc::from("hello")));
    }

    #[test]
    fn vec_ref_to_blob() {
        let bytes = vec![1_u8, 2, 3];
        let v = Value::from(&bytes);
        assert_eq!(v, Value::Blob(Arc::from(&[1_u8, 2, 3][..])));
    }

    #[test]
    fn option_some_delegates_to_inner() {
        let v: Value = Some(42_i32).into();
        assert_eq!(v, Value::Integer(42));
    }

    #[test]
    fn option_none_is_null() {
        let v: Value = Option::<i32>::None.into();
        assert_eq!(v, Value::Null);
    }
}
