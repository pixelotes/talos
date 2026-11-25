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

    /// The date `months` calendar months later (earlier when negative),
    /// keeping the day where the month has it and the month's last day
    /// otherwise (31 January plus one month is 28 or 29 February). `None`
    /// when the year is out of range.
    #[must_use]
    pub fn add_months(self, months: i64) -> Option<Date> {
        let index = i64::from(self.year) * 12 + i64::from(self.month) - 1 + months;
        let year = i32::try_from(index.div_euclid(12)).ok()?;
        let month = u8::try_from(index.rem_euclid(12) + 1).ok()?;
        Date::new(year, month, self.day.min(days_in_month(year, month)))
    }

    /// Whole days from `self` to `later` (negative when `later` is earlier).
    #[must_use]
    pub fn days_until(self, later: Date) -> i64 {
        later.days_since_epoch() - self.days_since_epoch()
    }

    /// The calendar span from `self` to `later` as whole years, months and
    /// days, the way people count it: 2000-01-01 to 2027-03-14 is 27 years,
    /// 2 months and 13 days. All zero when `later` is not after `self`.
    #[must_use]
    pub fn calendar_span(self, later: Date) -> (u32, u32, u32) {
        if later <= self {
            return (0, 0, 0);
        }
        let mut months = (i64::from(later.year) - i64::from(self.year)) * 12
            + i64::from(later.month)
            - i64::from(self.month);
        if later.day < self.day {
            months -= 1;
        }
        // The anchor is `self` plus whole months, never after `later`.
        let anchor = self.add_months(months).unwrap_or(self);
        let days = anchor.days_until(later);
        (
            u32::try_from(months / 12).unwrap_or(0),
            u32::try_from(months % 12).unwrap_or(0),
            u32::try_from(days).unwrap_or(0),
        )
    }
}

/// Easter Sunday of a Gregorian `year` (the anonymous Gregorian algorithm of
/// Meeus, Jones and Butcher).
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the published algorithm's own names"
)]
pub fn easter(year: i32) -> Date {
    let y = i64::from(year);
    let (a, b, c) = (y % 19, y.div_euclid(100), y.rem_euclid(100));
    let (d, e) = (b / 4, b % 4);
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Date::new(
        year,
        u8::try_from(month).unwrap_or(3),
        u8::try_from(day).unwrap_or(1),
    )
    .unwrap_or(Date {
        year,
        month: 3,
        day: 22,
    })
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

#[cfg(test)]
mod span_tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    #[test]
    fn months_are_added_by_the_calendar() {
        assert_eq!(date(2027, 1, 31).add_months(1), Some(date(2027, 2, 28)));
        assert_eq!(date(2028, 1, 31).add_months(1), Some(date(2028, 2, 29)));
        assert_eq!(date(2027, 11, 15).add_months(3), Some(date(2028, 2, 15)));
        assert_eq!(date(2027, 3, 14).add_months(-14), Some(date(2026, 1, 14)));
        assert_eq!(date(2027, 12, 31).add_months(12), Some(date(2028, 12, 31)));
    }

    #[test]
    fn spans_count_years_months_and_days() {
        let now = date(2027, 3, 14);
        assert_eq!(date(2000, 1, 1).calendar_span(now), (27, 2, 13));
        assert_eq!(date(1969, 7, 20).calendar_span(now), (57, 7, 22));
        assert_eq!(date(2026, 3, 14).calendar_span(now), (1, 0, 0));
        assert_eq!(date(2026, 3, 15).calendar_span(now), (0, 11, 27));
        // Month ends: 31 January to 1 March is one month and one day from the
        // clamped anchor of 28 February.
        assert_eq!(date(2027, 1, 31).calendar_span(date(2027, 3, 1)), (0, 1, 1));
        // Across a leap day.
        assert_eq!(
            date(2028, 2, 29).calendar_span(date(2029, 2, 28)),
            (0, 11, 30)
        );
        assert_eq!(date(2028, 2, 29).calendar_span(date(2029, 3, 1)), (1, 0, 1));
        assert_eq!(now.calendar_span(now), (0, 0, 0));
        assert_eq!(now.days_until(date(2027, 12, 25)), 286);
        assert_eq!(now.days_until(date(2000, 1, 1)), -9_934);
    }

    #[test]
    fn easter_follows_the_church_calendar() {
        for (year, month, day) in [
            (2024, 3, 31),
            (2025, 4, 20),
            (2026, 4, 5),
            (2027, 3, 28),
            (2028, 4, 16),
            (1969, 4, 6),
            (2000, 4, 23),
        ] {
            assert_eq!(easter(year), date(year, month, day), "{year}");
        }
    }
}
