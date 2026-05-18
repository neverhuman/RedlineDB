use crate::error::{Error, Result};

use super::DateTime;

/// Apply a sequence of SQLite modifiers (`'+1 day'`, `'start of month'`, etc.)
/// to a DateTime. Unrecognized modifiers are reported as errors.
pub fn apply_modifiers(mut dt: DateTime, mods: &[&str]) -> Result<DateTime> {
    for raw in mods {
        let m = raw.trim().to_ascii_lowercase();
        if m == "utc" {
            dt.is_local = false;
            continue;
        }
        if m == "localtime" {
            dt.is_local = true;
            continue;
        }
        if m == "start of month" {
            dt.day = 1;
            dt.hour = 0;
            dt.minute = 0;
            dt.second = 0;
            dt.micro = 0;
            continue;
        }
        if m == "start of year" {
            dt.month = 1;
            dt.day = 1;
            dt.hour = 0;
            dt.minute = 0;
            dt.second = 0;
            dt.micro = 0;
            continue;
        }
        if m == "start of day" {
            dt.hour = 0;
            dt.minute = 0;
            dt.second = 0;
            dt.micro = 0;
            continue;
        }
        if let Some(arith) = apply_arithmetic_modifier(&dt, &m)? {
            dt = arith;
            continue;
        }
        return Err(Error::UnsupportedSql(format!(
            "unsupported datetime modifier: {raw}"
        )));
    }
    Ok(dt)
}

fn apply_arithmetic_modifier(dt: &DateTime, m: &str) -> Result<Option<DateTime>> {
    let mut iter = m.splitn(2, ' ');
    let qty = match iter.next() {
        Some(q) => q,
        None => return Ok(None),
    };
    let unit = match iter.next() {
        Some(u) => u.trim(),
        None => return Ok(None),
    };
    let value: f64 = match qty.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut next = *dt;
    match unit {
        "day" | "days" => {
            let secs = next.to_unix() + (value * 86_400.0).round() as i64;
            next = DateTime::from_unix(secs, next.micro);
        }
        "hour" | "hours" => {
            let secs = next.to_unix() + (value * 3600.0).round() as i64;
            next = DateTime::from_unix(secs, next.micro);
        }
        "minute" | "minutes" => {
            let secs = next.to_unix() + (value * 60.0).round() as i64;
            next = DateTime::from_unix(secs, next.micro);
        }
        "second" | "seconds" => {
            let secs = next.to_unix() + value.round() as i64;
            next = DateTime::from_unix(secs, next.micro);
        }
        "month" | "months" => {
            let total_months =
                (next.year as i64 * 12 + next.month as i64 - 1) + value.round() as i64;
            let new_year = total_months.div_euclid(12) as i32;
            let new_month = (total_months.rem_euclid(12) + 1) as u32;
            next.year = new_year;
            next.month = new_month;
            next.day = next.day.min(days_in_month(new_year, new_month));
        }
        "year" | "years" => {
            next.year += value.round() as i32;
            next.day = next.day.min(days_in_month(next.year, next.month));
        }
        _ => return Ok(None),
    }
    next.is_local = dt.is_local;
    Ok(Some(next))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 30,
    }
}
