//! Turning a unix timestamp into a date a person can read.
//!
//! The installed hook says when it was installed and backup refs are named by
//! date, both of which need calendar arithmetic that `std` does not provide.
//! Rather than take a dependency for it, the civil-date conversion is done
//! here — about a dozen lines, and pinned down by tests against known dates.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the unix epoch, or 0 if the clock is before it.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Today's UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    date(now())
}

/// A UTC timestamp as `YYYYMMDDTHHMMSS`, for names that must sort and be unique.
pub fn stamp_now() -> String {
    stamp(now())
}

/// Formats a unix timestamp as `YYYY-MM-DD` in UTC.
pub fn date(seconds: u64) -> String {
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Formats a unix timestamp as `YYYYMMDDTHHMMSS` in UTC.
pub fn stamp(seconds: u64) -> String {
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let remainder = seconds % 86_400;
    let (hour, minute, second) = (remainder / 3600, (remainder % 3600) / 60, remainder % 60);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}")
}

/// Days since 1970-01-01 to a calendar date, by Howard Hinnant's algorithm.
///
/// The era trick moves the leap-year rules to the end of a 400-year cycle, so
/// there are no special cases for February.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_the_epoch() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(stamp(0), "19700101T000000");
    }

    #[test]
    fn formats_a_date_with_a_time() {
        assert_eq!(date(1_758_100_000), "2025-09-17");
        assert_eq!(stamp(1_758_100_000), "20250917T090640");
    }

    #[test]
    fn handles_a_leap_day() {
        assert_eq!(date(951_782_400), "2000-02-29");
    }

    #[test]
    fn handles_the_day_after_a_leap_day() {
        assert_eq!(date(1_583_020_800), "2020-03-01");
    }

    #[test]
    fn handles_a_century_that_is_not_a_leap_year() {
        // 2100 is divisible by 100 but not 400, so it has no 29th of February.
        assert_eq!(date(4_107_542_400), "2100-03-01");
    }

    #[test]
    fn handles_dates_past_the_32_bit_cliff() {
        assert_eq!(date(2_145_916_800), "2038-01-01");
    }

    #[test]
    fn today_is_plausible() {
        let today = today();
        assert_eq!(today.len(), 10);
        assert!(today.starts_with("20"), "{today}");
    }
}
