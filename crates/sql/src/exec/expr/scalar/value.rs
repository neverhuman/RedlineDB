//! Value-shaped scalar helpers: vectors, date/time, and the
//! lowest-common-denominator `value_to_string` / parameter binding helpers
//! used by every other scalar submodule.
//!
//! These live together because they all take a `SqlValue` (or a slice of
//! them) and produce another `SqlValue` without touching row context.

use super::*;

#[path = "value/datetime.rs"]
mod datetime;

pub(crate) use datetime::*;

pub(crate) fn value_to_string(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => String::new(),
        SqlValue::Integer(v) => v.to_string(),
        SqlValue::Real(v) => v.to_string(),
        SqlValue::Text(v) => v.to_string(),
        SqlValue::Blob(v) => String::from_utf8_lossy(v).into_owned(),
    }
}

pub(crate) fn resolve_binding(name: &str, bindings: &[Option<SqlValue>]) -> Result<SqlValue> {
    if let Some(rest) = name.strip_prefix('?') {
        let slot = rest
            .parse::<usize>()
            .map_err(|_| Error::Parse(format!("invalid parameter {name}")))?;
        return Ok(bindings
            .get(slot)
            .and_then(|v| v.clone())
            .unwrap_or(SqlValue::Null));
    }
    Err(Error::Bind(format!("unknown parameter {name}")))
}

pub(crate) fn sqlite_substr_function(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Ok(SqlValue::Null);
    }
    if matches!(values[0], SqlValue::Null)
        || matches!(values[1], SqlValue::Null)
        || matches!(values.get(2), Some(SqlValue::Null))
    {
        return Ok(SqlValue::Null);
    }
    let start = match &values[1] {
        SqlValue::Integer(n) => *n,
        other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
    };
    let len = values.get(2).map(|value| match value {
        SqlValue::Integer(n) => *n,
        other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
    });
    match &values[0] {
        SqlValue::Blob(bytes) => Ok(SqlValue::Blob(sqlite_substr_bytes(bytes, start, len))),
        other => Ok(SqlValue::Text(Arc::from(sqlite_substr_text(
            &value_to_string(other),
            start,
            len,
        )))),
    }
}

