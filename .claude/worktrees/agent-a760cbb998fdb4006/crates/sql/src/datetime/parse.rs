use crate::error::{Error, Result};

use super::DateTime;

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
        return Ok(julian_to_dt(jd));
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
    let total_seconds = (jd - 2_440_587.5) * 86_400.0;
    let secs = total_seconds.floor() as i64;
    let micro = ((total_seconds.fract().abs()) * 1_000_000.0) as u32;
    DateTime::from_unix(secs, micro)
}
