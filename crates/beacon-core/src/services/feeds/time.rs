// SPDX-License-Identifier: GPL-3.0-or-later

//! Durations and dates as podcast feeds write them, which is often loosely.

use std::time::Duration;

use chrono::{DateTime, Utc};

/// Parse a duration written as `SS`, `MM:SS` or `HH:MM:SS`, each optionally
/// with `.mmm`. Covers `<itunes:duration>` and Podlove Normal Play Time.
pub fn parse_npt(value: &str) -> Option<Duration> {
    let (clock, millis) = match value.trim().split_once('.') {
        Some((clock, fraction)) => {
            let digits: String = fraction.chars().take(3).collect();
            if digits.is_empty() {
                return None;
            }
            let scale = 10u64.pow(3 - digits.len() as u32);
            (clock, digits.parse::<u64>().ok()? * scale)
        }
        None => (value.trim(), 0),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut seconds = 0u64;
    for part in parts {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(part.parse().ok()?)?;
    }
    Some(Duration::from_millis(seconds.checked_mul(1000)? + millis))
}

/// Parse a `<pubDate>`. RFC 2822 is the standard, but feeds also get the
/// weekday wrong or use RFC 3339, so try those too.
pub fn parse_date(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    let without_weekday = value
        .split_once(',')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(value);
    DateTime::parse_from_rfc2822(value)
        .or_else(|_| DateTime::parse_from_rfc2822(without_weekday))
        .or_else(|_| DateTime::parse_from_rfc3339(value))
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn npt_formats() {
        assert_eq!(parse_npt("0"), Some(Duration::ZERO));
        assert_eq!(parse_npt("3120"), Some(Duration::from_secs(3120)));
        assert_eq!(parse_npt("52:00"), Some(Duration::from_secs(3120)));
        assert_eq!(parse_npt("01:02:03"), Some(Duration::from_secs(3723)));
        assert_eq!(parse_npt(" 00:00:01.5 "), Some(Duration::from_millis(1500)));
        assert_eq!(parse_npt("1.250"), Some(Duration::from_millis(1250)));
    }

    #[test]
    fn npt_rejects_garbage() {
        for bad in ["", "abc", "1:2:3:4", "-5", "1:", "1.", "+3"] {
            assert_eq!(parse_npt(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn dates() {
        let expected = Utc.with_ymd_and_hms(2026, 10, 27, 8, 0, 0).unwrap();
        assert_eq!(parse_date("Tue, 27 Oct 2026 08:00:00 GMT"), Some(expected));
        assert_eq!(
            parse_date("Tue, 27 Oct 2026 09:00:00 +0100"),
            Some(expected)
        );
        assert_eq!(parse_date("Tue, 27 Oct 2026 01:00:00 PDT"), Some(expected));
        // Wrong weekday, as some feeds publish.
        assert_eq!(parse_date("Mon, 27 Oct 2026 08:00:00 GMT"), Some(expected));
        assert_eq!(parse_date("27 Oct 2026 08:00:00 GMT"), Some(expected));
        assert_eq!(parse_date("2026-10-27T08:00:00Z"), Some(expected));
        assert_eq!(parse_date("yesterday"), None);
    }
}
