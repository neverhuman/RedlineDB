# Bug: Datetime Text Stored in Integer Column Causes `value is not integer` on Read

## Summary

When inserting a `datetime` column value into an `integer` column via SQL `SELECT ... INSERT`, SQLite stores it as text (because the text cannot be coerced to integer). A subsequent `row.get::<Value>(col)` returns `Value::Text(...)`. Calling `SystemTime::try_from(&Value::Text(...))` then fails with `Error { code: Mismatch, message: "value is not integer" }` because `TryFrom<&Value> for SystemTime` calls `value.as_integer()` unconditionally.

## Root Cause

In `crates/redlinedb/src/value_conv.rs`:

```rust
impl TryFrom<&Value> for std::time::SystemTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        let micros = value.as_integer()?;  // fails if Value::Text
        ...
    }
}
```

`as_integer()` is:
```rust
pub fn as_integer(&self) -> Result<i64> {
    match self {
        Self::Integer(n) => Ok(*n),
        _ => Err(Error::new(ErrorCode::Mismatch, "value is not integer")),
    }
}
```

This means `SystemTime::try_from` only handles `Value::Integer`. If a `datetime` text column value leaks into an `integer` column (via SQL `INSERT ... SELECT`), the stored value's affinity is text, and `as_integer()` will fail.

## Observed Failure

In jansu storage tests (`jansu-broker::txn::redlinedb::simple_txn_commit_offset_commit`):

```
Error: Storage(RedlineDb(Error { code: Mismatch, message: "value is not integer", source: None }))
```

This was caused by `consumer_offset_insert_from_txn.sql` reading `txn_offset_commit_tp.created_at` (a `datetime` column, stored as ISO8601 text by SQLite) and inserting it into `consumer_offset.timestamp` (an `integer` column). SQLite stores the text in the integer column as-is (text affinity wins when conversion is impossible), so `Value::Text` comes back on read.

## Recommended Fix

`SystemTime::try_from` should handle `Value::Text` by attempting RFC3339/ISO8601 parse, consistent with how other datetime-aware systems work:

```rust
impl TryFrom<&Value> for std::time::SystemTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Integer(micros) => {
                let micros = *micros;
                if micros >= 0 {
                    std::time::UNIX_EPOCH
                        .checked_add(std::time::Duration::from_micros(micros as u64))
                        .ok_or_else(|| Error::new(ErrorCode::Mismatch, "timestamp overflow"))
                } else {
                    std::time::UNIX_EPOCH
                        .checked_sub(std::time::Duration::from_micros(micros.unsigned_abs()))
                        .ok_or_else(|| Error::new(ErrorCode::Mismatch, "timestamp underflow"))
                }
            }
            Value::Text(s) => {
                // Parse ISO8601/RFC3339 datetime strings (e.g. "1970-01-01T00:00:00Z")
                // that SQLite stores for datetime columns with default current_timestamp.
                chrono::DateTime::parse_from_rfc3339(s)
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
                        .map(|dt| dt.and_utc().fixed_offset()))
                    .map(|dt| {
                        let micros = dt.timestamp_micros();
                        if micros >= 0 {
                            std::time::UNIX_EPOCH + std::time::Duration::from_micros(micros as u64)
                        } else {
                            std::time::UNIX_EPOCH - std::time::Duration::from_micros(micros.unsigned_abs() as u64)
                        }
                    })
                    .map_err(|e| Error::new(ErrorCode::Mismatch, format!("invalid datetime text: {e}")))
            }
            _ => Err(Error::new(ErrorCode::Mismatch, "value is not integer or text datetime")),
        }
    }
}
```

This requires the `chrono` feature (already available as `features = ["chrono"]`).

## Workaround Applied in jansu

Changed `consumer_offset_insert_from_txn.sql` to pass `SystemTime::now()` as an explicit integer parameter (`$5`) instead of relying on `txn_oc_tp.created_at` (datetime text). The integer is produced by `Value::from(SystemTime)` which stores microseconds as `Value::Integer`. This avoids the type mismatch.

## Affected Versions

redlinedb `fix/pool-rollback-on-drop` branch, based at `8f252e05`.

## SQLite Context

SQLite uses "type affinity" rather than strict typing. When inserting a text value (e.g., `"1970-01-01T00:00:00Z"`) into a column declared as `INTEGER`, SQLite stores it as text because it cannot parse the ISO8601 string as an integer. The column affinity does not force conversion for non-numeric text. This means callers must not assume that values from `integer` columns are always `Value::Integer`.
