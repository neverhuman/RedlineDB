use super::DateTime;

/// SQLite-style `strftime(format, time)`.
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
                b'Y' => out.push_str(&super::format_year_4(dt.year)),
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
                b'T' => out.push_str(&format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second)),
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

fn day_of_year(dt: &DateTime) -> u32 {
    let mut total: u32 = 0;
    for m in 1..dt.month {
        total += days_in_month(dt.year, m);
    }
    total + dt.day
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
        out_of_range: None,
    };
    let jan1_dow = day_of_week(&jan1);
    let doy = day_of_year(dt);
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
        out_of_range: None,
    };
    let jan1_dow = day_of_week(&jan1);
    let jan1_mon_dow = (jan1_dow + 6) % 7;
    let doy = day_of_year(dt);
    let first_monday_offset = (7 - jan1_mon_dow) % 7;
    if doy <= first_monday_offset {
        0
    } else {
        ((doy - first_monday_offset - 1) / 7) + 1
    }
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
