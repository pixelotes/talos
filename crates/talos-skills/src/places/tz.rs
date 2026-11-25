//! Civil time in a zone: a POSIX TZ rule (`CET-1CEST,M3.5.0,M10.5.0/3`)
//! read and evaluated at an instant.
//!
//! The rule is what the IANA database itself appends to every compiled
//! zone for the years after its last listed transition: a standard offset
//! and, optionally, a daylight-saving offset with the two days of the year
//! it starts and ends ("the last Sunday of March at 02:00"). Evaluated for
//! any year it gives the zone's current rules; earlier years, when the
//! rules were different, are not this skill's business (docs/DEVELOPMENT.md,
//! "Zone data").

use talos_core::Date;

/// A zone's rule: its standard offset and, if it has one, its summer time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    /// Seconds east of UTC outside summer time.
    standard: i32,
    /// Summer time, if the zone observes it.
    summer: Option<Summer>,
}

/// Summer time: its offset and when it starts and ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Summer {
    /// Seconds east of UTC during summer time.
    offset: i32,
    /// When it starts, on the standard wall clock.
    start: Day,
    /// When it ends, on the summer wall clock.
    end: Day,
}

/// "The `week`th `weekday` of `month` at `time`" (`Mm.w.d/time`); week 5
/// is the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Day {
    month: u8,
    week: u8,
    /// 0 for Sunday up to 6 for Saturday, as POSIX numbers them.
    weekday: u8,
    /// Seconds after local midnight; may be negative or past 24 h.
    time: i32,
}

/// A cursor over a rule's text.
struct Reader<'a> {
    text: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        if found {
            self.at += 1;
        }
        found
    }

    fn done(&self) -> bool {
        self.at == self.text.len()
    }

    /// A zone abbreviation: letters, or anything between `<` and `>`.
    fn name(&mut self) -> Option<()> {
        if self.eat(b'<') {
            while self.peek()? != b'>' {
                self.at += 1;
            }
            self.at += 1;
            return Some(());
        }
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            self.at += 1;
        }
        (self.at - start >= 3).then_some(())
    }

    /// A decimal number of at most three digits.
    fn number(&mut self) -> Option<i32> {
        let start = self.at;
        let mut value = 0;
        while let Some(b) = self.peek().filter(u8::is_ascii_digit) {
            value = value * 10 + i32::from(b - b'0');
            self.at += 1;
            if self.at - start > 3 {
                return None;
            }
        }
        (self.at > start).then_some(value)
    }

    /// `[+-]hh[:mm[:ss]]`, in seconds.
    fn clock(&mut self) -> Option<i32> {
        let sign = if self.eat(b'-') {
            -1
        } else {
            self.eat(b'+');
            1
        };
        let mut seconds = self.number()? * 3_600;
        if self.eat(b':') {
            seconds += self.number()? * 60;
            if self.eat(b':') {
                seconds += self.number()?;
            }
        }
        Some(sign * seconds)
    }

    /// `Mm.w.d[/time]`.
    fn day(&mut self) -> Option<Day> {
        if !self.eat(b'M') {
            return None;
        }
        let month = u8::try_from(self.number()?).ok()?;
        self.eat(b'.').then_some(())?;
        let week = u8::try_from(self.number()?).ok()?;
        self.eat(b'.').then_some(())?;
        let weekday = u8::try_from(self.number()?).ok()?;
        let time = if self.eat(b'/') { self.clock()? } else { 7_200 };
        ((1..=12).contains(&month) && (1..=5).contains(&week) && weekday <= 6).then_some(Day {
            month,
            week,
            weekday,
            time,
        })
    }
}

impl Rule {
    /// A fixed offset from UTC, in seconds: "UTC+2".
    pub(crate) const fn fixed(offset: i32) -> Rule {
        Rule {
            standard: offset,
            summer: None,
        }
    }

    /// Reads a POSIX TZ rule. Only the `Mm.w.d` form of a day is read: it
    /// is the one every zone in the table uses.
    pub(crate) fn parse(text: &str) -> Option<Rule> {
        let mut r = Reader {
            text: text.as_bytes(),
            at: 0,
        };
        r.name()?;
        // POSIX counts offsets west of UTC.
        let standard = -r.clock()?;
        if r.done() {
            return Some(Rule::fixed(standard));
        }
        r.name()?;
        let offset = if matches!(r.peek(), Some(b',')) {
            standard + 3_600
        } else {
            -r.clock()?
        };
        r.eat(b',').then_some(())?;
        let start = r.day()?;
        r.eat(b',').then_some(())?;
        let end = r.day()?;
        r.done().then_some(Rule {
            standard,
            summer: Some(Summer { offset, start, end }),
        })
    }

