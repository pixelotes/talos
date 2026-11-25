//! Civil dates: the proleptic Gregorian calendar, with no time zones.
//!
//! Enough calendar for the date extractors and for reading the local date
//! off an injected [`Timestamp`](crate::Timestamp). Time zones and their
//! rules are a Slice C decision; nothing here knows any.

use std::fmt;

/// A day of the week. Weeks start on Monday (docs/spec/basics.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

impl Weekday {
    /// Every weekday, Monday first.
    pub const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    /// 0 for Monday up to 6 for Sunday.
    #[must_use]
    pub const fn days_from_monday(self) -> u8 {
        self as u8
    }
}

/// A valid calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

/// Whether `year` is a leap year.
#[must_use]
pub const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Days in a month (1–12) of a year; 0 for a month out of range.
#[must_use]
pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

impl Date {
    /// A date, if it exists ("31 April" does not).
    #[must_use]
    pub const fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        if day >= 1 && day <= days_in_month(year, month) {
            Some(Date { year, month, day })
        } else {
            None
        }
    }

    /// The year.
    #[must_use]
    pub const fn year(self) -> i32 {
        self.year
    }

    /// The month, 1–12.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, 1–31.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01 (negative before it). Howard Hinnant's
    /// `days_from_civil`.
    #[must_use]
    pub fn days_since_epoch(self) -> i64 {
        let (m, d) = (i64::from(self.month), i64::from(self.day));
        let y = i64::from(self.year) - i64::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` after 1970-01-01. Howard Hinnant's `civil_from_days`.
    /// Saturates at the ends of the representable years.
    #[must_use]
    pub fn from_days_since_epoch(days: i64) -> Date {
        let z = days.clamp(-1_000_000_000, 1_000_000_000) + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Date {
            year: i32::try_from(year).unwrap_or(i32::MAX),
            month: u8::try_from(month).unwrap_or(1),
            day: u8::try_from(day).unwrap_or(1),
        }
    }

    /// The date `days` later (earlier when negative).
    #[must_use]
    pub fn add_days(self, days: i64) -> Date {
        Date::from_days_since_epoch(self.days_since_epoch().saturating_add(days))
    }

    /// The day of the week.
    #[must_use]
    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday (index 3).
        let index = (self.days_since_epoch() + 3).rem_euclid(7);
        Weekday::ALL[usize::try_from(index).unwrap_or(0)]
    }

    /// The last day of this date's month.
    #[must_use]
    pub const fn end_of_month(self) -> Date {
        Date {
            year: self.year,
            month: self.month,
            day: days_in_month(self.year, self.month),
        }
    }
}

/// ISO 8601: `2027-03-14`. This is the form outcome lines and golden files
/// use; replies write dates out in words.
impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_round_trips() {
        let d = Date::new(1970, 1, 1).unwrap();
        assert_eq!(d.days_since_epoch(), 0);
        assert_eq!(d.weekday(), Weekday::Thursday);
        for days in [-800_000, -1, 0, 1, 20_000, 2_932_896] {
            assert_eq!(Date::from_days_since_epoch(days).days_since_epoch(), days);
        }
    }

    #[test]
    fn the_specs_sunday() {
        let d = Date::new(2027, 3, 14).unwrap();
        assert_eq!(d.weekday(), Weekday::Sunday);
        assert_eq!(d.add_days(5).to_string(), "2027-03-19");
        assert_eq!(d.add_days(-14).to_string(), "2027-02-28");
        assert_eq!(d.end_of_month().day(), 31);
    }

    #[test]
    fn nonexistent_dates_are_rejected() {
        assert!(Date::new(2027, 4, 31).is_none());
        assert!(Date::new(2027, 2, 29).is_none());
        assert!(Date::new(2028, 2, 29).is_some());
        assert!(Date::new(1900, 2, 29).is_none());
        assert!(Date::new(2000, 2, 29).is_some());
        assert!(Date::new(2027, 13, 1).is_none());
        assert!(Date::new(2027, 1, 0).is_none());
    }
}
