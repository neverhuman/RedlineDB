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

// --- Postgres-style type bridges (each behind a Cargo feature) --------------
//
// All bridges store into one of the existing SQLite storage classes (no new
// Value variants), so the sqlite drop-in surface is unchanged. Consumers
// opt in via the `chrono`, `uuid`, `json`, or `decimal` Cargo features.

// SystemTime (always-on; no Cargo feature — std type)
impl From<std::time::SystemTime> for Value {
    fn from(value: std::time::SystemTime) -> Self {
        let micros = match value.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_micros()).unwrap_or(i64::MAX),
            Err(err) => -i64::try_from(err.duration().as_micros()).unwrap_or(i64::MAX),
        };
        Self::Integer(micros)
    }
}

impl TryFrom<&Value> for std::time::SystemTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        let micros = value.as_integer()?;
        if micros >= 0 {
            match std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_micros(micros as u64))
            {
                Some(t) => Ok(t),
                None => Err(Error::new(ErrorCode::Mismatch, "timestamp overflow")),
            }
        } else {
            match std::time::UNIX_EPOCH
                .checked_sub(std::time::Duration::from_micros(micros.unsigned_abs()))
            {
                Some(t) => Ok(t),
                None => Err(Error::new(ErrorCode::Mismatch, "timestamp underflow")),
            }
        }
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::DateTime<chrono::Utc>> for Value {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::Integer(value.timestamp_micros())
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<&Value> for chrono::DateTime<chrono::Utc> {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        let micros = value.as_integer()?;
        match chrono::DateTime::<chrono::Utc>::from_timestamp_micros(micros) {
            Some(dt) => Ok(dt),
            None => Err(Error::new(
                ErrorCode::Mismatch,
                "chrono timestamp out of range",
            )),
        }
    }
}

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for Value {
    fn from(value: uuid::Uuid) -> Self {
        use std::sync::Arc;
        Self::Blob(Arc::from(value.as_bytes().as_slice()))
    }
}

#[cfg(feature = "uuid")]
impl TryFrom<&Value> for uuid::Uuid {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        let bytes = value.as_blob()?;
        let arr: [u8; 16] = bytes
            .try_into()
            .map_err(|_| Error::new(ErrorCode::Mismatch, "uuid blob must be exactly 16 bytes"))?;
        Ok(uuid::Uuid::from_bytes(arr))
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        use std::sync::Arc;
        // Always serialize to canonical JSON text. Matches sqlite's JSON1
        // convention (TEXT storage class with parsed-on-access semantics).
        Self::Text(Arc::from(value.to_string()))
    }
}

#[cfg(feature = "json")]
impl TryFrom<&Value> for serde_json::Value {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        let text = value.as_text()?;
        serde_json::from_str(text)
            .map_err(|err| Error::new(ErrorCode::Mismatch, format!("invalid json: {err}")))
    }
}

#[cfg(feature = "decimal")]
impl From<rust_decimal::Decimal> for Value {
    fn from(value: rust_decimal::Decimal) -> Self {
        use std::sync::Arc;
        // Postgres NUMERIC convention: canonical text preserves precision.
        Self::Text(Arc::from(value.to_string()))
    }
}

#[cfg(feature = "decimal")]
impl TryFrom<&Value> for rust_decimal::Decimal {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        use std::str::FromStr;
        let text = value.as_text()?;
        rust_decimal::Decimal::from_str(text)
            .map_err(|err| Error::new(ErrorCode::Mismatch, format!("invalid decimal: {err}")))
    }
}
