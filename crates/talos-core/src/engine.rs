//! The engine: one entry point, [`Engine::respond`], and the pipeline written
//! out in order in one function, [`Engine::run`] (ADR 0004).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::extract;
use crate::language::{self, Decided, Evidence};
use crate::manifest::{Intent, Manifest};
use crate::normalise::{Normalised, normalise};
use crate::random::{Draw, Random, ScriptReport, turn_seed};
use crate::render;
use crate::rules::{Candidate, Rules};
use crate::skill::{Call, Context, Message, Registration, Skill, SkillResult};
use crate::state::{ConversationState, Conversations, lock_state};
use crate::value::{SlotType, Slots, Value};
use crate::{
    ConvId, Error, IntentId, Lang, Outcome, Reason, Status, Trace, TraceStep, Turn, fallback,
};

/// Settings fixed when the engine is built (DESIGN.md section 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// The language of a fresh conversation's turn with no hint and no
    /// decisive evidence.
    pub default_lang: Lang,
    /// Added to every turn's random seed. Fixed in tests; with it fixed, a
    /// replay is exact (docs/spec/conversation.md 5.3).
    pub seed: u64,
    /// The most conversations whose state is held at once; the least
    /// recently seen is forgotten first.
    pub max_conversations: usize,
    /// A conversation idle for longer than this, measured with the turns'
    /// injected `now`, starts afresh.
    pub idle_expiry_seconds: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_lang: Lang::Es,
            seed: 0,
            max_conversations: 1_024,
            idle_expiry_seconds: 30 * 60,
        }
    }
}

/// The Talos engine. Built once; shareable across threads (DESIGN.md
/// section 7): the manifests, rules and handlers are immutable, and each
/// conversation's state has its own lock, so turns of one conversation are
/// serialised and turns of different ones run independently.
pub struct Engine {
    config: Config,
    /// Manifests in registration (catalogue) order.
    manifests: Vec<Manifest>,
    /// Handlers, parallel to `manifests`.
    handlers: Vec<Box<dyn Skill>>,
    /// Skill id to position.
    by_id: BTreeMap<String, usize>,
    rules: Rules,
    conversations: Conversations,
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
/// nothing otherwise: notes are closures, never called when off.
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

    fn finish(self, internal_slots: Slots) -> Option<Trace> {
        self.0.map(|steps| Trace {
            steps,
            internal_slots,
        })
    }
}

/// What dispatch and rendering produced.
struct Handled {
    reply: String,
    status: Status,
    reason: Option<Reason>,
    intent: Option<IntentId>,
    slots: Slots,
    internal: Slots,
}

impl Handled {
    /// An `Unhandled` result with the fixed reply for its reason.
    fn fallback(lang: Lang, reason: Reason, intent: Option<IntentId>) -> Self {
        Self {
            reply: fallback::reply(lang, &reason).to_owned(),
            status: Status::Unhandled,
            reason: Some(reason),
            intent,
            slots: Slots::new(),
            internal: Slots::new(),
        }
    }
}

/// The trace's account of the language decision.
fn describe_language(lang: Lang, decided: Decided) -> String {
    let counts = |e: Evidence| format!("marker words es={} en={}", e.es, e.en);
    match decided {
        Decided::Hint => format!("{lang}: the caller's hint"),
        Decided::Evidence(e) => format!("{lang}: decided by {}", counts(e)),
        Decided::Switched(e) => format!(
            "{lang}: switched from {} on clear evidence ({})",
            lang.other(),
            counts(e)
        ),
        Decided::Kept(e) => format!(
            "{lang}: kept; {} (a switch needs {} of the other language, and more than of this)",
            counts(e),
            language::SWITCH_WORDS
        ),
        Decided::Default(e) => format!("{lang}: default, no decisive evidence ({})", counts(e)),
    }
}