pub(crate) fn sqlite_trim_function(value: &SqlValue, chars: Option<&SqlValue>) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(chars, Some(SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let s = value_to_string(value);
    let result = match chars {
        None => s.trim_matches(' ').to_owned(),
        Some(chars) => {
            let strip: Vec<char> = value_to_string(chars).chars().collect();
            s.trim_matches(strip.as_slice()).to_owned()
        }
    };
    Ok(SqlValue::Text(Arc::from(result)))
}

pub(crate) fn sqlite_ltrim_function(
    value: &SqlValue,
    chars: Option<&SqlValue>,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(chars, Some(SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let s = value_to_string(value);
    let result = match chars {
        None => s.trim_start_matches(' ').to_owned(),
        Some(chars) => {
            let strip: Vec<char> = value_to_string(chars).chars().collect();
            s.trim_start_matches(strip.as_slice()).to_owned()
        }
    };
    Ok(SqlValue::Text(Arc::from(result)))
}

pub(crate) fn sqlite_rtrim_function(
    value: &SqlValue,
    chars: Option<&SqlValue>,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(chars, Some(SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let s = value_to_string(value);
    let result = match chars {
        None => s.trim_end_matches(' ').to_owned(),
        Some(chars) => {
            let strip: Vec<char> = value_to_string(chars).chars().collect();
            s.trim_end_matches(strip.as_slice()).to_owned()
        }
    };
    Ok(SqlValue::Text(Arc::from(result)))
}

fn sqlite_substr_text(input: &str, start: i64, len: Option<i64>) -> String {
    let chars: Vec<char> = input.chars().collect();
    let total = chars.len() as i64;
    let Some((start_range, take_range)) = sqlite_substr_range(total, start, len) else {
        return String::new();
    };
    let start_idx = start_range.max(0) as usize;
    let take = take_range.max(0) as usize;
    if start_idx >= chars.len() {
        return String::new();
    }
    chars.iter().skip(start_idx).take(take).collect()
}

fn sqlite_substr_bytes(input: &[u8], start: i64, len: Option<i64>) -> Arc<[u8]> {
    let total = input.len() as i64;
    match sqlite_substr_range(total, start, len) {
        Some((start_idx, take)) => {
            let start_idx = start_idx.max(0) as usize;
            let take = take.max(0) as usize;
            if start_idx >= input.len() {
                Arc::from(&[][..])
            } else {
                let end = start_idx.saturating_add(take).min(input.len());
                Arc::from(&input[start_idx..end])
            }
        }
        None => Arc::from(&[][..]),
    }
}

fn sqlite_substr_range(len: i64, mut start: i64, take: Option<i64>) -> Option<(i64, i64)> {
    let mut take = take.unwrap_or(i64::MAX);
    if start < 0 {
        start += len;
        if start < 0 {
            if take < 0 {
                take = 0;
            } else {
                take += start;
            }
            start = 0;
        }
    } else if start > 0 {
        start -= 1;
    } else if take > 0 {
        take -= 1;
    }
    if take < 0 {
        if take < -start {
            take = start;
        } else {
            take = -take;
        }
        start -= take;
    }
    if start < 0 {
        return None;
    }
    Some((start, take))
}

pub(crate) enum VectorOpMetric {
    L2,
    Cosine,
    InnerProduct,
}

impl From<VectorOpMetric> for redlinedb_kernel::vector::VectorMetric {
    fn from(m: VectorOpMetric) -> Self {
        match m {
            VectorOpMetric::L2 => Self::L2,
            VectorOpMetric::Cosine => Self::Cosine,
            VectorOpMetric::InnerProduct => Self::InnerProduct,
        }
    }
}

pub(crate) fn vector_construct_from_value(value: &SqlValue) -> Result<SqlValue> {
    match value {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Text(s) => {
            let v = redlinedb_kernel::vector::Vector::from_json_literal(s.as_ref())
                .map_err(|e| Error::UnsupportedSql(format!("vector(): {e}")))?;
            Ok(SqlValue::Blob(Arc::from(v.encode())))
        }
        SqlValue::Blob(bytes) => {
            redlinedb_kernel::vector::decode_vector(bytes)
                .map_err(|e| Error::UnsupportedSql(format!("vector(): {e}")))?;
            Ok(SqlValue::Blob(bytes.clone()))
        }
        _ => Err(Error::DatatypeMismatch),
    }
}

pub(crate) fn vector_dims_value(value: &SqlValue) -> Result<SqlValue> {
    match value {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Blob(bytes) => {
            let v = redlinedb_kernel::vector::decode_vector(bytes)
                .map_err(|e| Error::UnsupportedSql(format!("vector_dims: {e}")))?;
            Ok(SqlValue::Integer(v.len() as i64))
        }
        _ => Err(Error::DatatypeMismatch),
    }
}

pub(crate) fn vector_pair_distance(
    values: &[SqlValue],
    metric: VectorOpMetric,
) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "vector_distance_* requires exactly 2 args".to_owned(),
        ));
    }
    if matches!(values[0], SqlValue::Null) || matches!(values[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let (a, b) = match try_vector_pair(&values[0], &values[1]) {
        Some(p) => p,
        None => return Err(Error::DatatypeMismatch),
    };
    vector_distance_to_value(metric, &a, &b)
}

pub(crate) fn try_vector_pair(left: &SqlValue, right: &SqlValue) -> Option<(Vec<f32>, Vec<f32>)> {
    let SqlValue::Blob(la) = left else {
        return None;
    };
    let SqlValue::Blob(rb) = right else {
        return None;
    };
    let a = redlinedb_kernel::vector::decode_vector(la).ok()?;
    let b = redlinedb_kernel::vector::decode_vector(rb).ok()?;
    Some((a, b))
}

pub(crate) fn vector_distance_to_value(
    metric: VectorOpMetric,
    a: &[f32],
    b: &[f32],
) -> Result<SqlValue> {
    let m: redlinedb_kernel::vector::VectorMetric = metric.into();
    let d = m
        .distance(a, b)
        .map_err(|e| Error::UnsupportedSql(format!("vector distance: {e}")))?;
    Ok(SqlValue::Real(d as f64))
}
