//! Value-shaped scalar helpers: vectors, date/time, and the
//! lowest-common-denominator `value_to_string` / parameter binding helpers
//! used by every other scalar submodule.
//!
//! These live together because they all take a `SqlValue` (or a slice of
//! them) and produce another `SqlValue` without touching row context.

use super::*;

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

#[derive(Copy, Clone)]
pub(crate) enum DateTimeKind {
    Date,
    Time,
    Datetime,
    JulianDay,
    Unix,
}

pub(crate) fn datetime_function(values: &[SqlValue], kind: DateTimeKind) -> Result<SqlValue> {
    let dt = parse_dt_args(values)?;
    Ok(match kind {
        DateTimeKind::Date => SqlValue::Text(Arc::from(dt.format_date())),
        DateTimeKind::Time => SqlValue::Text(Arc::from(dt.format_time())),
        DateTimeKind::Datetime => SqlValue::Text(Arc::from(dt.format_datetime())),
        DateTimeKind::JulianDay => SqlValue::Real(dt.julian_day()),
        DateTimeKind::Unix => SqlValue::Integer(dt.to_unix()),
    })
}

pub(crate) fn strftime_function(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql("strftime requires format".to_owned()));
    }
    let format = value_to_string(&values[0]);
    let dt = parse_dt_args(&values[1..])?;
    Ok(SqlValue::Text(Arc::from(crate::datetime::strftime(
        &format, &dt,
    ))))
}

fn parse_dt_args(values: &[SqlValue]) -> Result<crate::datetime::DateTime> {
    let base = match values.first() {
        Some(v) => value_to_string(v),
        None => "now".to_owned(),
    };
    let dt = crate::datetime::parse_timestring(&base)?;
    if values.len() <= 1 {
        return Ok(dt);
    }
    let mods: Vec<String> = values[1..].iter().map(value_to_string).collect();
    let refs: Vec<&str> = mods.iter().map(String::as_str).collect();
    crate::datetime::apply_modifiers(dt, &refs)
}
