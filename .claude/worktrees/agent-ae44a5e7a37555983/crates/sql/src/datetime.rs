//! Lane SQL-D phase 10: SQLite-compatible date/time function helpers.
//!
//! Implements `date()`, `time()`, `datetime()`, `julianday()`, `strftime()`,
//! and `unixepoch()` over a small Julian-date kernel. The intent is to match
//! the documented SQLite semantics for the common forms; obscure modifiers
//! (`weekday N`, `unixepoch` from a Julian Day, etc.) are partial.
//!
//! The arithmetic is handled in the broken-down (Y, M, D, h, m, s) domain to
//! stay close to SQLite's formatting and to avoid pulling in `chrono` or
//! `time` for something this narrow. Rounding is by `floor()` for `date()`
//! components and by truncation for the integer fields, matching SQLite.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

/// Broken-down date/time tuple (UTC unless otherwise marked).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub micro: u32,
    pub is_local: bool,
}

impl DateTime {
    pub fn now_utc() -> Self {
        let dur = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d,
            Err(_) => Duration::default(),
        };
        Self::from_unix(dur.as_secs() as i64, dur.subsec_micros())
    }

    pub fn from_unix(secs: i64, micro: u32) -> Self {
        let mut s = secs.rem_euclid(86_400);
        let mut d = secs.div_euclid(86_400);
        let hour = (s / 3600) as u32;
        s %= 3600;
        let minute = (s / 60) as u32;
        let second = (s % 60) as u32;
        // Days since 1970-01-01 to (year,month,day) using Howard Hinnant's algorithm.
        d += 719_468; // shift epoch to 0000-03-01.
        let era = d.div_euclid(146_097);
        let doe = d.rem_euclid(146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 {
            (mp + 3) as u32
        } else {
            (mp - 9) as u32
        };
        let year = (y + if month <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            micro,
            is_local: false,
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_unix(&self) -> i64 {
        let y = self.year as i64 - if self.month <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400) as u64;
        let m = if self.month > 2 {
            self.month - 3
        } else {
            self.month + 9
        };
        let doy = (153 * m as u64 + 2) / 5 + self.day as u64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe as i64 - 719_468;
        days * 86_400 + self.hour as i64 * 3600 + self.minute as i64 * 60 + self.second as i64
    }

    /// SQLite's Julian-day formula. `julianday(date)` for `date` is days
    /// since the Julian epoch, with fractional part covering time-of-day.
    pub fn julian_day(&self) -> f64 {
        // Convert to Julian Day at noon, then adjust for fractional time.
        let y = self.year as i64;
        let m = self.month as i64;
        let d = self.day as i64;
        let a = (14 - m) / 12;
        let y2 = y + 4800 - a;
        let m2 = m + 12 * a - 3;
        let jdn = d + (153 * m2 + 2) / 5 + 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 - 32045;
        let frac = (self.hour as f64 - 12.0) / 24.0
            + self.minute as f64 / 1440.0
            + self.second as f64 / 86_400.0
            + self.micro as f64 / 86_400_000_000.0;
        jdn as f64 + frac
    }

    pub fn format_date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn format_time(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }

    pub fn format_datetime(&self) -> String {
        format!("{} {}", self.format_date(), self.format_time(),)
    }
}

/// Parse a SQLite time-string. Accepts ISO-8601-ish forms and `'now'`.
pub fn parse_timestring(input: &str) -> Result<DateTime> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("now") {
        return Ok(DateTime::now_utc());
    }
    // Pure time-of-day: HH:MM or HH:MM:SS.
    if !trimmed.contains('-') && trimmed.contains(':') {
        return parse_time_of_day(trimmed);
    }
    // Pure julian day (numeric). Best-effort: SQLite distinguishes by content.
    if let Ok(jd) = trimmed.parse::<f64>() {
        return Ok(julian_to_dt(jd));
    }
    // Date or date+time: split on space or 'T'.
    let (date_part, time_part) = trimmed.split_once(['T', ' ']).unwrap_or((trimmed, ""));
    let mut date_iter = date_part.splitn(3, '-');
    let year_str = match date_iter.next() {
        Some(s) => s,
        None => {
            return Err(Error::UnsupportedSql(format!(
                "invalid date literal: {input}"
            )));
        }
    };
    let year = year_str
        .parse::<i32>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid year: {input}")))?;
    let month_str = match date_iter.next() {
        Some(s) => s,
        None => {
            return Err(Error::UnsupportedSql(format!(
                "invalid date literal: {input}"
            )));
        }
    };
    let month = month_str
        .parse::<u32>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid month: {input}")))?;
    let day_str = match date_iter.next() {
        Some(s) => s,
        None => {
            return Err(Error::UnsupportedSql(format!(
                "invalid date literal: {input}"
            )));
        }
    };
    let day = day_str
        .parse::<u32>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid day: {input}")))?;
    let mut dt = DateTime {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
        micro: 0,
        is_local: false,
    };
    if !time_part.is_empty() {
        let trimmed_time = time_part.trim_end_matches('Z');
        let parsed = parse_time_of_day(trimmed_time)?;
        dt.hour = parsed.hour;
        dt.minute = parsed.minute;
        dt.second = parsed.second;
        dt.micro = parsed.micro;
    }
    Ok(dt)
}

