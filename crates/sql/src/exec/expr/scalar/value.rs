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

/// Format a `f64` the way SQLite renders REAL values in text contexts
/// (`%!.17g` semantics with trailing-`.0` for whole-valued finite reals).
///
/// SQLite's `printf("%g", x)` prints 17 significant digits unconditionally
/// (matching its `sqlite3FpDecode` behaviour) and trims trailing zeros. This
/// differs from Rust's default `{}` formatting which prints the shortest
/// round-tripping representation (often 15-16 digits). Matching the 17-digit
/// rendering exactly is required for sqlite-parity on math-function output
/// (e.g. `acos(0.5)` must render as `1.0471975511965979`, not
/// `1.0471975511965978`).
///
/// Non-finite values render as SQLite's `"Inf"` / `"-Inf"` / `"NaN"` literals.
pub fn format_real_sqlite(v: f64) -> String {
    format_real_g(v, 17)
}

/// SQLite-compatible `%g`-style formatter. Produces `precision` significant
/// digits, trims trailing zeros, chooses fixed vs. scientific notation per
/// POSIX `%g` rules, and appends `.0` for whole-valued finite reals so that
/// `format_real_g(1.0, 17) == "1.0"`.
///
/// Mirrors the heart of SQLite's `sqlite3FpDecode`/`%!.17g` semantics
/// (see `util.c`):
///   1. Decompose into a digit string and a decimal exponent using
///      `{:.precision+1 e}` (one extra digit acting as the rounding
///      indicator). Apply round-half-up at the indicator.
///   2. When `precision == 17` AND the 18-digit prefix has trailing zeros
///      at positions 13/14/15 OR the 16th/15th digits are both `9`, try
///      progressively shorter representations and pick the shortest that
///      bit-exactly round-trips to the original `f64`. This matches the
///      special-case path in `sqlite3FpDecode` that lets `0.1` render as
///      `"0.1"` rather than `"0.10000000000000001"`.
///   3. Trim trailing zeros from the chosen digit string.
///   4. Emit fixed form when `-4 <= exp < precision`, scientific otherwise.
///      Scientific notation uses SQLite's `e[+-]NN` convention with 2+
///      exponent digits.
fn format_real_g(v: f64, precision: usize) -> String {
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
        return "0.0".to_owned();
    }
    let neg = v < 0.0;
    let av = if neg { -v } else { v };
    // {:.N e} produces N+1 significant digits. Request precision+1 so we
    // have one rounding-indicator digit.
    let raw = format!("{:.*e}", precision + 1, av);
    let pos_e = raw.find('e').expect("rust f64 fmt always emits 'e'");
    let mantissa = &raw[..pos_e];
    let mut exp: i32 = raw[pos_e + 1..]
        .parse()
        .expect("rust f64 fmt always emits valid exponent");
    let dot_pos = mantissa.find('.').unwrap_or(mantissa.len());
    let mut digits: Vec<u8> = Vec::with_capacity(precision + 4);
    digits.extend_from_slice(mantissa[..dot_pos].as_bytes());
    if dot_pos < mantissa.len() {
        digits.extend_from_slice(mantissa[dot_pos + 1..].as_bytes());
    }
    // We have precision+2 digits (e.g. 19 for precision=17). For the
    // round-trip shortening to work we keep the 18-digit form around: round
    // the 19th into the 18th using round-half-up.
    let work_len = precision + 1; // 18 for precision=17
    if digits.len() > work_len {
        let indicator = digits[work_len];
        digits.truncate(work_len);
        if indicator >= b'5' {
            round_up_carry(&mut digits, &mut exp);
        }
    }
    // Now `digits` has `work_len` digits. Apply SQLite's shortening special
    // case for precision==17 (the `!`-flag in `%!.17g`).
    if precision == 17 && digits.len() == 18 {
        if let Some((shortened, new_exp)) = sqlite_shorten(&digits, exp, av, neg) {
            let mut digits = shortened;
            // Trim trailing zeros (keep at least one digit).
            while digits.len() > 1 && *digits.last().unwrap() == b'0' {
                digits.pop();
            }
            return assemble_g(&digits, new_exp, neg, precision as i32);
        }
    }
    // Fallback: round the 18-digit form down to 17 digits.
    if digits.len() > precision {
        let indicator = digits[precision];
        digits.truncate(precision);
        if indicator >= b'5' {
            round_up_carry(&mut digits, &mut exp);
        }
    }
    while digits.len() > 1 && *digits.last().unwrap() == b'0' {
        digits.pop();
    }
    assemble_g(&digits, exp, neg, precision as i32)
}

