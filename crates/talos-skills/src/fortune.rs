//! `fortune`: a saying drawn from a bundled list (docs/spec/basics.md).
//!
//! The lists are data files in `skills/fortune/`, one saying per line,
//! embedded at build time; the handler only chooses a line with one draw.

use talos_core::{Call, Context, IntentId, Lang, Message, Skill, SkillResult};

/// The `fortune` handler.
#[derive(Debug, Default)]
pub(crate) struct Fortune;

const ES: &str = include_str!("../../../skills/fortune/es.txt");
const EN: &str = include_str!("../../../skills/fortune/en.txt");

/// The sayings of a language: non-empty lines that are not comments.
fn sayings(lang: Lang) -> Vec<&'static str> {
    let text = match lang {
        Lang::Es => ES,
        Lang::En => EN,
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

impl Fortune {
    fn tell(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        // "otro" / "another one" repeats a fortune only right after one.
        let again = call.slots.contains_key("again");
        if again
            && cx.conversation().last_intent().map(IntentId::as_str) != Some("fortune.tell")
        {
            return SkillResult::Decline;
        }
        // Never the saying just given (docs/spec/basics.md, fortune): it is
        // left out of the draw, so one draw still decides.
        let mut list = sayings(call.lang);
        if let Some(previous) = cx.conversation().last_reply() {
            list.retain(|saying| *saying != previous);
        }
        let index = cx.random().entry(&list);
        match list.get(index) {
            Some(saying) => SkillResult::Answer(Message::new("saying").var("text", *saying)),
            None => SkillResult::Decline,
        }
    }
}

impl Skill for Fortune {
    fn id(&self) -> &'static str {
        "fortune"
    }

    fn replies(&self) -> &'static [&'static str] {
        &["saying"]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "fortune.tell" => Self::tell(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{Draw, Random, Slots, Timestamp, Value};

    fn tell(lang: Lang, slots: &Slots, random: Random) -> SkillResult {
        let intent = IntentId("fortune.tell".to_owned());
        let mut cx = Context::standalone(Timestamp::from_unix(0), random);
        let call = Call {
            intent: &intent,
            lang,
            slots,
            text: "x",
        };
        Fortune.handle(&mut cx, &call)
    }

    #[test]
    fn each_language_has_at_least_forty_distinct_sayings() {
        for lang in [Lang::Es, Lang::En] {
            let mut list = sayings(lang);
            assert!(list.len() >= 40);
            list.sort_unstable();
            list.dedup();
            assert!(list.len() >= 40);
        }
    }

    #[test]
    fn answers_the_scripted_entry() {
        let random = Random::scripted(vec![Draw::Entry("early bird".to_owned())]);
        let result = tell(Lang::En, &Slots::new(), random);
        let expected = Message::new("saying").var("text", "The early bird catches the worm.");
        assert_eq!(result, SkillResult::Answer(expected));
    }

    #[test]
    fn again_without_an_earlier_fortune_declines() {
        let slots: Slots = [("again".to_owned(), Value::Text("yes".to_owned()))]
            .into_iter()
            .collect();
        assert_eq!(
            tell(Lang::Es, &slots, Random::seeded(0)),
            SkillResult::Decline
        );
    }
}
