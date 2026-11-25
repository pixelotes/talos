//! The engine: one entry point, [`Engine::respond`], and the pipeline written
//! out in order in one function, [`Engine::run`] (ADR 0004).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::dialog::{self, CommandLine};
use crate::extract;
use crate::fallback::Move;
use crate::language::{self, Decided, Evidence};
use crate::manifest::{Intent, Manifest, SlotDecl};
use crate::normalise::{Normalised, normalise};
use crate::random::{Draw, Random, ScriptReport, turn_seed};
use crate::reflector::Reflector;
use crate::render;
use crate::rules::{Candidate, Rules};
use crate::safety::{self, DEFAULT_EMERGENCY, Emergency};
use crate::skill::{Call, Context, Expects, Message, Registration, Skill, SkillResult};
use crate::state::{ConversationState, Conversations, Open, lock_state};
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
    /// Emergency numbers by region, for the distress reply
    /// (docs/spec/reflector.md S29). A place with no entry, or no place,
    /// gets the reply that names no number.
    pub emergency: &'static [Emergency],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_lang: Lang::Es,
            seed: 0,
            max_conversations: 1_024,
            idle_expiry_seconds: 30 * 60,
            emergency: DEFAULT_EMERGENCY,
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
    /// Command names (both languages) to (skill position, intent position).
    commands: BTreeMap<String, (usize, usize)>,
    /// The fallback scripts, and the distress patterns.
    reflector: Reflector,
    conversations: Conversations,
}

/// How many turns after the one that asked an open question stays open,
/// and for how long by the turns' clock (docs/spec/conversation.md 3.7).
const OPEN_TURNS: u32 = 3;
const OPEN_SECONDS: i64 = 5 * 60;

/// What every stage of one turn reads: the message, its language, the
/// turn's number and the turn itself.
#[derive(Clone, Copy)]
struct At<'a> {
    message: &'a Normalised,
    lang: Lang,
    number: u32,
    turn: &'a Turn,
}

/// What the pending-dialog stage decided.
enum Pending {
    /// It answered the turn.
    Done(Handled),
    /// The rest of the pipeline goes on: with the rule candidates it has
    /// already found, if it looked, and the open question when the message
    /// repeats the request that asked it (so asking again is counted).
    Route {
        candidates: Option<Vec<Candidate>>,
        previous: Option<Open>,
    },
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
/// nothing otherwise: notes are closures, never called when off. (The
/// reflector and the safety check build their one note regardless; it is a
/// few short strings on a turn nothing else answered.)
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
    /// The question left open for the next turn, if any.
    open: Option<Open>,
    /// The slot asked for, when the turn asked back.
    asks: Option<String>,
}

impl Handled {
    /// An `Unhandled` result with the fixed reply for its reason.
    fn fallback(lang: Lang, reason: Reason, intent: Option<IntentId>) -> Self {
        let reply = fallback::reply(lang, &reason).to_owned();
        Self::fixed(reply, Status::Unhandled, reason, intent)
    }

    /// A reply of the reflector or of the safety check: no intent, the
    /// shape or the reason kept.
    fn reflected(reply: String, status: Status, reason: Reason) -> Self {
        Self::fixed(reply, status, reason, None)
    }

    /// A dialog move's fixed reply.
    fn dialog(lang: Lang, kind: Move, status: Status, reason: Reason) -> Self {
        Self::fixed(
            fallback::dialog(lang, kind).to_owned(),
            status,
            reason,
            None,
        )
    }

    /// An open question dropped after two failed answers (3.4).
    fn gave_up(lang: Lang, intent: &IntentId) -> Self {
        let reply = fallback::dialog(lang, Move::GaveUp).to_owned();
        Self::fixed(
            reply,
            Status::Unhandled,
            Reason::GAVE_UP,
            Some(intent.clone()),
        )
    }

