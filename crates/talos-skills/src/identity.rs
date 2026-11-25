//! `identity`: who Talos is and what it is (docs/spec/basics.md). Talos names
//! no creator beyond the configured identity line and says plainly that it
//! is a program, not a language model. The wording lives in the manifest.

use talos_core::{Call, Context, Message, Skill, SkillError, SkillResult};

/// The `identity` handler.
#[derive(Debug, Default)]
pub(crate) struct Identity;

/// Every (intent, topic) with its reply id and whether that reply is a full
/// answer. The topic is the internal slot fixed by the pattern. A topic
/// Talos has nothing to say about ("how old are you", comparisons) is
/// `Unable`.
const TOPICS: &[(&str, &str, &str, bool)] = &[
    ("identity.who", "name", "who_name", true),
    ("identity.who", "intro", "who_intro", true),
    ("identity.who", "creator", "who_creator", true),
    ("identity.who", "unknown", "who_unknown", false),
    ("identity.nature", "ai", "nature_ai", true),
    ("identity.nature", "kind", "nature_kind", true),
    ("identity.nature", "feelings", "nature_feelings", true),
    ("identity.nature", "love", "nature_love", true),
    ("identity.nature", "conscious", "nature_conscious", true),
    ("identity.nature", "llm", "nature_llm", true),
    ("identity.nature", "named", "nature_named", true),
    ("identity.nature", "how", "nature_how", true),
    ("identity.nature", "runs_on", "nature_runs_on", true),
    ("identity.nature", "understand", "nature_understand", true),
    ("identity.nature", "compare", "nature_compare", false),
];

impl Skill for Identity {
    fn id(&self) -> &'static str {
        "identity"
    }

    fn replies(&self) -> &'static [&'static str] {
        &[
            "who_name",
            "who_intro",
            "who_creator",
            "who_unknown",
            "nature_ai",
            "nature_kind",
            "nature_feelings",
            "nature_love",
            "nature_conscious",
            "nature_llm",
            "nature_named",
            "nature_how",
            "nature_runs_on",
            "nature_understand",
            "nature_compare",
        ]
    }

    fn handle(&self, _cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let intent = call.intent.as_str();
        if intent != "identity.who" && intent != "identity.nature" {
            return SkillResult::Decline;
        }
        let topic = call.slots.get("topic").and_then(|v| v.as_text());
        let Some(&(_, _, reply, answered)) = TOPICS
            .iter()
            .find(|(i, t, _, _)| *i == intent && Some(*t) == topic)
        else {
            return SkillResult::Fail(SkillError(format!(
                "`{intent}` called without a known topic ({topic:?})"
            )));
        };
        let message = Message::new(reply);
        if answered {
            SkillResult::Answer(message)
        } else {
            SkillResult::Unable(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{IntentId, Lang, Random, Slots, Timestamp, Value};

    fn handle(intent: &str, topic: Option<&str>) -> SkillResult {
        let intent = IntentId(intent.to_owned());
        let slots: Slots = topic
            .iter()
            .map(|t| ("topic".to_owned(), Value::Text((*t).to_owned())))
            .collect();
        let mut cx = Context::standalone(Timestamp::from_unix(0), Random::seeded(0));
        let call = Call {
            intent: &intent,
            lang: Lang::En,
            slots: &slots,
            text: "x",
        };
        Identity.handle(&mut cx, &call)
    }

    #[test]
    fn answers_and_declines_by_topic() {
        assert_eq!(
            handle("identity.who", Some("creator")),
            SkillResult::Answer(Message::new("who_creator"))
        );
        assert_eq!(
            handle("identity.who", Some("unknown")),
            SkillResult::Unable(Message::new("who_unknown"))
        );
        assert_eq!(
            handle("identity.nature", Some("compare")),
            SkillResult::Unable(Message::new("nature_compare"))
        );
    }

    #[test]
    fn declines_foreign_intents_and_fails_without_a_topic() {
        assert_eq!(handle("smalltalk.greet", None), SkillResult::Decline);
        assert!(matches!(handle("identity.who", None), SkillResult::Fail(_)));
        assert!(matches!(
            handle("identity.nature", Some("name")),
            SkillResult::Fail(_)
        ));
    }

    #[test]
    fn every_listed_reply_is_a_topic_and_back() {
        let listed = Identity.replies();
        assert_eq!(listed.len(), TOPICS.len());
        for (_, _, reply, _) in TOPICS {
            assert!(listed.contains(reply), "{reply}");
        }
    }
}
