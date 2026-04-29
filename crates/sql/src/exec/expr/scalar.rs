//! Scalar-expression building blocks.
//!
//! This module groups the bulk of the leaf evaluation logic that the
//! `Expr` dispatcher in `mod.rs` calls into:
//!
//!   * arithmetic / numeric helpers (`arithmetic`, `negate`, `parse_number`,
//!     `numeric_value`)
//!   * string-pattern matching (`like_*`, `glob_*`, `match_glob_class`)
//!   * scalar utility functions exposed via SQL (`round_function`,
//!     `hex_value`, `quote_value`, `random_i64`, `value_to_string`)
//!   * vector and date/time helpers (used by both `eval_binary` and the
//!     `eval_function` dispatcher in `json_dispatch.rs`)
//!   * row-context plumbing (`SqlRow`, `RowContext`, `TableRow`,
//!     `JoinedRow`, lookup helpers)
//!   * row-oriented helpers used by other exec modules (`row_width`,
//!     `compare_row_ordering`, `unique_key_bytes`, `encode_sql_row`,
//!     `decode_sql_row`, `key_values_equal`, `scalar_to_usize`)
//!
//! Helpers used only inside `expr/` use `pub(super)` visibility; ones
//! reachable from other parts of the SQL crate keep `pub(crate)`.

use super::*;

pub(crate) fn row_width(row: &[SqlValue]) -> usize {
    row.iter().map(row_width_value).sum()
}

pub(crate) fn row_width_value(value: &SqlValue) -> usize {
    match value {
        SqlValue::Null => 0,
        SqlValue::Integer(_) | SqlValue::Real(_) => 8,
        SqlValue::Text(value) => value.len(),
        SqlValue::Blob(value) => value.len(),
    }
}

// Lane VE: the legacy SQL-A in-place row-sort path was replaced by
// `vec::SpillSort` + top-K heap, so this helper is currently unused
// outside tests. Marked allow(dead_code) instead of removed to keep
// the SQL-A surface intact.
#[allow(dead_code)]
pub(crate) fn compare_row_ordering(
    left: &SqlRow,
    right: &SqlRow,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<Ordering> {
    for order in order_by {
        let collation = collation_from_expr(&order.expr);
        let left_value = eval_scalar(&order.expr, &left.context(), bindings)?;
        let right_value = eval_scalar(&order.expr, &right.context(), bindings)?;
        let mut ord = collation
            .and_then(|c| c.compare_values(&left_value, &right_value))
            .unwrap_or_else(|| compare_values(&left_value, &right_value));
        if matches!(order.options.asc, Some(false)) {
            ord = ord.reverse();
        }
        if ord != Ordering::Equal {
            return Ok(ord);
        }
    }
    Ok(Ordering::Equal)
}

pub(super) fn like_result(
    value: SqlValue,
    pattern: SqlValue,
    negated: bool,
    escape_char: Option<Value>,
    case_insensitive: bool,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern = value_to_string(&pattern);
    let escape = match escape_char {
        Some(Value::SingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::SingleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DoubleQuotedRawStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleSingleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::TripleDoubleQuotedString(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::EscapedStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::UnicodeStringLiteral(s)) if s.chars().count() == 1 => {
            Some(s.chars().next().unwrap())
        }
        Some(Value::DollarQuotedString(s)) if s.value.chars().count() == 1 => {
            Some(s.value.chars().next().unwrap())
        }
        None => None,
        Some(other) => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported LIKE escape literal: {other:?}"
            )));
        }
    };
    let matched = like_match(&text, &pattern, escape, case_insensitive);
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

fn like_match(text: &str, pattern: &str, escape: Option<char>, case_insensitive: bool) -> bool {
    let text = if case_insensitive {
        text.to_ascii_lowercase()
    } else {
        text.to_owned()
    };
    let pattern = if case_insensitive {
        pattern.to_ascii_lowercase()
    } else {
        pattern.to_owned()
    };
    like_match_inner(
        text.as_bytes(),
        pattern.as_bytes(),
        escape.map(|c| c.to_ascii_lowercase()),
    )
}

fn like_match_inner(text: &[u8], pattern: &[u8], escape: Option<char>) -> bool {
    fn inner(text: &[u8], pattern: &[u8], escape: Option<u8>) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                b'%' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..], escape) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                b'_' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                b if Some(b) == escape => {
                    pi += 1;
                    if pi >= pattern.len() || ti >= text.len() || pattern[pi] != text[ti] {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }
    inner(text, pattern, escape.map(|c| c as u8))
}

