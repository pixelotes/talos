//! `smalltalk`: greetings, farewells, thanks and the "how are you" ritual
//! (docs/spec/basics.md).

use talos_core::{Call, Context, Lang, Message, Skill, SkillError, SkillResult};

/// The `smalltalk` handler.
#[derive(Debug, Default)]
pub(crate) struct Smalltalk;

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
    fn greet(cx: &Context<'_>, call: &Call<'_>) -> SkillResult {
        if let Some(said) = call.slots.get("daypart").and_then(|v| v.as_text()) {
            let Some(daypart) = Daypart::from_slot(said) else {
                return SkillResult::Fail(SkillError(format!("unknown daypart `{said}`")));
            };
            let agrees = cx
                .now()
                .local_hour()
                .is_some_and(|hour| daypart.agrees(call.lang, hour));
            let id = if agrees { daypart.reply_id() } else { "hello" };
            return SkillResult::Answer(Message::new(id));
        }
        if call.slots.get("register").and_then(|v| v.as_text()) == Some("casual") {
            return SkillResult::Answer(Message::new("casual"));
        }
        SkillResult::Answer(Message::new("hello"))
    }
}

impl Smalltalk {
    /// Answers `special` when the internal slot `slot` holds `value`, and
    /// `default` otherwise.
    fn pick(
        call: &Call<'_>,
        slot: &str,
        value: &str,
        special: &'static str,
        default: &'static str,
    ) -> SkillResult {
        let id = if call.slots.get(slot).and_then(|v| v.as_text()) == Some(value) {
            special
        } else {
            default
        };
        SkillResult::Answer(Message::new(id))
    }
}

impl Skill for Smalltalk {
    fn id(&self) -> &'static str {
        "smalltalk"
    }

    fn replies(&self) -> &'static [&'static str] {
        &[
            "hello",
            "casual",
            "morning",
            "afternoon",
            "evening",
            "bye",
            "night",
            "thanks",
            "status",
            "feelings",
        ]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "smalltalk.greet" => Self::greet(cx, call),
            "smalltalk.bye" => Self::pick(call, "kind", "night", "night", "bye"),
            "smalltalk.thanks" => SkillResult::Answer(Message::new("thanks")),
            "smalltalk.how_are_you" => {
                Self::pick(call, "topic", "feelings", "feelings", "status")
            }
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{IntentId, Random, Slots, Timestamp, Value};

    /// 2027-03-14T17:42:00Z, a Sunday.
    const T: i64 = 1_805_046_120;

    fn handle(intent: &str, lang: Lang, slots: &[(&str, &str)], now: Timestamp) -> SkillResult {
        let intent = IntentId(intent.to_owned());
        let slots: Slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::Text((*v).to_owned())))
            .collect();
        let mut cx = Context::standalone(now, Random::seeded(0));
        let call = Call {
            intent: &intent,
            lang,
            slots: &slots,
            text: "x",
        };
        Smalltalk.handle(&mut cx, &call)
    }

    fn greet(lang: Lang, slots: &[(&str, &str)], now: Timestamp) -> SkillResult {
        handle("smalltalk.greet", lang, slots, now)
    }

    fn answer(id: &'static str) -> SkillResult {
        SkillResult::Answer(Message::new(id))
    }

    #[test]
    fn echoes_the_time_of_day_only_when_the_clock_agrees() {
        let madrid = Timestamp::from_unix(T).with_utc_offset(3_600); // 18:42
        assert_eq!(
            greet(Lang::Es, &[("daypart", "evening")], madrid),
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
        // Without a local clock: a plain hello.
        let utc_only = Timestamp::from_unix(T);
        assert_eq!(
            greet(Lang::Es, &[("daypart", "afternoon")], utc_only),
            answer("hello")
        );
    }

    #[test]
    fn declines_intents_it_does_not_own_and_fails_on_bad_slots() {
        let now = Timestamp::from_unix(T);
        assert_eq!(handle("smalltalk.nope", Lang::Es, &[], now), SkillResult::Decline);
        assert!(matches!(
            greet(Lang::Es, &[("daypart", "noon")], now),
            SkillResult::Fail(_)
        ));
    }

    #[test]
    fn farewells_thanks_and_status_pick_their_reply_from_internal_slots() {
        let now = Timestamp::from_unix(T);
        let ask = |intent, slots: &[(&str, &str)]| handle(intent, Lang::En, slots, now);
        assert_eq!(ask("smalltalk.bye", &[]), answer("bye"));
        assert_eq!(ask("smalltalk.bye", &[("kind", "night")]), answer("night"));
        assert_eq!(ask("smalltalk.thanks", &[]), answer("thanks"));
        assert_eq!(ask("smalltalk.how_are_you", &[]), answer("status"));
        assert_eq!(
            ask("smalltalk.how_are_you", &[("topic", "feelings")]),
            answer("feelings")
        );
    }
}
