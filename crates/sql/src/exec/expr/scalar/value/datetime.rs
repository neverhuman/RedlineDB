use super::*;

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
        let mut left_align = false;
        let mut zero_pad = false;
        let mut width: Option<usize> = None;
        // flags
        while let Some(&f) = chars.peek() {
            if matches!(f, '-' | '+' | ' ' | '0' | '#') {
                spec.push(f);
                match f {
                    '-' => left_align = true,
                    '0' => zero_pad = true,
                    _ => {}
                }
                chars.next();
            } else {
                break;
            }
        }
        // width
        let mut width_buf = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() {
                spec.push(d);
                width_buf.push(d);
                chars.next();
            } else {
                break;
            }
        }
        if !width_buf.is_empty() {
            width = width_buf.parse::<usize>().ok();
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
                let rendered = value_to_string(arg);
                out.push_str(&apply_width(rendered, width, left_align, false));
            }
            'd' | 'i' => {
                arg_idx += 1;
                let n = match arg {
                    SqlValue::Integer(v) => *v,
                    SqlValue::Real(v) => *v as i64,
                    SqlValue::Null => 0,
                    other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
                };
                let rendered = n.to_string();
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'u' => {
                arg_idx += 1;
                let n = match arg {
                    SqlValue::Integer(v) => *v as u64,
                    SqlValue::Real(v) => *v as u64,
                    SqlValue::Null => 0u64,
                    other => value_to_string(other).trim().parse::<u64>().unwrap_or(0),
                };
                let rendered = n.to_string();
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'f' | 'F' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                let rendered = format!("{v:.6}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'e' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                let rendered = format!("{v:e}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'E' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                let rendered = format!("{v:E}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'g' | 'G' => {
                arg_idx += 1;
                let v = numeric_f64(arg);
                // SQLite %g removes trailing zeros.
                let rendered = format!("{v}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'x' => {
                arg_idx += 1;
                let n = int_arg(arg);
                let rendered = format!("{n:x}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'X' => {
                arg_idx += 1;
                let n = int_arg(arg);
                let rendered = format!("{n:X}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'o' => {
                arg_idx += 1;
                let n = int_arg(arg) as u64;
                let rendered = format!("{n:o}");
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'c' => {
                arg_idx += 1;
                let cp = int_arg(arg) as u32;
                let rendered = char::from_u32(cp)
                    .unwrap_or(char::REPLACEMENT_CHARACTER)
                    .to_string();
                out.push_str(&apply_width(rendered, width, left_align, zero_pad));
            }
            'q' => {
                // SQLite %q: escape single-quotes by doubling them.
                arg_idx += 1;
                let s = value_to_string(arg);
                let rendered = s.replace('\'', "''");
                out.push_str(&apply_width(rendered, width, left_align, false));
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

fn apply_width(value: String, width: Option<usize>, left_align: bool, zero_pad: bool) -> String {
    let Some(width) = width else {
        return value;
    };
    let len = value.chars().count();
    if len >= width {
        return value;
    }
    let pad = width - len;
    if left_align {
        return format!("{value}{}", " ".repeat(pad));
    }
    if zero_pad {
        let mut chars = value.chars();
        if let Some(sign) = chars.next() {
            if matches!(sign, '+' | '-') {
                let rest: String = chars.collect();
                return format!("{sign}{}{}", "0".repeat(pad), rest);
            }
        }
        return format!("{}{}", "0".repeat(pad), value);
    }
    format!("{}{}", " ".repeat(pad), value)
}
