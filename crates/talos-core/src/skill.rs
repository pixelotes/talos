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

/// What a skill expects next when it asks back (docs/spec/conversation.md
/// section 3). The engine keeps it in the conversation with the request's
/// slots; the next message is offered to it first, and an answer completes
/// the original request: the same handler is called again with the slot
/// filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectation {
    /// The slot the answer fills; one the intent declares, and not internal.
    pub slot: String,
    /// What is accepted as an answer.
    pub expects: Expects,
}

/// The kinds of answer an [`Expectation`] accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expects {
    /// A value of the slot's declared type, read by its extractor: bare
    /// ("Tokio"), framed ("en Tokio, por favor") or in a repetition of the
    /// request ("la hora en Tokio").
    Value,
    /// Text that completes what the user already wrote: the slot is filled
    /// with this prefix, a space and the answer ("2 +" and then "3").
    Completion(String),
    /// One of these options, by position or by name; the chosen option's
    /// `value` fills the slot, through the slot's extractor.
    Choice(Vec<Choice>),
    /// Yes or no; the slot receives [`Value::Confirm`].
    Confirm,
}

/// One option offered by an [`Expects::Choice`] question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The text that fills the slot when this option is chosen, read by
    /// the slot's extractor ("4 march", for a date).
    pub value: String,
    /// How the user may name it, in either language; kept in the match
    /// form (lower case, no accents).
    pub names: Vec<String>,
}

impl Choice {
    /// An option: what fills the slot, and the ways to name it.
    #[must_use]
    pub fn new<S: AsRef<str>>(
        value: impl Into<String>,
        names: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            value: value.into(),
            names: names
                .into_iter()
                .map(|n| crate::normalise::normalise(n.as_ref()).folded)
                .collect(),
        }
    }
}

impl Expectation {
    /// Asks for a value of the slot's type.
    #[must_use]
    pub fn slot(slot: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            expects: Expects::Value,
        }
    }

    /// Asks for the rest of what the user started: the answer is appended
    /// to `prefix`.
    #[must_use]
    pub fn completing(slot: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            expects: Expects::Completion(prefix.into()),
        }
    }

    /// Asks which of `options` (at most one question, conversation.md 3.4).
    #[must_use]
    pub fn choice(slot: impl Into<String>, options: Vec<Choice>) -> Self {
        Self {
            slot: slot.into(),
            expects: Expects::Choice(options),
        }
    }

    /// Asks yes or no.
    #[must_use]
    pub fn confirm(slot: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            expects: Expects::Confirm,
        }
    }
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
    /// A question back to the user: the message is the question, and the
    /// expectation says what answers it. The turn is `Answered` with
    /// `pending` set; the next message of the conversation is offered to
    /// the expectation first.
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

/// The unit system a user prefers (docs/spec/state.md, preferences).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Units {
    /// Metres, kilograms, litres, degrees Celsius.
    Metric,
    /// Miles, pounds, gallons, degrees Fahrenheit.
    Imperial,
}

impl Units {
    /// The canonical id: `metric` or `imperial`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Units::Metric => "metric",
            Units::Imperial => "imperial",
        }
    }

    /// The units an id names.
    #[must_use]
    pub fn parse(id: &str) -> Option<Units> {
        match id {
            "metric" => Some(Units::Metric),
            "imperial" => Some(Units::Imperial),
            _ => None,
        }
    }
}

/// What Talos remembers about the user across turns and restarts
/// (docs/spec/state.md, preferences): a name, a place and a unit system,
/// each optional. One set per installation. Handlers read it through
/// [`Context::preferences`] and change it only by asking the engine
/// ([`Context::remember`]); the engine stores it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Preferences {
    name: Option<String>,
    place: Option<String>,
    units: Option<Units>,
}

impl Preferences {
    /// No preferences.
    pub const NONE: Preferences = Preferences {
        name: None,
        place: None,
        units: None,
    };

    /// The user's name, as it was stored (leading capital added).
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The remembered place, as stored: the user's spelling with a leading
    /// capital ("Sevilla", "New York"). Resolving it to a zone is the
    /// reading skill's business.
    #[must_use]
    pub fn place(&self) -> Option<&str> {
        self.place.as_deref()
    }

    /// The remembered unit system.
    #[must_use]
    pub const fn units(&self) -> Option<Units> {
        self.units
    }

    /// Whether nothing is stored.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.name.is_none() && self.place.is_none() && self.units.is_none()
    }

    /// The same preferences with a name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The same preferences with a place.
    #[must_use]
    pub fn with_place(mut self, place: impl Into<String>) -> Self {
        self.place = Some(place.into());
        self
    }

    /// The same preferences with a unit system.
    #[must_use]
    pub const fn with_units(mut self, units: Units) -> Self {
        self.units = Some(units);
        self
    }

    /// Applies one change; whether anything changed.
    pub fn apply(&mut self, change: &PreferenceChange) -> bool {
        let before = self.clone();
        match change {
            PreferenceChange::Name(name) => self.name.clone_from(name),
            PreferenceChange::Place(place) => self.place.clone_from(place),
            PreferenceChange::Units(units) => self.units = *units,
        }
        *self != before
    }
}

