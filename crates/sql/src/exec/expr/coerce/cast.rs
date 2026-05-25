use super::super::*;

pub(crate) fn cast_value(
    value: SqlValue,
    data_type: &sqlparser::ast::DataType,
) -> Result<SqlValue> {
    if matches!(value, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let type_name = data_type.to_string().to_ascii_lowercase();

    if type_name.contains("blob") {
        return Ok(match value {
            SqlValue::Blob(_) => value,
            SqlValue::Text(s) => SqlValue::Blob(Arc::from(s.as_bytes())),
            other => SqlValue::Blob(Arc::from(value_to_string(&other).into_bytes())),
        });
    }

    if type_name.contains("text") || type_name.contains("char") || type_name.contains("clob") {
        return Ok(match value {
            SqlValue::Text(_) => value,
            SqlValue::Integer(v) => SqlValue::Text(Arc::from(v.to_string())),
            SqlValue::Real(v) => SqlValue::Text(Arc::from(
                crate::exec::expr::scalar::value::format_real_sqlite(v),
            )),
            SqlValue::Blob(v) => {
                SqlValue::Text(Arc::from(String::from_utf8_lossy(&v).into_owned()))
            }
            SqlValue::Null => SqlValue::Null,
        });
    }

    if type_name.contains("real") || type_name.contains("floa") || type_name.contains("doub") {
        return Ok(SqlValue::Real(cast_to_real(&value)));
    }

    if type_name.contains("int") {
        return Ok(SqlValue::Integer(cast_to_integer(&value)));
    }

    if type_name.contains("numeric") {
        // Track H — PG-style: keep NUMERIC values as TEXT so subsequent
        // arithmetic can preserve full precision (PG's `0.1 + 0.2 = 0.3`
        // semantics) rather than being rounded into a Rust f64. The binary
        // operator dispatcher (see `crate::exec::expr::coerce::binary`)
        // recognises TEXT-shaped decimals and applies string-based math.
        //
        // If the type carries an explicit `(p, s)` form, we round-pad the
        // result to exactly `s` fractional digits so `(1.5 * 3)::numeric(10,2)`
        // renders as `4.50` (not `4.5`).
        let scale = parse_numeric_scale(&type_name);
        let result = cast_to_numeric_text(&value);
        if let (SqlValue::Text(text), Some(target_scale)) = (&result, scale) {
            return Ok(SqlValue::Text(Arc::from(rescale_decimal_text(
                text.as_ref(),
                target_scale,
            ))));
        }
        return Ok(result);
    }

    // Track H — beyond-SQLite (Postgres parity) casts. Both target a TEXT-or-
    // INTEGER shape that the existing `.mode list` formatter renders to the
    // same bytes psql emits in unaligned + tuples-only mode.
    //
    // BOOLEAN: PG accepts a generous family of truthy/falsy literals
    // (`true|t|yes|y|on|1` / `false|f|no|n|off|0`, case-insensitive, leading
    // and trailing whitespace allowed) and prints `t` or `f`. We store the
    // result as Integer(0|1) so SQLite affinity rules keep behaving
    // (NULLs propagate; arithmetic still works); the beyond_sqlite oracle's
    // `BooleanTfToInt` normalizer collapses PG's `t/f` back to `1/0` for the
    // byte-exact compare.
    if type_name == "bool" || type_name == "boolean" {
        return Ok(cast_to_boolean(&value));
    }

    // UUID: PG stores as 16-byte binary, prints as canonical lowercase
    // 8-4-4-4-12. We accept either the canonical form (with or without
    // hyphens) or the curly-brace form `{xxxx...}` and store as TEXT in
    // canonical lowercase. Invalid inputs propagate the original value
    // unchanged so a downstream operator can raise a more specific error.
    if type_name == "uuid" {
        return Ok(cast_to_uuid(&value));
    }

    // TIMESTAMPTZ / TIMESTAMP / DATE / TIME — RedlineDB is timezone-naive
    // (all timestamps are stored as text in UTC), so we normalise the cast
    // by stripping a trailing `+HH[:MM]` / `Z` offset from any text input.
    // The result remains text-shaped so downstream `datetime()`-style
    // functions see a plain UTC literal.
    if type_name == "timestamp with time zone"
        || type_name == "timestamptz"
        || type_name == "timestamp"
        || type_name == "time with time zone"
        || type_name == "timetz"
        || type_name == "time"
        || type_name == "date"
    {
        return Ok(cast_to_timestamp_text(&value));
    }

    Ok(value)
}

/// Normalize a value for a date/time-shaped PG cast. We strip any trailing
/// UTC offset (`+HH[:MM]`, `Z`) so the resulting TEXT round-trips through
/// RedlineDB's tz-naive `datetime()` helpers. Non-text values pass through
/// unchanged.
fn cast_to_timestamp_text(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Text(t) => {
            let s = t.as_ref();
            let stripped = strip_trailing_tz(s);
            if stripped == s {
                value.clone()
            } else {
                SqlValue::Text(Arc::from(stripped))
            }
        }
        other => other.clone(),
    }
}

