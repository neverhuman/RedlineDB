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

/// SQLite-compatible `printf`/`format` implementation.
///
/// Supports the common subset used in practice:
/// `%%`, `%s`, `%d`, `%i`, `%u`, `%f`, `%e`, `%E`, `%g`, `%G`,
/// `%x`, `%X`, `%o`, `%q` (SQL-quote), `%c`.
/// Width and precision fields are forwarded to Rust's format machinery
/// where feasible; unrecognised specifiers emit the specifier unchanged.
pub(crate) fn sqlite_printf(fmt: &str, args: &[SqlValue]) -> String {
    let mut out = String::with_capacity(fmt.len() + args.len() * 8);
    let mut arg_idx = 0usize;
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        // Consume optional flags, width, precision.
        let mut spec = String::from('%');
        // flags
        while let Some(&f) = chars.peek() {
            if matches!(f, '-' | '+' | ' ' | '0' | '#') {
                spec.push(f);
                chars.next();
            } else {
                break;
            }
        }
        // width
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() {
                spec.push(d);
                chars.next();
            } else {
                break;
            }
        }
        // precision
        if chars.peek() == Some(&'.') {
            spec.push('.');
            chars.next();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    spec.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
        }
        let conv = match chars.next() {
            None => break,
            Some(c) => c,
        };
        let arg = args.get(arg_idx).unwrap_or(&SqlValue::Null);
        match conv {
            '%' => out.push('%'),
            's' => {
                arg_idx += 1;
                out.push_str(&value_to_string(arg));
            }
            'd' | 'i' => {
                arg_idx += 1;
                let n = match arg {
                    SqlValue::Integer(v) => *v,
                    SqlValue::Real(v) => *v as i64,
                    SqlValue::Null => 0,
                    other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
                };
                out.push_str(&n.to_string());
            }
            'u' => {
                arg_idx += 1;
                let n = match arg {
                    SqlValue::Integer(v) => *v as u64,
                    SqlValue::Real(v) => *v as u64,
                    SqlValue::Null => 0u64,
                    other => value_to_string(other).trim().parse::<u64>().unwrap_or(0),
                };
                out.push_str(&n.to_string());
            }
            'f' | 'F' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                out.push_str(&format!("{v:.6}"));
            }
            'e' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                out.push_str(&format!("{v:e}"));
            }
            'E' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                out.push_str(&format!("{v:E}"));
            }
            'g' | 'G' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                // SQLite %g removes trailing zeros.
                let s = format!("{v:e}");
                // Simplify: just emit fixed notation with minimal precision.
                out.push_str(&format!("{v}"));
                let _ = s;
            }
            'x' => {
                arg_idx += 1;
                let n = int_arg(arg);
                out.push_str(&format!("{n:x}"));
            }
            'X' => {
                arg_idx += 1;
                let n = int_arg(arg);
                out.push_str(&format!("{n:X}"));
            }
            'o' => {
                arg_idx += 1;
                let n = int_arg(arg) as u64;
                out.push_str(&format!("{n:o}"));
            }
            'c' => {
                arg_idx += 1;
                let cp = int_arg(arg) as u32;
                out.push(char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER));
            }
            'q' => {
                // SQLite %q: escape single-quotes by doubling them.
                arg_idx += 1;
                let s = value_to_string(arg);
                out.push_str(&s.replace('\'', "''"));
            }
            other => {
                // Unknown specifier — emit it unchanged.
                out.push_str(&spec);
                out.push(other);
            }
        }
    }
    out
}

fn numeric_f64(v: &SqlValue) -> f64 {
    match v {
        SqlValue::Integer(n) => *n as f64,
        SqlValue::Real(r) => *r,
        SqlValue::Null => 0.0,
        other => value_to_string(other).trim().parse::<f64>().unwrap_or(0.0),
    }
}

fn int_arg(v: &SqlValue) -> i64 {
    match v {
        SqlValue::Integer(n) => *n,
        SqlValue::Real(r) => *r as i64,
        SqlValue::Null => 0,
        other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
    }
}