/// A change to [`Preferences`] a handler asks for: `Some` stores the value,
/// `None` forgets it. Applied by the engine after the handler answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferenceChange {
    /// The user's name.
    Name(Option<String>),
    /// The remembered place.
    Place(Option<String>),
    /// The unit system.
    Units(Option<Units>),
}

/// Everything a handler may use besides the request itself, and nothing
/// else (docs/SKILLS.md section 2): the injected clock, the caller's place,
/// the turn's random source, this conversation's state, the user's
/// preferences, the installed skills and, with the `knowledge` feature, the
/// knowledge store (read-only). A handler reads no system clock, no
/// environment, no files and no global state.
#[derive(Debug)]
pub struct Context<'a> {
    now: Timestamp,
    place: Option<&'a Place>,
    random: Random,
    conversation: &'a ConversationState,
    preferences: &'a Preferences,
    skills: &'a [Manifest],
    #[cfg(feature = "knowledge")]
    knowledge: Option<&'a crate::knowledge::Knowledge>,
    /// Slots the handler reported for the outcome.
    reported: Slots,
    /// Whether the request was recognised by the classifier, not a rule.
    guessed: bool,
    /// Preference changes the handler asked for.
    changes: Vec<PreferenceChange>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        now: Timestamp,
        place: Option<&'a Place>,
        random: Random,
        conversation: &'a ConversationState,
        preferences: &'a Preferences,
        skills: &'a [Manifest],
    ) -> Self {
        Self {
            now,
            place,
            random,
            conversation,
            preferences,
            skills,
            #[cfg(feature = "knowledge")]
            knowledge: None,
            reported: Slots::new(),
            guessed: false,
            changes: Vec::new(),
        }
    }

    /// A context outside any engine, for a handler's unit tests: a fresh
    /// conversation, no place, no other skills, and `random` as the source
    /// (`Random::seeded(0)`, or `Random::scripted(…)` to fix the draws).
    #[must_use]
    pub fn standalone(now: Timestamp, random: Random) -> Context<'static> {
        static NEW: ConversationState = ConversationState::NEW;
        Context::new(now, None, random, &NEW, &Preferences::NONE, &[])
    }

    /// The same context with these preferences, for a handler's unit tests.
    #[must_use]
    pub fn with_preferences(mut self, preferences: &'a Preferences) -> Self {
        self.preferences = preferences;
        self
    }

    /// The same context with a knowledge store: what the engine passes when
    /// it has one ([`crate::Engine::use_knowledge`]), and what a handler's
    /// unit tests pass (`Knowledge::from_source` of the seed).
    #[cfg(feature = "knowledge")]
    #[must_use]
    pub fn with_knowledge(mut self, knowledge: &'a crate::knowledge::Knowledge) -> Self {
        self.knowledge = Some(knowledge);
        self
    }

    /// The knowledge store: `user.db` and the attached packs, read-only
    /// (docs/KNOWLEDGE.md). `None` when the engine has none; a handler then
    /// says it does not know, as for an unknown word.
    #[cfg(feature = "knowledge")]
    #[must_use]
    pub const fn knowledge(&self) -> Option<&'a crate::knowledge::Knowledge> {
        self.knowledge
    }

    /// The same context as for a request the classifier recognised, for a
    /// handler's unit tests.
    #[must_use]
    pub fn as_guess(mut self) -> Self {
        self.guessed = true;
        self
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

    /// What Talos remembers about the user: name, place, units. The
    /// remembered place is the reference place when the message names none
    /// (docs/spec/state.md, "Place memory").
    #[must_use]
    pub const fn preferences(&self) -> &Preferences {
        self.preferences
    }

    /// Whether this request was recognised by the classifier and not by a
    /// rule, a command or a taught phrase (an answer to a question keeps
    /// what the question's request was). A handler that changes something
    /// asks the user to confirm before acting on a guess (DESIGN.md
    /// section 11).
    #[must_use]
    pub const fn guessed(&self) -> bool {
        self.guessed
    }

    /// Asks the engine to store or forget a preference. Applied, and saved,
    /// only if the handler answers ([`SkillResult::Answer`]); a turn that
    /// changes nothing writes nothing.
    pub fn remember(&mut self, change: PreferenceChange) {
        self.changes.push(change);
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

    /// Takes the preference changes asked for, leaving none.
    pub(crate) fn take_changes(&mut self) -> Vec<PreferenceChange> {
        std::mem::take(&mut self.changes)
    }

    /// Marks the next call as a guess, or not.
    pub(crate) fn set_guessed(&mut self, guessed: bool) {
        self.guessed = guessed;
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
