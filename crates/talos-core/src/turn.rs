//! The turn: one user message and everything the engine needs to answer it
//! (DESIGN.md section 3). Context travels in this value and nowhere else.

use crate::Lang;
use crate::civil::Date;

/// Identifies the dialog a turn belongs to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConvId(pub String);

impl ConvId {
    /// A conversation identifier from any string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A caller-provided place: a place name or an IANA zone. Carried but not yet
/// interpreted; resolving it needs the date/time crate chosen in Slice C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place(pub String);

impl Place {
    /// The place as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An instant, injected by the caller. The engine never reads the system
/// clock (DESIGN.md section 3).
///
/// A deliberately minimal civil-time representation: a Unix instant plus, when
/// the caller knows it, the offset of the local wall clock from UTC. Without
/// that offset the engine knows *when* but not *what the wall clock reads*,
/// and anything that depends on local time falls back to a time-neutral
/// answer. The real date/time and zone handling is a Slice C decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Seconds since 1970-01-01T00:00:00Z.
    pub unix_seconds: i64,
    /// Local wall clock minus UTC, in seconds, if the caller knows it.
    pub utc_offset_seconds: Option<i32>,
}

impl Timestamp {
    /// An instant with no known local offset.
    #[must_use]
    pub const fn from_unix(unix_seconds: i64) -> Self {
        Self {
            unix_seconds,
            utc_offset_seconds: None,
        }
    }

    /// The same instant, with the local wall clock's offset from UTC.
    #[must_use]
    pub const fn with_utc_offset(self, offset_seconds: i32) -> Self {
        Self {
            unix_seconds: self.unix_seconds,
            utc_offset_seconds: Some(offset_seconds),
        }
    }

    /// An instant from a wall-clock reading and its offset from UTC: the
    /// inverse of [`Timestamp::local_date`] and
    /// [`Timestamp::local_seconds_of_day`].
    #[must_use]
    pub fn from_local(date: Date, seconds_of_day: u32, utc_offset_seconds: i32) -> Self {
        let local = date.days_since_epoch() * 86_400 + i64::from(seconds_of_day);
        Self::from_unix(local - i64::from(utc_offset_seconds)).with_utc_offset(utc_offset_seconds)
    }

    /// Seconds since 1970 on the local wall clock, if the offset is known.
    fn local_seconds(self) -> Option<i64> {
        Some(self.unix_seconds + i64::from(self.utc_offset_seconds?))
    }

    /// The hour (0–23) on the local wall clock, if the offset is known.
    #[must_use]
    pub fn local_hour(self) -> Option<u8> {
        u8::try_from(self.local_seconds_of_day()? / 3_600).ok()
    }

    /// Seconds since local midnight (0–86 399), if the offset is known.
    #[must_use]
    pub fn local_seconds_of_day(self) -> Option<u32> {
        u32::try_from(self.local_seconds()?.rem_euclid(86_400)).ok()
    }

    /// The date on the local wall clock, if the offset is known.
    #[must_use]
    pub fn local_date(self) -> Option<Date> {
        Some(Date::from_days_since_epoch(
            self.local_seconds()?.div_euclid(86_400),
        ))
    }

    /// The local date when the offset is known, else the date in UTC: what
    /// relative dates ("mañana") are counted from.
    #[must_use]
    pub fn date_or_utc(self) -> Date {
        self.local_date()
            .unwrap_or_else(|| Date::from_days_since_epoch(self.unix_seconds.div_euclid(86_400)))
    }
}

/// One user message, with its context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// The message exactly as received.
    pub text: String,
    /// The caller's language hint; detected when absent.
    pub lang: Option<Lang>,
    /// Which dialog this turn belongs to. The engine keeps that dialog's
    /// state (language, turn counter, recent topic) itself.
    pub conversation: ConvId,
    /// The current instant, injected.
    pub now: Timestamp,
    /// Where the caller is, if it says.
    pub place: Option<Place>,
    /// Whether the outcome should carry a stage-by-stage trace.
    pub trace: bool,
}

impl Turn {
    /// A turn with no language hint, no place and no trace.
    #[must_use]
    pub fn new(text: impl Into<String>, conversation: ConvId, now: Timestamp) -> Self {
        Self {
            text: text.into(),
            lang: None,
            conversation,
            now,
            place: None,
            trace: false,
        }
    }

    /// The same turn with a language hint.
    #[must_use]
    pub fn with_lang(mut self, lang: Option<Lang>) -> Self {
        self.lang = lang;
        self
    }

    /// The same turn with the caller's place.
    #[must_use]
    pub fn with_place(mut self, place: Option<Place>) -> Self {
        self.place = place;
        self
    }

    /// The same turn, asking for a trace in the outcome.
    #[must_use]
    pub fn with_trace(mut self) -> Self {
        self.trace = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hour_needs_an_offset() {
        // 2027-03-14T17:42:00Z
        let t = Timestamp::from_unix(1_805_046_120);
        assert_eq!(t.local_hour(), None);
        assert_eq!(t.with_utc_offset(0).local_hour(), Some(17));
        assert_eq!(t.with_utc_offset(3_600).local_hour(), Some(18));
        assert_eq!(t.with_utc_offset(-18 * 3_600).local_hour(), Some(23));
    }

    #[test]
    fn local_date_and_its_inverse() {
        let t = Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600);
        let date = t.local_date().unwrap();
        assert_eq!(date.to_string(), "2027-03-14");
        assert_eq!(t.local_seconds_of_day(), Some(18 * 3_600 + 42 * 60));
        assert_eq!(Timestamp::from_local(date, 18 * 3_600 + 42 * 60, 3_600), t);
        // 23:30 UTC on the 14th is already the 15th in Madrid.
        let late = Timestamp::from_unix(1_805_046_120 + 5 * 3_600 + 48 * 60);
        assert_eq!(late.date_or_utc().to_string(), "2027-03-14");
        assert_eq!(
            late.with_utc_offset(3_600).date_or_utc().to_string(),
            "2027-03-15"
        );
    }

    #[test]
    fn local_hour_before_the_epoch() {
        assert_eq!(
            Timestamp::from_unix(-1).with_utc_offset(0).local_hour(),
            Some(23)
        );
    }
}
