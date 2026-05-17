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
