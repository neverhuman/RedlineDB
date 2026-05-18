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
            SqlValue::Real(v) => SqlValue::Text(Arc::from(v.to_string())),
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
        return Ok(cast_to_numeric(&value));
    }

    Ok(value)
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

fn cast_to_numeric(value: &SqlValue) -> SqlValue {
    match value {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Integer(_) | SqlValue::Real(_) => value.clone(),
        SqlValue::Text(t) => parse_numeric_text(t.as_ref()),
        SqlValue::Blob(b) => parse_numeric_text(&String::from_utf8_lossy(b)),
    }
}

fn parse_numeric_text(text: &str) -> SqlValue {
    let trimmed = text.trim();
    if let Ok(v) = trimmed.parse::<i64>() {
        SqlValue::Integer(v)
    } else if let Ok(v) = trimmed.parse::<f64>() {
        SqlValue::Real(v)
    } else {
        SqlValue::Integer(parse_integer_prefix(trimmed))
    }
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
