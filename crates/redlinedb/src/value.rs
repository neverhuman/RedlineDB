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
            match std::time::UNIX_EPOCH
                .checked_add(std::time::Duration::from_micros(micros as u64))
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
            None => Err(Error::new(ErrorCode::Mismatch, "chrono timestamp out of range")),
        }
    }
}

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for Value {
    fn from(value: uuid::Uuid) -> Self {
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

    #[test]
    fn is_null_matches_only_null() {
        assert!(Value::Null.is_null());
        assert!(!Value::Integer(0).is_null());
        assert!(!Value::Text(Arc::from("")).is_null());
    }

    #[test]
    fn as_accessors_typed() {
        assert_eq!(Value::Integer(42).as_integer().unwrap(), 42);
        assert_eq!(Value::Real(3.14).as_real().unwrap(), 3.14);
        assert_eq!(Value::Text(Arc::from("x")).as_text().unwrap(), "x");
        assert_eq!(Value::Blob(Arc::from(&b"yz"[..])).as_blob().unwrap(), b"yz");
    }

    #[test]
    fn as_accessors_mismatch() {
        assert!(Value::Null.as_integer().is_err());
        assert!(Value::Integer(0).as_text().is_err());
        assert!(Value::Text(Arc::from("a")).as_blob().is_err());
    }

    #[test]
    fn try_from_signed_narrowing() {
        let v = Value::Integer(7);
        assert_eq!(i64::try_from(&v).unwrap(), 7);
        assert_eq!(i32::try_from(&v).unwrap(), 7);
        assert_eq!(i16::try_from(&v).unwrap(), 7);
        assert_eq!(i8::try_from(&v).unwrap(), 7);

        let big = Value::Integer(i64::from(i32::MAX) + 1);
        assert!(i32::try_from(&big).is_err());
    }

    #[test]
    fn try_from_unsigned_narrowing() {
        let v = Value::Integer(7);
        assert_eq!(u64::try_from(&v).unwrap(), 7);
        assert_eq!(u32::try_from(&v).unwrap(), 7);
        assert_eq!(u16::try_from(&v).unwrap(), 7);
        assert_eq!(u8::try_from(&v).unwrap(), 7);

        let neg = Value::Integer(-1);
        assert!(u64::try_from(&neg).is_err());
    }

    #[test]
    fn try_from_bool() {
        assert!(bool::try_from(&Value::Integer(1)).unwrap());
        assert!(!bool::try_from(&Value::Integer(0)).unwrap());
        assert!(bool::try_from(&Value::Integer(42)).unwrap());
    }

    #[test]
    fn try_from_owned_string_and_vec() {
        let s = Value::Text(Arc::from("hi"));
        assert_eq!(String::try_from(&s).unwrap(), "hi");

        let b = Value::Blob(Arc::from(&[9_u8, 8][..]));
        assert_eq!(Vec::<u8>::try_from(&b).unwrap(), vec![9, 8]);
    }

    #[test]
    fn system_time_round_trips_through_value() {
        let now = std::time::SystemTime::now();
        let v: Value = now.into();
        let back = std::time::SystemTime::try_from(&v).unwrap();
        // Round-trip precision is microseconds (Value::Integer epoch micros).
        let diff = match (now.duration_since(std::time::UNIX_EPOCH), back.duration_since(std::time::UNIX_EPOCH)) {
            (Ok(a), Ok(b)) => a.as_micros().abs_diff(b.as_micros()),
            _ => u128::MAX,
        };
        assert!(diff <= 1);
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_datetime_round_trips_through_value() {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_micros(1_700_000_000_000_000).unwrap();
        let v: Value = dt.into();
        let back: chrono::DateTime<chrono::Utc> = (&v).try_into().unwrap();
        assert_eq!(dt, back);
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn uuid_round_trips_through_value_as_16_byte_blob() {
        let id = uuid::Uuid::from_u128(0x0123_4567_89ab_cdef_0011_2233_4455_6677_u128);
        let v: Value = id.into();
        let back: uuid::Uuid = (&v).try_into().unwrap();
        assert_eq!(id, back);
        match v {
            Value::Blob(bytes) => assert_eq!(bytes.len(), 16),
            _ => panic!("expected blob storage"),
        }
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn uuid_from_invalid_blob_length_errors() {
        let v = Value::Blob(Arc::from(&[0_u8; 15][..]));
        assert!(uuid::Uuid::try_from(&v).is_err());
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_round_trips_through_value_as_text() {
        let j = serde_json::json!({"name": "Ada", "count": 42, "active": true});
        let v: Value = j.clone().into();
        let back: serde_json::Value = (&v).try_into().unwrap();
        assert_eq!(j, back);
    }

    #[cfg(feature = "decimal")]
    #[test]
    fn decimal_round_trips_through_value_as_text() {
        use std::str::FromStr;
        let d = rust_decimal::Decimal::from_str("12345.6789").unwrap();
        let v: Value = d.into();
        let back: rust_decimal::Decimal = (&v).try_into().unwrap();
        assert_eq!(d, back);
    }
}
