//! The engine: one entry point, [`Engine::respond`], and the pipeline written
//! out in order in one function (ADR 0004).

use std::collections::BTreeMap;

use crate::language::{self, Decided};
use crate::manifest::Manifest;
use crate::normalise::normalise;
use crate::render;
use crate::rules::{Candidate, Rules};
use crate::skill::{Call, Message, Registration, Skill, SkillResult};
use crate::{Error, Lang, Outcome, Reason, Slots, Status, Trace, TraceStep, Turn, fallback};

/// Settings fixed when the engine is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// The language of a turn with no hint and no decisive evidence.
    pub default_lang: Lang,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_lang: Lang::Es,
        }
    }
}

/// A skill as registered: its manifest and its handler.
struct Registered {
    manifest: Manifest,
    handler: Box<dyn Skill>,
}

/// The Talos engine. Immutable once built, so it can be shared freely.
pub struct Engine {
    config: Config,
    skills: Vec<Registered>,
    /// Skill id to position in `skills`.
    by_id: BTreeMap<String, usize>,
    rules: Rules,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("skills", &self.by_id.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// Records stage decisions when the turn asked for a trace, and costs
/// nothing otherwise.
struct Tracer(Option<Vec<TraceStep>>);

impl Tracer {
    fn note(&mut self, stage: &'static str, note: impl FnOnce() -> String) {
        if let Some(steps) = &mut self.0 {
            steps.push(TraceStep {
                stage,
                note: note(),
            });
        }
    }

    fn finish(self) -> Option<Trace> {
        self.0.map(|steps| Trace { steps })
    }
}

/// What dispatch and rendering produced for one candidate.
struct Handled {
    reply: String,
    status: Status,
    reason: Option<Reason>,
}

impl Engine {
    /// Builds an engine from skills, each a manifest with its handler.
    /// Patterns are compiled here, once.
    ///
    /// # Errors
    ///
    /// [`Error::Registry`] if a handler's id differs from its manifest's or a
    /// skill id is repeated; [`Error::Manifest`] if a pattern does not
    /// compile.
    pub fn new(config: Config, skills: Vec<Registration>) -> Result<Self, Error> {
        let mut by_id = BTreeMap::new();
        let mut registered = Vec::with_capacity(skills.len());
        for (index, (manifest, handler)) in skills.into_iter().enumerate() {
            if handler.id() != manifest.id {
                return Err(Error::Registry(format!(
                    "handler `{}` registered with manifest `{}`",
                    handler.id(),
                    manifest.id
                )));
            }
            if by_id.insert(manifest.id.clone(), index).is_some() {
                return Err(Error::Registry(format!(
                    "skill `{}` registered twice",
                    manifest.id
                )));
            }
            registered.push(Registered { manifest, handler });
        }
        let manifests: Vec<Manifest> = registered.iter().map(|r| r.manifest.clone()).collect();
        let rules = Rules::compile(&manifests)?;
        Ok(Self {
            config,
            skills: registered,
            by_id,
            rules,
        })
    }

    /// The configuration the engine was built with.
    #[must_use]
    pub fn config(&self) -> Config {
        self.config
    }

    /// Answers one turn. Never fails and never panics: whatever happens
    /// becomes an outcome with an honest status. Reads no clock and keeps no
    /// state, so the same turn always gives the same outcome.
    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the turn is an owned value handed to the pipeline (ADR 0004); \
                  later stages (dialog state) will consume it"
    )]
    pub fn respond(&self, turn: Turn) -> Outcome {
        let mut trace = Tracer(turn.trace.then(Vec::new));

        // 0. Normalise.
        let message = normalise(&turn.text);
        trace.note("normalise", || format!("{:?}", message.text));

        // 1. Language.
        let (lang, decided) = language::detect(turn.lang, &message.text, self.config.default_lang);
        trace.note("language", || match decided {
            Decided::Hint => format!("{lang}: caller's hint"),
            Decided::Markers { es, en } => format!("{lang}: markers es={es} en={en}"),
            Decided::Default { es, en } => {
                format!("{lang}: default (markers es={es} en={en})")
            }
        });

        if message.is_empty() {
            trace.note("fallback", || "empty message".to_owned());
            return unhandled(lang, Reason::Empty, trace);
        }

        // 1b. Safety check and 2. pending dialog: not in this version.

        // 3. Rules: the turn's language first, then the other (ADR 0011).
        let mut candidates = self.rules.candidates(lang, &message.text);
        if candidates.is_empty() {
            candidates = self.rules.candidates(lang.other(), &message.text);
        }
        trace.note("rules", || {
            if candidates.is_empty() {
                "no rule matched".to_owned()
            } else {
                candidates
                    .iter()
                    .map(|c| format!("{} (priority {}, /{}/)", c.intent, c.priority, c.pattern))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        });

        // 4. Classifier: not in this version.

        // 5. Dispatch and 6. render, candidate by candidate until one does
        // not decline.
        for candidate in candidates {
            let Some(handled) = self.dispatch(&turn, lang, &candidate, &mut trace) else {
                continue;
            };
            let Candidate { intent, slots, .. } = candidate;
            return Outcome {
                reply: handled.reply,
                status: handled.status,
                lang,
                intent: Some(intent),
                slots,
                confidence: 1.0,
                reason: handled.reason,
                pending: false,
                generated: false,
                trace: trace.finish(),
            };
        }

        // 7. Fallback (the reflector, from Slice D).
        trace.note("fallback", || "nothing recognised the message".to_owned());
        unhandled(lang, Reason::NotRecognised, trace)
    }

    /// Stages 5 and 6 for one candidate; `None` when its skill declines.
    fn dispatch(
        &self,
        turn: &Turn,
        lang: Lang,
        candidate: &Candidate,
        trace: &mut Tracer,
    ) -> Option<Handled> {
        let Some(&index) = self.by_id.get(candidate.intent.skill()) else {
            trace.note("dispatch", || format!("no skill owns {}", candidate.intent));
            return None;
        };
        let skill = &self.skills[index];
        let call = Call {
            intent: &candidate.intent,
            slots: &candidate.slots,
            lang,
            turn,
        };
        let result = skill.handler.handle(&call);
        trace.note("dispatch", || format!("{} -> {result:?}", candidate.intent));

        let failed = |trace: &mut Tracer, why: String| {
            trace.note("render", || why);
            Handled {
                reply: fallback::reply(lang, Reason::Failed).to_owned(),
                status: Status::Unhandled,
                reason: Some(Reason::Failed),
            }
        };
        let (message, status, reason) = match result {
            SkillResult::Decline => return None,
            SkillResult::Answer(message) => (message, Status::Answered, None),
            SkillResult::Unable(message) => (message, Status::Unhandled, Some(Reason::Unable)),
            SkillResult::Ask(..) => {
                return Some(Handled {
                    reply: fallback::reply(lang, Reason::CannotAsk).to_owned(),
                    status: Status::Unhandled,
                    reason: Some(Reason::CannotAsk),
                });
            }
            SkillResult::Fail(error) => return Some(failed(trace, error.0)),
        };
        match self.render(index, lang, turn, &message) {
            Ok(reply) => {
                trace.note("render", || format!("reply `{}`", message.id));
                Some(Handled {
                    reply,
                    status,
                    reason,
                })
            }
            Err(error) => Some(failed(trace, error.to_string())),
        }
    }

    /// Stage 6: looks the message up in its skill's manifest, picks the
    /// variant for this turn and fills it in.
    fn render(
        &self,
        skill: usize,
        lang: Lang,
        turn: &Turn,
        message: &Message,
    ) -> Result<String, Error> {
        let manifest = &self.skills[skill].manifest;
        let variants = manifest
            .replies
            .get(&message.id)
            .ok_or_else(|| {
                Error::Render(format!(
                    "skill `{}` has no reply `{}`",
                    manifest.id, message.id
                ))
            })?
            .get(lang);
        let chosen = render::variant(&turn.conversation, turn.number, variants.len());
        render::fill(&variants[chosen], &message.vars)
    }
}

/// An `Unhandled` outcome with the fixed reply for its reason.
fn unhandled(lang: Lang, reason: Reason, trace: Tracer) -> Outcome {
    Outcome {
        reply: fallback::reply(lang, reason).to_owned(),
        status: Status::Unhandled,
        lang,
        intent: None,
        slots: Slots::new(),
        confidence: 0.0,
        reason: Some(reason),
        pending: false,
        generated: false,
        trace: trace.finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{Expectation, SkillError};
    use crate::{ConvId, IntentId, Timestamp};

    /// A skill whose behaviour per intent is fixed by the test.
    struct Fixed;

    impl Skill for Fixed {
        fn id(&self) -> &'static str {
            "t"
        }

        fn handle(&self, call: &Call<'_>) -> SkillResult {
            match call.intent.as_str() {
                "t.decline" => SkillResult::Decline,
                "t.answer" => SkillResult::Answer(Message::new("echo").var("x", "uno")),
                "t.unable" => SkillResult::Unable(Message::new("echo").var("x", "dos")),
                "t.ask" => SkillResult::Ask(
                    Expectation {
                        slot: "x".to_owned(),
                    },
                    Message::new("echo"),
                ),
                "t.fail" => SkillResult::Fail(SkillError("boom".to_owned())),
                _ => SkillResult::Answer(Message::new("no_such_reply")),
            }
        }
    }

    const MANIFEST: &str = r#"
[skill]
id = "t"
summary.es = "prueba"
summary.en = "test"

[[intent]]
id = "t.decline"
priority = 90
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^(declina|responde)$' }]

[[intent]]
id = "t.answer"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^(declina|responde)$' }, { lang = "en", regex = '^answer$' }]

[[intent]]
id = "t.unable"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^no puedes$' }]

[[intent]]
id = "t.ask"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^pregunta$' }]

[[intent]]
id = "t.fail"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^falla$' }]

[[intent]]
id = "t.broken"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^rota$' }]

[reply.echo]
es = ["Eco {x}."]
en = ["Echo {x}."]
"#;

    fn engine() -> Engine {
        let manifest = Manifest::parse("t", MANIFEST).unwrap();
        Engine::new(Config::default(), vec![(manifest, Box::new(Fixed))]).unwrap()
    }

    fn respond(text: &str, lang: Option<Lang>) -> Outcome {
        let turn = Turn::new(text, ConvId::new("c"), 1, Timestamp::from_unix(0)).with_lang(lang);
        engine().respond(turn)
    }

    fn intent(outcome: &Outcome) -> Option<&str> {
        outcome.intent.as_ref().map(IntentId::as_str)
    }

    #[test]
    fn a_declining_skill_passes_to_the_next_candidate() {
        let outcome = respond("declina", None);
        assert_eq!(outcome.reply, "Eco uno.");
        assert_eq!(outcome.status, Status::Answered);
        assert_eq!(intent(&outcome), Some("t.answer"));
    }

    #[test]
    fn the_reply_is_in_the_turns_language() {
        let outcome = respond("responde", Some(Lang::En));
        assert_eq!(outcome.reply, "Echo uno.");
        assert_eq!(outcome.lang, Lang::En);
        // Matched by the other language's rules (ADR 0011).
        let outcome = respond("answer", Some(Lang::Es));
        assert_eq!(outcome.reply, "Eco uno.");
    }

    #[test]
    fn unable_is_rendered_and_unhandled_with_the_intent_kept() {
        let outcome = respond("no puedes", None);
        assert_eq!(outcome.reply, "Eco dos.");
        assert_eq!(outcome.status, Status::Unhandled);
        assert_eq!(outcome.reason, Some(Reason::Unable));
        assert_eq!(intent(&outcome), Some("t.unable"));
    }

    #[test]
    fn ask_fail_and_a_missing_reply_do_not_panic() {
        for (text, reason) in [
            ("pregunta", Reason::CannotAsk),
            ("falla", Reason::Failed),
            ("rota", Reason::Failed),
        ] {
            let outcome = respond(text, None);
            assert_eq!(outcome.status, Status::Unhandled, "{text}");
            assert_eq!(outcome.reason, Some(reason), "{text}");
            assert!(!outcome.pending);
            assert!(intent(&outcome).is_some());
            assert_ne!(outcome.reply, "");
        }
    }

    #[test]
    fn construction_checks_the_registry() {
        let manifest = Manifest::parse("t", MANIFEST).unwrap();
        let twice: Vec<Registration> = vec![
            (manifest.clone(), Box::new(Fixed)),
            (manifest.clone(), Box::new(Fixed)),
        ];
        assert!(matches!(
            Engine::new(Config::default(), twice),
            Err(Error::Registry(_))
        ));

        let mut renamed = manifest;
        renamed.id = "u".to_owned();
        let mismatched: Vec<Registration> = vec![(renamed, Box::new(Fixed))];
        assert!(matches!(
            Engine::new(Config::default(), mismatched),
            Err(Error::Registry(_))
        ));
    }
}
