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
                b'Y' => out.push_str(&format!("{:04}", dt.year)),
                b'm' => out.push_str(&format!("{:02}", dt.month)),
                b'd' => out.push_str(&format!("{:02}", dt.day)),
                b'H' => out.push_str(&format!("{:02}", dt.hour)),
                b'M' => out.push_str(&format!("{:02}", dt.minute)),
                b'S' => out.push_str(&format!("{:02}", dt.second)),
                b'j' => out.push_str(&format!("{:03}", day_of_year(dt))),
                b's' => out.push_str(&dt.to_unix().to_string()),
                b'w' => out.push_str(&day_of_week(dt).to_string()),
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