/// The trace's account of a handler's result.
fn describe_result(result: &SkillResult) -> String {
    match result {
        SkillResult::Answer(m) => format!("answer `{}`", m.id),
        SkillResult::Ask(e, m) => format!("ask for `{}` with `{}`", e.slot, m.id),
        SkillResult::Unable(m) => format!("unable, `{}`", m.id),
        SkillResult::Decline => "decline".to_owned(),
        SkillResult::Fail(e) => format!("fail: {}", e.0),
    }
}

fn describe_slots(slots: &Slots) -> String {
    slots
        .iter()
        .map(|(name, value)| match value {
            Value::Invalid { raw, problem } => format!("{name}={raw:?} ({problem:?})"),
            _ => format!("{name}={value}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Engine {
    /// Builds an engine from skills, each a manifest with its handler, in
    /// catalogue order. Patterns are compiled here, once.
    ///
    /// # Errors
    ///
    /// [`Error::Registry`] if a handler's id differs from its manifest's or a
    /// skill id is repeated; [`Error::Manifest`] if a pattern does not
    /// compile or captures an undeclared slot.
    pub fn new(config: Config, skills: Vec<Registration>) -> Result<Self, Error> {
        let mut by_id = BTreeMap::new();
        let mut manifests = Vec::with_capacity(skills.len());
        let mut handlers = Vec::with_capacity(skills.len());
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
            manifests.push(manifest);
            handlers.push(handler);
        }
        let rules = Rules::compile(&manifests)?;
        Ok(Self {
            conversations: Conversations::new(config.max_conversations, config.idle_expiry_seconds),
            config,
            manifests,
            handlers,
            by_id,
            rules,
        })
    }

    /// The configuration the engine was built with.
    #[must_use]
    pub fn config(&self) -> Config {
        self.config
    }

    /// The installed skills' manifests, in catalogue order.
    #[must_use]
    pub fn skills(&self) -> &[Manifest] {
        &self.manifests
    }

    /// The compiled rules, for `talos skills check`.
    #[must_use]
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Forgets a conversation's state; its next turn starts afresh.
    pub fn end(&self, conversation: &ConvId) {
        self.conversations.end(conversation);
    }

    /// Answers one turn. Never fails and never panics: whatever happens
    /// becomes an outcome with an honest status. Reads no clock; the same
    /// turn in the same conversation state always gives the same outcome.
    #[must_use]
    pub fn respond(&self, turn: Turn) -> Outcome {
        self.run(turn, None).0
    }

    /// Answers one turn with its random draws fixed by a script instead of
    /// the seed: golden files' `Given: draw = …`. The report says whether
    /// the turn drew exactly what was scripted.
    #[must_use]
    pub fn respond_scripted(&self, turn: Turn, draws: Vec<Draw>) -> (Outcome, ScriptReport) {
        let (outcome, report) = self.run(turn, Some(draws));
        (outcome, report.unwrap_or_default())
    }

    /// The pipeline, stage by stage, in order (DESIGN.md section 4).
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the turn is an owned value handed to the pipeline (ADR 0004)"
    )]
    fn run(&self, turn: Turn, script: Option<Vec<Draw>>) -> (Outcome, Option<ScriptReport>) {
        let mut trace = Tracer(turn.trace.then(Vec::new));
        let entry = self.conversations.enter(&turn.conversation, turn.now);
        let mut state = lock_state(&entry);
        let number = state.begin_turn();

        // 0. Normalise.
        let message = normalise(&turn.text);
        trace.note("normalise", || {
            if message.folded == message.text {
                format!("{:?}", message.text)
            } else {
                format!("{:?}, match form {:?}", message.text, message.folded)
            }
        });

        // 1. Language, sticky per conversation.
        let (lang, decided) = language::decide(
            turn.lang,
            &message.text,
            state.lang_hold(),
            self.config.default_lang,
        );
        state.set_lang(lang, decided.hold());
        trace.note("language", || describe_language(lang, decided));

        let random = match script {
            Some(draws) => Random::scripted(draws),
            None => Random::seeded(turn_seed(
                self.config.seed,
                turn.conversation.as_str(),
                number,
                &message.text,
            )),
        };
        let mut cx = Context::new(
            turn.now,
            turn.place.as_ref(),
            random,
            &state,
            &self.manifests,
        );

        let handled = if message.is_empty() {
            trace.note("fallback", || "empty message".to_owned());
            Handled::fallback(lang, Reason::EMPTY, None)
        } else {
            // 1b. Safety check: Slice D. 2. Pending dialog: v0.2.

            // 3. Rules.
            let candidates = self.recognise(lang, &message, &mut trace);

            // 4. Classifier: v0.2.

            // 5. Dispatch and 6. render, candidate by candidate until one
            // does not decline.
            let mut handled = None;
            for candidate in &candidates {
                handled = self.dispatch(
                    &mut cx, &message, lang, candidate, number, &turn, &mut trace,
                );
                if handled.is_some() {
                    break;
                }
            }

            // 7. Fallback (the reflector, from Slice D).
            handled.unwrap_or_else(|| {
                trace.note("fallback", || "nothing recognised the message".to_owned());
                Handled::fallback(lang, Reason::CATCH_ALL, None)
            })
        };
        let (_, random) = cx.finish();
        trace.note("random", || format!("{} draws", random.draws()));

        state.set_last_intent(handled.intent.clone());
        trace.note("state", || describe_state(&turn.conversation, &state));
        let outcome = Outcome {
            reply: handled.reply,
            status: handled.status,
            lang,
            confidence: if handled.intent.is_some() { 1.0 } else { 0.0 },
            intent: handled.intent,
            slots: handled.slots,
            reason: handled.reason,
            pending: false,
            generated: false,
            trace: trace.finish(handled.internal),
        };
        (outcome, random.into_report())
    }

    /// Stage 3: the rules of the turn's language, then the other's; failing
    /// both, the same after dropping a courtesy ("hola, …").
    fn recognise(&self, lang: Lang, message: &Normalised, trace: &mut Tracer) -> Vec<Candidate> {
        let text = &message.folded;
        let candidates = self.rules.candidates_any(lang, text, 0);
        if !candidates.is_empty() {
            trace.note("rules", || describe_candidates(lang, &candidates));
            return candidates;
        }
        if let Some((courtesy, candidates)) = self.rules.without_courtesy(lang, text) {
            trace.note("rules", || {
                format!(
                    "no rule matched the whole message; dropped courtesy {:?} ({}); {}",
                    message.text_of(courtesy.dropped.clone()),
                    courtesy.intent,
                    describe_candidates(lang, &candidates)
                )
            });
            return candidates;
        }
        trace.note("rules", || {
            format!(
                "no rule matched ({} {lang} and {} {} patterns tried)",
                self.rules.len(lang),
                self.rules.len(lang.other()),
                lang.other()
            )
        });
        Vec::new()
    }

    /// Typed slots for a candidate: fixed values from its pattern, then each
    /// capture through its slot type's extractor.
    fn extract(
        intent: &Intent,
        candidate: &Candidate,
        message: &Normalised,
        lang: Lang,
        cx: &Context<'_>,
    ) -> Slots {
        let mut slots: Slots = candidate
            .fixed
            .iter()
            .map(|(name, value)| (name.clone(), Value::Text(value.clone())))
            .collect();
        for (name, range) in &candidate.captures {
            let Some(decl) = intent.slot(name) else {
                continue;
            };
            let original = message.original_of(range.clone());
            let folded = &message.folded[range.clone()];
            if original.trim().is_empty() {
                continue;
            }
            let invalid = |problem| Value::Invalid {
                raw: original.trim().to_owned(),
                problem,
            };
            let value = match decl.kind {
                SlotType::Term if decl.repeated => extract::list(original),
                SlotType::Term => extract::term(original),
                SlotType::Number => {
                    extract::number(folded, lang).map_or_else(invalid, Value::Number)
                }
                SlotType::Date => {
                    extract::date(folded, lang, cx.now().date_or_utc()).unwrap_or_else(invalid)
                }
            };
            slots.insert(name.clone(), value);
        }
        slots
    }

    /// Stages 5 and 6 for one candidate; `None` when its skill declines.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pipeline passes each stage what it reads, explicitly (ADR 0004)"
    )]
    fn dispatch(
        &self,
        cx: &mut Context<'_>,
        message: &Normalised,
        lang: Lang,
        candidate: &Candidate,
        number: u32,
        turn: &Turn,
        trace: &mut Tracer,
    ) -> Option<Handled> {
        let intent_id = &candidate.intent;
        let Some(&index) = self.by_id.get(intent_id.skill()) else {
            trace.note("dispatch", || format!("no skill owns {intent_id}"));
            return None;
        };
        let manifest = &self.manifests[index];
        let handler = &self.handlers[index];
        let intent = manifest.intents.iter().find(|i| &i.id == intent_id)?;
        let slots = Self::extract(intent, candidate, message, lang, cx);
        trace.note("slots", || {
            if slots.is_empty() {
                "none".to_owned()
            } else {
                describe_slots(&slots)
            }
        });

        let call = Call {
            intent: intent_id,
            lang,
            slots: &slots,
            text: &message.original,
        };
        let result = handler.handle(cx, &call);
        let reported = cx.take_reported();
        trace.note("dispatch", || {
            format!("{intent_id} → {}", describe_result(&result))
        });

        let failed = |trace: &mut Tracer, why: String| {
            trace.note("render", || why);
            Some(Handled::fallback(
                lang,
                Reason::FAILED,
                Some(intent_id.clone()),
            ))
        };
        let (message, status, reason) = match result {
            SkillResult::Decline => return None,
            SkillResult::Answer(message) => (message, Status::Answered, None),
            SkillResult::Unable(message) => (message, Status::Unhandled, Some(Reason::CANNOT)),
            SkillResult::Ask(..) => {
                return Some(Handled::fallback(
                    lang,
                    Reason::CANNOT_ASK,
                    Some(intent_id.clone()),
                ));
            }
            SkillResult::Fail(error) => return failed(trace, error.0),
        };
        if !handler.replies().contains(&message.id) {
            return failed(
                trace,
                format!(
                    "reply `{}` is not among the handler's declared replies",
                    message.id
                ),
            );
        }
        if let Some(name) = reported
            .keys()
            .find(|name| intent.slot(name).is_none_or(|s| s.internal))
        {
            return failed(
                trace,
                format!("reported slot `{name}` is not a declared, public slot of {intent_id}"),
            );
        }
        let (mut public, internal): (Slots, Slots) = slots
            .into_iter()
            .partition(|(name, _)| intent.slot(name).is_some_and(|s| !s.internal));
        public.extend(reported);
        match Self::render(manifest, lang, turn, number, &message, trace) {
            Ok(reply) => Some(Handled {
                reply,
                status,
                reason,
                intent: Some(intent_id.clone()),
                slots: public,
                internal,
            }),
            Err(error) => failed(trace, error.to_string()),
        }
    }

    /// Stage 6: looks the message up in its skill's manifest, picks the
    /// variant for this conversation and turn number, and fills it in.
    fn render(
        manifest: &Manifest,
        lang: Lang,
        turn: &Turn,
        number: u32,
        message: &Message,
        trace: &mut Tracer,
    ) -> Result<String, Error> {
        let variants = manifest
            .replies
            .get(message.id)
            .ok_or_else(|| {
                Error::Render(format!(
                    "skill `{}` has no reply `{}`",
                    manifest.id, message.id
                ))
            })?
            .get(lang);
        let chosen = render::variant(&turn.conversation, number, variants.len());
        let reply = render::fill(&variants[chosen], &message.vars)?;
        trace.note("render", || {
            let mut note = format!(
                "reply `{}`, variant {} of {} (turn {number})",
                message.id,
                chosen + 1,
                variants.len()
            );
            if !message.vars.is_empty() {
                let vars: Vec<String> = message
                    .vars
                    .iter()
                    .map(|(k, v)| format!("{k}={v:?}"))
                    .collect();
                let _ = write!(note, "; {}", vars.join(" "));
            }
            note
        });
        Ok(reply)
    }
}

