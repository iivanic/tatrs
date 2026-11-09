//! Human-Unique IDentifiers: `YYYYMMDD-HHMMSS` in UTC, with an optional suffix.
//!
//! IDs are generated at human speed (one per second), which is what makes them
//! merge-friendly across branches: no shared counter exists to contend with.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Huid(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HuidError {
    #[error("`{0}` is not a task id; expected YYYYMMDD-HHMMSS with an optional -suffix")]
    Shape(String),
    #[error("`{0}` is not a valid UTC calendar timestamp")]
    Calendar(String),
    #[error("`{0}` is not a valid task id suffix; use letters, digits, and `-` only")]
    Suffix(String),
}

impl Huid {
    /// Parse and validate an ID. The suffix, when present, must be non-empty.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, HuidError> {
        let s = raw.as_ref();
        let bytes = s.as_bytes();

        if bytes.len() < 15 || bytes[8] != b'-' {
            return Err(HuidError::Shape(s.to_owned()));
        }
        if !bytes[..8].iter().all(|b| b.is_ascii_digit())
            || !bytes[9..15].iter().all(|b| b.is_ascii_digit())
        {
            return Err(HuidError::Shape(s.to_owned()));
        }
        if bytes.len() > 15 {
            if bytes[15] != b'-' || bytes.len() == 16 {
                return Err(HuidError::Suffix(s.to_owned()));
            }
            if !bytes[16..]
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
            {
                return Err(HuidError::Suffix(s.to_owned()));
            }
        }

        let year: i64 = s[0..4]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;
        let month: u32 = s[4..6]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;
        let day: u32 = s[6..8]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;
        let hour: u32 = s[9..11]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;
        let minute: u32 = s[11..13]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;
        let second: u32 = s[13..15]
            .parse()
            .map_err(|_| HuidError::Shape(s.to_owned()))?;

        if !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(HuidError::Calendar(s.to_owned()));
        }

        Ok(Self(s.to_owned()))
    }

    /// Build an ID from a Unix timestamp in seconds (UTC).
    pub fn from_unix(secs: u64) -> Self {
        let days = (secs / 86_400) as i64;
        let rem = (secs % 86_400) as u32;
        let (year, month, day) = civil_from_days(days);
        let hour = rem / 3_600;
        let minute = (rem % 3_600) / 60;
        let second = rem % 60;
        Self(format!(
            "{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}"
        ))
    }

    /// Current UTC time.
    pub fn now() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self::from_unix(secs)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Append a suffix, returning a new ID. Empty suffixes are rejected so that
    /// a trailing `-` can never be produced.
    pub fn with_suffix(&self, suffix: &str) -> Result<Self, HuidError> {
        if suffix.is_empty() {
            return Err(HuidError::Suffix(self.0.clone()));
        }
        Self::parse(format!("{}-{}", self.0, suffix))
    }

    pub fn is_valid(raw: impl AsRef<str>) -> bool {
        Self::parse(raw).is_ok()
    }
}

impl fmt::Display for Huid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for Huid {
    type Err = HuidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

pub(crate) fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian civil date.
/// Algorithm by Howard Hinnant, valid for the whole `i64` range in practice.
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_suffixed_ids() {
        assert_eq!(
            Huid::parse("20261017-045007").unwrap().as_str(),
            "20261017-045007"
        );
        assert_eq!(
            Huid::parse("20260830-000838-rexim").unwrap().as_str(),
            "20260830-000838-rexim"
        );
        assert_eq!(
            Huid::parse("20260829-235902-01").unwrap().as_str(),
            "20260829-235902-01"
        );
    }

    #[test]
    fn rejects_malformed_ids() {
        for bad in [
            "",
            "2026-10-17",
            "20261017",
            "20261017-0450",
            "20261301-000000",
            "20261017-250000",
            "20260230-000000",
            "20261017-045007-",
            "20261017-045007-a_b",
            "20261017-045007-a b",
        ] {
            assert!(Huid::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn accepts_leap_day() {
        assert!(Huid::parse("20240229-120000").is_ok());
        assert!(Huid::parse("20230229-120000").is_err());
    }

    #[test]
    fn unix_epoch_and_known_timestamp() {
        assert_eq!(Huid::from_unix(0).as_str(), "19700101-000000");
        assert_eq!(Huid::from_unix(1_700_000_000).as_str(), "20231114-221320");
    }

    #[test]
    fn suffix_round_trip() {
        let base = Huid::parse("20261109-133012").unwrap();
        let suffixed = base.with_suffix("agent-a").unwrap();
        assert_eq!(suffixed.as_str(), "20261109-133012-agent-a");
        assert!(base.with_suffix("").is_err());
        assert!(base.with_suffix("bad suffix").is_err());
    }

    #[test]
    fn now_is_valid() {
        assert!(Huid::parse(Huid::now().as_str()).is_ok());
    }
}