    fn fixed(reply: String, status: Status, reason: Reason, intent: Option<IntentId>) -> Self {
        Self {
            reply,
            status,
            reason: Some(reason),
            intent,
            slots: Slots::new(),
            internal: Slots::new(),
            open: None,
            asks: None,
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
    /// compile or captures an undeclared slot; [`Error::Script`] if an
    /// embedded reflector script is malformed (a build defect, caught by
    /// the tests).
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
        let commands = command_table(&manifests)?;
        Ok(Self {
            conversations: Conversations::new(config.max_conversations, config.idle_expiry_seconds),
            config,
            manifests,
            handlers,
            by_id,
            rules,
            commands,
            reflector: Reflector::new()?,
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
    #[expect(
        clippy::too_many_lines,
        reason = "the pipeline is written out in order in one function (ADR 0004)"
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

        let command = dialog::command_line(&message);

        // 1. Language, sticky per conversation. A command line is no
        // evidence: the reply is in the conversation's language (ADR 0016).
        let evidence = if command.is_some() { "" } else { &message.text };
        let (lang, decided) = language::decide(
            turn.lang,
            evidence,
            state.lang_hold(),
            self.config.default_lang,
        );
        state.set_lang(lang, decided.hold());
        trace.note("language", || describe_language(lang, decided));

        // The open question, if one is open and has not expired.
        let mut open = Self::open_question(&mut state, number, turn.now, &mut trace);

        let random = match script {
            Some(draws) => Random::scripted(draws),
            None => Random::seeded(turn_seed(
                self.config.seed,
                turn.conversation.as_str(),
                number,
                &message.text,
            )),
        };
        // 1b. Safety check, ahead of everything that could claim the
        // message (docs/spec/conversation.md 3.9). Not on a command line.
        let alarm = if message.is_empty() || command.is_some() {
            None
        } else {
            let alarm = safety::check(
                &self.reflector,
                lang,
                &message,
                turn.place.as_ref(),
                self.config.emergency,
            );
            trace.note("safety", || {
                alarm
                    .as_ref()
                    .map_or_else(|| "no distress".to_owned(), |a| a.note.clone())
            });
            alarm
        };

        let mut cx = Context::new(
            turn.now,
            turn.place.as_ref(),
            random,
            &state,
            &self.manifests,
        );
        let at = At {
            message: &message,
            lang,
            number,
            turn: &turn,
        };
        let answered = if message.is_empty() || alarm.is_some() {
            None
        } else if let Some(line) = &command {
            // A command goes straight to its intent; it closes any open
            // question, as an unrelated message would.
            if open.take().is_some() {
                trace.note("dialog", || {
                    "a command line closes the open question".to_owned()
                });
            }
            Some(self.command(&mut cx, at, line, &mut trace))
        } else {
            // 2. Pending dialog.
            match self.pending(&mut cx, at, open.take(), &mut trace) {
                Pending::Done(handled) => Some(handled),
                Pending::Route {
                    candidates: found,
                    previous,
                } => {
                    // 3. Rules.
                    let candidates =
                        found.unwrap_or_else(|| self.recognise(lang, &message, &mut trace));

                    // 4. Classifier: v0.2.

                    // 5. Dispatch and 6. render, candidate by candidate until
                    // one does not decline.
                    let mut handled = None;
                    for candidate in &candidates {
                        handled =
                            self.dispatch(&mut cx, at, candidate, &mut trace, previous.as_ref());
                        if handled.is_some() {
                            break;
                        }
                    }
                    handled
                }
            }
        };
        let (_, random) = cx.finish();

        let handled = if let Some(handled) = answered {
            handled
        } else if message.is_empty() {
            trace.note("fallback", || "empty message".to_owned());
            // An empty message leaves an open question as it was.
            Handled {
                open: open.take(),
                ..Handled::fallback(lang, Reason::EMPTY, None)
            }
        } else if let Some(alarm) = alarm {
            if open.is_some() {
                trace.note("dialog", || "distress closes the open question".to_owned());
            }
            Handled::reflected(alarm.reply, alarm.status, alarm.reason)
        } else {
            // 7. The reflector.
            let reply =
                self.reflector
                    .respond(lang, &message, &turn.conversation, number, &mut state);
            trace.note("reflector", || reply.note.clone());
            Handled::reflected(reply.text, reply.status, reply.reason)
        };
        trace.note("random", || format!("{} draws", random.draws()));

        state.set_last_intent(handled.intent.clone());
        state.remember_reply(&handled.reply, handled.status);
        let pending = handled.open.is_some();
        state.set_open(handled.open);
        trace.note("state", || describe_state(&turn.conversation, &state));
        let outcome = Outcome {
            reply: handled.reply,
            status: handled.status,
            lang,
            confidence: if handled.intent.is_some() { 1.0 } else { 0.0 },
            intent: handled.intent,
            slots: handled.slots,
            reason: handled.reason,
            pending,
            asks: handled.asks,
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
            let value = extract::slot(decl, original, folded, lang, cx.now());
            slots.insert(name.clone(), value);
        }
        slots
    }

    /// The skill position and the intent for an intent id.
    fn intent(&self, id: &IntentId) -> Option<(usize, &Intent)> {
        let &index = self.by_id.get(id.skill())?;
        let intent = self.manifests[index].intents.iter().find(|i| &i.id == id)?;
        Some((index, intent))
    }

    /// The open question carried over from the last turn, unless it has
    /// expired: more than [`OPEN_TURNS`] turns or [`OPEN_SECONDS`] seconds
    /// after the turn that asked it (docs/spec/conversation.md 3.7).
    fn open_question(
        state: &mut ConversationState,
        number: u32,
        now: crate::Timestamp,
        trace: &mut Tracer,
    ) -> Option<Open> {
        let open = state.take_open()?;
        let turns = number.saturating_sub(open.turn);
        let seconds = now.unix_seconds.saturating_sub(open.at);
        if turns > OPEN_TURNS || seconds > OPEN_SECONDS {
            trace.note("dialog", || {
                format!(
                    "the question for `{}` ({}) expired after {turns} turns and {seconds} s",
                    open.expectation.slot, open.intent
                )
            });
            return None;
        }
        Some(open)
    }

    /// Stage 2: the pending dialog (docs/spec/conversation.md section 3).
    /// With a question open, the message may cancel it, hesitate, answer it
    /// (and complete the original request), fail to answer it, or be
    /// unrelated and release it. With none open, a bare acknowledgement or
    /// yes/no is answered here.
    fn pending(
        &self,
        cx: &mut Context<'_>,
        at: At<'_>,
        open: Option<Open>,
        trace: &mut Tracer,
    ) -> Pending {
        let folded = at.message.folded.as_str();
        let lang = at.lang;
        let route = |candidates, previous| Pending::Route {
            candidates,
            previous,
        };
        let Some(open) = open else {
            return Self::nothing_open(lang, folded, trace)
                .map_or_else(|| route(None, None), Pending::Done);
        };
        let Some((_, intent)) = self.intent(&open.intent) else {
            return route(None, None);
        };
        let Some(decl) = intent.slot(&open.expectation.slot) else {
            return route(None, None);
        };
        let framed = dialog::frame(at.message, decl.kind == SlotType::Term);
        let original = at.message.original_of(framed.clone());
        let answer = &folded[framed];
        let slot = open.expectation.slot.clone();

        // A yes/no question takes "mejor no" or "vale" as its answer
        // before anything else.
        if open.expectation.expects == Expects::Confirm
            && let Ok(yes) = extract::confirm(answer)
        {
            return self.fill(cx, at, open, Value::Confirm(yes), trace);
        }
        if dialog::is_cancel(folded) {
            trace.note("dialog", || {
                format!("the question for `{slot}` is cancelled")
            });
            return Pending::Done(Handled::dialog(
                lang,
                Move::Cancelled,
                Status::Answered,
                Reason::CANCELLED,
            ));
        }
        if dialog::is_hedge(folded) || dialog::is_ack(folded) {
            trace.note("dialog", || {
                format!("a hesitation; the question for `{slot}` stays open")
            });
            let nth = at.number.saturating_sub(open.turn).saturating_sub(1);
            return Pending::Done(Handled {
                open: Some(open),
                ..Handled::fixed(
                    fallback::hedge(lang, nth).to_owned(),
                    Status::Answered,
                    Reason::HEDGE,
                    None,
                )
            });
        }
        // An option, by position or by name.
        if let Expects::Choice(options) = &open.expectation.expects {
            let names: Vec<Vec<String>> = options.iter().map(|o| o.names.clone()).collect();
            if let Ok(index) = extract::choice(answer, &names) {
                let chosen = options[index].value.clone();
                let value = Self::read(decl, &chosen, at, cx);
                return self.fill(cx, at, open, value, trace);
            }
        }
        // A value whose type is not free text is taken before the rules:
        // "21" while a percentage is open.
        if open.expectation.expects == Expects::Value && decl.kind != SlotType::Term {
            let value = extract::slot(decl, original, answer, lang, cx.now());
            if !matches!(value, Value::Invalid { .. }) {
                return self.fill(cx, at, open, value, trace);
            }
        }
        // A recognised request: the same one again, or another (3.6).
        let candidates = self.recognise(lang, at.message, trace);
        let completing = matches!(open.expectation.expects, Expects::Completion(_));
        if let Some(first) = candidates.first()
            && !(completing && first.intent == open.intent)
        {
            if first.intent == open.intent {
                trace.note("dialog", || "the request is repeated".to_owned());
                return route(Some(candidates), Some(open));
            }
            trace.note("dialog", || {
                format!("not an answer: the question for `{slot}` is released and closed")
            });
            return route(Some(candidates), None);
        }
        // An answer of the expected kind, or a failed one. A bare yes or no
        // is never a value.
        let value = match &open.expectation.expects {
            _ if extract::confirm(answer).is_ok() => None,
            Expects::Value => Some(extract::slot(decl, original, answer, lang, cx.now())),
            Expects::Completion(prefix) => {
                let whole = normalise(&format!("{prefix} {original}"));
                Some(extract::slot(
                    decl,
                    &whole.original,
                    &whole.folded,
                    lang,
                    cx.now(),
                ))
            }
            Expects::Choice(_) | Expects::Confirm => None,
        }
        .filter(|v| !matches!(v, Value::Invalid { .. }));
        match value {
            Some(value) => self.fill(cx, at, open, value, trace),
            None => Pending::Done(self.not_understood(at, open, trace)),
        }
    }

    /// With no question open: a bare acknowledgement, or a bare yes or no
    /// (docs/spec/conversation.md 1.2, 3.5, 3.8).
    fn nothing_open(lang: Lang, folded: &str, trace: &mut Tracer) -> Option<Handled> {
        if dialog::is_ack(folded) || dialog::is_cancel(folded) {
            trace.note("dialog", || "nothing open: an acknowledgement".to_owned());
            return Some(Handled::dialog(
                lang,
                Move::Acknowledged,
                Status::Answered,
                Reason::ACKNOWLEDGEMENT,
            ));
        }
        // "vale, gracias" is a thanks: an acknowledgement word is never a
        // bare yes.
        if extract::confirm(folded).is_ok() && !dialog::starts_with_ack(folded) {
            trace.note("dialog", || "nothing open: a bare yes or no".to_owned());
            return Some(Handled::dialog(
                lang,
                Move::NothingPending,
                Status::Unhandled,
                Reason::NOTHING_PENDING,
            ));
        }
        None
    }

    /// Reads `text` as the slot's type, outside any message (a chosen
    /// option's value).
    fn read(decl: &SlotDecl, text: &str, at: At<'_>, cx: &Context<'_>) -> Value {
        let n = normalise(text);
        extract::slot(decl, &n.original, &n.folded, at.lang, cx.now())
    }

    /// An answer: the original request, completed with it, runs again.
    fn fill(
        &self,
        cx: &mut Context<'_>,
        at: At<'_>,
        open: Open,
        value: Value,
        trace: &mut Tracer,
    ) -> Pending {
        trace.note("dialog", || {
            format!(
                "answers `{}` of {}: {value}",
                open.expectation.slot, open.intent
            )
        });
        let mut slots = open.slots.clone();
        slots.insert(open.expectation.slot.clone(), value);
        let handled = match self.intent(&open.intent) {
            Some((index, decl)) => self.call(cx, at, index, decl, slots, trace, Some(&open)),
            None => None,
        };
        // A handler that declines the completed request did not get an
        // answer it can use.
        Pending::Done(handled.unwrap_or_else(|| self.not_understood(at, open, trace)))
    }

    /// A message that is no answer to the open question: it is asked once
    /// more, then dropped (3.4).
    fn not_understood(&self, at: At<'_>, open: Open, trace: &mut Tracer) -> Handled {
        if open.asked >= 2 {
            trace.note("dialog", || {
                format!("no answer for `{}` twice: dropped", open.expectation.slot)
            });
            return Handled::gave_up(at.lang, &open.intent);
        }
        trace.note("dialog", || {
            format!("no answer for `{}`: asked again", open.expectation.slot)
        });
        let Some((index, intent)) = self.intent(&open.intent) else {
            return Handled::gave_up(at.lang, &open.intent);
        };
        let question = Self::render(
            &self.manifests[index],
            at.lang,
            at.turn,
            at.number,
            &open.question,
            trace,
        );
        let Ok(question) = question else {
            return Handled::gave_up(at.lang, &open.intent);
        };
        let (public, internal): (Slots, Slots) = open
            .slots
            .clone()
            .into_iter()
            .partition(|(name, _)| intent.slot(name).is_some_and(|s| !s.internal));
        Handled {
            reply: format!(
                "{} {question}",
                fallback::dialog(at.lang, Move::NotUnderstood)
            ),
            status: Status::Answered,
            reason: None,
            intent: Some(open.intent.clone()),
            slots: public,
            internal,
            asks: Some(open.expectation.slot.clone()),
            open: Some(Open { asked: 2, ..open }),
        }
    }

    /// A command line (ADR 0016): no recognition, the argument through the
    /// declared slot's extractor, then the ordinary dispatch.
    fn command(
        &self,
        cx: &mut Context<'_>,
        at: At<'_>,
        line: &CommandLine,
        trace: &mut Tracer,
    ) -> Handled {
        let lang = at.lang;
        let shown = at.message.original_of(line.name_range.clone()).to_owned();
        let Some(&(index, position)) = self.commands.get(&line.name) else {
            trace.note("command", || {
                format!("`{}` is not an installed command", line.name)
            });
            let help = [("/ayuda", Lang::Es), ("/help", Lang::En)]
                .into_iter()
                .find(|(name, l)| *l == lang && self.commands.contains_key(*name))
                .map(|(name, _)| name);
            let reply = fallback::unknown_command(lang, &shown, help);
            return Handled::fixed(reply, Status::Unhandled, Reason::UNKNOWN_COMMAND, None);
        };
        let intent = &self.manifests[index].intents[position];
        let slot = intent.command.as_ref().and_then(|c| c.slot.clone());
        let captures = match (slot, line.argument.is_empty()) {
            (Some(slot), false) => vec![(slot, line.argument.clone())],
            (None, false) => {
                trace.note("command", || format!("`{}` takes no argument", line.name));
                let reply = fallback::command_argument(lang, &shown);
                return Handled::fixed(
                    reply,
                    Status::Unhandled,
                    Reason::COMMAND_ARGUMENT,
                    Some(intent.id.clone()),
                );
            }
            (_, true) => Vec::new(),
        };
        trace.note("command", || format!("`{}` → {}", line.name, intent.id));
        let candidate = Candidate {
            intent: intent.id.clone(),
            priority: intent.priority,
            lang,
            pattern: line.name.clone(),
            fixed: BTreeMap::new(),
            captures,
        };
        self.dispatch(cx, at, &candidate, trace, None)
            .unwrap_or_else(|| Handled::fallback(lang, Reason::CANNOT, Some(intent.id.clone())))
    }

    /// Stages 5 and 6 for one candidate; `None` when its skill declines.
    /// `previous` is the open question when the message repeats the
    /// request that asked it.
    fn dispatch(
        &self,
        cx: &mut Context<'_>,
        at: At<'_>,
        candidate: &Candidate,
        trace: &mut Tracer,
        previous: Option<&Open>,
    ) -> Option<Handled> {
        let Some((index, intent)) = self.intent(&candidate.intent) else {
            trace.note("dispatch", || format!("no skill owns {}", candidate.intent));
            return None;
        };
        let slots = Self::extract(intent, candidate, at.message, at.lang, cx);
        trace.note("slots", || {
            if slots.is_empty() {
                "none".to_owned()
            } else {
                describe_slots(&slots)
            }
        });
        self.call(cx, at, index, intent, slots, trace, previous)
    }

    /// Calls the handler with typed slots and renders what it returns;
    /// `None` when it declines.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pipeline passes each stage what it reads, explicitly (ADR 0004)"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "one handler result, checked and rendered in the order it is read"
    )]
    fn call(
        &self,
        cx: &mut Context<'_>,
        at: At<'_>,
        index: usize,
        intent: &Intent,
        slots: Slots,
        trace: &mut Tracer,
        previous: Option<&Open>,
    ) -> Option<Handled> {
        let At {
            message,
            lang,
            number,
            turn,
        } = at;
        let intent_id = &intent.id;
        let manifest = &self.manifests[index];
        let handler = &self.handlers[index];
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
        let mut question = None;
        let (message, status, reason) = match result {
            SkillResult::Decline => return None,
            SkillResult::Answer(message) => (message, Status::Answered, None),
            SkillResult::Unable(message) => (message, Status::Unhandled, Some(Reason::CANNOT)),
            SkillResult::Ask(expectation, message) => {
                if intent.slot(&expectation.slot).is_none_or(|s| s.internal) {
                    return failed(
                        trace,
                        format!(
                            "asks for `{}`, which is not a declared, public slot of {intent_id}",
                            expectation.slot
                        ),
                    );
                }
                question = Some(expectation);
                (message, Status::Answered, None)
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
        // Asking the same question a third time is giving up (3.4).
        let again = previous.filter(|p| {
            question
                .as_ref()
                .is_some_and(|q| p.intent == *intent_id && p.expectation.slot == q.slot)
        });
        let asked = again.map_or(1, |p| p.asked.saturating_add(1));
        if question.is_some() && asked > 2 {
            trace.note("dialog", || "asked twice already: dropped".to_owned());
            return Some(Handled::gave_up(lang, intent_id));
        }
        let mut stored = slots.clone();
        let (mut public, internal): (Slots, Slots) = slots
            .into_iter()
            .partition(|(name, _)| intent.slot(name).is_some_and(|s| !s.internal));
        public.extend(reported);
        let reply = match Self::render(manifest, lang, turn, number, &message, trace) {
            Ok(reply) => reply,
            Err(error) => return failed(trace, error.to_string()),
        };
        let (reply, open, asks) = match question {
            None => (reply, None, None),
            Some(expectation) => {
                stored.remove(&expectation.slot);
                let reply = if asked == 2 {
                    format!("{} {reply}", fallback::dialog(lang, Move::NotUnderstood))
                } else {
                    reply
                };
                let asks = expectation.slot.clone();
                trace.note("dialog", || {
                    format!("asks for `{asks}`; the question stays open")
                });
                let open = Open {
                    intent: intent_id.clone(),
                    expectation,
                    slots: stored,
                    question: message,
                    turn: again.map_or(number, |p| p.turn),
                    at: again.map_or(turn.now.unix_seconds, |p| p.at),
                    asked,
                };
                (reply, Some(open), Some(asks))
            }
        };
        Some(Handled {
            reply,
            status,
            reason,
            intent: Some(intent_id.clone()),
            slots: public,
            internal,
            open,
            asks,
        })
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

/// Every command name to its (skill position, intent position).
///
/// # Errors
///
/// [`Error::Manifest`] when two intents declare the same name.
fn command_table(manifests: &[Manifest]) -> Result<BTreeMap<String, (usize, usize)>, Error> {
    let mut table: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (s, manifest) in manifests.iter().enumerate() {
        for (i, intent) in manifest.intents.iter().enumerate() {
            let Some(command) = &intent.command else {
                continue;
            };
            for name in [&command.names.es, &command.names.en] {
                match table.get(name) {
                    Some(&(ts, ti)) if (ts, ti) != (s, i) => {
                        let other = &manifests[ts].intents[ti].id;
                        return Err(Error::manifest(
                            &manifest.id,
                            format!(
                                "intent `{}`: command `{name}` is already `{other}`'s",
                                intent.id
                            ),
                        ));
                    }
                    _ => {
                        table.insert(name.clone(), (s, i));
                    }
                }
            }
        }
    }
    Ok(table)
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
    use crate::skill::{Choice, Expectation, SkillError};
    use crate::{Number, Timestamp};

    /// A skill whose behaviour per intent is fixed by the test.
    struct Fixed;

    impl Skill for Fixed {
        fn id(&self) -> &'static str {
            "t"
        }

        fn replies(&self) -> &'static [&'static str] {
            &["echo", "count", "question", "no_such_reply"]
        }

        fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
            let given = call.slots.values().next().map(ToString::to_string);
            let echo = |x: String| SkillResult::Answer(Message::new("echo").var("x", x));
            match call.intent.as_str() {
                "t.decline" => SkillResult::Decline,
                "t.answer" => SkillResult::Answer(Message::new("echo").var("x", "uno")),
                "t.unable" => SkillResult::Unable(Message::new("echo").var("x", "dos")),
                "t.ask" => match call.slots.get("x") {
                    Some(Value::Number(n)) => echo(n.to_string()),
                    // Asked again while the answer is not a number.
                    _ => SkillResult::Ask(Expectation::slot("x"), Message::new("question")),
                },
                "t.confirm" => match given {
                    Some(answer) => echo(answer),
                    None => SkillResult::Ask(Expectation::confirm("ok"), Message::new("question")),
                },
                "t.choose" => match given {
                    Some(answer) => echo(answer),
                    None => SkillResult::Ask(
                        Expectation::choice(
                            "pick",
                            vec![
                                Choice::new("rojo", ["rojo", "red"]),
                                Choice::new("verde", ["verde", "green"]),
                            ],
                        ),
                        Message::new("question"),
                    ),
                },
                "t.sum" => match given {
                    Some(answer) if answer.contains('+') => echo(answer),
                    Some(answer) => SkillResult::Ask(
                        Expectation::completing("expr", answer),
                        Message::new("question"),
                    ),
                    None => SkillResult::Ask(Expectation::slot("expr"), Message::new("question")),
                },
                "t.internal" => {
                    SkillResult::Ask(Expectation::slot("mode"), Message::new("question"))
                }
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
slot = [{ name = "x", type = "number", required = true }]
command = { es = "/pregunta", en = "/ask", slot = "x" }
pattern = [{ lang = "es", regex = '^pregunta(?: (?<x>.+))?$' }]

[[intent]]
id = "t.confirm"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "ok", type = "confirm", required = true }]
pattern = [{ lang = "es", regex = '^confirma$' }]

[[intent]]
id = "t.choose"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "pick", type = "term", required = true }]
pattern = [{ lang = "es", regex = '^escoge$' }]

[[intent]]
id = "t.sum"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "expr", type = "term", required = true }]
pattern = [{ lang = "es", regex = '^suma(?: (?<expr>.+))?$' }]

