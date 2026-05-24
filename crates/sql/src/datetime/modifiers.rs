use crate::error::{Error, Result};

use super::DateTime;

/// Apply a sequence of SQLite modifiers (`'+1 day'`, `'start of month'`, etc.)
/// to a DateTime. Unrecognized modifiers are reported as errors.
///
/// When `dt` is flagged out-of-range (set by `parse_timestring` for numeric
/// inputs outside the valid julian-day window), the rules mirror SQLite:
///
/// * `'utc'` clears the flag and the value becomes julian-day 0
///   (`-4713-11-24 12:00:00`).
/// * `'localtime'` clears the flag and the value resets to Y2K
///   (`2000-01-01 12:00:00`), matching SQLite's behaviour for failed
///   timezone lookups on bogus baselines.
/// * `'unixepoch'` reinterprets the original numeric input as unix
///   seconds.
/// * `'auto'` reinterprets large finite values as unix seconds, the
///   documented SQLite heuristic.
/// * `'julianday'` / `'subsec'` / `'subsecond'` keep the flag (the value
///   is still out of range).
/// * Any other modifier (arithmetic, `start of *`, `weekday N`) preserves
///   the flag, so the eventual `format_*`/`julian_day` call returns NULL.
pub fn apply_modifiers(mut dt: DateTime, mods: &[&str]) -> Result<DateTime> {
    for raw in mods {
        let m = raw.trim().to_ascii_lowercase();
        if m == "utc" {
            dt.is_local = false;
            if dt.out_of_range.is_some() {
                dt = super::DateTime::from_unix(-210_866_760_000, 0);
            }
            continue;
        }
        if m == "localtime" {
            dt.is_local = true;
            if dt.out_of_range.is_some() {
                dt = super::DateTime::from_unix(946_684_800 + 43_200, 0);
            }
            continue;
        }
        if m == "unixepoch" {
            if let Some(raw_val) = dt.out_of_range
                && raw_val.is_finite()
            {
                let secs = raw_val.trunc() as i64;
                let micro = ((raw_val.fract().abs()) * 1_000_000.0) as u32;
                dt = super::DateTime::from_unix(secs, micro);
            }
            continue;
        }
        if m == "auto" {
            if let Some(raw_val) = dt.out_of_range
                && raw_val.is_finite()
            {
                dt = super::DateTime::from_unix(raw_val as i64, 0);
            }
            continue;
        }
        if m == "julianday" || m == "subsec" || m == "subsecond" {
            continue;
        }
        if dt.out_of_range.is_some() {
            // Arithmetic and start-of/weekday modifiers cannot apply to an
            // out-of-range value. SQLite reports the result as NULL; we
            // surface that by preserving the flag through to the caller.
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
        if let Some(weekday) = m.strip_prefix("weekday ") {
            let target: u32 = weekday
                .trim()
                .parse()
                .map_err(|_| Error::UnsupportedSql(format!("invalid weekday modifier: {raw}")))?;
            if target > 6 {
                return Err(Error::UnsupportedSql(format!(
                    "weekday must be in 0..=6: {raw}"
                )));
            }
            dt = advance_to_weekday(dt, target);
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
            let secs = next.to_unix();
            let normalised = DateTime::from_unix(secs, next.micro);
            next.year = normalised.year;
            next.month = normalised.month;
            next.day = normalised.day;
        }
        "week" | "weeks" => {
            let secs = next.to_unix() + (value * 7.0 * 86_400.0).round() as i64;
            next = DateTime::from_unix(secs, next.micro);
        }
        "year" | "years" => {
            next.year += value.round() as i32;
            let secs = next.to_unix();
            let normalised = DateTime::from_unix(secs, next.micro);
            next.year = normalised.year;
            next.month = normalised.month;
            next.day = normalised.day;
        }
        _ => return Ok(None),
    }
    next.is_local = dt.is_local;
    Ok(Some(next))
}

fn advance_to_weekday(dt: DateTime, target: u32) -> DateTime {
    // SQLite `weekday N`: advance forward (or stay) to the next occurrence.
    let cur = day_of_week(&dt);
    let delta = (target as i64 - cur as i64).rem_euclid(7);
    if delta == 0 {
        return dt;
    }
    let secs = dt.to_unix() + delta * 86_400;
    let mut next = DateTime::from_unix(secs, dt.micro);
    next.is_local = dt.is_local;
    next
}

fn day_of_week(dt: &DateTime) -> u32 {
    let m = if dt.month < 3 {
        dt.month + 12
    } else {
        dt.month
    };
    let y = if dt.month < 3 { dt.year - 1 } else { dt.year };
    let k = y % 100;
    let j = y / 100;
    let h = (dt.day as i32 + (13 * (m as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    ((h + 6) % 7) as u32
}