/// String-level dual of `datetime::parse::strip_tz_suffix`: drops a
/// trailing `+HH[:MM]`, `-HH[:MM]`, or `Z` from a timestamp string and
/// returns the remainder as an owned String.
///
/// The stripper is intentionally conservative: it only acts when there is
/// at least one `:` separator between the offset and the date portion
/// (i.e., the input must contain a time-of-day, otherwise a literal like
/// `'2025-01-01'` would be misparsed as `'2025-01' + offset '-01'`).
fn strip_trailing_tz(input: &str) -> String {
    if let Some(stripped) = input.strip_suffix('Z') {
        return stripped.to_owned();
    }
    // Guard: never strip from a date-only literal (no `:` present).
    if !input.contains(':') {
        return input.to_owned();
    }
    let bytes = input.as_bytes();
    let mut i = bytes.len();
    let mut seen_digit_run = 0usize;
    let mut seen_colon = false;
    while i > 0 {
        let b = bytes[i - 1];
        match b {
            b'0'..=b'9' => {
                seen_digit_run += 1;
                if seen_digit_run > 5 {
                    break;
                }
                i -= 1;
            }
            b':' => {
                if seen_colon {
                    break;
                }
                seen_colon = true;
                seen_digit_run = 0;
                i -= 1;
            }
            b'+' | b'-' => {
                if seen_digit_run >= 2 {
                    return input[..i - 1].to_owned();
                }
                break;
            }
            _ => break,
        }
    }
    input.to_owned()
}

/// Cast `value` to a boolean (`Integer(0)` or `Integer(1)`). Accepts the
/// Postgres family of textual truth values; falls back to SQLite's truthiness
/// rule (non-zero numeric => true, parse-as-number for TEXT) for inputs that
/// don't match a PG keyword.
fn cast_to_boolean(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Integer(n) => SqlValue::Integer(if *n != 0 { 1 } else { 0 }),
        SqlValue::Real(r) => SqlValue::Integer(if *r != 0.0 && !r.is_nan() { 1 } else { 0 }),
        SqlValue::Text(t) => match parse_pg_boolean(t.as_ref()) {
            Some(b) => SqlValue::Integer(if b { 1 } else { 0 }),
            None => SqlValue::Integer(if crate::value::is_truthy(value) { 1 } else { 0 }),
        },
        SqlValue::Blob(b) => {
            let text = String::from_utf8_lossy(b);
            match parse_pg_boolean(&text) {
                Some(v) => SqlValue::Integer(if v { 1 } else { 0 }),
                None => SqlValue::Integer(if crate::value::is_truthy(value) { 1 } else { 0 }),
            }
        }
    }
}