pub(super) fn glob_result(value: SqlValue, pattern: SqlValue, negated: bool) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern = value_to_string(&pattern);
    let matched = glob_match(text.as_bytes(), pattern.as_bytes());
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

fn glob_match(text: &[u8], pattern: &[u8]) -> bool {
    // SQLite GLOB grammar:
    //   *           — matches zero or more characters
    //   ?           — matches exactly one character
    //   [abc]       — character class (any of)
    //   [a-z]       — character range
    //   [!abc]      — negated class (matches one char NOT in abc)
    //   [^abc]      — also a negated class (compatibility)
    //   anything else — literal (case-sensitive, unlike LIKE)
    // An unterminated `[` is treated as a literal `[`.
    fn inner(text: &[u8], pattern: &[u8]) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                b'*' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..]) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                b'?' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                b'[' => {
                    if let Some((matched, advance)) = match_glob_class(&pattern[pi..], text.get(ti))
                    {
                        if !matched {
                            return false;
                        }
                        ti += 1;
                        pi += advance;
                    } else {
                        // Unterminated class: treat `[` as a literal.
                        if ti >= text.len() || text[ti] != b'[' {
                            return false;
                        }
                        ti += 1;
                        pi += 1;
                    }
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }
    inner(text, pattern)
}

/// Try to match a `[...]` character class at the start of `pattern` against
/// the optional next byte of the input. Returns `(matched, pattern_advance)`
/// on success; `None` when the class is unterminated (caller should treat
/// the leading `[` as a literal).
fn match_glob_class(pattern: &[u8], target: Option<&u8>) -> Option<(bool, usize)> {
    debug_assert!(pattern.first() == Some(&b'['));
    let mut idx = 1usize;
    let negate = matches!(pattern.get(idx), Some(&b'!') | Some(&b'^'));
    if negate {
        idx += 1;
    }
    let class_start = idx;
    let mut matched = false;
    let target_byte = match target {
        Some(&b) => b,
        None => 0,
    };

    // SQLite allows a literal `]` only as the first character of the class.
    // So `[]abc]` matches `]`, `a`, `b`, or `c`.
    if pattern.get(idx) == Some(&b']') {
        if target.is_some() && target_byte == b']' {
            matched = true;
        }
        idx += 1;
    }

    while idx < pattern.len() && pattern[idx] != b']' {
        let lo = pattern[idx];
        if idx + 2 < pattern.len() && pattern[idx + 1] == b'-' && pattern[idx + 2] != b']' {
            let hi = pattern[idx + 2];
            if target.is_some() && target_byte >= lo.min(hi) && target_byte <= lo.max(hi) {
                matched = true;
            }
            idx += 3;
        } else {
            if target.is_some() && target_byte == lo {
                matched = true;
            }
            idx += 1;
        }
    }

    if idx >= pattern.len() {
        // No closing `]` — pattern is malformed. Caller falls back to literal.
        if class_start == idx {
            return None;
        }
        return None;
    }

    let final_match = if target.is_none() {
        false
    } else if negate {
        !matched
    } else {
        matched
    };

    Some((final_match, idx + 1))
}

