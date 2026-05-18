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
        let micros = match value {
            Value::Integer(micros) => *micros,
            Value::Text(text) => parse_datetime_text_micros(text.as_ref())?,
            _ => return Err(Error::new(ErrorCode::Mismatch, "value is not a timestamp")),
        };
        system_time_from_epoch_micros(micros)
    }
}

fn system_time_from_epoch_micros(micros: i64) -> Result<std::time::SystemTime> {
    if micros >= 0 {
        match std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_micros(micros as u64)) {
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

fn parse_datetime_text_micros(text: &str) -> Result<i64> {
    parse_datetime_text_micros_inner(text)
        .and_then(|micros| i64::try_from(micros).ok())
        .ok_or_else(|| Error::new(ErrorCode::Mismatch, "invalid datetime text"))
}

fn parse_datetime_text_micros_inner(text: &str) -> Option<i128> {
    let input = text.trim();
    let bytes = input.as_bytes();
    if bytes.len() < 19
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !matches!(bytes.get(10), Some(b' ' | b'T'))
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }

    let year = parse_fixed_i32(input.get(0..4)?)?;
    let month = parse_fixed_u32(input.get(5..7)?)?;
    let day = parse_fixed_u32(input.get(8..10)?)?;
    let hour = parse_fixed_u32(input.get(11..13)?)?;
    let minute = parse_fixed_u32(input.get(14..16)?)?;
    let second = parse_fixed_u32(input.get(17..19)?)?;

    if !valid_date(year, month, day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let mut rest = &input[19..];
    let mut micros = 0_i128;
    if let Some(after_dot) = rest.strip_prefix('.') {
        let digits_len = after_dot
            .as_bytes()
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits_len == 0 || digits_len > 6 {
            return None;
        }
        let digits = &after_dot[..digits_len];
        micros = parse_fixed_u32(digits)? as i128;
        for _ in digits_len..6 {
            micros *= 10;
        }
        rest = &after_dot[digits_len..];
    }

    let offset_seconds = parse_timezone_offset_seconds(rest)?;
    let days = days_from_civil(year, month, day);
    let local_seconds =
        days * 86_400 + i128::from(hour) * 3_600 + i128::from(minute) * 60 + i128::from(second);
    Some((local_seconds - i128::from(offset_seconds)) * 1_000_000 + micros)
}

fn parse_fixed_i32(value: &str) -> Option<i32> {
    if value.as_bytes().iter().all(u8::is_ascii_digit) {
        value.parse().ok()
    } else {
        None
    }
}

fn parse_fixed_u32(value: &str) -> Option<u32> {
    if value.as_bytes().iter().all(u8::is_ascii_digit) {
        value.parse().ok()
    } else {
        None
    }
}

fn parse_timezone_offset_seconds(value: &str) -> Option<i32> {
    match value {
        "" | "Z" => Some(0),
        _ => {
            let sign = match value.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if value.len() != 6 || value.as_bytes().get(3) != Some(&b':') {
                return None;
            }
            let hour = parse_fixed_i32(value.get(1..3)?)?;
            let minute = parse_fixed_i32(value.get(4..6)?)?;
            if hour > 23 || minute > 59 {
                return None;
            }
            Some(sign * (hour * 3_600 + minute * 60))
        }
    }
}

fn valid_date(year: i32, month: u32, day: u32) -> bool {
    (1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i128 {
    let adjusted_year = i128::from(year) - i128::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year.rem_euclid(400);
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * i128::from(shifted_month) + 2) / 5 + i128::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
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
