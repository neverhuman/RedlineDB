use crate::error::{Error, Result};

use super::{DateTime, MAX_JULIAN_DAY, MIN_JULIAN_DAY};

/// Parse a SQLite time-string. Accepts ISO-8601-ish forms and `'now'`.
pub fn parse_timestring(input: &str) -> Result<DateTime> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("now") {
        return Ok(DateTime::now_utc());
    }
    if !trimmed.contains('-') && trimmed.contains(':') {
        return parse_time_of_day(trimmed);
    }
    if let Ok(jd) = trimmed.parse::<f64>() {
        return Ok(julian_to_dt_checked(jd));
    }
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
        out_of_range: None,
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
    // Track H — strip a trailing UTC-offset suffix (`+HH[:MM]`, `-HH[:MM]`,
    // `Z`) so PG-style `'12:00:00+00'` strings parse. The numeric offset is
    // ignored because RedlineDB stores all timestamps as tz-naive UTC.
    let stripped = strip_tz_suffix(input);
    let (clock, frac) = stripped.split_once('.').unwrap_or((stripped, ""));
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
        out_of_range: None,
    })
}

/// Convert a numeric input to a DateTime, treating it as a julian-day
/// number. If `jd` is outside the SQLite-representable range, return a
/// julian-day-0 DateTime stamped with the original numeric in
/// `out_of_range`. Downstream modifier/format logic inspects that flag
/// to either rescue the value (e.g. `'unixepoch'`, `'utc'`) or to
/// surface NULL.
fn julian_to_dt_checked(jd: f64) -> DateTime {
    if jd.is_nan() || !(MIN_JULIAN_DAY..=MAX_JULIAN_DAY).contains(&jd) {
        let mut dt = julian_to_dt_raw(0.0);
        dt.out_of_range = Some(jd);
        return dt;
    }
    julian_to_dt_raw(jd)
}

fn julian_to_dt_raw(jd: f64) -> DateTime {
    let total_seconds = (jd - 2_440_587.5) * 86_400.0;
    let secs = total_seconds.floor() as i64;
    let micro = ((total_seconds.fract().abs()) * 1_000_000.0) as u32;
    DateTime::from_unix(secs, micro)
}

/// Drop a trailing `+HH[:MM]`, `-HH[:MM]`, or `Z` UTC-offset marker from a
/// time-of-day string. Returns the remaining clock portion. The function is
/// permissive: it only strips a recognisable suffix, never the middle of
/// the input.
///
/// Note: this is only called from `parse_time_of_day`, so the input is
/// already known to have at least one `:` (i.e., it's a time, not a date).
/// The walker therefore doesn't need a date-vs-time guard here.
///
/// Examples:
///   `"12:00:00+00"`       → `"12:00:00"`
///   `"12:00:00-05:30"`    → `"12:00:00"`
///   `"12:00:00.123Z"`     → `"12:00:00.123"`
///   `"12:00:00"`          → `"12:00:00"` (unchanged)
fn strip_tz_suffix(input: &str) -> &str {
    if let Some(stripped) = input.strip_suffix('Z') {
        return stripped;
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
                // Require at least 2 digits in the offset (PG always emits
                // 2-digit hours). If the digit count is 0 the `+`/`-` is
                // part of the time itself, not an offset.
                if seen_digit_run >= 2 {
                    return &input[..i - 1];
                }
                break;
            }
            _ => break,
        }
    }
    input
}