fn describe_candidates(lang: Lang, candidates: &[Candidate]) -> String {
    let list = candidates
        .iter()
        .map(|c| {
            let other = if c.lang == lang {
                String::new()
            } else {
                format!(", {} rules", c.lang)
            };
            format!(
                "{} (priority {}{other}, /{}/)",
                c.intent, c.priority, c.pattern
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("matched {list}")
}

fn describe_state(conversation: &ConvId, state: &ConversationState) -> String {
    let lang = state.lang_hold().map_or_else(
        || "none".to_owned(),
        |(lang, hold)| format!("{lang} ({hold:?})").to_lowercase(),
    );
    let topic = state
        .last_intent()
        .map_or_else(|| "none".to_owned(), ToString::to_string);
    format!(
        "conversation {:?}: turn {}, language {lang}, recent topic {topic}",
        conversation.as_str(),
        state.turn()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{Expectation, SkillError};
    use crate::{Number, Timestamp};

    /// A skill whose behaviour per intent is fixed by the test.
    struct Fixed;

    impl Skill for Fixed {
        fn id(&self) -> &'static str {
            "t"
        }

        fn replies(&self) -> &'static [&'static str] {
            &["echo", "count", "no_such_reply"]
        }

        fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
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
                "t.roll" => {
                    let n = call.slots.get("n").and_then(Value::as_number);
                    let n = n.and_then(Number::to_i64).unwrap_or(1);
                    cx.report("n", Value::Number(Number::from_int(n)));
                    let face = cx.random().int(1, 6);
                    SkillResult::Answer(Message::new("count").var("n", face.to_string()))
                }
                "t.undeclared" => SkillResult::Answer(Message::new("undeclared")),
                "t.bad_report" => {
                    cx.report("mode", Value::Text("x".to_owned()));
                    SkillResult::Answer(Message::new("echo").var("x", "y"))
                }
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
courtesy = true
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

[[intent]]
id = "t.undeclared"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^indeclarada$' }]

[[intent]]
id = "t.bad_report"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "mode", type = "term", internal = true }]
pattern = [{ lang = "es", regex = '^informa$' }]

[[intent]]
id = "t.roll"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "n", type = "number" }, { name = "mode", type = "term", internal = true }]
pattern = [{ lang = "es", regex = '^tira(?: (?P<n>.+))?$', slots = { mode = "dice" } }]

