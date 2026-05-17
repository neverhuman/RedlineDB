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

mod format;
mod modifiers;
mod parse;

pub use format::strftime;
pub use modifiers::apply_modifiers;
pub use parse::parse_timestring;

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
