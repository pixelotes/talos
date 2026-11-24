//! The outcome: exactly one per turn, with an explicit status (DESIGN.md
//! section 3, ADR 0009).

use std::collections::BTreeMap;
use std::fmt;

use crate::Lang;

/// Whether Talos handled the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// A complete and correct response, including a correct "no".
    Answered,
    /// The reflector matched a real sentence shape.
    Deflected,
    /// Talos could not provide what was asked; a host may pass the turn on.
    Unhandled,
}

impl Status {
    /// The lower-case name used in transcripts and on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Status::Answered => "answered",
            Status::Deflected => "deflected",
            Status::Unhandled => "unhandled",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a turn is [`Status::Unhandled`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    /// The message was empty or whitespace only.
    Empty,
    /// No stage recognised the message.
    NotRecognised,
    /// A skill recognised it and could not provide it.
    Unable,
    /// A skill wanted to ask back, which this version cannot do yet.
    CannotAsk,
    /// A skill or the renderer failed.
    Failed,
}

impl Reason {
    /// The lower-case name used on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Reason::Empty => "empty",
            Reason::NotRecognised => "unknown",
            Reason::Unable => "cannot",
            Reason::CannotAsk => "cannot_ask",
            Reason::Failed => "failed",
        }
    }
}

/// A fixed intent identifier such as `smalltalk.greet` (docs/spec/README.md).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntentId(pub String);

impl IntentId {
    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The skill that owns the intent: the part before the first dot.
    #[must_use]
    pub fn skill(&self) -> &str {
        self.0
            .split_once('.')
            .map_or(self.0.as_str(), |(skill, _)| skill)
    }
}

impl fmt::Display for IntentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What was extracted from the message, by name. Ordered, so that outcomes
/// compare and print deterministically.
pub type Slots = BTreeMap<String, String>;

/// One stage's decision, for `talos explain` and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    /// The stage that decided.
    pub stage: &'static str,
    /// What it decided and why.
    pub note: String,
}

/// Stage-by-stage decisions, in pipeline order. Minimal in Slice A.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    /// The steps, in order.
    pub steps: Vec<TraceStep>,
}

/// The engine's answer to one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The text to show.
    pub reply: String,
    /// Whether the message was handled.
    pub status: Status,
    /// The language of the turn, and of the reply.
    pub lang: Lang,
    /// The recognised intent; kept even when the status is `Unhandled`.
    pub intent: Option<IntentId>,
    /// What was extracted.
    pub slots: Slots,
    /// Confidence of the intent recognition (not of any fact): 1.0 for a rule.
    pub confidence: f32,
    /// Why the turn is `Unhandled`, when it is.
    pub reason: Option<Reason>,
    /// Talos asked back and expects the next message.
    pub pending: bool,
    /// True only when an external skill used a model.
    pub generated: bool,
    /// The trace, when the turn asked for one.
    pub trace: Option<Trace>,
}