[reply.echo]
es = ["Eco {x}."]
en = ["Echo {x}."]

[reply.count]
es = ["Sale {n}.", "Ha salido {n}."]
en = ["{n}.", "Rolled {n}."]
"#;

    fn engine() -> Engine {
        let manifest = Manifest::parse("t", MANIFEST).unwrap();
        Engine::new(Config::default(), vec![(manifest, Box::new(Fixed))]).unwrap()
    }

    fn turn(text: &str, lang: Option<Lang>) -> Turn {
        Turn::new(text, ConvId::new("c"), Timestamp::from_unix(0)).with_lang(lang)
    }

    fn respond(text: &str, lang: Option<Lang>) -> Outcome {
        engine().respond(turn(text, lang))
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
        assert_eq!(outcome.reason, Some(Reason::CANNOT));
        assert_eq!(intent(&outcome), Some("t.unable"));
        assert_eq!(outcome.line(None), "unhandled · t.unable");
    }

    #[test]
    fn ask_fail_and_handler_mistakes_do_not_panic() {
        for (text, reason) in [
            ("pregunta", Reason::CANNOT_ASK),
            ("falla", Reason::FAILED),
            ("rota", Reason::FAILED),
            ("indeclarada", Reason::FAILED),
            ("informa", Reason::FAILED),
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
    fn slots_are_typed_reported_and_internal_ones_kept_apart() {
        let e = engine();
        let outcome = e.respond(turn("tira tres", None).with_trace());
        assert_eq!(outcome.line(None), "answered · t.roll · n=3");
        let trace = outcome.trace.unwrap();
        assert_eq!(trace.internal_slots["mode"], Value::Text("dice".to_owned()));
        assert_eq!(outcome.slots.len(), 1);
        // A default the handler applied is reported on the outcome.
        let outcome = e.respond(turn("tira", None));
        assert_eq!(outcome.line(None), "answered · t.roll · n=1");
        // A capture that is not a number stays visible as what was typed.
        let outcome = e.respond(turn("tira muchos", None).with_trace());
        let steps = outcome.trace.unwrap().steps;
        assert!(
            steps.iter().any(|s| s.note.contains("Unreadable")),
            "{steps:?}"
        );
    }

    #[test]
    fn draws_come_from_the_turn_or_from_a_script() {
        let e = engine();
        let (outcome, report) =
            e.respond_scripted(turn("tira", None), Draw::parse_list("4").unwrap());
        assert_eq!(outcome.reply, "Sale 4.");
        assert_eq!(report, ScriptReport::default());
        let (_, report) =
            e.respond_scripted(turn("responde", None), Draw::parse_list("4").unwrap());
        assert_eq!(report.unused.len(), 1);
        // Seeded draws replay exactly in a new engine.
        let roll = |e: &Engine| e.respond(turn("tira", None)).reply;
        let once = engine();
        let first: Vec<String> = (0..5).map(|_| roll(&once)).collect();
        let again = engine();
        let second: Vec<String> = (0..5).map(|_| roll(&again)).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn a_courtesy_prefix_is_dropped() {
        let outcome = respond("responde, no puedes", None);
        assert_eq!(intent(&outcome), Some("t.unable"));
    }

    #[test]
    fn the_conversation_owns_the_turn_counter_and_the_language() {
        let e = engine();
        let replies: Vec<String> = (0..4)
            .map(|_| e.respond(turn("tira", None)).reply)
            .collect();
        assert!(replies[0].starts_with("Sale"), "{replies:?}");
        assert!(
            replies.iter().any(|r| r.starts_with("Ha salido")),
            "{replies:?}"
        );
        e.end(&ConvId::new("c"));
        assert!(e.respond(turn("tira", None)).reply.starts_with("Sale"));
        // A settled language holds against one word of the other.
        let e = engine();
        assert_eq!(
            e.respond(turn("hello there my friend", None)).lang,
            Lang::En
        );
        assert_eq!(e.respond(turn("responde", None)).lang, Lang::En);
        assert_eq!(
            e.respond(turn("responde por favor y gracias", None)).lang,
            Lang::Es
        );
    }

    #[test]
    fn turns_of_different_conversations_run_on_many_threads() {
        fn assert_shareable<T: Send + Sync>() {}
        assert_shareable::<Engine>();
        let e = engine();
        std::thread::scope(|scope| {
            for t in 0..8 {
                let e = &e;
                scope.spawn(move || {
                    for _ in 0..50 {
                        let turn = Turn::new(
                            "tira",
                            ConvId::new(format!("c{t}")),
                            Timestamp::from_unix(0),
                        );
                        assert_eq!(e.respond(turn).status, Status::Answered);
                    }
                });
            }
        });
        let turn = Turn::new("tira", ConvId::new("c3"), Timestamp::from_unix(0)).with_trace();
        let steps = e.respond(turn).trace.unwrap().steps;
        assert!(steps.last().unwrap().note.contains("turn 51"), "{steps:?}");
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