/// Round-up-with-carry helper — increments the last digit, cascading `9→0`
/// carries leftward, and prepending `1` (with exponent adjustment) on
/// overflow. Used by both the rounding-indicator step in `format_real_g`
/// and the shortening probe in `sqlite_shorten`.
fn round_up_carry(digits: &mut Vec<u8>, exp: &mut i32) {
    if digits.is_empty() {
        digits.push(b'1');
        return;
    }
    let mut i = digits.len() - 1;
    loop {
        if digits[i] < b'9' {
            digits[i] += 1;
            return;
        }
        digits[i] = b'0';
        if i == 0 {
            digits.insert(0, b'1');
            digits.pop();
            *exp += 1;
            return;
        }
        i -= 1;
    }
}

/// Implements SQLite's `%!.17g` shortening probe (see `sqlite3FpDecode`):
/// when the 18-digit prefix has trailing zeros at positions 13/14/15 *or*
/// digits 15 and 14 are both `9`, try progressively shorter representations
/// and return the shortest that bit-exactly round-trips to the original
/// `f64`. Returns `None` to fall back to the standard 17-digit form.
fn sqlite_shorten(
    digits: &[u8],
    exp: i32,
    original: f64,
    neg: bool,
) -> Option<(Vec<u8>, i32)> {
    debug_assert_eq!(digits.len(), 18);
    // SQLite's trigger conditions (0-indexed against z[]).
    let trigger_trailing_zeros = digits[13] == b'0' && digits[14] == b'0' && digits[15] == b'0';
    let trigger_nines = digits[14] == b'9' && digits[15] == b'9';
    if !trigger_trailing_zeros && !trigger_nines {
        return None;
    }
    // Walk from shortest (1 digit) up — first round-trip wins (matches
    // SQLite's preference for the shortest exact representation).
    for new_len in 1..digits.len() {
        let mut shortened = digits[..new_len].to_vec();
        let mut new_exp = exp;
        let indicator = digits[new_len];
        if indicator >= b'5' {
            round_up_carry(&mut shortened, &mut new_exp);
        }
        // Trim trailing zeros — sqlite normalises this way during the
        // round-trip check too.
        let mut trimmed = shortened.clone();
        while trimmed.len() > 1 && *trimmed.last().unwrap() == b'0' {
            trimmed.pop();
        }
        let candidate = assemble_g(&trimmed, new_exp, neg, 17);
        if let Ok(parsed) = candidate.parse::<f64>() {
            if parsed.to_bits() == original.to_bits()
                || (neg && (-parsed).to_bits() == original.to_bits())
            {
                return Some((trimmed, new_exp));
            }
        }
    }
    None
}

