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
    let diff = match (
        now.duration_since(std::time::UNIX_EPOCH),
        back.duration_since(std::time::UNIX_EPOCH),
    ) {
        (Ok(a), Ok(b)) => a.as_micros().abs_diff(b.as_micros()),
        _ => u128::MAX,
    };
    assert!(diff <= 1);
}

#[test]
fn system_time_integer_epoch_micros_round_trips() {
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_micros(1_700_000_000_123_456);
    let value: Value = time.into();
    assert_eq!(value, Value::Integer(1_700_000_000_123_456));
    assert_eq!(std::time::SystemTime::try_from(&value).unwrap(), time);
}

#[test]
fn system_time_accepts_rfc3339_z_datetime_text() {
    let value = Value::from("1970-01-01T00:00:00Z");
    assert_eq!(
        std::time::SystemTime::try_from(&value).unwrap(),
        std::time::UNIX_EPOCH
    );
}

#[test]
fn system_time_accepts_sqlite_datetime_text_as_utc() {
    let value = Value::from("1970-01-01 00:00:00");
    assert_eq!(
        std::time::SystemTime::try_from(&value).unwrap(),
        std::time::UNIX_EPOCH
    );
}

#[test]
fn system_time_preserves_fractional_microseconds_from_text() {
    let value = Value::from("1970-01-01T00:00:01.123456Z");
    assert_eq!(
        micros_since_epoch(std::time::SystemTime::try_from(&value).unwrap()),
        1_123_456
    );
}

#[test]
fn system_time_normalizes_offset_datetime_text_to_utc() {
    let value = Value::from("1970-01-01T01:30:00+01:30");
    assert_eq!(
        std::time::SystemTime::try_from(&value).unwrap(),
        std::time::UNIX_EPOCH
    );

    let value = Value::from("1969-12-31T18:30:00-05:30");
    assert_eq!(
        std::time::SystemTime::try_from(&value).unwrap(),
        std::time::UNIX_EPOCH
    );
}

#[test]
fn system_time_accepts_pre_epoch_datetime_text() {
    let value = Value::from("1969-12-31T23:59:59.500000Z");
    assert_eq!(
        micros_since_epoch(std::time::SystemTime::try_from(&value).unwrap()),
        -500_000
    );
}

#[test]
fn system_time_invalid_datetime_text_returns_mismatch() {
    let value = Value::from("not a datetime");
    let err = std::time::SystemTime::try_from(&value).expect_err("invalid text should fail");
    assert_eq!(err.code(), ErrorCode::Mismatch);
}

#[test]
fn datetime_text_insert_select_keeps_text_and_system_time_accepts_it() {
    use crate::{Database, Step};

    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("datetime_text.redline")).expect("db");
    let mut conn = db.connect().expect("conn");

    conn.execute("CREATE TABLE src(ts DATETIME)", ())
        .expect("create src");
    conn.execute("CREATE TABLE dst(ts INTEGER)", ())
        .expect("create dst");
    conn.execute(
        "INSERT INTO src(ts) VALUES (datetime('1970-01-01 00:00:00'))",
        (),
    )
    .expect("insert src");
    conn.execute("INSERT INTO dst(ts) SELECT ts FROM src", ())
        .expect("insert select");

    let mut stmt = conn.prepare("SELECT ts FROM dst").expect("prepare");
    match stmt.step().expect("step") {
        Step::Row(row) => {
            let value = row.get::<Value>(0).expect("value");
            assert_eq!(value, Value::Text(Arc::from("1970-01-01 00:00:00")));
            assert_eq!(
                std::time::SystemTime::try_from(&value).unwrap(),
                std::time::UNIX_EPOCH
            );
        }
        Step::Done => panic!("expected row"),
    }
}

fn micros_since_epoch(value: std::time::SystemTime) -> i128 {
    match value.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_micros() as i128,
        Err(err) => -(err.duration().as_micros() as i128),
    }
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
