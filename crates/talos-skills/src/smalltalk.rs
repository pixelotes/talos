//! `smalltalk`: greetings (docs/spec/basics.md). Slice A: `smalltalk.greet`.

use talos_core::{Call, Lang, Message, Skill, SkillError, SkillResult};

/// The `smalltalk` handler.
#[derive(Debug, Default)]
pub struct Smalltalk;

/// The part of the day a greeting names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Daypart {
    Morning,
    Afternoon,
    Evening,
}

impl Daypart {
    fn from_slot(value: &str) -> Option<Self> {
        match value {
            "morning" => Some(Self::Morning),
            "afternoon" => Some(Self::Afternoon),
            "evening" => Some(Self::Evening),
            _ => None,
        }
    }

    /// The reply that echoes this greeting.
    fn reply_id(self) -> &'static str {
        match self {
            Self::Morning => "morning",
            Self::Afternoon => "afternoon",
            Self::Evening => "evening",
        }
    }

    /// Whether greeting with this part of the day is right at `hour` (local,
    /// 0–23) by the conventions of `lang`. Spanish "buenas tardes" runs from
    /// lunch until night, covering the English afternoon and evening.
    fn agrees(self, lang: Lang, hour: u8) -> bool {
        match (lang, self) {
            (Lang::Es, Self::Morning) => (6..14).contains(&hour),
            (Lang::Es, Self::Afternoon | Self::Evening) => (14..21).contains(&hour),
            (Lang::En, Self::Morning) => (5..12).contains(&hour),
            (Lang::En, Self::Afternoon) => (12..18).contains(&hour),
            (Lang::En, Self::Evening) => hour >= 18,
        }
    }
}

impl Smalltalk {
    /// Greets back in kind. A time-of-day greeting is echoed only when the
    /// local clock is known and agrees with it; otherwise a plain hello,
    /// which is never wrong ("never contradicts the clock").
    fn greet(call: &Call<'_>) -> SkillResult {
        if let Some(said) = call.slots.get("daypart") {
            let Some(daypart) = Daypart::from_slot(said) else {
                return SkillResult::Fail(SkillError(format!("unknown daypart `{said}`")));
            };
            let agrees = call
                .turn
                .now
                .local_hour()
                .is_some_and(|hour| daypart.agrees(call.lang, hour));
            let id = if agrees { daypart.reply_id() } else { "hello" };
            return SkillResult::Answer(Message::new(id));
        }
        if call.slots.get("register").is_some_and(|r| r == "casual") {
            return SkillResult::Answer(Message::new("casual"));
        }
        SkillResult::Answer(Message::new("hello"))
    }
}

impl Skill for Smalltalk {
    fn id(&self) -> &'static str {
        "smalltalk"
    }

    fn handle(&self, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "smalltalk.greet" => Self::greet(call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{ConvId, IntentId, Slots, Timestamp, Turn};

    /// 2027-03-14T17:42:00Z, a Sunday.
    const T: i64 = 1_805_046_120;

    fn greet(lang: Lang, slots: &[(&str, &str)], now: Timestamp) -> SkillResult {
        let intent = IntentId("smalltalk.greet".to_owned());
        let slots: Slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let turn = Turn::new("x", ConvId::new("t"), 1, now);
        Smalltalk.handle(&Call {
            intent: &intent,
            slots: &slots,
            lang,
            turn: &turn,
        })
    }

    fn answer(id: &str) -> SkillResult {
        SkillResult::Answer(Message::new(id))
    }

    #[test]
    fn plain_and_casual() {
        let now = Timestamp::from_unix(T);
        assert_eq!(greet(Lang::Es, &[], now), answer("hello"));
        assert_eq!(
            greet(Lang::En, &[("register", "casual")], now),
            answer("casual")
        );
    }

    #[test]
    fn echoes_the_time_of_day_only_when_the_clock_agrees() {
        let madrid = Timestamp::from_unix(T).with_utc_offset(3_600); // 18:42
        assert_eq!(
            greet(Lang::Es, &[("daypart", "afternoon")], madrid),
            answer("afternoon")
        );
        assert_eq!(
            greet(Lang::Es, &[("daypart", "evening")], madrid),
            answer("evening")
        );
        assert_eq!(
            greet(Lang::Es, &[("daypart", "morning")], madrid),
            answer("hello")
        );
        assert_eq!(
            greet(Lang::En, &[("daypart", "evening")], madrid),
            answer("evening")
        );
        assert_eq!(
            greet(Lang::En, &[("daypart", "afternoon")], madrid),
            answer("hello")
        );
        let morning = Timestamp::from_unix(T - 9 * 3_600).with_utc_offset(3_600); // 09:42
        assert_eq!(
            greet(Lang::En, &[("daypart", "morning")], morning),
            answer("morning")
        );
    }

    #[test]
    fn without_a_local_clock_says_plain_hello() {
        let utc_only = Timestamp::from_unix(T);
        assert_eq!(
            greet(Lang::Es, &[("daypart", "afternoon")], utc_only),
            answer("hello")
        );
    }

    #[test]
    fn declines_intents_it_does_not_own_and_fails_on_bad_slots() {
        let intent = IntentId("smalltalk.bye".to_owned());
        let slots = Slots::new();
        let turn = Turn::new("x", ConvId::new("t"), 1, Timestamp::from_unix(T));
        let call = Call {
            intent: &intent,
            slots: &slots,
            lang: Lang::Es,
            turn: &turn,
        };
        assert_eq!(Smalltalk.handle(&call), SkillResult::Decline);
        assert!(matches!(
            greet(Lang::Es, &[("daypart", "noon")], Timestamp::from_unix(T)),
            SkillResult::Fail(_)
        ));
    }
}