/// Final emit step shared by `format_real_g` and the round-trip probe.
/// Chooses fixed vs. scientific notation per POSIX `%g` rules and ensures
/// whole-valued reals keep their `.0` suffix.
fn assemble_g(digits: &[u8], exp: i32, neg: bool, precision: i32) -> String {
    let sign = if neg { "-" } else { "" };
    if exp >= -4 && exp < precision {
        if exp >= 0 {
            let int_digits = (exp + 1) as usize;
            if int_digits >= digits.len() {
                let mut out = String::from(sign);
                out.push_str(std::str::from_utf8(digits).unwrap());
                for _ in digits.len()..int_digits {
                    out.push('0');
                }
                out.push_str(".0");
                out
            } else {
                let mut out = String::from(sign);
                out.push_str(std::str::from_utf8(&digits[..int_digits]).unwrap());
                out.push('.');
                out.push_str(std::str::from_utf8(&digits[int_digits..]).unwrap());
                out
            }
        } else {
            let zeros = (-exp - 1) as usize;
            let mut out = String::from(sign);
            out.push_str("0.");
            for _ in 0..zeros {
                out.push('0');
            }
            out.push_str(std::str::from_utf8(digits).unwrap());
            out
        }
    } else {
        let mut out = String::from(sign);
        out.push(digits[0] as char);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(std::str::from_utf8(&digits[1..]).unwrap());
        }
        out.push('e');
        if exp < 0 {
            out.push('-');
        } else {
            out.push('+');
        }
        out.push_str(&format!("{:02}", exp.abs()));
        out
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

// SQLite's unhex(X[, IGNORE]). Returns the BLOB obtained by decoding the
// hex digits in X, optionally skipping any character that appears in IGNORE.
// Any other character (besides matched hex pair or ignored char) yields NULL.
// Empty / fully ignored input → empty BLOB.
pub(crate) fn sqlite_unhex(input: &str, ignore: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut nibble: Option<u8> = None;
    for &b in bytes {
        if ignore.as_bytes().contains(&b) {
            continue;
        }
        let value = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return None,
        };
        match nibble.take() {
            Some(high) => out.push((high << 4) | value),
            None => nibble = Some(value),
        }
    }
    if nibble.is_some() {
        // Odd hex digit count is an error in sqlite.
        return None;
    }
    Some(out)
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

// ---------------------------------------------------------------------------
// Track H — beyond-SQLite (Postgres parity) value helpers.
//
// These helpers implement Postgres-style scalar semantics so that the
// beyond_sqlite oracle can compare RedlineDB output against psql 16 without
// needing a separate compatibility mode. Each helper returns a TEXT-shaped
// SqlValue formatted the way PG renders the result in unaligned + tuples-only
// mode (`-A -t`), which is the format the runner captures.
// ---------------------------------------------------------------------------

/// `date_trunc(field, ts)` — truncates a timestamp/date string to the
/// granularity of `field`. The supported field values match PG's documented
/// set (`year`, `quarter`, `month`, `week`, `day`, `hour`, `minute`,
/// `second`, `millennium`, `century`, `decade`); unrecognised fields propagate
/// NULL the same way an out-of-range julian-day input does.
///
/// The output is always rendered as `YYYY-MM-DD HH:MM:SS` (PG's default
/// timestamp text shape under `format=unaligned`), even when the input was a
/// pure date — matches `psql -A -t`.
pub(crate) fn pg_date_trunc(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 || values.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let field = value_to_string(&values[0]).to_ascii_lowercase();
    let ts_str = value_to_string(&values[1]);
    let mut dt = match crate::datetime::parse_timestring(&ts_str) {
        Ok(v) => v,
        Err(_) => return Ok(SqlValue::Null),
    };
    // Always clear sub-second precision before applying coarser truncation.
    dt.micro = 0;
    match field.as_str() {
        "second" => {}
        "minute" => {
            dt.second = 0;
        }
        "hour" => {
            dt.second = 0;
            dt.minute = 0;
        }
        "day" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
        }
        "week" => {
            // PG truncates to Monday of the week. Compute the offset using
            // Zeller-style civil-day math via to_unix.
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            let unix_days = dt.to_unix().div_euclid(86_400);
            // 1970-01-01 was a Thursday → weekday index 3 (Mon=0).
            let wd = (unix_days + 3).rem_euclid(7);
            let monday = unix_days - wd;
            dt = crate::datetime::DateTime::from_unix(monday * 86_400, 0);
        }
        "month" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
        }
        "quarter" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
            // Quarter: 1,2,3 → 1; 4,5,6 → 4; 7,8,9 → 7; 10,11,12 → 10.
            dt.month = ((dt.month - 1) / 3) * 3 + 1;
        }
        "year" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
            dt.month = 1;
        }
        "decade" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
            dt.month = 1;
            dt.year = (dt.year / 10) * 10;
        }
        "century" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
            dt.month = 1;
            // PG: 2000 belongs to the 20th century, so floor by 100 and shift.
            dt.year = ((dt.year - 1) / 100) * 100 + 1;
        }
        "millennium" => {
            dt.second = 0;
            dt.minute = 0;
            dt.hour = 0;
            dt.day = 1;
            dt.month = 1;
            dt.year = ((dt.year - 1) / 1000) * 1000 + 1;
        }
        _ => return Ok(SqlValue::Null),
    }
    if !dt.is_formattable() {
        return Ok(SqlValue::Null);
    }
    Ok(SqlValue::Text(Arc::from(dt.format_datetime())))
}