    /// The offset from UTC, in seconds, at an instant.
    pub(crate) fn offset_at(&self, unix: i64) -> i32 {
        let Some(summer) = self.summer else {
            return self.standard;
        };
        let year = year_of(unix + i64::from(self.standard));
        let start = summer.start.instant(year, self.standard);
        let end = summer.end.instant(year, summer.offset);
        let inside = if start < end {
            start <= unix && unix < end
        } else {
            // Summer time across the new year (the southern hemisphere), or
            // a "summer" offset below the standard one (Dublin).
            !(end <= unix && unix < start)
        };
        if inside { summer.offset } else { self.standard }
    }

    /// The changes of offset after `from` and up to `to` (Unix seconds),
    /// in order: when, and the offset from then on.
    pub(crate) fn changes(&self, from: i64, to: i64) -> Vec<(i64, i32)> {
        let Some(summer) = self.summer else {
            return Vec::new();
        };
        let mut found = Vec::new();
        let first = year_of(from + i64::from(self.standard)) - 1;
        let last = year_of(to + i64::from(self.standard)) + 1;
        for year in first..=last {
            for (instant, offset) in [
                (summer.start.instant(year, self.standard), summer.offset),
                (summer.end.instant(year, summer.offset), self.standard),
            ] {
                if from < instant && instant <= to {
                    found.push((instant, offset));
                }
            }
        }
        found.sort_unstable();
        found
    }
}

impl Day {
    /// The instant this day's change happens in `year`, the wall clock
    /// before it being `offset` seconds east of UTC.
    fn instant(self, year: i32, offset: i32) -> i64 {
        let first =
            Date::new(year, self.month, 1).unwrap_or_else(|| Date::from_days_since_epoch(0));
        // Monday-based weekday of the 1st, and the wanted one.
        let first_weekday = i64::from(first.weekday().days_from_monday());
        let wanted = (i64::from(self.weekday) + 6) % 7;
        let mut day = 1 + (wanted - first_weekday).rem_euclid(7) + 7 * (i64::from(self.week) - 1);
        let length = i64::from(talos_core::civil::days_in_month(year, self.month));
        while day > length {
            day -= 7;
        }
        let midnight = (first.days_since_epoch() + day - 1) * 86_400;
        midnight + i64::from(self.time) - i64::from(offset)
    }
}

/// The year of a Unix time read on some wall clock.
fn year_of(local: i64) -> i32 {
    Date::from_days_since_epoch(local.div_euclid(86_400)).year()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u8, d: u8, h: i64, min: i64) -> i64 {
        Date::new(y, m, d).unwrap().days_since_epoch() * 86_400 + h * 3_600 + min * 60
    }

    #[test]
    fn reads_the_forms_in_the_table() {
        let madrid = Rule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        assert_eq!(madrid.standard, 3_600);
        let summer = madrid.summer.unwrap();
        assert_eq!(summer.offset, 7_200);
        assert_eq!(summer.end.time, 10_800);
        assert_eq!(Rule::parse("<+0545>-5:45"), Some(Rule::fixed(20_700)));
        let nuuk = Rule::parse("<-02>2<-01>,M3.5.0/-1,M10.5.0/0").unwrap();
        assert_eq!(nuuk.summer.unwrap().start.time, -3_600);
        assert!(Rule::parse("IST-2IDT,M3.4.4/26,M10.5.0").is_some());
        assert_eq!(Rule::parse(""), None);
        assert_eq!(Rule::parse("CET-1CEST,J60,J300"), None);
    }

    #[test]
    fn summer_time_starts_and_ends_at_the_stated_instant() {
        let madrid = Rule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        // 2027-03-28 is the last Sunday of March: 02:00 CET is 01:00 UTC.
        assert_eq!(madrid.offset_at(at(2027, 3, 28, 0, 59)), 3_600);
        assert_eq!(madrid.offset_at(at(2027, 3, 28, 1, 0)), 7_200);
        assert_eq!(madrid.offset_at(at(2027, 10, 31, 0, 59)), 7_200);
        assert_eq!(madrid.offset_at(at(2027, 10, 31, 1, 0)), 3_600);
        let sydney = Rule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(sydney.offset_at(at(2027, 1, 1, 0, 0)), 39_600);
        assert_eq!(sydney.offset_at(at(2027, 7, 1, 0, 0)), 36_000);
        assert_eq!(
            sydney.changes(at(2027, 3, 14, 17, 42), at(2027, 4, 13, 17, 42)),
            // 4 April 03:00 AEDT is 3 April 16:00 UTC.
            vec![(at(2027, 4, 3, 16, 0), 36_000)]
        );
    }
}
