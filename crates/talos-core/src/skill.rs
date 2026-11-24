//! The handler half of a skill (docs/SKILLS.md section 2).
//!
//! A handler receives a recognised intent and returns a structured message: a
//! reply id from its own manifest and variables. It never builds sentences.

use crate::render::Vars;
use crate::{IntentId, Lang, Manifest, Slots, Turn};

/// A skill as handed to the engine: its manifest and its handler.
pub type Registration = (Manifest, Box<dyn Skill>);

/// A reply to render: a template id from the skill's manifest, and variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The `[reply.<id>]` to use.
    pub id: String,
    /// Values for the template's `{name}` placeholders.
    pub vars: Vars,
}

impl Message {
    /// A message with no variables.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
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
    /// Facts to render; the turn is `Answered`.
    Answer(Message),
    /// A question back to the user.
    Ask(Expectation, Message),
    /// Recognised, but Talos cannot provide it; `Unhandled`, intent kept.
    Unable(Message),
    /// Not mine after all; the next candidate is tried.
    Decline,
    /// Something went wrong.
    Fail(SkillError),
}

/// One recognised request, as a handler sees it.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    /// The recognised intent.
    pub intent: &'a IntentId,
    /// What was extracted.
    pub slots: &'a Slots,
    /// The language of the turn, already decided.
    pub lang: Lang,
    /// The turn itself, for `now`, the original text and the conversation.
    pub turn: &'a Turn,
}

/// A built-in skill's handler.
pub trait Skill: Send + Sync {
    /// The skill id; must equal its manifest's `[skill] id`.
    fn id(&self) -> &'static str;

    /// Handles one recognised request. Must not panic.
    fn handle(&self, call: &Call<'_>) -> SkillResult;
}
