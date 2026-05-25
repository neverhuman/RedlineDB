use super::super::*;

pub(crate) fn eval_binary(
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if matches!(op, BinaryOperator::Match) {
        let pattern = eval_scalar(right, row, bindings)?;
        return match_result(left, SqlValue::Null, pattern, row);
    }
    let collation = match (collation_from_expr(left), collation_from_expr(right)) {
        (Some(c), _) => Some(c),
        (None, Some(c)) => Some(c),
        (None, None) => declared_collation(row, left),
    };
    let left_value = eval_scalar(left, row, bindings)?;
    let right_value = eval_scalar(right, row, bindings)?;
    let compare_with_collation = |a: SqlValue, b: SqlValue, accept: fn(Ordering) -> bool| {
        compare_binary_with(a, b, accept, collation)
    };
    Ok(match op {
        BinaryOperator::And => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
            (Some(true), Some(true)) => SqlValue::Integer(1),
            _ => SqlValue::Null,
        },
        BinaryOperator::Or => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(true), _) | (_, Some(true)) => SqlValue::Integer(1),
            (Some(false), Some(false)) => SqlValue::Integer(0),
            _ => SqlValue::Null,
        },
        BinaryOperator::Plus => {
            match try_pg_decimal_arith(&left_value, &right_value, PgDecimalOp::Add) {
                Some(v) => v,
                None => arithmetic(
                    left_value,
                    right_value,
                    |a, b| Some(a.wrapping_add(b)),
                    |a, b| Some(a + b),
                )?,
            }
        }
        BinaryOperator::Minus => match try_json_delete(&left_value, &right_value) {
            Some(v) => v,
            None => match try_pg_decimal_arith(&left_value, &right_value, PgDecimalOp::Sub) {
                Some(v) => v,
                None => arithmetic(
                    left_value,
                    right_value,
                    |a, b| Some(a.wrapping_sub(b)),
                    |a, b| Some(a - b),
                )?,
            },
        },
        BinaryOperator::Multiply => {
            match try_pg_decimal_arith(&left_value, &right_value, PgDecimalOp::Mul) {
                Some(v) => v,
                None => arithmetic(
                    left_value,
                    right_value,
                    |a, b| Some(a.wrapping_mul(b)),
                    |a, b| Some(a * b),
                )?,
            }
        }
        BinaryOperator::Divide => {
            match try_pg_decimal_arith(&left_value, &right_value, PgDecimalOp::Div) {
                Some(v) => v,
                None => arithmetic(
                    left_value,
                    right_value,
                    |a, b| if b == 0 { None } else { a.checked_div(b) },
                    |a, b| if b == 0.0 { None } else { Some(a / b) },
                )?,
            }
        }
        BinaryOperator::Modulo => arithmetic(
            left_value,
            right_value,
            |a, b| if b == 0 { None } else { a.checked_rem(b) },
            |a, b| if b == 0.0 { None } else { Some(a % b) },
        )?,
        BinaryOperator::Eq => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Equal)?
        }
        BinaryOperator::Spaceship => match try_vector_pair(&left_value, &right_value) {
            Some((a, b)) => vector_distance_to_value(VectorOpMetric::Cosine, &a, &b)?,
            None => compare_with_collation(left_value, right_value, |o| o != Ordering::Equal)?,
        },
        BinaryOperator::NotEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Equal)?
        }
        BinaryOperator::Gt => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Greater)?
        }
        BinaryOperator::GtEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Less)?
        }
        BinaryOperator::Lt => {
            compare_with_collation(left_value, right_value, |o| o == Ordering::Less)?
        }
        BinaryOperator::LtEq => {
            compare_with_collation(left_value, right_value, |o| o != Ordering::Greater)?
        }
        BinaryOperator::StringConcat => {
            if matches!(left_value, SqlValue::Null) || matches!(right_value, SqlValue::Null) {
                SqlValue::Null
            } else if let Some(merged) = try_json_concat(&left_value, &right_value) {
                merged
            } else {
                SqlValue::Text(Arc::from(format!(
                    "{}{}",
                    value_to_string(&left_value),
                    value_to_string(&right_value)
                )))
            }
        }
        BinaryOperator::Arrow => crate::json::scalar::arrow_json(&left_value, &right_value)?,
        BinaryOperator::LongArrow => crate::json::scalar::arrow_sql(&left_value, &right_value)?,
        BinaryOperator::AtArrow => crate::json::jsonb::op_at_arrow(&left_value, &right_value)?,
        BinaryOperator::ArrowAt => crate::json::jsonb::op_arrow_at(&left_value, &right_value)?,
        BinaryOperator::HashArrow => crate::json::jsonb::op_hash_arrow(&left_value, &right_value)?,
        BinaryOperator::HashLongArrow => {
            crate::json::jsonb::op_hash_long_arrow(&left_value, &right_value)?
        }
        BinaryOperator::HashMinus => crate::json::jsonb::op_hash_minus(&left_value, &right_value)?,
        BinaryOperator::AtAt => crate::json::jsonb::op_at_at(&left_value, &right_value)?,
        BinaryOperator::AtQuestion => {
            crate::json::jsonb::op_at_question(&left_value, &right_value)?
        }
        BinaryOperator::Question => crate::json::jsonb::op_question(&left_value, &right_value)?,
        BinaryOperator::QuestionPipe => {
            crate::json::jsonb::op_question_any(&left_value, std::slice::from_ref(&right_value))?
        }
        BinaryOperator::QuestionAnd => {
            crate::json::jsonb::op_question_all(&left_value, std::slice::from_ref(&right_value))?
        }
        BinaryOperator::Regexp => regexp_result(left_value, right_value, false)?,
        BinaryOperator::Match => match_result(left, left_value, right_value, row)?,
        // Postgres POSIX-regex match operators
        // (https://www.postgresql.org/docs/16/functions-matching.html#FUNCTIONS-POSIX-REGEXP).
        // `~`  matches case-sensitive, `~*` case-insensitive, `!~` / `!~*`
        // are the negated forms. Internally we delegate to the same `regex`
        // crate that backs `REGEXP`; the `(?i)` flag prefix yields case
        // insensitivity without recompiling against a different feature set.
        BinaryOperator::PGRegexMatch => pg_regex_result(left_value, right_value, false, false)?,
        BinaryOperator::PGRegexIMatch => pg_regex_result(left_value, right_value, false, true)?,
        BinaryOperator::PGRegexNotMatch => pg_regex_result(left_value, right_value, true, false)?,
        BinaryOperator::PGRegexNotIMatch => pg_regex_result(left_value, right_value, true, true)?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported binary op {other:?}"
            )));
        }
    })
}