/// `gen_random_uuid()` — emits a random UUID v4 in canonical 8-4-4-4-12
/// lowercase hex form. Uses the existing kernel RNG (no new crate dep) and
/// sets the version (bits 12-15 of byte 6) and variant (bits 6-7 of byte 8)
/// fields per RFC 4122 §4.4.
pub(crate) fn pg_gen_random_uuid(values: &[SqlValue]) -> Result<SqlValue> {
    if !values.is_empty() {
        return Err(Error::UnsupportedSql(
            "gen_random_uuid takes no arguments".to_owned(),
        ));
    }
    let mut bytes = [0u8; 16];
    // Each random_i64() call gives us 8 random bytes; one call covers half
    // the UUID, two cover the whole 16-byte buffer.
    let lo = random_i64() as u64;
    let hi = random_i64() as u64;
    bytes[..8].copy_from_slice(&lo.to_le_bytes());
    bytes[8..].copy_from_slice(&hi.to_le_bytes());
    // RFC 4122 §4.4 — v4 marker bits.
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    Ok(SqlValue::Text(Arc::from(format_uuid_bytes(&bytes))))
}

/// Render 16 raw bytes as a canonical lowercase 8-4-4-4-12 UUID string.
pub(crate) fn format_uuid_bytes(bytes: &[u8; 16]) -> String {
    let mut out = String::with_capacity(36);
    use std::fmt::Write as _;
    for (i, b) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// `pg_array_contains(lhs, rhs)` — PG `lhs @> rhs`. Returns 1 when every
/// element of the JSON array `rhs` is present in `lhs`, else 0. NULL
/// propagates. The inputs may arrive as TEXT or BLOB — both decode via
/// `serde_json`.
pub(crate) fn pg_array_contains(values: &[SqlValue]) -> Result<SqlValue> {
    pg_array_set_op(values, ArraySetOp::Contains)
}

/// `pg_array_contained(lhs, rhs)` — PG `lhs <@ rhs`. Inverse of contains.
pub(crate) fn pg_array_contained(values: &[SqlValue]) -> Result<SqlValue> {
    pg_array_set_op(values, ArraySetOp::Contained)
}

/// `pg_array_overlap(lhs, rhs)` — PG `lhs && rhs`. Returns 1 when the two
/// JSON arrays share at least one element, else 0.
pub(crate) fn pg_array_overlap(values: &[SqlValue]) -> Result<SqlValue> {
    pg_array_set_op(values, ArraySetOp::Overlap)
}

#[derive(Copy, Clone)]
enum ArraySetOp {
    Contains,
    Contained,
    Overlap,
}

fn pg_array_set_op(values: &[SqlValue], op: ArraySetOp) -> Result<SqlValue> {
    if values.len() != 2 || values.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let lhs = match parse_json_array(&values[0]) {
        Some(v) => v,
        None => return Ok(SqlValue::Null),
    };
    let rhs = match parse_json_array(&values[1]) {
        Some(v) => v,
        None => return Ok(SqlValue::Null),
    };
    let result = match op {
        ArraySetOp::Contains => rhs.iter().all(|e| lhs.contains(e)),
        ArraySetOp::Contained => lhs.iter().all(|e| rhs.contains(e)),
        ArraySetOp::Overlap => lhs.iter().any(|e| rhs.contains(e)),
    };
    Ok(SqlValue::Integer(if result { 1 } else { 0 }))
}

/// Decode `value` as a JSON array. Returns `None` when the value is not a
/// valid JSON array text; callers treat `None` as NULL.
fn parse_json_array(value: &SqlValue) -> Option<Vec<serde_json::Value>> {
    let text = match value {
        SqlValue::Text(t) => t.to_string(),
        SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
        _ => return None,
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Array(a)) => Some(a),
        _ => None,
    }
}
