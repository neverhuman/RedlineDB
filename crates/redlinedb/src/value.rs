use std::sync::Arc;

use crate::error::{Error, ErrorCode, Result};

#[path = "value_conv.rs"]
mod conv;

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

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn as_integer(&self) -> Result<i64> {
        match self {
            Self::Integer(value) => Ok(*value),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not integer")),
        }
    }

    pub fn as_real(&self) -> Result<f64> {
        match self {
            Self::Real(value) => Ok(*value),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not real")),
        }
    }

    pub fn as_text(&self) -> Result<&str> {
        match self {
            Self::Text(value) => Ok(value.as_ref()),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not text")),
        }
    }

    pub fn as_blob(&self) -> Result<&[u8]> {
        match self {
            Self::Blob(value) => Ok(value.as_ref()),
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not blob")),
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

#[cfg(test)]
#[path = "value/tests.rs"]
mod tests;