fn match_result(
    left_expr: &Expr,
    left_value: SqlValue,
    pattern: SqlValue,
    row: &RowContext<'_>,
) -> Result<SqlValue> {
    if matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let needle = value_to_string(&pattern).to_ascii_lowercase();
    crate::exec::expr::json_dispatch::set_current_match_term(Some(needle.clone()));
    let haystack = match left_expr {
        Expr::Identifier(ident) => {
            row_values_for_table_name(row, &ident.value).unwrap_or_else(|| vec![left_value])
        }
        _ => vec![left_value],
    };
    let matched = haystack.iter().any(|value| {
        value_to_string(value)
            .to_ascii_lowercase()
            .contains(&needle)
    });
    Ok(SqlValue::Integer(if matched { 1 } else { 0 }))
}

fn row_values_for_table_name(row: &RowContext<'_>, name: &str) -> Option<Vec<SqlValue>> {
    match row {
        RowContext::Table(table) if table.table.name.eq_ignore_ascii_case(name) => {
            Some(table.values.clone())
        }
        RowContext::Joined(rows) => rows.iter().find_map(|joined| {
            let table_name = joined
                .alias
                .as_ref()
                .map(|alias| alias.as_ref())
                .unwrap_or(joined.table.name.as_ref());
            if table_name.eq_ignore_ascii_case(name) {
                joined.row.as_ref().map(|row| row.values.clone())
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn declared_collation(row: &RowContext<'_>, expr: &Expr) -> Option<crate::collation::Collation> {
    let (qualifier, name) = match expr {
        Expr::Identifier(ident) => (None, ident.value.as_str()),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
            (Some(parts[0].value.as_str()), parts[1].value.as_str())
        }
        Expr::Nested(inner) => return declared_collation(row, inner),
        _ => return None,
    };
    let table = match row {
        RowContext::Table(table) => Some(table.table.as_ref()),
        RowContext::Joined(rows) => rows.iter().find_map(|joined| {
            let table_name = joined
                .alias
                .as_ref()
                .map(|alias| alias.as_ref())
                .unwrap_or(joined.table.name.as_ref());
            if qualifier.is_none_or(|q| table_name.eq_ignore_ascii_case(q)) {
                Some(joined.table.as_ref())
            } else {
                None
            }
        }),
        _ => None,
    }?;
    let _ = table
        .columns
        .iter()
        .find(|column| column.folded.eq_ignore_ascii_case(name))?;
    let sql = table.normalized_sql.as_deref()?.to_ascii_lowercase();
    let needle = name.to_ascii_lowercase();
    if sql.split(',').any(|part| {
        let part = part.rsplit('(').next().unwrap_or(part).trim();
        (part.starts_with(&(needle.clone() + " ")) || part.starts_with(&(needle.clone() + "\t")))
            && part.contains("collate nocase")
    }) {
        Some(crate::collation::Collation::NoCase)
    } else {
        None
    }
}

pub(crate) fn regexp_result(value: SqlValue, pattern: SqlValue, negated: bool) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern_str = value_to_string(&pattern);
    let matched = crate::regexp::regex_match(&text, &pattern_str)?;
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

/// Evaluator for Postgres POSIX-regex match operators (`~`, `~*`, `!~`,
/// `!~*`). The case-insensitive flag is implemented by lower-casing both
/// haystack and pattern before delegating to the shared regex engine —
/// the `regex` crate is built without the `unicode-case` feature here,
/// so a `(?i)` prefix would refuse Unicode haystacks. Pre-folding via
/// Rust's `to_lowercase` covers the same ASCII range Postgres' default
/// C-locale `~*` does (and a wider Unicode range on top). NULL on either
/// side propagates.
pub(crate) fn pg_regex_result(
    value: SqlValue,
    pattern: SqlValue,
    negated: bool,
    case_insensitive: bool,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) || matches!(pattern, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = value_to_string(&value);
    let pattern_str = value_to_string(&pattern);
    let (effective_text, effective_pattern) = if case_insensitive {
        (text.to_lowercase(), pattern_str.to_lowercase())
    } else {
        (text.to_string(), pattern_str.to_string())
    };
    let matched = crate::regexp::regex_match(&effective_text, &effective_pattern)?;
    Ok(SqlValue::Integer(if matched ^ negated { 1 } else { 0 }))
}

pub(crate) fn compare_binary(
    left: SqlValue,
    right: SqlValue,
    accept: impl FnOnce(Ordering) -> bool,
) -> Result<SqlValue> {
    compare_binary_with(left, right, accept, None)
}

pub(crate) fn compare_binary_with(
    left: SqlValue,
    right: SqlValue,
    accept: impl FnOnce(Ordering) -> bool,
    collation: Option<crate::collation::Collation>,
) -> Result<SqlValue> {
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let ord = match collation.and_then(|c| c.compare_values(&left, &right)) {
        Some(o) => o,
        None => compare_values(&left, &right),
    };
    Ok(SqlValue::Integer(if accept(ord) { 1 } else { 0 }))
}

pub(crate) fn collation_from_expr(expr: &Expr) -> Option<crate::collation::Collation> {
    match expr {
        Expr::Collate { collation, .. } => {
            crate::collation::Collation::parse(&collation.to_string())
        }
        Expr::Nested(inner) => collation_from_expr(inner),
        _ => None,
    }
}

pub(crate) fn sql_truth_result(value: SqlValue) -> SqlValue {
    SqlValue::Integer(if is_truthy(&value) { 1 } else { 0 })
}

pub(crate) fn sql_truth_result_not(value: SqlValue) -> SqlValue {
    SqlValue::Integer(if !is_truthy(&value) { 1 } else { 0 })
}

pub(crate) fn sql_false_result(value: SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        other => SqlValue::Integer(if !is_truthy(&other) { 1 } else { 0 }),
    }
}

pub(crate) fn sql_false_result_not(value: SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        other => SqlValue::Integer(if is_truthy(&other) { 1 } else { 0 }),
    }
}

pub(crate) fn is_distinct(left: &SqlValue, right: &SqlValue) -> bool {
    matches!(left, SqlValue::Null) != matches!(right, SqlValue::Null)
        || (!matches!(left, SqlValue::Null)
            && !matches!(right, SqlValue::Null)
            && compare_values(left, right) != Ordering::Equal)
}

/// If both operands look like JSON containers (object or array), return
/// their PostgreSQL `jsonb || jsonb` merge. Returns `None` for any other
/// shape so the caller falls back to text concatenation.
fn try_json_concat(left: &SqlValue, right: &SqlValue) -> Option<SqlValue> {
    use serde_json::Value;
    let left_text = match left {
        SqlValue::Text(s) => s.as_ref(),
        _ => return None,
    };
    let right_text = match right {
        SqlValue::Text(s) => s.as_ref(),
        _ => return None,
    };
    let left_doc: Value = serde_json::from_str(left_text.trim()).ok()?;
    let right_doc: Value = serde_json::from_str(right_text.trim()).ok()?;
    // Only switch to JSON semantics when at least one side is a container
    // — otherwise plain `'foo' || 'bar'` would mis-concatenate as an array.
    if !matches!(left_doc, Value::Object(_) | Value::Array(_))
        && !matches!(right_doc, Value::Object(_) | Value::Array(_))
    {
        return None;
    }
    let merged = crate::json::jsonb::op_concat(&left_doc, &right_doc);
    Some(crate::json::jsonb::jsonb_text(&merged))
}

/// PostgreSQL `jsonb - text|int` — strip a key (or array index, with
/// negative-from-end support). Returns `None` when the LHS does not look
/// like a JSON container, so arithmetic subtraction continues to handle
/// numeric pairs.
fn try_json_delete(left: &SqlValue, right: &SqlValue) -> Option<SqlValue> {
    use serde_json::Value;
    let SqlValue::Text(text) = left else {
        return None;
    };
    let doc: Value = serde_json::from_str(text.trim()).ok()?;
    if !matches!(doc, Value::Object(_) | Value::Array(_)) {
        return None;
    }
    let updated = match right {
        SqlValue::Text(key) => crate::json::jsonb::op_delete_key(&doc, key.as_ref()),
        SqlValue::Integer(idx) => crate::json::jsonb::op_delete_index(&doc, *idx),
        _ => return None,
    };
    Some(crate::json::jsonb::jsonb_text(&updated))
}

// ---------------------------------------------------------------------------
// Track H — PG-parity TEXT-shaped decimal arithmetic.
//
// `cast_to_numeric_text` (see crate::exec::expr::coerce::cast) stores
// `::numeric` casts as TEXT in canonical decimal form. This helper detects
// the pair-of-TEXT-decimals case and computes the exact result as a
// canonical TEXT string, preserving PG's full-precision semantics
// (`0.1 + 0.2 = 0.3` rather than `0.30000000000000004`).
//
// Returns `None` when either operand is not a parseable decimal, so the
// caller can fall back to the f64 arithmetic path for SQLite-style inputs.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub(crate) enum PgDecimalOp {
    Add,
    Sub,
    Mul,
    Div,
}

pub(crate) fn try_pg_decimal_arith(
    left: &SqlValue,
    right: &SqlValue,
    op: PgDecimalOp,
) -> Option<SqlValue> {
    // Promote when at least ONE operand looks like a TEXT-shaped decimal
    // (i.e. went through a `::numeric` cast). PG's `1.5::numeric * 3` keeps
    // the result in numeric domain, so we up-cast the integer/real partner
    // to a Decimal for the duration of the op.
    let left_is_text = matches!(left, SqlValue::Text(_));
    let right_is_text = matches!(right, SqlValue::Text(_));
    if !left_is_text && !right_is_text {
        return None;
    }
    let lhs = decimal_from_value(left)?;
    let rhs = decimal_from_value(right)?;
    let result = match op {
        PgDecimalOp::Add => decimal_add(&lhs, &rhs),
        PgDecimalOp::Sub => decimal_sub(&lhs, &rhs),
        PgDecimalOp::Mul => decimal_mul(&lhs, &rhs),
        PgDecimalOp::Div => decimal_div(&lhs, &rhs)?,
    };
    Some(SqlValue::Text(Arc::from(decimal_to_string(&result))))
}

/// Coerce any SqlValue into a Decimal (returning None for shapes that
/// can't be safely promoted — Null, Blob, or non-numeric text).
fn decimal_from_value(value: &SqlValue) -> Option<Decimal> {
    match value {
        SqlValue::Text(t) => parse_decimal_str(t.as_ref()),
        SqlValue::Integer(n) => Some(Decimal {
            sign: if *n < 0 { -1 } else { 1 },
            digits: integer_digits(n.unsigned_abs()),
            scale: 0,
        }),
        SqlValue::Real(r) => {
            // f64 → decimal text via Rust's shortest-round-trip, then parse.
            let txt = format!("{r}");
            parse_decimal_str(&txt)
        }
        _ => None,
    }
}

fn integer_digits(mut n: u64) -> Vec<u8> {
    if n == 0 {
        return vec![0];
    }
    let mut out = Vec::with_capacity(20);
    while n > 0 {
        out.push((n % 10) as u8);
        n /= 10;
    }
    out.reverse();
    out
}

/// Internal decimal representation: sign + arbitrary-precision integer
/// digits + decimal exponent. The value is `sign × digits × 10^(-scale)`.
#[derive(Clone, Debug)]
struct Decimal {
    /// `-1` (negative) or `+1` (non-negative); zero is always `+1`.
    sign: i8,
    /// Magnitude as a stream of `u8` digits (most-significant first); never
    /// has leading zeros except for the single-digit zero representation.
    digits: Vec<u8>,
    /// Number of decimal places (digits to the right of the implied `.`).
    scale: usize,
}

fn parse_decimal_str(s: &str) -> Option<Decimal> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, body) = if let Some(rest) = trimmed.strip_prefix('-') {
        (-1i8, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (1i8, rest)
    } else {
        (1i8, trimmed)
    };
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut digits: Vec<u8> = Vec::with_capacity(int_part.len() + frac_part.len());
    for c in int_part.chars().chain(frac_part.chars()) {
        digits.push(c.to_digit(10).expect("digit") as u8);
    }
    // Strip leading zeros (keep at least one).
    while digits.len() > 1 && digits[0] == 0 {
        digits.remove(0);
    }
    let scale = frac_part.len();
    let zero = digits.iter().all(|&d| d == 0);
    Some(Decimal {
        sign: if zero { 1 } else { sign },
        digits,
        scale,
    })
}