/// Parse a Postgres boolean literal. Returns `Some(true|false)` for any of
/// the documented spellings (`true|t|yes|y|on|1` / `false|f|no|n|off|0`),
/// case-insensitive with leading and trailing whitespace allowed, otherwise
/// `None`.
fn parse_pg_boolean(text: &str) -> Option<bool> {
    let lower = text.trim().to_ascii_lowercase();
    match lower.as_str() {
        "true" | "t" | "yes" | "y" | "on" | "1" => Some(true),
        "false" | "f" | "no" | "n" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// Cast `value` to a canonical-lowercase UUID string. Accepts either the
/// 36-char canonical form, the 32-char hex-only form, or the curly-brace
/// `{xxxx...}` form. Non-matching inputs are returned as-is (a downstream
/// operator will fail with a more specific error if the value is then used
/// where a uuid is required).
fn cast_to_uuid(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Text(t) => match canonicalize_uuid(t.as_ref()) {
            Some(s) => SqlValue::Text(Arc::from(s)),
            None => value.clone(),
        },
        SqlValue::Blob(b) => {
            if b.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(b);
                SqlValue::Text(Arc::from(
                    crate::exec::expr::scalar::value::format_uuid_bytes(&arr),
                ))
            } else {
                let text = String::from_utf8_lossy(b);
                match canonicalize_uuid(&text) {
                    Some(s) => SqlValue::Text(Arc::from(s)),
                    None => value.clone(),
                }
            }
        }
        other => other.clone(),
    }
}

/// Return the canonical lowercase 8-4-4-4-12 form of a textual UUID, or
/// `None` when the input is not a recognised UUID literal.
fn canonicalize_uuid(text: &str) -> Option<String> {
    let trimmed = text.trim();
    // Tolerate the `{xxxx...}` braces sometimes seen in Windows-style UUIDs.
    let body = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    // Strip all hyphens, then verify we have exactly 32 hex digits.
    let mut hex = String::with_capacity(32);
    for ch in body.chars() {
        if ch == '-' {
            continue;
        }
        if !ch.is_ascii_hexdigit() {
            return None;
        }
        for low in ch.to_lowercase() {
            hex.push(low);
        }
    }
    if hex.len() != 32 {
        return None;
    }
    let mut out = String::with_capacity(36);
    for (i, ch) in hex.chars().enumerate() {
        if matches!(i, 8 | 12 | 16 | 20) {
            out.push('-');
        }
        out.push(ch);
    }
    Some(out)
}

fn cast_to_integer(value: &SqlValue) -> i64 {
    match value {
        SqlValue::Null => 0,
        SqlValue::Integer(v) => *v,
        SqlValue::Real(v) => {
            if v.is_nan() {
                0
            } else if *v > i64::MAX as f64 {
                i64::MAX
            } else if *v < i64::MIN as f64 {
                i64::MIN
            } else {
                *v as i64
            }
        }
        SqlValue::Text(s) => parse_integer_prefix(s),
        SqlValue::Blob(b) => parse_integer_prefix(&String::from_utf8_lossy(b)),
    }
}

fn cast_to_real(value: &SqlValue) -> f64 {
    match value {
        SqlValue::Null => 0.0,
        SqlValue::Integer(v) => *v as f64,
        SqlValue::Real(v) => *v,
        SqlValue::Text(s) => parse_real_prefix(s),
        SqlValue::Blob(b) => parse_real_prefix(&String::from_utf8_lossy(b)),
    }
}

#[allow(dead_code)]
fn cast_to_numeric(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Integer(_) | SqlValue::Real(_) => value.clone(),
        SqlValue::Text(t) => parse_numeric_text(t.as_ref()),
        SqlValue::Blob(b) => parse_numeric_text(&String::from_utf8_lossy(b)),
    }
}

