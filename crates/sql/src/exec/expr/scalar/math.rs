//! Numeric / arithmetic / hex helpers for scalar expression evaluation.
//!
//! Covers:
//!   * `round_function`, `numeric_value`, `parse_number`
//!   * `arithmetic` and `negate` (used by `eval_binary` and the unary
//!     minus path)
//!   * `hex_value`, `hex_string_to_bytes`, `quote_value`, `random_i64`
//!
//! Visibility mirrors the pre-split surface. Items needed elsewhere in the
//! SQL crate stay `pub(crate)`; helpers only used inside `expr/` stay
//! `pub(super)`.

use super::*;

pub(crate) fn round_function(values: &[SqlValue]) -> Result<SqlValue> {
    // SQLite: round(NULL, ...) and round(x, NULL) return NULL.
    if values.is_empty() || matches!(values[0], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    if values.len() > 1 && matches!(values[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let value = numeric_value(&values[0])?;
    let digits = if values.len() > 1 {
        numeric_value(&values[1])? as i32
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    Ok(SqlValue::Real((value * factor).round() / factor))
}

pub(crate) fn numeric_value(value: &SqlValue) -> Result<f64> {
    match value {
        SqlValue::Null => Ok(0.0),
        SqlValue::Integer(v) => Ok(*v as f64),
        SqlValue::Real(v) => Ok(*v),
        SqlValue::Text(v) => v.trim().parse::<f64>().map_err(|_| Error::DatatypeMismatch),
        SqlValue::Blob(v) => String::from_utf8_lossy(v)
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::DatatypeMismatch),
    }
}

pub(crate) fn hex_value(value: &SqlValue) -> String {
    let bytes: Vec<u8> = match value {
        SqlValue::Null => Vec::new(),
        SqlValue::Integer(v) => v.to_string().into_bytes(),
        SqlValue::Real(v) => super::value::format_real_sqlite(*v).into_bytes(),
        SqlValue::Text(v) => v.as_bytes().to_vec(),
        SqlValue::Blob(v) => v.to_vec(),
    };
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02X}", byte);
    }
    out
}

pub(crate) fn quote_value(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "NULL".to_owned(),
        SqlValue::Integer(v) => v.to_string(),
        SqlValue::Real(v) => super::value::format_real_sqlite(*v),
        SqlValue::Text(v) => format!("'{}'", v.replace('\'', "''")),
        SqlValue::Blob(v) => {
            let mut out = String::from("X'");
            for byte in v.iter() {
                use std::fmt::Write;
                let _ = write!(&mut out, "{:02X}", byte);
            }
            out.push('\'');
            out
        }
    }
}

pub(crate) fn random_i64() -> i64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let state = now
        ^ COUNTER
            .fetch_add(1, AtomicOrdering::Relaxed)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut x = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as i64).wrapping_abs()
}

// `hex_string_to_bytes` lives in `crate::parser::helpers`; re-exported
// here so existing `use super::*` glob imports still resolve.
pub(crate) use crate::parser::hex_string_to_bytes;

pub(crate) fn negate(value: SqlValue) -> Result<SqlValue> {
    match value {
        SqlValue::Integer(v) => Ok(SqlValue::Integer(-v)),
        SqlValue::Real(v) => Ok(SqlValue::Real(-v)),
        SqlValue::Null => Ok(SqlValue::Null),
        _ => Err(Error::DatatypeMismatch),
    }
}

pub(crate) fn arithmetic(
    left: SqlValue,
    right: SqlValue,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    real_op: impl FnOnce(f64, f64) -> Option<f64>,
) -> Result<SqlValue> {
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    // Closures may return None (e.g. divide / modulo by zero) — in that case
    // SQLite returns NULL rather than raising an error or panicking.
    fn lift_int(opt: Option<i64>) -> SqlValue {
        match opt {
            Some(v) => SqlValue::Integer(v),
            None => SqlValue::Null,
        }
    }
    fn lift_real(opt: Option<f64>) -> SqlValue {
        match opt {
            Some(v) => SqlValue::Real(v),
            None => SqlValue::Null,
        }
    }
    match (left, right) {
        (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(lift_int(int_op(a, b))),
        (SqlValue::Integer(a), SqlValue::Real(b)) => Ok(lift_real(real_op(a as f64, b))),
        (SqlValue::Real(a), SqlValue::Integer(b)) => Ok(lift_real(real_op(a, b as f64))),
        (SqlValue::Real(a), SqlValue::Real(b)) => Ok(lift_real(real_op(a, b))),
        (SqlValue::Text(a), SqlValue::Text(b)) => {
            let a = a
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::DatatypeMismatch)?;
            let b = b
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::DatatypeMismatch)?;
            Ok(lift_real(real_op(a, b)))
        }
        _ => Err(Error::DatatypeMismatch),
    }
}