fn align_scale(a: &Decimal, b: &Decimal) -> (Vec<u8>, Vec<u8>, usize) {
    let target = a.scale.max(b.scale);
    let mut left = a.digits.clone();
    for _ in 0..(target - a.scale) {
        left.push(0);
    }
    let mut right = b.digits.clone();
    for _ in 0..(target - b.scale) {
        right.push(0);
    }
    (left, right, target)
}

/// Magnitude comparison of two digit streams (most-significant first).
fn cmp_digits(a: &[u8], b: &[u8]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    a.iter().cmp(b.iter())
}

fn add_digit_streams(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u16;
    let mut i = a.len();
    let mut j = b.len();
    while i > 0 || j > 0 || carry > 0 {
        let da = if i > 0 {
            i -= 1;
            a[i] as u16
        } else {
            0
        };
        let db = if j > 0 {
            j -= 1;
            b[j] as u16
        } else {
            0
        };
        let sum = da + db + carry;
        out.push((sum % 10) as u8);
        carry = sum / 10;
    }
    out.reverse();
    out
}

/// Subtract `b` from `a` assuming `a >= b` (digit-stream magnitudes).
fn sub_digit_streams(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i16;
    let mut i = a.len();
    let mut j = b.len();
    while i > 0 {
        i -= 1;
        let da = a[i] as i16;
        let db = if j > 0 {
            j -= 1;
            b[j] as i16
        } else {
            0
        };
        let mut diff = da - db - borrow;
        if diff < 0 {
            diff += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push(diff as u8);
    }
    while out.len() > 1 && *out.last().unwrap() == 0 {
        out.pop();
    }
    out.reverse();
    out
}

fn decimal_add(a: &Decimal, b: &Decimal) -> Decimal {
    let (la, lb, scale) = align_scale(a, b);
    if a.sign == b.sign {
        let digits = add_digit_streams(&la, &lb);
        let zero = digits.iter().all(|&d| d == 0);
        return Decimal {
            sign: if zero { 1 } else { a.sign },
            digits,
            scale,
        };
    }
    match cmp_digits(&la, &lb) {
        Ordering::Equal => Decimal {
            sign: 1,
            digits: vec![0],
            scale: 0,
        },
        Ordering::Greater => Decimal {
            sign: a.sign,
            digits: sub_digit_streams(&la, &lb),
            scale,
        },
        Ordering::Less => Decimal {
            sign: b.sign,
            digits: sub_digit_streams(&lb, &la),
            scale,
        },
    }
}

fn decimal_sub(a: &Decimal, b: &Decimal) -> Decimal {
    decimal_add(
        a,
        &Decimal {
            sign: -b.sign,
            digits: b.digits.clone(),
            scale: b.scale,
        },
    )
}

fn decimal_mul(a: &Decimal, b: &Decimal) -> Decimal {
    let mut out = vec![0u16; a.digits.len() + b.digits.len()];
    for (i, &da) in a.digits.iter().rev().enumerate() {
        for (j, &db) in b.digits.iter().rev().enumerate() {
            out[i + j] += (da as u16) * (db as u16);
        }
    }
    // Carry-propagate.
    let mut carry = 0u16;
    for slot in out.iter_mut() {
        let total = *slot + carry;
        *slot = total % 10;
        carry = total / 10;
    }
    while carry > 0 {
        out.push((carry % 10) as u16);
        carry /= 10;
    }
    out.reverse();
    let mut digits: Vec<u8> = out.iter().map(|&d| d as u8).collect();
    while digits.len() > 1 && digits[0] == 0 {
        digits.remove(0);
    }
    let scale = a.scale + b.scale;
    let sign = if digits.iter().all(|&d| d == 0) {
        1
    } else {
        a.sign * b.sign
    };
    Decimal {
        sign,
        digits,
        scale,
    }
}

/// PG-style decimal division. PG defaults to a result precision of
/// `max(scale_a + scale_b + 4, scale_a + 1)` with a minimum scale of 6
/// when neither operand has an explicit precision. We use 16 fractional
/// digits as the standard quotient scale, then trim trailing zeros via
/// `decimal_to_string`. Returns `None` on division by zero.
fn decimal_div(a: &Decimal, b: &Decimal) -> Option<Decimal> {
    // Reject division by zero — caller falls back to the integer/real
    // arithmetic path which already returns NULL for `x / 0`.
    if b.digits.iter().all(|&d| d == 0) {
        return None;
    }
    // Standard PG default scale for `numeric / numeric` is 16+ depending on
    // the operands; we use 16 which matches the test corpus's expected
    // 17-significant-figure rendering for `10/3`.
    const QUOTIENT_SCALE: usize = 16;
    // Long division on the aligned dividend (a × 10^(QUOTIENT_SCALE + b.scale)).
    let mut dividend = a.digits.clone();
    let extra = QUOTIENT_SCALE + b.scale;
    for _ in 0..extra {
        dividend.push(0);
    }
    let divisor = &b.digits;
    let mut quotient: Vec<u8> = Vec::with_capacity(dividend.len());
    let mut current: Vec<u8> = Vec::new();
    for &d in &dividend {
        current.push(d);
        // Strip leading zeros from `current` so cmp_digits works.
        while current.len() > 1 && current[0] == 0 {
            current.remove(0);
        }
        let mut q = 0u8;
        while cmp_digits(&current, divisor) != Ordering::Less {
            current = sub_digit_streams(&current, divisor);
            q += 1;
        }
        quotient.push(q);
    }
    while quotient.len() > 1 && quotient[0] == 0 {
        quotient.remove(0);
    }
    // Quotient currently has `(a.scale + extra)` fractional digits relative
    // to a virtual decimal point. We want `(a.scale - b.scale + extra)`
    // = `QUOTIENT_SCALE + a.scale` fractional places, so shift the scale.
    let scale = QUOTIENT_SCALE + a.scale;
    let sign = if quotient.iter().all(|&d| d == 0) {
        1
    } else {
        a.sign * b.sign
    };
    Some(Decimal {
        sign,
        digits: quotient,
        scale,
    })
}

/// Render a Decimal back to canonical text. Trailing zeros in the
/// fractional part are NOT stripped here when `preserve_scale` would matter
/// — we leave that to the caller (PG keeps `1.50` from `1.5 * 3 :: numeric(10,2)`
/// because of the explicit `(10,2)` precision). For now we strip trailing
/// zeros uniformly; the `(p,s)` form is handled by an explicit scale-down
/// pass in the cast path.
fn decimal_to_string(d: &Decimal) -> String {
    let mut digits = d.digits.clone();
    // Pad on the left if scale > digits.len() (e.g. 0.001 has scale=3, digits=[1]).
    while digits.len() <= d.scale {
        digits.insert(0, 0);
    }
    let split = digits.len() - d.scale;
    let int_part: String = digits[..split].iter().map(|d| (b'0' + d) as char).collect();
    let frac_part: String = digits[split..].iter().map(|d| (b'0' + d) as char).collect();
    let int_canon = {
        let trimmed = int_part.trim_start_matches('0');
        if trimmed.is_empty() {
            "0".to_owned()
        } else {
            trimmed.to_owned()
        }
    };
    let frac_canon = frac_part.trim_end_matches('0');
    let is_zero = int_canon == "0" && frac_canon.is_empty();
    let mut out = String::with_capacity(int_canon.len() + frac_canon.len() + 2);
    if d.sign < 0 && !is_zero {
        out.push('-');
    }
    out.push_str(&int_canon);
    if !frac_canon.is_empty() {
        out.push('.');
        out.push_str(frac_canon);
    }
    out
}