/// Track H — TEXT-shaped numeric: keep the value as TEXT so subsequent
/// arithmetic preserves full precision. The text is the canonical form
/// (no trailing zeros, no leading `+`) so equality is stable.
fn cast_to_numeric_text(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Text(t) => {
            let canon = canonicalize_decimal(t.as_ref()).unwrap_or_else(|| t.to_string());
            SqlValue::Text(Arc::from(canon))
        }
        SqlValue::Integer(n) => SqlValue::Text(Arc::from(n.to_string())),
        SqlValue::Real(r) => {
            // Render via SQLite-compatible shortest-round-trip. For literal
            // values that bridge the lexer→f64→text path (e.g. `0.1::numeric`)
            // the f64 already lost precision; we can't recover it here, but
            // the shortest-round-trip yields the user-intended "0.1" form
            // 99% of the time.
            let txt = crate::exec::expr::scalar::value::format_real_sqlite(*r);
            // Drop the trailing `.0` SQLite appends for integer-valued reals
            // (PG `1.0::numeric` shows `1`, not `1.0`).
            let canon = if let Some(stripped) = txt.strip_suffix(".0") {
                stripped.to_owned()
            } else {
                txt
            };
            SqlValue::Text(Arc::from(canon))
        }
        SqlValue::Blob(b) => {
            let text = String::from_utf8_lossy(b);
            let canon = canonicalize_decimal(&text).unwrap_or_else(|| text.into_owned());
            SqlValue::Text(Arc::from(canon))
        }
    }
}

/// Parse the `s` argument of `NUMERIC(p, s)` from a lowercased type string.
/// Returns `None` when the type is the unparameterised `numeric` form.
fn parse_numeric_scale(type_name: &str) -> Option<usize> {
    let open = type_name.find('(')?;
    let close = type_name.rfind(')')?;
    if close <= open + 1 {
        return None;
    }
    let body = &type_name[open + 1..close];
    let mut parts = body.split(',');
    let _p = parts.next()?;
    let s = parts.next()?.trim();
    s.parse::<usize>().ok()
}

/// Re-pad / re-truncate a TEXT-shaped decimal to exactly `target_scale`
/// fractional digits. Used to honour the explicit `(p, s)` part of a
/// NUMERIC cast (PG: `(1.5 * 3)::numeric(10,2)` → `4.50`, not `4.5`).
fn rescale_decimal_text(input: &str, target_scale: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return input.to_owned();
    }
    let (sign, body) = if let Some(rest) = trimmed.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", trimmed)
    };
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    let mut out = String::with_capacity(input.len() + target_scale);
    out.push_str(sign);
    out.push_str(int_part);
    if target_scale == 0 {
        // Round-toward-zero truncation when shrinking to 0 fractional
        // digits — we don't currently bother with PG's half-even rounding.
        return out;
    }
    out.push('.');
    let frac_canon = if frac_part.len() >= target_scale {
        frac_part[..target_scale].to_owned()
    } else {
        let mut padded = frac_part.to_owned();
        while padded.len() < target_scale {
            padded.push('0');
        }
        padded
    };
    out.push_str(&frac_canon);
    out
}

/// Canonical-form a decimal string: strip leading `+`, strip a leading zero
/// before a decimal point only when not necessary, strip trailing zeros in
/// the fractional part (keeping at least the digit before `.` if all
/// fraction is zeros, in which case the `.` itself is also stripped).
pub(crate) fn canonicalize_decimal(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, body) = if let Some(rest) = trimmed.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        ("", rest)
    } else {
        ("", trimmed)
    };
    // Validate digits + at most one `.`
    let mut seen_dot = false;
    for c in body.chars() {
        if c == '.' {
            if seen_dot {
                return None;
            }
            seen_dot = true;
        } else if !c.is_ascii_digit() {
            return None;
        }
    }
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    // Strip leading zeros from the integer part (keep at least one).
    let int_trimmed = int_part.trim_start_matches('0');
    let int_canon = if int_trimmed.is_empty() {
        "0"
    } else {
        int_trimmed
    };
    // Strip trailing zeros from the fractional part.
    let frac_trimmed = frac_part.trim_end_matches('0');
    let mut out = String::with_capacity(input.len());
    // Avoid printing `-0` for `-0.0`.
    let is_zero = int_canon == "0" && frac_trimmed.is_empty();
    if !is_zero {
        out.push_str(sign);
    }
    out.push_str(int_canon);
    if !frac_trimmed.is_empty() {
        out.push('.');
        out.push_str(frac_trimmed);
    }
    Some(out)
}

