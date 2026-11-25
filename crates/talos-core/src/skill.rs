//! The handler half of a skill (docs/SKILLS.md section 2).
//!
//! A handler receives a recognised request ([`Call`]) and a [`Context`]
//! holding everything else it may use, and returns a [`SkillResult`]: a
//! reply id from its own manifest with variables, never a sentence. The
//! contract for skill authors is `docs/SKILL_AUTHORING.md`.

use crate::random::Random;
use crate::render::Vars;
use crate::state::ConversationState;
use crate::value::{Slots, Value};
use crate::{IntentId, Lang, Manifest, Place, Timestamp};

/// A skill as handed to the engine: its manifest and its handler.
pub type Registration = (Manifest, Box<dyn Skill>);

/// A reply to render: a template id from the skill's manifest, and variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The `[reply.<id>]` to use; it must be one of [`Skill::replies`].
    pub id: &'static str,
    /// Values for the template's `{name}` placeholders, already written in
    /// the turn's language (see [`Number::format`](crate::Number::format)).
    pub vars: Vars,
}

impl Message {
    /// A message with no variables.
    #[must_use]
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            vars: Vars::new(),
        }
    }

    /// The same message with one more variable.
    #[must_use]
    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(name.into(), value.into());
        self
    }
}

/// What a skill expects next when it asks back. A placeholder until open
/// questions arrive (v0.2); the engine answers an `Ask` honestly as
/// `Unhandled` rather than pretending to wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectation {
    /// The slot that is open.
    pub slot: String,
}

/// A failure inside a handler, rendered as an honest error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError(pub String);

/// What a handler decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillResult {
    /// Facts to render; the turn is `Answered`. Includes a correct "no" and
    /// a correct "that cannot be done" (docs/spec/README.md).
    Answer(Message),
    /// A question back to the user. Not available in v0.1: the turn becomes
    /// `Unhandled` with reason `cannot-ask`; answer a missing slot with a
    /// usage hint as `Unable` instead.
    Ask(Expectation, Message),
    /// Recognised, but Talos cannot provide it: rendered, `Unhandled`, the
    /// intent kept.
    Unable(Message),
    /// Not mine after all; the next candidate is tried.
    Decline,
    /// Something went wrong: an honest error, `Unhandled`.
    Fail(SkillError),
}

/// One recognised request, as a handler sees it.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    /// The recognised intent; always one of the skill's own.
    pub intent: &'a IntentId,
    /// The language of the turn, already decided: reply in it.
    pub lang: Lang,
    /// The intent's slots, typed and already extracted, internal ones
    /// included. A slot the message did not give is absent.
    pub slots: &'a Slots,
    /// The message exactly as received. For reading a term the slots do not
    /// cover; never for deciding what was asked (ADR 0007).
    pub text: &'a str,
}

/// What Talos remembers about the user. Empty until the `preferences` skill
/// and the knowledge store exist (v0.3); handlers receive it now so that
/// adding it changes no signature.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Preferences {}

impl Preferences {
    /// No preferences.
    pub const NONE: Preferences = Preferences {};
}

/// Everything a handler may use besides the request itself, and nothing
/// else (docs/SKILLS.md section 2): the injected clock, the caller's place,
/// the turn's random source, this conversation's state, the user's
/// preferences, and the installed skills. A handler reads no system clock,
/// no environment, no files and no global state.
#[derive(Debug)]
pub struct Context<'a> {
    now: Timestamp,
    place: Option<&'a Place>,
    random: Random,
    conversation: &'a ConversationState,
    preferences: &'a Preferences,
    skills: &'a [Manifest],
    /// Slots the handler reported for the outcome.
    reported: Slots,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        now: Timestamp,
        place: Option<&'a Place>,
        random: Random,
        conversation: &'a ConversationState,
        skills: &'a [Manifest],
    ) -> Self {
        Self {
            now,
            place,
            random,
            conversation,
            preferences: &Preferences::NONE,
            skills,
            reported: Slots::new(),
        }
    }

    /// A context outside any engine, for a handler's unit tests: a fresh
    /// conversation, no place, no other skills, and `random` as the source
    /// (`Random::seeded(0)`, or `Random::scripted(…)` to fix the draws).
    #[must_use]
    pub fn standalone(now: Timestamp, random: Random) -> Context<'static> {
        static NEW: ConversationState = ConversationState::NEW;
        Context::new(now, None, random, &NEW, &[])
    }

    /// The turn's instant, injected by the caller. With
    /// [`Timestamp::local_date`] and friends when the local offset is known.
    #[must_use]
    pub const fn now(&self) -> Timestamp {
        self.now
    }

    /// Where the caller says the user is, if it says.
    #[must_use]
    pub const fn place(&self) -> Option<&Place> {
        self.place
    }

    /// The turn's random source. Draw only through it; every method is one
    /// draw, and a turn that never calls this draws nothing.
    pub fn random(&mut self) -> &mut Random {
        &mut self.random
    }

    /// This conversation: the turn number, its language so far, the recent
    /// topic.
    #[must_use]
    pub const fn conversation(&self) -> &ConversationState {
        self.conversation
    }

    /// What Talos remembers about the user (nothing yet).
    #[must_use]
    pub const fn preferences(&self) -> &Preferences {
        self.preferences
    }

    /// The installed skills' manifests, in catalogue order, for answers
    /// generated from them (`help`).
    #[must_use]
    pub const fn skills(&self) -> &[Manifest] {
        self.skills
    }

    /// Puts a slot on the outcome, replacing what was extracted: a default
    /// the handler applied ("tira un dado" → `count=1 sides=6`) or a value it
    /// resolved. The slot must be one the intent declares and not internal,
    /// or the turn fails.
    pub fn report(&mut self, name: &str, value: Value) {
        self.reported.insert(name.to_owned(), value);
    }

    /// The handler's reported slots and the random source, back to the
    /// engine.
    pub(crate) fn finish(self) -> (Slots, Random) {
        (self.reported, self.random)
    }

    /// Takes the reported slots, leaving none (between candidates).
    pub(crate) fn take_reported(&mut self) -> Slots {
        std::mem::take(&mut self.reported)
    }
}

/// A built-in skill's handler.
pub trait Skill: Send + Sync {
    /// The skill id; must equal its manifest's `[skill] id`.
    fn id(&self) -> &'static str;

    /// Every reply id [`Skill::handle`] can return. `talos skills check`
    /// verifies that each exists in the manifest and that every reply of the
    /// manifest is listed; the engine refuses (as a failure) a message whose
    /// id is not listed.
    fn replies(&self) -> &'static [&'static str];

    /// Handles one recognised request. Synchronous, quick, and must not
    /// panic.
    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult;
}
