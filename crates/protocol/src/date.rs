const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "June", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub fn format_http_date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs_of_day = unix % 86_400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let weekday = WEEKDAYS[((days + 3).rem_euclid(7)) as usize];
    let (year, month, day) = civil_from_days(days);
    format!(
        "{weekday}, {day:02} {month} {year} {hour:02}:{minute:02}:{second:02} GMT",
        month = MONTHS[(month -1) as usize]
    )
}

pub fn parse_http_date(s: &str) -> Option<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 6 {
        return None;
    }
    if !parts[0].ends_with(',') || parts[0].len() != 4 {
        return None;
    }
    let day: u32 = parts[1].parse().ok()?;
    let month = MONTHS.iter().position(|m| *m == parts[2])? as u32 + 1;
    let year: i64 = parts[3].parse().ok()?;
    let time: Vec<&str> = parts[4].split(":").collect() ;
    if time.len() != 3 {
        return None;
    }
    let hour: u32 = time[0].parse().ok()?;
    let minute: u32 = time[1].parse().ok()?;
    let second: u32 = time[2].parse().ok()?;
    if parts[5] != "GMT" {
        return None;
    }
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64)
}

fn civil_from_days(z: i64) -> (i64, u32, u32)  {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); 
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2 ) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 {y + 1} else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400; 
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

pub fn system_time_to_unix(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_epoch() {
        assert_eq!(format_http_date(0), "Thu, 01 Jan 1970 00:00:00 GMT");
    }
    #[test]
    fn formats_known_date() {
        assert_eq!(format_http_date(1_759_276_800), "Wed, 01 Oct 2025 00:00:00 GMT");
    }
    #[test] 
    fn roundtrip() {
        for ts in [0u64, 1, 86_400, 951_782_400, 1_759_276_800, 4_102_444_799] {
            let s = format_http_date(ts);
            let back = parse_http_date(&s).unwrap_or_else(|| panic!("parse failed: {s}"));
            assert_eq!(back, ts - ts % 1, "mismatch for {ts} ({s})");
        }
    }
    #[test]
    fn rejects_garbage() {
        assert!(parse_http_date("n").is_none());
        assert!(parse_http_date("Thu, 32 Jan 1970 00:00:00 GMT").is_none());
        assert!(parse_http_date("Thu, 01 Foo 1970 00:00:00 GMT").is_none());
        assert!(parse_http_date("Thu, 01 Jan 1970 25:00:00 GMT").is_none());
        assert!(parse_http_date("Thu, 01 Jan 1970 00:00:00 PST").is_none());
    }
    #[test]
    fn weekday_math() {
        assert!(format_http_date(86_400 * 3).starts_with("Sun,"));
        assert!(format_http_date(86_400 * 3).starts_with("Mon,"));
    }
}