fn parse_numeric_text(text: &str) -> SqlValue {
    let trimmed = text.trim();
    if let Ok(v) = trimmed.parse::<i64>() {
        return SqlValue::Integer(v);
    }
    if let Ok(v) = trimmed.parse::<f64>() {
        return SqlValue::Real(v);
    }
    // SQLite's CAST(x AS NUMERIC) tries a real-prefix parse before
    // falling back to integer-prefix: `CAST('3.14abc' AS NUMERIC)` is
    // `real|3.14`, not `integer|3`. Only treat the leading slice as a
    // real if it actually contains a fractional part or an exponent;
    // bare digit prefixes still go through the integer path so that
    // `CAST('42abc' AS NUMERIC)` stays an integer.
    let real_len = real_prefix_length(trimmed);
    if real_len > 0 {
        let prefix = &trimmed[..real_len];
        if (prefix.contains('.') || prefix.contains('e') || prefix.contains('E'))
            && let Ok(v) = prefix.parse::<f64>()
        {
            return SqlValue::Real(v);
        }
    }
    SqlValue::Integer(parse_integer_prefix(trimmed))
}

/// Length (in bytes) of the longest leading slice of `s` that parses as
/// a real-number literal in SQLite's grammar: optional sign, then digits
/// with an optional decimal point and an optional `e[+-]?digits`
/// exponent. Returns 0 when there is no numeric prefix at all.
fn real_prefix_length(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    if let Some(&first) = bytes.first()
        && (first == b'+' || first == b'-')
    {
        idx = 1;
    }
    let mut saw_digit = false;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
        saw_digit = true;
    }
    if idx < bytes.len() && bytes[idx] == b'.' {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
            saw_digit = true;
        }
    }
    if saw_digit && idx < bytes.len() && (bytes[idx] == b'e' || bytes[idx] == b'E') {
        let after_e = idx + 1;
        let mut exp_idx = after_e;
        if exp_idx < bytes.len() && (bytes[exp_idx] == b'+' || bytes[exp_idx] == b'-') {
            exp_idx += 1;
        }
        let exp_digits_start = exp_idx;
        while exp_idx < bytes.len() && bytes[exp_idx].is_ascii_digit() {
            exp_idx += 1;
        }
        if exp_idx > exp_digits_start {
            idx = exp_idx;
        }
    }
    if saw_digit { idx } else { 0 }
}

fn parse_integer_prefix(s: &str) -> i64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    let mut neg = false;
    if let Some(&first) = bytes.first() {
        if first == b'+' {
            idx = 1;
        } else if first == b'-' {
            idx = 1;
            neg = true;
        }
    }
    let start = idx;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == start {
        return 0;
    }
    let digits = &s[start..idx];
    let mag: i64 = digits.parse().unwrap_or(i64::MAX);
    if neg { mag.wrapping_neg() } else { mag }
}