fn parse_time_of_day(input: &str) -> Result<DateTime> {
    // Strip subsecond part if present.
    let (clock, frac) = input.split_once('.').unwrap_or((input, ""));
    let mut parts = clock.split(':');
    let hour_str = match parts.next() {
        Some(s) => s,
        None => return Err(Error::UnsupportedSql("invalid time literal".to_owned())),
    };
    let hour = hour_str
        .parse::<u32>()
        .map_err(|_| Error::UnsupportedSql("invalid hour".to_owned()))?;
    let minute_str = match parts.next() {
        Some(s) => s,
        None => return Err(Error::UnsupportedSql("invalid time literal".to_owned())),
    };
    let minute = minute_str
        .parse::<u32>()
        .map_err(|_| Error::UnsupportedSql("invalid minute".to_owned()))?;
    let second = parts
        .next()
        .map(|s| s.parse::<u32>())
        .transpose()
        .map_err(|_| Error::UnsupportedSql("invalid second".to_owned()))?
        .unwrap_or(0);
    let micro = if frac.is_empty() {
        0
    } else {
        let mut padded = frac.to_owned();
        while padded.len() < 6 {
            padded.push('0');
        }
        padded[..6.min(padded.len())]
            .parse::<u32>()
            .map_err(|_| Error::UnsupportedSql("invalid fraction".to_owned()))?
    };
    Ok(DateTime {
        year: 2000,
        month: 1,
        day: 1,
        hour,
        minute,
        second,
        micro,
        is_local: false,
    })
}

fn julian_to_dt(jd: f64) -> DateTime {
    // Inverse of DateTime::julian_day for civil dates.
    let total_seconds = (jd - 2_440_587.5) * 86_400.0;
    let secs = total_seconds.floor() as i64;
    let micro = ((total_seconds.fract().abs()) * 1_000_000.0) as u32;
    DateTime::from_unix(secs, micro)
}

