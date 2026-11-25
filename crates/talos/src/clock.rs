//! The current instant, read in the front end before each turn; the engine
//! never reads the clock (DESIGN.md section 3).

use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use talos_core::Timestamp;

/// The machine's current offset from UTC, by the smallest means that needs
/// neither `unsafe` nor a date/time crate: the `date` utility, which every
/// Unix has and which honours `TZ`. `None` where it is missing (Windows,
/// minimal containers); time-of-day answers then stay time-neutral.
fn local_offset() -> Option<i32> {
    let output = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_offset(std::str::from_utf8(&output.stdout).ok()?.trim())
}

/// `+0100`, `-0430`.
#[must_use]
pub fn parse_offset(text: &str) -> Option<i32> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &text[1..];
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    Some(sign * (hours * 3_600 + minutes * 60))
}

/// Seconds since the Unix epoch by the system clock.
fn unix_now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
    }
}

/// The current instant with the local offset read afresh: what `ask` and
/// `chat` use before each turn.
#[must_use]
pub fn now() -> Timestamp {
    let now = Timestamp::from_unix(unix_now());
    match local_offset() {
        Some(offset) => now.with_utc_offset(offset),
        None => now,
    }
}

/// The current instant with the local offset read at most once a minute:
/// what the server uses, where running `date` on every request would cost
/// more than the rest of the request. An offset changes at most twice a
/// year, on the hour; it is picked up within a minute.
#[derive(Debug, Default)]
pub struct LocalClock {
    /// When the offset was last read, and what it was.
    offset: Mutex<Option<(i64, Option<i32>)>>,
}

impl LocalClock {
    /// How long a read offset is trusted, in seconds.
    const REFRESH_SECONDS: i64 = 60;

    /// The current instant.
    pub fn now(&self) -> Timestamp {
        let seconds = unix_now();
        let mut cached = self.offset.lock().unwrap_or_else(PoisonError::into_inner);
        let offset = match *cached {
            Some((read_at, offset))
                if (0..Self::REFRESH_SECONDS).contains(&seconds.saturating_sub(read_at)) =>
            {
                offset
            }
            _ => {
                let offset = local_offset();
                *cached = Some((seconds, offset));
                offset
            }
        };
        let now = Timestamp::from_unix(seconds);
        match offset {
            Some(offset) => now.with_utc_offset(offset),
            None => now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_offsets_from_date() {
        assert_eq!(parse_offset("+0100"), Some(3_600));
        assert_eq!(parse_offset("-0430"), Some(-16_200));
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("CET"), None);
        assert_eq!(parse_offset("+01"), None);
    }

    #[test]
    fn the_cached_clock_agrees_with_the_fresh_one() {
        let clock = LocalClock::default();
        let (a, b) = (clock.now(), now());
        assert!((b.unix_seconds - a.unix_seconds).abs() <= 1);
        assert_eq!(a.utc_offset_seconds, b.utc_offset_seconds);
        assert_eq!(clock.now().utc_offset_seconds, a.utc_offset_seconds);
    }
}
