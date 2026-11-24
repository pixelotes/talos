//! The turn: one user message and everything the engine needs to answer it
//! (DESIGN.md section 3). Context travels in this value and nowhere else.

use crate::Lang;

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

    /// The hour (0–23) on the local wall clock, if the offset is known.
    #[must_use]
    pub fn local_hour(self) -> Option<u8> {
        let offset = i64::from(self.utc_offset_seconds?);
        let seconds_of_day = (self.unix_seconds + offset).rem_euclid(86_400);
        u8::try_from(seconds_of_day / 3_600).ok()
    }
}

/// One user message, with its context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// The message exactly as received.
    pub text: String,
    /// The caller's language hint; detected when absent.
    pub lang: Option<Lang>,
    /// Which dialog this turn belongs to.
    pub conversation: ConvId,
    /// Position of this turn in its conversation, starting at 1. Supplied by
    /// the caller until a per-conversation state holder owns the counter.
    pub number: u32,
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
    pub fn new(text: impl Into<String>, conversation: ConvId, number: u32, now: Timestamp) -> Self {
        Self {
            text: text.into(),
            lang: None,
            conversation,
            number,
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
    fn local_hour_before_the_epoch() {
        assert_eq!(
            Timestamp::from_unix(-1).with_utc_offset(0).local_hour(),
            Some(23)
        );
    }
}