[[intent]]
id = "t.internal"
priority = 50
examples = { es = ["x"], en = ["x"] }
slot = [{ name = "mode", type = "term", internal = true }]
command = { es = "/interno", en = "/internal" }
pattern = [{ lang = "es", regex = '^interna$' }]

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

[reply.question]
es = ["¿Cuántos?"]
en = ["How many?"]
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
    fn fail_and_handler_mistakes_do_not_panic() {
        for (text, reason) in [
            ("interna", Reason::FAILED),
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

    /// One conversation, turn by turn: (reply, outcome line, pending).
    fn talk(e: &Engine, texts: &[&str]) -> Vec<(String, String, bool)> {
        texts
            .iter()
            .map(|text| {
                let o = e.respond(turn(text, Some(Lang::Es)));
                (o.reply.clone(), o.line(None), o.pending)
            })
            .collect()
    }

    fn line(text: &str, outcome: &str, pending: bool) -> (String, String, bool) {
        (text.to_owned(), outcome.to_owned(), pending)
    }

    #[test]
    fn an_open_question_takes_the_next_message() {
        let e = engine();
        assert_eq!(
            talk(&e, &["pregunta", "pues 3, por favor"]),
            [
                line("¿Cuántos?", "asks · t.ask · x", true),
                line("Eco 3.", "answered · t.ask · x=3", false),
            ]
        );
        // The request repeated with the value is an answer too.
        assert_eq!(
            talk(&e, &["pregunta", "pregunta 4"])[1],
            line("Eco 4.", "answered · t.ask · x=4", false)
        );
        let outcome = e.respond(turn("pregunta", None));
        assert_eq!(outcome.status, Status::Answered);
        assert_eq!(outcome.asks.as_deref(), Some("x"));
        assert_eq!(
            e.respond(turn("cinco", None)).line(None),
            "answered · t.ask · x=5"
        );
    }

    #[test]
    fn cancelling_releasing_and_giving_up() {
        let e = engine();
        assert_eq!(
            talk(&e, &["pregunta", "déjalo, da igual", "3"])[1..],
            [
                line("Vale, lo dejo.", "answered · cancelled", false),
                line("No te he entendido.", "unhandled · noise", false),
            ]
        );
        // An unrelated request is handled and closes the question.
        assert_eq!(
            talk(&e, &["pregunta", "responde", "3"])[1..],
            [
                line("Eco uno.", "answered · t.answer", false),
                line("No te he entendido.", "unhandled · noise", false),
            ]
        );
        // Asked once more, then dropped.
        assert_eq!(
            talk(&e, &["pregunta", "azul", "verde"])[1..],
            [
                line("No te he entendido. ¿Cuántos?", "asks · t.ask · x", true),
                line(
                    "No he conseguido entenderlo, así que lo dejo.",
                    "unhandled · t.ask",
                    false
                ),
            ]
        );
        // The handler asking again counts the same way.
        assert_eq!(
            talk(&e, &["pregunta", "pregunta", "pregunta"])[1..],
            [
                line("No te he entendido. ¿Cuántos?", "asks · t.ask · x", true),
                line(
                    "No he conseguido entenderlo, así que lo dejo.",
                    "unhandled · t.ask",
                    false
                ),
            ]
        );
    }

    #[test]
    fn hedges_keep_it_open_until_it_expires() {
        let e = engine();
        assert_eq!(
            talk(&e, &["pregunta", "mmm", "a ver", "espera", "3"])[1..],
            [
                line("Tómate tu tiempo.", "answered · hedge", true),
                line("Vale.", "answered · hedge", true),
                line("Aquí sigo.", "answered · hedge", true),
                line("No te he entendido.", "unhandled · noise", false),
            ]
        );
        // By the clock: five minutes.
        let e = engine();
        let at = |text: &str, seconds| {
            e.respond(Turn::new(
                text,
                ConvId::new("c"),
                Timestamp::from_unix(seconds),
            ))
            .line(None)
        };
        assert_eq!(at("pregunta", 0), "asks · t.ask · x");
        assert_eq!(at("", 100), "unhandled · empty");
        assert_eq!(at("3", 200), "answered · t.ask · x=3");
        assert_eq!(at("pregunta", 1_000), "asks · t.ask · x");
        assert_eq!(at("3", 1_301), "unhandled · noise");
    }

    #[test]
    fn distress_and_commands_close_the_question() {
        let e = engine();
        let replies = talk(&e, &["pregunta", "no quiero seguir viviendo", "3"]);
        assert_eq!(replies[1].1, "unhandled · distress");
        assert_eq!(replies[2].1, "unhandled · noise");
        let replies = talk(&e, &["pregunta", "/ask 7", "3"]);
        assert_eq!(replies[1], line("Eco 7.", "answered · t.ask · x=7", false));
        assert_eq!(replies[2].1, "unhandled · noise");
    }

    #[test]
    fn yes_no_choices_and_completions() {
        let e = engine();
        assert_eq!(
            talk(&e, &["confirma", "mejor no"])[1],
            line("Eco no.", "answered · t.confirm · ok=no", false)
        );
        assert_eq!(
            talk(&e, &["confirma", "vale"])[1],
            line("Eco yes.", "answered · t.confirm · ok=yes", false)
        );
        assert_eq!(
            talk(&e, &["escoge", "la segunda"])[1],
            line("Eco verde.", "answered · t.choose · pick=verde", false)
        );
        assert_eq!(
            talk(&e, &["escoge", "red"])[1],
            line("Eco rojo.", "answered · t.choose · pick=rojo", false)
        );
        assert_eq!(talk(&e, &["escoge", "no"])[1].1, "asks · t.choose · pick");
        assert_eq!(
            talk(&e, &["suma 2", "+ 3"])[1],
            line("Eco 2 + 3.", "answered · t.sum · expr=2 + 3", false)
        );
    }

    #[test]
    fn nothing_open() {
        let e = engine();
        assert_eq!(
            talk(&e, &["sí", "no, gracias", "vale", "olvídalo"]),
            [
                line(
                    "No tengo ninguna pregunta pendiente.",
                    "unhandled · nothing-pending",
                    false
                ),
                line(
                    "No tengo ninguna pregunta pendiente.",
                    "unhandled · nothing-pending",
                    false
                ),
                line("Vale.", "answered · acknowledgement", false),
                line("Vale.", "answered · acknowledgement", false),
            ]
        );
    }

    #[test]
    fn commands() {
        let e = engine();
        // Both names work; the reply keeps the conversation's language.
        let o = e.respond(turn("/pregunta 3", Some(Lang::En)));
        assert_eq!((o.reply.as_str(), o.lang), ("Echo 3.", Lang::En));
        assert_eq!(o.line(None), "answered · t.ask · x=3");
        assert_eq!(e.respond(turn("/ASK three", None)).reply, "Echo 3.");
        // Missing argument: asked, as for a sentence.
        assert_eq!(e.respond(turn("/ask", None)).line(None), "asks · t.ask · x");
        // Unknown: never the reflector; no help command is installed here.
        let o = e.respond(turn("/pregnta 3", Some(Lang::Es)));
        assert_eq!(o.reply, "No conozco el comando /pregnta.");
        assert_eq!(o.line(None), "unhandled · unknown-command");
        let o = e.respond(turn("/interno algo", Some(Lang::Es)));
        assert_eq!(o.reply, "El comando /interno no lleva nada detrás.");
        assert_eq!(o.line(None), "unhandled · t.internal");
        // A command does not switch the language.
        let o = e.respond(turn("/ask the number please, thank you", None));
        assert_eq!(o.lang, Lang::Es);
        // Duplicate names are refused.
        let twice = MANIFEST.replace("en = \"/internal\"", "en = \"/ask\"");
        let manifest = Manifest::parse("t", &twice).unwrap();
        assert!(matches!(
            Engine::new(Config::default(), vec![(manifest, Box::new(Fixed))]),
            Err(Error::Manifest { .. })
        ));
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