fn parse_real_prefix(s: &str) -> f64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    if let Some(&first) = bytes.first()
        && (first == b'+' || first == b'-')
    {
        idx = 1;
    }
    let mut saw_digit = false;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
        saw_digit = true;
    }
    if idx < bytes.len() && bytes[idx] == b'.' {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
            saw_digit = true;
        }
    }
    if saw_digit && idx < bytes.len() && (bytes[idx] == b'e' || bytes[idx] == b'E') {
        let mut after_e = idx + 1;
        if after_e < bytes.len() && (bytes[after_e] == b'+' || bytes[after_e] == b'-') {
            after_e += 1;
        }
        let exp_start = after_e;
        while after_e < bytes.len() && bytes[after_e].is_ascii_digit() {
            after_e += 1;
        }
        if after_e > exp_start {
            idx = after_e;
        }
    }
    if !saw_digit {
        return 0.0;
    }
    s[..idx].parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod track_h_cast_tests {
    use super::*;

    #[test]
    fn canonicalize_decimal_strips_trailing_zeros() {
        assert_eq!(canonicalize_decimal("1.50").as_deref(), Some("1.5"));
        assert_eq!(canonicalize_decimal("-0").as_deref(), Some("0"));
        assert_eq!(canonicalize_decimal("+3.14").as_deref(), Some("3.14"));
        assert_eq!(canonicalize_decimal("00012.300").as_deref(), Some("12.3"));
    }

    #[test]
    fn parse_numeric_scale_recognises_p_s() {
        assert_eq!(parse_numeric_scale("numeric(10,2)"), Some(2));
        assert_eq!(parse_numeric_scale("numeric(5,0)"), Some(0));
        assert_eq!(parse_numeric_scale("numeric"), None);
    }

    #[test]
    fn rescale_decimal_pads_to_target_scale() {
        assert_eq!(rescale_decimal_text("4.5", 2), "4.50");
        assert_eq!(rescale_decimal_text("4.500", 2), "4.50");
        assert_eq!(rescale_decimal_text("-4.5", 2), "-4.50");
        assert_eq!(rescale_decimal_text("12", 3), "12.000");
    }

    #[test]
    fn parse_pg_boolean_accepts_full_family() {
        assert_eq!(parse_pg_boolean("yes"), Some(true));
        assert_eq!(parse_pg_boolean("YES"), Some(true));
        assert_eq!(parse_pg_boolean("y"), Some(true));
        assert_eq!(parse_pg_boolean("on"), Some(true));
        assert_eq!(parse_pg_boolean("1"), Some(true));
        assert_eq!(parse_pg_boolean("no"), Some(false));
        assert_eq!(parse_pg_boolean("OFF"), Some(false));
        assert_eq!(parse_pg_boolean("0"), Some(false));
        assert_eq!(parse_pg_boolean("maybe"), None);
    }

    #[test]
    fn canonicalize_uuid_accepts_hex_and_braces() {
        assert_eq!(
            canonicalize_uuid("00000000-0000-0000-0000-000000000001").as_deref(),
            Some("00000000-0000-0000-0000-000000000001")
        );
        assert_eq!(
            canonicalize_uuid("00000000000000000000000000000001").as_deref(),
            Some("00000000-0000-0000-0000-000000000001")
        );
        assert_eq!(
            canonicalize_uuid("{ABCDEF01-2345-6789-ABCD-EF0123456789}").as_deref(),
            Some("abcdef01-2345-6789-abcd-ef0123456789")
        );
        assert_eq!(canonicalize_uuid("not-a-uuid"), None);
    }

    #[test]
    fn strip_trailing_tz_only_acts_on_time_strings() {
        // Date-only literals are left untouched (no `:`).
        assert_eq!(strip_trailing_tz("2025-01-01"), "2025-01-01");
        // Timestamp with a numeric offset → stripped.
        assert_eq!(
            strip_trailing_tz("2025-01-15 12:00:00+00"),
            "2025-01-15 12:00:00"
        );
        // Long offset.
        assert_eq!(
            strip_trailing_tz("2025-01-15 12:00:00-05:30"),
            "2025-01-15 12:00:00"
        );
        // Z suffix.
        assert_eq!(
            strip_trailing_tz("2025-01-15T12:00:00Z"),
            "2025-01-15T12:00:00"
        );
    }
}
