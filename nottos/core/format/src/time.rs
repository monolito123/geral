// SPDX-License-Identifier: Apache-2.0

//! Points in time, as written in NottOS files.
//!
//! The only textual form is `YYYY-MM-DDTHH:MM:SSZ` in UTC, from
//! 1970-01-01T00:00:00Z to 9999-12-31T23:59:59Z. Leap seconds are not
//! represented. Conversions use the proleptic Gregorian calendar.

use crate::text::parse_digits;
use std::fmt;

/// Seconds since 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

/// 9999-12-31T23:59:59Z.
const MAX_UNIX: u64 = 253_402_300_799;
const SECONDS_PER_DAY: u64 = 86_400;

impl Timestamp {
    /// The earliest representable time.
    pub const EPOCH: Timestamp = Timestamp(0);

    /// From seconds since the epoch. `None` beyond year 9999.
    pub fn from_unix(seconds: u64) -> Option<Self> {
        (seconds <= MAX_UNIX).then_some(Self(seconds))
    }

    /// Seconds since the epoch.
    pub fn unix(self) -> u64 {
        self.0
    }

    /// Parses exactly `YYYY-MM-DDTHH:MM:SSZ` with a valid calendar date.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let separators = [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ];
        if bytes.len() != 20 || separators.iter().any(|&(at, byte)| bytes[at] != byte) {
            return None;
        }
        let field = |range: std::ops::Range<usize>| parse_digits(&text[range]);
        let (year, month, day) = (field(0..4)?, field(5..7)?, field(8..10)?);
        let (hour, minute, second) = (field(11..13)?, field(14..16)?, field(17..19)?);
        if year < 1970
            || !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        let days = days_from_civil(year, month, day);
        Some(Self(
            days * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second,
        ))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil_from_days(self.0 / SECONDS_PER_DAY);
        let rest = self.0 % SECONDS_PER_DAY;
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            rest / 3600,
            rest / 60 % 60,
            rest % 60
        )
    }
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a date in 1970..=9999. The well-known
/// era-based algorithm, with March as the first month of the year.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values computed with Python's `datetime`.
    const KNOWN: [(&str, u64); 7] = [
        ("1970-01-01T00:00:00Z", 0),
        ("2000-03-01T00:00:00Z", 951_868_800),
        ("2024-02-29T12:34:56Z", 1_709_210_096),
        ("2026-11-13T19:45:30Z", 1_794_599_130),
        ("2038-01-19T03:14:08Z", 2_147_483_648),
        ("2100-03-01T00:00:00Z", 4_107_542_400),
        ("9999-12-31T23:59:59Z", 253_402_300_799),
    ];

    #[test]
    fn known_values() {
        for (text, seconds) in KNOWN {
            let parsed = Timestamp::parse(text).unwrap_or_else(|| panic!("{text} parses"));
            assert_eq!(parsed.unix(), seconds, "{text}");
            assert_eq!(parsed.to_string(), text);
            assert_eq!(Timestamp::from_unix(seconds), Some(parsed));
        }
        assert_eq!(Timestamp::from_unix(MAX_UNIX + 1), None);
    }

    #[test]
    fn every_day_round_trips() {
        // Every day from 1970 to 2500, at a varying time of day.
        let mut previous = None;
        for day in 0..(530 * 366) {
            let seconds = day * SECONDS_PER_DAY + (day * 7919) % SECONDS_PER_DAY;
            let stamp = Timestamp::from_unix(seconds).expect("in range");
            let text = stamp.to_string();
            assert_eq!(Timestamp::parse(&text), Some(stamp), "{text}");
            if let Some(previous) = previous {
                assert!(text > previous, "text order follows time order");
            }
            previous = Some(text);
        }
    }

    #[test]
    fn rejects_non_canonical_and_impossible_times() {
        for bad in [
            "",
            "2026-11-13",
            "2026-11-13T19:45:30",
            "2026-11-13 19:45:30Z",
            "2026-11-13T19:45:30z",
            "2026-11-13T19:45:30+00:00",
            "2026-11-13T19:45:30.0Z",
            "2026-1-13T19:45:30Z",
            "+026-11-13T19:45:30Z",
            "1969-12-31T23:59:59Z",
            "2026-00-13T19:45:30Z",
            "2026-13-13T19:45:30Z",
            "2026-11-00T19:45:30Z",
            "2026-11-31T19:45:30Z",
            "2023-02-29T00:00:00Z",
            "2100-02-29T00:00:00Z",
            "2026-11-13T24:00:00Z",
            "2026-11-13T19:60:00Z",
            "2026-11-13T19:45:60Z",
        ] {
            assert_eq!(Timestamp::parse(bad), None, "{bad:?}");
        }
        assert!(Timestamp::parse("2000-02-29T00:00:00Z").is_some());
        assert!(Timestamp::parse("2024-02-29T00:00:00Z").is_some());
    }
}
