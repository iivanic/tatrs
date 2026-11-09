//! Time helpers: UTC epoch seconds, ISO-8601 formatting/parsing, and duration
//! parsing for claim TTLs. No time crate: the project only needs UTC.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::huid::{civil_from_days, days_in_month};

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-11-09T13:41:02Z`
pub fn format_iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let remainder = secs.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remainder / 3_600,
        (remainder % 3_600) / 60,
        remainder % 60
    )
}

pub fn parse_iso(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return None;
    }

    let year: i64 = text[0..4].parse().ok()?;
    let month: u32 = text[5..7].parse().ok()?;
    let day: u32 = text[8..10].parse().ok()?;
    let hour: u32 = text[11..13].parse().ok()?;
    let minute: u32 = text[14..16].parse().ok()?;
    let second: u32 = text[17..19].parse().ok()?;

    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    Some(
        days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3_600
            + minute as i64 * 60
            + second as i64,
    )
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = (year - era * 400) as u64;
    let month_prime = if month > 2 { month - 3 } else { month + 9 } as u64;
    let day_of_year = (153 * month_prime + 2) / 5 + day as u64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era as i64 - 719_468
}

/// Parse a human duration (`30s`, `15m`, `2h`, `1d`, or bare seconds).
pub fn parse_duration(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let split = text
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(split);
    let amount: u64 = digits.parse().ok()?;
    let multiplier = match unit.trim() {
        "" | "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return None,
    };
    amount.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_and_parses() {
        assert_eq!(format_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn round_trips_arbitrary_timestamps() {
        for secs in [1_700_000_000, 1_760_000_000, 1_800_000_000] {
            assert_eq!(parse_iso(&format_iso(secs)), Some(secs));
        }
    }

    #[test]
    fn rejects_malformed_timestamps() {
        for bad in [
            "2026-11-09",
            "2026-11-09T13:41:02",
            "2026-13-09T13:41:02Z",
            "2026-11-32T13:41:02Z",
            "2026-11-09T24:00:00Z",
            "2026-11-09T13:61:02Z",
            "hello",
        ] {
            assert_eq!(parse_iso(bad), None, "{bad:?} should be rejected");
        }
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("90"), Some(90));
        assert_eq!(parse_duration("2h"), Some(7_200));
        assert_eq!(parse_duration("15m"), Some(900));
        assert_eq!(parse_duration("1d"), Some(86_400));
        assert_eq!(parse_duration("2h30m"), None);
        assert_eq!(parse_duration("soon"), None);
        assert_eq!(parse_duration(""), None);
    }
}
