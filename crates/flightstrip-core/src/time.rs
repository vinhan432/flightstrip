//! Wall-clock time, formatted without a date crate.
//!
//! Everything stored is a UTC timestamp: the sidecar's RFC 3339 start and the
//! session id's compact stamp. Event times themselves are relative milliseconds
//! and never need a calendar, so this module is the only place that does.
//!
//! The civil-date conversion is Howard Hinnant's `days_from_civil` /
//! `civil_from_days`: it is exact for the whole proleptic Gregorian range and is
//! a dozen lines, which beats a dependency on `chrono`.

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 } as i64;
    let day_of_year = (153 * month_index + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let day_of_era = (days - era * 146097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn split_ms(epoch_ms: u64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let seconds = (epoch_ms / 1000) as i64;
    let millis = (epoch_ms % 1000) as u32;
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        (second_of_day / 3600) as u32,
        ((second_of_day % 3600) / 60) as u32,
        (second_of_day % 60) as u32,
        millis,
    )
}

/// `2026-11-15T08:55:00.000Z`, the form the export stores.
pub fn format_rfc3339(epoch_ms: u64) -> String {
    let (year, month, day, hour, minute, second, millis) = split_ms(epoch_ms);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// `20261115T085500Z`, used as the readable prefix of a session id.
pub fn format_stamp(epoch_ms: u64) -> String {
    let (year, month, day, hour, minute, second, _) = split_ms(epoch_ms);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

/// Parse the export format. Only UTC (`Z`) and numeric offsets are accepted;
/// anything else is `None`, because guessing a timezone is worse than failing.
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let year: i64 = text.get(0..4)?.parse().ok()?;
    let month: u32 = text.get(5..7)?.parse().ok()?;
    let day: u32 = text.get(8..10)?.parse().ok()?;
    let hour: u32 = text.get(11..13)?.parse().ok()?;
    let minute: u32 = text.get(14..16)?.parse().ok()?;
    let second: u32 = text.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }

    let mut rest = &text[19..];
    let mut millis = 0u32;
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits: String = fraction
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            return None;
        }
        let padded = format!("{digits:0<3}");
        millis = padded[..3].parse().ok()?;
        rest = &fraction[digits.len()..];
    }

    let offset_seconds: i64 = match rest {
        "Z" | "z" => 0,
        other => {
            let sign = match other.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits = &other[1..];
            let (hours, minutes) = digits.split_once(':')?;
            sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60)
        }
    };

    let days = days_from_civil(year, month, day);
    let seconds =
        days * 86_400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64 - offset_seconds;
    if seconds < 0 {
        return None;
    }
    Some(seconds as u64 * 1000 + millis as u64)
}

/// Parse `20261115T085500Z`. Used to recover a start time from an id when the
/// sidecar is missing.
pub fn parse_stamp(text: &str) -> Option<u64> {
    if text.len() != 16 || text.as_bytes()[8] != b'T' || !text.ends_with('Z') {
        return None;
    }
    let year: i64 = text.get(0..4)?.parse().ok()?;
    let month: u32 = text.get(4..6)?.parse().ok()?;
    let day: u32 = text.get(6..8)?.parse().ok()?;
    let hour: u32 = text.get(9..11)?.parse().ok()?;
    let minute: u32 = text.get(11..13)?.parse().ok()?;
    let second: u32 = text.get(13..15)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400
        + hour as i64 * 3600
        + minute as i64 * 60
        + second as i64;
    if seconds < 0 {
        return None;
    }
    Some(seconds as u64 * 1000)
}

/// `1m 05s`, for the CLI and the report.
pub fn human_duration(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epoch_converts_both_ways() {
        // 1700000000 = 2023-11-14T22:13:20Z, a widely used fixed point.
        assert_eq!(
            format_rfc3339(1_700_000_000_000),
            "2023-11-14T22:13:20.000Z"
        );
        assert_eq!(
            parse_rfc3339("2023-11-14T22:13:20.000Z"),
            Some(1_700_000_000_000)
        );
    }

    #[test]
    fn leap_day_and_epoch_edges() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(
            format_rfc3339(1_700_000_000_123),
            "2023-11-14T22:13:20.123Z"
        );
    }

    #[test]
    fn stamps_round_trip() {
        let ms = 1_763_196_900_000;
        let stamp = format_stamp(ms);
        assert_eq!(stamp, "20251115T085500Z", "stamp format changed");
        assert_eq!(parse_stamp(&stamp), Some(ms));
        assert!(parse_stamp("nonsense").is_none());
    }

    #[test]
    fn offsets_are_subtracted() {
        let utc = parse_rfc3339("2024-01-01T00:00:00Z").unwrap();
        let plus_two = parse_rfc3339("2024-01-01T02:00:00+02:00").unwrap();
        assert_eq!(utc, plus_two);
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(parse_rfc3339("").is_none());
        assert!(parse_rfc3339("2024-13-01T00:00:00Z").is_none());
        assert!(parse_rfc3339("2024-01-01").is_none());
    }

    #[test]
    fn durations_read_like_a_person_wrote_them() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(9_000), "9s");
        assert_eq!(human_duration(125_000), "2m 05s");
        assert_eq!(human_duration(3_780_000), "1h 03m");
    }
}