pub(super) fn round_function(values: &[SqlValue]) -> Result<SqlValue> {
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
    Ok(canonicalize(SqlValue::Real(
        (value * factor).round() / factor,
    )))
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

pub(super) fn hex_value(value: &SqlValue) -> String {
    let bytes: Vec<u8> = match value {
        SqlValue::Null => Vec::new(),
        SqlValue::Integer(v) => v.to_string().into_bytes(),
        SqlValue::Real(v) => v.to_string().into_bytes(),
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

pub(super) fn quote_value(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "NULL".to_owned(),
        SqlValue::Integer(v) => v.to_string(),
        SqlValue::Real(v) => v.to_string(),
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

pub(super) fn random_i64() -> i64 {
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

pub(super) fn hex_string_to_bytes(input: &str) -> Result<Arc<[u8]>> {
    if !input.len().is_multiple_of(2) {
        return Err(Error::UnsupportedSql(format!(
            "invalid hex string literal: {input}"
        )));
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let hi = hex_digit(pair[0])?;
        let lo = hex_digit(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(Arc::from(out))
}

fn hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::UnsupportedSql(format!(
            "invalid hex digit in blob literal: {}",
            byte as char
        ))),
    }
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

pub(super) fn vector_construct_from_value(value: &SqlValue) -> Result<SqlValue> {
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

pub(super) fn vector_dims_value(value: &SqlValue) -> Result<SqlValue> {
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

pub(super) fn vector_pair_distance(
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

pub(super) fn try_vector_pair(left: &SqlValue, right: &SqlValue) -> Option<(Vec<f32>, Vec<f32>)> {
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

pub(super) fn vector_distance_to_value(
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
pub(super) enum DateTimeKind {
    Date,
    Time,
    Datetime,
    JulianDay,
    Unix,
}

pub(super) fn datetime_function(values: &[SqlValue], kind: DateTimeKind) -> Result<SqlValue> {
    let dt = parse_dt_args(values)?;
    Ok(match kind {
        DateTimeKind::Date => SqlValue::Text(Arc::from(dt.format_date())),
        DateTimeKind::Time => SqlValue::Text(Arc::from(dt.format_time())),
        DateTimeKind::Datetime => SqlValue::Text(Arc::from(dt.format_datetime())),
        DateTimeKind::JulianDay => SqlValue::Real(dt.julian_day()),
        DateTimeKind::Unix => SqlValue::Integer(dt.to_unix()),
    })
}

pub(super) fn strftime_function(values: &[SqlValue]) -> Result<SqlValue> {
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

pub(super) fn negate(value: SqlValue) -> Result<SqlValue> {
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

pub(super) fn parse_number(input: &str) -> Result<SqlValue> {
    if let Ok(v) = input.parse::<i64>() {
        return Ok(SqlValue::Integer(v));
    }
    if let Ok(v) = input.parse::<f64>() {
        return Ok(canonicalize(SqlValue::Real(v)));
    }
    Err(Error::Parse(format!("invalid numeric literal {input}")))
}

pub(crate) fn value_to_string(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => String::new(),
        SqlValue::Integer(v) => v.to_string(),
        SqlValue::Real(v) => v.to_string(),
        SqlValue::Text(v) => v.to_string(),
        SqlValue::Blob(v) => String::from_utf8_lossy(v).into_owned(),
    }
}

pub(super) fn resolve_binding(name: &str, bindings: &[Option<SqlValue>]) -> Result<SqlValue> {
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

pub(super) fn lookup_column(row: &RowContext<'_>, name: &str) -> Result<SqlValue> {
    match row {
        RowContext::Table(row) => lookup_table_column(row, name),
        RowContext::Upsert { current, .. } => lookup_table_column(current, name),
        RowContext::Joined(rows) => {
            let mut found = None;
            for row in rows.iter() {
                if row.row.is_none() {
                    continue;
                }
                if let Ok(value) = lookup_joined_row_column(row, name) {
                    if found.is_some() {
                        return Err(Error::UnsupportedSql(format!(
                            "ambiguous column name: {name}"
                        )));
                    }
                    found = Some(value);
                }
            }
            found.ok_or_else(|| Error::UnknownColumn(name.to_owned()))
        }
        RowContext::SqliteSchema(row) => match name.to_ascii_lowercase().as_str() {
            "type" => Ok(SqlValue::Text(Arc::from(row.type_name.as_ref()))),
            "name" => Ok(SqlValue::Text(Arc::from(row.name.as_ref()))),
            "tbl_name" => Ok(SqlValue::Text(Arc::from(row.tbl_name.as_ref()))),
            "rootpage" => Ok(SqlValue::Integer(row.rootpage as i64)),
            "sql" => Ok(SqlValue::Text(Arc::from(row.sql.as_ref()))),
            _ => Err(Error::UnknownColumn(name.to_owned())),
        },
        RowContext::Empty => Err(Error::UnknownColumn(name.to_owned())),
    }
}

pub(super) fn lookup_qualified_column(
    row: &RowContext<'_>,
    qualifier: &str,
    name: &str,
) -> Result<SqlValue> {
    match row {
        RowContext::Table(row) => {
            if row_matches_qualifier(row, qualifier) {
                lookup_table_column(row, name)
            } else {
                Err(Error::UnknownColumn(format!("{qualifier}.{name}")))
            }
        }
        RowContext::Joined(rows) => {
            let mut found = None;
            for row in rows.iter() {
                if row_matches_joined_qualifier(row, qualifier) {
                    let value = lookup_joined_row_column(row, name)?;
                    if found.is_some() {
                        return Err(Error::UnsupportedSql(format!(
                            "ambiguous column reference: {qualifier}.{name}"
                        )));
                    }
                    found = Some(value);
                }
            }
            found.ok_or_else(|| Error::UnknownColumn(format!("{qualifier}.{name}")))
        }
        RowContext::Upsert { current, excluded } => {
            if row_matches_qualifier(current, qualifier) {
                lookup_table_column(current, name)
            } else if qualifier.eq_ignore_ascii_case("excluded") {
                lookup_excluded_column(current.table.as_ref(), excluded, name)
            } else {
                Err(Error::UnknownColumn(format!("{qualifier}.{name}")))
            }
        }
        RowContext::SqliteSchema(row) => match qualifier.to_ascii_lowercase().as_str() {
            "sqlite_schema" | "sqlite_master" => lookup_schema_column(row, name),
            _ => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
        },
        RowContext::Empty => Err(Error::UnknownColumn(format!("{qualifier}.{name}"))),
    }
}

fn row_matches_qualifier(row: &TableRow, qualifier: &str) -> bool {
    if let Some(alias) = &row.alias
        && alias.as_ref().eq_ignore_ascii_case(qualifier)
    {
        return true;
    }
    row.table.name.to_string().eq_ignore_ascii_case(qualifier)
}

fn row_matches_joined_qualifier(row: &JoinedRow, qualifier: &str) -> bool {
    if let Some(alias) = &row.alias
        && alias.as_ref().eq_ignore_ascii_case(qualifier)
    {
        return true;
    }
    row.table.name.to_string().eq_ignore_ascii_case(qualifier)
}

fn lookup_schema_column(row: &SqliteSchemaRow, name: &str) -> Result<SqlValue> {
    match name.to_ascii_lowercase().as_str() {
        "type" => Ok(SqlValue::Text(Arc::from(row.type_name.as_ref()))),
        "name" => Ok(SqlValue::Text(Arc::from(row.name.as_ref()))),
        "tbl_name" => Ok(SqlValue::Text(Arc::from(row.tbl_name.as_ref()))),
        "rootpage" => Ok(SqlValue::Integer(row.rootpage as i64)),
        "sql" => Ok(SqlValue::Text(Arc::from(row.sql.as_ref()))),
        _ => Err(Error::UnknownColumn(name.to_owned())),
    }
}

fn lookup_table_column(row: &TableRow, name: &str) -> Result<SqlValue> {
    if name.eq_ignore_ascii_case("rowid")
        || name.eq_ignore_ascii_case("_rowid_")
        || name.eq_ignore_ascii_case("oid")
    {
        return Ok(SqlValue::Integer(row.rowid.0 as i64));
    }
    let idx = row
        .table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
    Ok(row.values[idx].clone())
}

fn lookup_joined_row_column(row: &JoinedRow, name: &str) -> Result<SqlValue> {
    match &row.row {
        Some(present) => lookup_table_column(present, name),
        None => {
            if name.eq_ignore_ascii_case("rowid")
                || name.eq_ignore_ascii_case("_rowid_")
                || name.eq_ignore_ascii_case("oid")
            {
                return Ok(SqlValue::Null);
            }
            row.table
                .columns
                .iter()
                .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
                .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
            Ok(SqlValue::Null)
        }
    }
}

fn lookup_excluded_column(table: &TableDef, excluded: &[SqlValue], name: &str) -> Result<SqlValue> {
    if name.eq_ignore_ascii_case("rowid")
        || name.eq_ignore_ascii_case("_rowid_")
        || name.eq_ignore_ascii_case("oid")
    {
        if let Some(alias) = table.rowid_alias_column
            && let Some(value) = excluded.get(alias as usize)
        {
            return Ok(value.clone());
        }
        return Err(Error::UnknownColumn(name.to_owned()));
    }
    let idx = table
        .columns
        .iter()
        .position(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))?;
    Ok(excluded.get(idx).cloned().unwrap_or(SqlValue::Null))
}

pub(crate) fn unique_key_bytes(
    table_id: u64,
    constraint_id: u64,
    values: &[SqlValue],
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(&table_id.to_le_bytes());
    out.extend_from_slice(&constraint_id.to_le_bytes());
    let refs = values.iter().map(|v| v.as_ref()).collect::<Vec<_>>();
    encode_record(&refs, &mut out).map_err(|_| Error::DatatypeMismatch)?;
    Ok(out)
}

pub(crate) fn key_values_equal(left: &[SqlValue], right: &[SqlValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(a, b)| compare_values(a, b) == Ordering::Equal)
}

pub(crate) fn encode_sql_row(table_id: u64, values: &[SqlValue]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut refs = Vec::with_capacity(values.len() + 1);
    refs.push(ValueRef::Integer(table_id as i64));
    refs.extend(values.iter().map(|value| value.as_ref()));
    encode_record(&refs, &mut out).map_err(|_| Error::DatatypeMismatch)?;
    Ok(out)
}

pub(crate) fn decode_sql_row(bytes: &[u8]) -> Result<Option<(u64, Vec<SqlValue>)>> {
    let record = RecordRef::new(bytes).map_err(|_| Error::DatatypeMismatch)?;
    let mut scratch = RecordScratch::default();
    record
        .decode_into(&mut scratch)
        .map_err(|_| Error::DatatypeMismatch)?;
    let mut values = Vec::new();
    let table_id = match record
        .value_at(&scratch, 0)
        .map_err(|_| Error::DatatypeMismatch)?
    {
        ValueRef::Integer(v) => v as u64,
        _ => return Err(Error::DatatypeMismatch),
    };
    for idx in 1..record.column_count().map_err(|_| Error::DatatypeMismatch)? {
        let value = record
            .value_at(&scratch, idx)
            .map_err(|_| Error::DatatypeMismatch)?;
        values.push(value.to_owned());
    }
    Ok(Some((table_id, values)))
}

pub(crate) fn scalar_to_usize(value: &SqlValue) -> Result<usize> {
    match value {
        SqlValue::Integer(v) => Ok((*v).max(0) as usize),
        SqlValue::Real(v) => Ok((*v).max(0.0) as usize),
        SqlValue::Null => Ok(0),
        _ => Err(Error::DatatypeMismatch),
    }
}

#[derive(Clone)]
pub(crate) struct TableRow {
    pub(crate) rowid: RowId,
    pub(crate) values: Vec<SqlValue>,
    pub(crate) table: Arc<TableDef>,
    pub(crate) alias: Option<Arc<str>>,
}

#[derive(Clone)]
pub(crate) struct JoinedRow {
    pub(crate) table: Arc<TableDef>,
    pub(crate) alias: Option<Arc<str>>,
    pub(crate) row: Option<TableRow>,
}

pub(crate) struct TableRowSource<'a> {
    pub(crate) values: &'a [SqlValue],
}

impl RowValueSource for TableRowSource<'_> {
    fn value_at(&self, col: u16) -> Option<OwnedValue> {
        self.values.get(col as usize).cloned()
    }
}

#[derive(Clone)]
pub(crate) enum SqlRow {
    Table(TableRow),
    Joined(Vec<JoinedRow>),
    SqliteSchema(SqliteSchemaRow),
    Static(Vec<SqlValue>),
    Empty,
}

pub(crate) enum RowContext<'a> {
    Table(&'a TableRow),
    Joined(&'a [JoinedRow]),
    Upsert {
        current: &'a TableRow,
        excluded: &'a [SqlValue],
    },
    SqliteSchema(&'a SqliteSchemaRow),
    Empty,
}

impl SqlRow {
    pub(crate) fn context(&self) -> RowContext<'_> {
        match self {
            SqlRow::Table(row) => RowContext::Table(row),
            SqlRow::Joined(rows) => RowContext::Joined(rows),
            SqlRow::SqliteSchema(row) => RowContext::SqliteSchema(row),
            SqlRow::Static(_) => RowContext::Empty,
            SqlRow::Empty => RowContext::Empty,
        }
    }

    pub(crate) fn values(&self) -> Result<Vec<SqlValue>> {
        match self {
            SqlRow::Table(row) => Ok(row.values.clone()),
            SqlRow::Joined(rows) => Ok(rows
                .iter()
                .flat_map(|row| match &row.row {
                    Some(present) => present.values.clone(),
                    None => vec![SqlValue::Null; row.table.columns.len()],
                })
                .collect::<Vec<_>>()),
            SqlRow::SqliteSchema(row) => Ok(vec![
                SqlValue::Text(Arc::from(row.type_name.as_ref())),
                SqlValue::Text(Arc::from(row.name.as_ref())),
                SqlValue::Text(Arc::from(row.tbl_name.as_ref())),
                SqlValue::Integer(row.rootpage as i64),
                SqlValue::Text(Arc::from(row.sql.as_ref())),
            ]),
            SqlRow::Static(values) => Ok(values.clone()),
            SqlRow::Empty => Ok(Vec::new()),
        }
    }
}
