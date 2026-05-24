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
        SqlValue::Real(v) => format_real_sqlite(*v),
        SqlValue::Text(v) => v.to_string(),
        SqlValue::Blob(v) => String::from_utf8_lossy(v).into_owned(),
    }
}

/// Format a `f64` the way SQLite renders REAL values in text contexts.
///
/// Mirrors `sqlite3_str_appendf(..,"%!.*g", nFpDigit, r)` from the SQLite
/// source: scientific when the decimal exponent is `< -4` or `>= 17`,
/// 17 significant digits, trailing zeros stripped, exponent zero-padded
/// to two digits with an explicit sign, and a `.0` suffix when the
/// mantissa would otherwise be integer. The two reduction rules
/// (trailing-9 round-up and trailing-0 truncation against `z[13..15]`)
/// are applied so that 1.5e100 renders as `1.5e+100` and not the verbose
/// 17-digit form. Non-finite values render as SQLite's
/// `"Inf"` / `"-Inf"` / `"NaN"` literals.
pub(crate) fn format_real_sqlite(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_owned();
    }
    if v.is_infinite() {
        return if v < 0.0 {
            "-Inf".to_owned()
        } else {
            "Inf".to_owned()
        };
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".to_owned()
        } else {
            "0.0".to_owned()
        };
    }

    // Acquire 18 significant digits — SQLite's FpDecode passes 18 to
    // Fp2Convert10 even when `iRound==17`, so the reduction checks can
    // inspect the 18th digit.
    let raw = format!("{:.17e}", v);
    let (mantissa_raw, exp_str) = match raw.split_once('e') {
        Some(pair) => pair,
        None => return raw,
    };
    let exp_e: i32 = exp_str.parse().unwrap_or(0);
    let (sign_str, mantissa_no_sign) = match mantissa_raw.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa_raw),
    };
    let (int_part, frac_part) = mantissa_no_sign
        .split_once('.')
        .unwrap_or((mantissa_no_sign, ""));
    let digits18: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes())
        .map(|b| b - b'0')
        .collect();

    let (effective_digits, adjusted_exp) =
        sqlite_real_reduce(&digits18, exp_e, sign_str, v);
    let needs_scientific = adjusted_exp < -4 || adjusted_exp >= 17;

    if needs_scientific {
        let mantissa = sqlite_real_build_mantissa(&effective_digits);
        let exp_sign = if adjusted_exp >= 0 { '+' } else { '-' };
        let exp_abs = adjusted_exp.unsigned_abs();
        let exp_padded = if exp_abs < 10 {
            format!("0{exp_abs}")
        } else {
            exp_abs.to_string()
        };
        format!("{sign_str}{mantissa}e{exp_sign}{exp_padded}")
    } else {
        let body = sqlite_real_render_plain(&effective_digits, adjusted_exp);
        format!("{sign_str}{body}")
    }
}

/// Apply SQLite's `iRound == 17` reduction rules to `digits18`. Returns
/// the trimmed digit sequence and the canonical decimal exponent (with
/// possible +1 shift from a rounding carry).
fn sqlite_real_reduce(
    digits18: &[u8],
    exp_e: i32,
    sign_str: &str,
    target: f64,
) -> (Vec<u8>, i32) {
    // Base: round 18 digits down to 17 (half-up on the 18th), strip
    // trailing zeros, track any rounding-carry exponent shift.
    let (mut base17, mut base_exp) = sqlite_real_round_to_17(digits18, exp_e);
    while base17.len() > 1 && *base17.last().unwrap() == 0 {
        base17.pop();
    }

    // Rule 1: z[14] == 9 && z[15] == 9 — try rounding up a trailing run
    // of 9s and accept the shorter form if it still round-trips.
    if digits18.len() >= 16 && digits18[14] == 9 && digits18[15] == 9 {
        let mut jj: usize = 14;
        while jj > 0 && digits18[jj - 1] == 9 {
            jj -= 1;
        }
        let (v2_digits, exp_shift) = if jj == 0 {
            (vec![1u8], 1_i32)
        } else {
            let mut new_digits: Vec<u8> = digits18[..jj].to_vec();
            let mut carry = 1u8;
            let mut shifted = false;
            for i in (0..new_digits.len()).rev() {
                let nv = new_digits[i] + carry;
                if nv >= 10 {
                    new_digits[i] = nv - 10;
                    carry = 1;
                } else {
                    new_digits[i] = nv;
                    carry = 0;
                    break;
                }
            }
            if carry == 1 {
                new_digits.insert(0, 1);
                shifted = true;
            }
            while new_digits.len() > 1 && *new_digits.last().unwrap() == 0 {
                new_digits.pop();
            }
            (new_digits, if shifted { 1 } else { 0 })
        };
        let new_exp = exp_e + exp_shift;
        let cand_str = sqlite_real_build_candidate(&v2_digits, new_exp, sign_str);
        let reparsed: f64 = cand_str.parse().unwrap_or(f64::NAN);
        if reparsed == target {
            return (v2_digits, new_exp);
        }
    }

    // Rule 2: z[13] == 0 && z[14] == 0 && z[15] == 0 — try truncating a
    // trailing run of zeros.
    if digits18.len() >= 16 && digits18[13] == 0 && digits18[14] == 0 && digits18[15] == 0 {
        let mut jj: usize = 13;
        while jj > 0 && digits18[jj - 1] == 0 {
            jj -= 1;
        }
        if jj > 0 {
            let candidate: Vec<u8> = digits18[..jj].to_vec();
            let cand_str = sqlite_real_build_candidate(&candidate, exp_e, sign_str);
            let reparsed: f64 = cand_str.parse().unwrap_or(f64::NAN);
            if reparsed == target {
                return (candidate, exp_e);
            }
        }
    }

    // Special-case the carry-overflow path so that `base_exp` already
    // accounts for the digit shift induced by `round_to_17`.
    let _ = &mut base_exp;
    (base17, base_exp)
}