pub(crate) fn parse_number(input: &str) -> Result<SqlValue> {
    if let Ok(v) = input.parse::<i64>() {
        return Ok(SqlValue::Integer(v));
    }
    if let Ok(v) = input.parse::<f64>() {
        return Ok(canonicalize(SqlValue::Real(v)));
    }
    Err(Error::Parse(format!("invalid numeric literal {input}")))
}

// ── SQLite math1 extension functions ─────────────────────────────────────────
//
// SQLite's math1 extension (enabled by default in the reference build) exposes
// the standard f64 transcendental functions. The shared NULL-and-domain wrapper
// returns SQLite's canonical NULL for any non-finite result (NaN/inf) so that
// e.g. `acos(2.0)`, `sqrt(-1.0)`, `log(0.0)` all return NULL rather than an
// error or a NaN literal.
//
// One-arg form. Returns NULL when the input is NULL, non-numeric, or the
// result is non-finite (NaN or infinity from out-of-domain inputs).
pub(crate) fn math1_unary(values: &[SqlValue], op: impl FnOnce(f64) -> f64) -> Result<SqlValue> {
    match values.first() {
        None | Some(SqlValue::Null) => Ok(SqlValue::Null),
        Some(v) => {
            let x = match numeric_value(v) {
                Ok(v) => v,
                Err(_) => return Ok(SqlValue::Null),
            };
            if !x.is_finite() {
                return Ok(SqlValue::Null);
            }
            let r = op(x);
            if r.is_finite() { Ok(SqlValue::Real(r)) } else { Ok(SqlValue::Null) }
        }
    }
}

// Two-arg form mirroring math1_unary's NULL/domain handling. Used by atan2 and
// the two-argument form of log.
pub(crate) fn math1_binary(
    values: &[SqlValue],
    op: impl FnOnce(f64, f64) -> f64,
) -> Result<SqlValue> {
    if values.len() < 2 {
        return Ok(SqlValue::Null);
    }
    if matches!(values[0], SqlValue::Null) || matches!(values[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let a = match numeric_value(&values[0]) {
        Ok(v) => v,
        Err(_) => return Ok(SqlValue::Null),
    };
    let b = match numeric_value(&values[1]) {
        Ok(v) => v,
        Err(_) => return Ok(SqlValue::Null),
    };
    if !a.is_finite() || !b.is_finite() {
        return Ok(SqlValue::Null);
    }
    let r = op(a, b);
    if r.is_finite() { Ok(SqlValue::Real(r)) } else { Ok(SqlValue::Null) }
}

// SQLite's mod(x,y) returns NULL when y == 0 (or either arg is non-numeric).
// Result is always REAL per sqlite docs.
pub(crate) fn math_mod(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Ok(SqlValue::Null);
    }
    if matches!(values[0], SqlValue::Null) || matches!(values[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let x = match numeric_value(&values[0]) {
        Ok(v) => v,
        Err(_) => return Ok(SqlValue::Null),
    };
    let y = match numeric_value(&values[1]) {
        Ok(v) => v,
        Err(_) => return Ok(SqlValue::Null),
    };
    if y == 0.0 || !x.is_finite() || !y.is_finite() {
        return Ok(SqlValue::Null);
    }
    Ok(SqlValue::Real(x % y))
}

// SQLite log() is overloaded: log(X) is the natural log (alias for ln), and
// log(B, X) is the base-B log of X. Single-arg log on a single arg actually
// uses log base 10 in some compile modes (legacy), but in the reference build
// with math1 it's the natural logarithm. We match the reference build.
pub(crate) fn math_log(values: &[SqlValue]) -> Result<SqlValue> {
    match values.len() {
        0 => Ok(SqlValue::Null),
        1 => math1_unary(values, f64::ln),
        _ => math1_binary(values, |b, x| x.log(b)),
    }
}

// trunc(x) - truncate towards zero. SQLite returns REAL with .0 suffix.
pub(crate) fn math_trunc(values: &[SqlValue]) -> Result<SqlValue> {
    math1_unary(values, f64::trunc)
}

pub(crate) fn math_pi() -> SqlValue {
    SqlValue::Real(std::f64::consts::PI)
}

pub(crate) fn math_degrees(values: &[SqlValue]) -> Result<SqlValue> {
    math1_unary(values, |x| x.to_degrees())
}

pub(crate) fn math_radians(values: &[SqlValue]) -> Result<SqlValue> {
    math1_unary(values, |x| x.to_radians())
}