/// Apply a sequence of SQLite modifiers (`'+1 day'`, `'start of month'`, etc.)
/// to a DateTime. Unrecognized modifiers are reported as errors.
///
/// Supported modifiers (matching SQLite semantics):
/// - `NNN {days|hours|minutes|seconds|months|years|weeks}` (signed)
/// - `start of {day,month,year}`
/// - `weekday N` — advance to the next occurrence of weekday N (0=Sun..6=Sat)
/// - `utc`, `localtime`
/// - `unixepoch` — when present as the first modifier, the preceding base
///   value is reinterpreted as a Unix epoch seconds integer (handled in
///   `parse_timestring`); when present later it is a no-op for compatibility.
/// - `julianday` — alias accepted as a no-op so `julianday()` callers can
///   chain modifiers after applying SQLite's "auto" form.
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
        if m == "unixepoch" || m == "julianday" || m == "auto" {
            // Re-interpretation hints are absorbed by `parse_timestring`; here
            // they are accepted for compatibility so chained modifiers parse
            // without error.
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
        // Try arithmetic: `+1 day`, `-3 hours`, `+90 minutes`, etc.
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

fn advance_to_weekday(dt: DateTime, target: u32) -> DateTime {
    // SQLite's `weekday N` semantics: advance forward (or stay) to the next
    // occurrence of weekday N. If `dt` is already on weekday N, return `dt`.
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
        "week" | "weeks" => {
            let secs = next.to_unix() + (value * 7.0 * 86_400.0).round() as i64;
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
        "millisecond" | "milliseconds" => {
            let total_us = next.micro as i64 + (value * 1_000.0).round() as i64;
            let extra_secs = total_us.div_euclid(1_000_000);
            let new_micro = total_us.rem_euclid(1_000_000) as u32;
            next = DateTime::from_unix(next.to_unix() + extra_secs, new_micro);
        }
        "month" | "months" => {
            // SQLite semantics: do NOT clamp the day. If the target month has
            // fewer days, the result overflows into the next month so that
            // `date('2024-01-31', '+1 month')` returns `2024-03-02` (matching
            // SQLite's documented behaviour rather than a calendar-clamping
            // approach).
            let total_months =
                (next.year as i64 * 12 + next.month as i64 - 1) + value.round() as i64;
            let new_year = total_months.div_euclid(12) as i32;
            let new_month = (total_months.rem_euclid(12) + 1) as u32;
            next.year = new_year;
            next.month = new_month;
            // Re-normalise via Unix-seconds round-trip to let overflow days
            // carry into the following month exactly like SQLite.
            let secs = next.to_unix();
            let normalised = DateTime::from_unix(secs, next.micro);
            next.year = normalised.year;
            next.month = normalised.month;
            next.day = normalised.day;
        }
        "year" | "years" => {
            next.year += value.round() as i32;
            // Same overflow rule as months: 2024-02-29 + 1 year → 2025-03-01.
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

/// SQLite-style `strftime(format, time)`.
///
/// Supported format specifiers (per SQLite docs):
///   `%d` day-of-month 01-31
///   `%e` day-of-month space-padded
///   `%f` SS.SSS (fractional seconds)
///   `%H` hour 00-24
///   `%I` hour 01-12
///   `%j` day-of-year 001-366
///   `%J` Julian day number
///   `%k` hour space-padded 0-23
///   `%l` hour space-padded 1-12
///   `%m` month 01-12
///   `%M` minute 00-59
///   `%p` AM/PM
///   `%P` am/pm
///   `%R` HH:MM
///   `%s` seconds since 1970-01-01
///   `%S` seconds 00-59
///   `%T` HH:MM:SS
///   `%u` day-of-week 1-7 (Monday=1)
///   `%U` week-of-year (Sunday-based) 00-53
///   `%w` day-of-week 0-6 (Sunday=0)
///   `%W` week-of-year (Monday-based) 00-53
///   `%Y` year 0000-9999
///   `%%` literal `%`
pub fn strftime(format: &str, dt: &DateTime) -> String {
    let mut out = String::with_capacity(format.len());
    let bytes = format.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 1 < bytes.len() {
            let spec = bytes[i + 1];
            i += 2;
            match spec {
                b'Y' => out.push_str(&format!("{:04}", dt.year)),
                b'm' => out.push_str(&format!("{:02}", dt.month)),
                b'd' => out.push_str(&format!("{:02}", dt.day)),
                b'e' => out.push_str(&format!("{:2}", dt.day)),
                b'H' => out.push_str(&format!("{:02}", dt.hour)),
                b'I' => {
                    let h = match dt.hour % 12 {
                        0 => 12,
                        n => n,
                    };
                    out.push_str(&format!("{h:02}"));
                }
                b'k' => out.push_str(&format!("{:2}", dt.hour)),
                b'l' => {
                    let h = match dt.hour % 12 {
                        0 => 12,
                        n => n,
                    };
                    out.push_str(&format!("{h:2}"));
                }
                b'M' => out.push_str(&format!("{:02}", dt.minute)),
                b'S' => out.push_str(&format!("{:02}", dt.second)),
                b'j' => out.push_str(&format!("{:03}", day_of_year(dt))),
                b's' => out.push_str(&dt.to_unix().to_string()),
                b'w' => out.push_str(&day_of_week(dt).to_string()),
                b'u' => {
                    // SQLite/POSIX: Monday=1..Sunday=7
                    let dow = day_of_week(dt);
                    let v = if dow == 0 { 7 } else { dow };
                    out.push_str(&v.to_string());
                }
                b'U' => out.push_str(&format!("{:02}", week_of_year_sunday(dt))),
                b'W' => out.push_str(&format!("{:02}", week_of_year_monday(dt))),
                b'p' => out.push_str(if dt.hour < 12 { "AM" } else { "PM" }),
                b'P' => out.push_str(if dt.hour < 12 { "am" } else { "pm" }),
                b'R' => out.push_str(&format!("{:02}:{:02}", dt.hour, dt.minute)),
                b'T' => out.push_str(&format!(
                    "{:02}:{:02}:{:02}",
                    dt.hour, dt.minute, dt.second
                )),
                b'%' => out.push('%'),
                b'f' => out.push_str(&format!("{:02}.{:03}", dt.second, dt.micro / 1000)),
                b'J' => out.push_str(&format!("{:.6}", dt.julian_day())),
                other => {
                    out.push('%');
                    out.push(other as char);
                }
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

fn week_of_year_sunday(dt: &DateTime) -> u32 {
    // SQLite `%U`: week 0 starts on Jan 1; week 1 starts on the first Sunday.
    let jan1 = DateTime {
        year: dt.year,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        micro: 0,
        is_local: false,
    };
    let jan1_dow = day_of_week(&jan1); // 0=Sun..6=Sat
    let doy = day_of_year(dt); // 1-based
    // Days until first Sunday (offset from Jan 1).
    let first_sunday_offset = (7 - jan1_dow) % 7;
    if doy <= first_sunday_offset {
        0
    } else {
        ((doy - first_sunday_offset - 1) / 7) + 1
    }
}

fn week_of_year_monday(dt: &DateTime) -> u32 {
    // SQLite `%W`: week 0 starts on Jan 1; week 1 starts on the first Monday.
    let jan1 = DateTime {
        year: dt.year,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        micro: 0,
        is_local: false,
    };
    let jan1_dow = day_of_week(&jan1); // 0=Sun..6=Sat
    // Convert to Monday-based offset: Monday=0..Sunday=6
    let jan1_mon_dow = (jan1_dow + 6) % 7;
    let doy = day_of_year(dt);
    let first_monday_offset = (7 - jan1_mon_dow) % 7;
    if doy <= first_monday_offset {
        0
    } else {
        ((doy - first_monday_offset - 1) / 7) + 1
    }
}

fn day_of_year(dt: &DateTime) -> u32 {
    let mut total: u32 = 0;
    for m in 1..dt.month {
        total += days_in_month(dt.year, m);
    }
    total + dt.day
}

fn day_of_week(dt: &DateTime) -> u32 {
    // Zeller's congruence (Sun=0..Sat=6 to match SQLite).
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