fn sqlite_real_round_to_17(digits18: &[u8], exp_e: i32) -> (Vec<u8>, i32) {
    let mut result: Vec<u8> = digits18[..17.min(digits18.len())].to_vec();
    let mut adjusted_exp = exp_e;
    if digits18.len() > 17 && digits18[17] >= 5 {
        let mut carry = 1u8;
        for i in (0..result.len()).rev() {
            let nv = result[i] + carry;
            if nv >= 10 {
                result[i] = nv - 10;
                carry = 1;
            } else {
                result[i] = nv;
                carry = 0;
                break;
            }
        }
        if carry == 1 {
            result.insert(0, 1);
            // Carry overflowed (e.g. 9.99...9 -> 10.00...0). The new
            // leading 1 sits one position to the left of the original,
            // so the exponent shifts up by 1. We also drop the now-spurious
            // last digit to keep the count at 17.
            result.pop();
            adjusted_exp += 1;
        }
    }
    (result, adjusted_exp)
}

fn sqlite_real_build_candidate(digits: &[u8], exp: i32, sign: &str) -> String {
    let mantissa = sqlite_real_build_mantissa(digits);
    let exp_sign = if exp >= 0 { '+' } else { '-' };
    let exp_abs = exp.unsigned_abs();
    let exp_padded = if exp_abs < 10 {
        format!("0{exp_abs}")
    } else {
        exp_abs.to_string()
    };
    format!("{sign}{mantissa}e{exp_sign}{exp_padded}")
}

fn sqlite_real_build_mantissa(digits: &[u8]) -> String {
    if digits.len() == 1 {
        format!("{}.0", (b'0' + digits[0]) as char)
    } else {
        let first = (b'0' + digits[0]) as char;
        let rest: String = digits[1..].iter().map(|&d| (b'0' + d) as char).collect();
        format!("{first}.{rest}")
    }
}

fn sqlite_real_render_plain(digits: &[u8], exp: i32) -> String {
    let int_digits = (exp + 1).max(0) as usize;
    if int_digits >= digits.len() {
        let mut s: String = digits.iter().map(|&d| (b'0' + d) as char).collect();
        let zeros_after = int_digits - digits.len();
        for _ in 0..zeros_after {
            s.push('0');
        }
        s.push_str(".0");
        s
    } else if int_digits == 0 {
        let leading_zeros = (-exp - 1) as usize;
        let mut s = String::from("0.");
        for _ in 0..leading_zeros {
            s.push('0');
        }
        for &d in digits {
            s.push((b'0' + d) as char);
        }
        s
    } else {
        let int_str: String = digits[..int_digits].iter().map(|&d| (b'0' + d) as char).collect();
        let frac_str: String = digits[int_digits..].iter().map(|&d| (b'0' + d) as char).collect();
        let frac_str = if frac_str.is_empty() {
            "0".to_owned()
        } else {
            frac_str
        };
        format!("{int_str}.{frac_str}")
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
