//! The outcome: exactly one per turn, with an explicit status (DESIGN.md
//! section 3, ADR 0009).

use std::borrow::Cow;
use std::fmt;

use crate::Lang;
use crate::value::Slots;

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

/// Why a turn is [`Status::Unhandled`], or which sentence shape the
/// reflector recognised for a [`Status::Deflected`] one (docs/API.md 2.2).
///
/// An open set of lower-case, hyphenated names: the engine's own are the
/// constants below; the reflector adds the shape names of
/// docs/spec/reflector.md (`opinion`, `generative`, `catch-all`, …).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reason(Cow<'static, str>);

impl Reason {
    /// The message was empty or whitespace only.
    pub const EMPTY: Reason = Reason(Cow::Borrowed("empty"));
    /// Nothing recognised the message (the reflector's bare catch-all).
    pub const CATCH_ALL: Reason = Reason(Cow::Borrowed("catch-all"));
    /// A skill recognised it and could not provide it.
    pub const CANNOT: Reason = Reason(Cow::Borrowed("cannot"));
    /// A skill or the renderer failed.
    pub const FAILED: Reason = Reason(Cow::Borrowed("failed"));
    /// An open question was cancelled ("déjalo", "never mind"); `Answered`.
    pub const CANCELLED: Reason = Reason(Cow::Borrowed("cancelled"));
    /// A hesitation ("a ver", "hold on") while a question is open: it stays
    /// open; `Answered`.
    pub const HEDGE: Reason = Reason(Cow::Borrowed("hedge"));
    /// A bare acknowledgement ("vale", "ok") with nothing to act on;
    /// `Answered`.
    pub const ACKNOWLEDGEMENT: Reason = Reason(Cow::Borrowed("acknowledgement"));
    /// A bare "sí"/"no" with no question open (docs/spec/conversation.md
    /// 3.8).
    pub const NOTHING_PENDING: Reason = Reason(Cow::Borrowed("nothing-pending"));
    /// An open question got no usable answer twice and was dropped.
    pub const GAVE_UP: Reason = Reason(Cow::Borrowed("gave-up"));
    /// A message starting with `/` that names no installed command.
    pub const UNKNOWN_COMMAND: Reason = Reason(Cow::Borrowed("unknown-command"));
    /// A command that takes no argument was given one.
    pub const COMMAND_ARGUMENT: Reason = Reason(Cow::Borrowed("command-argument"));

    /// A reason by name, e.g. a reflector shape.
    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    /// The name used in outcome lines and on the wire.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which cheaper source answered when the better one had nothing
/// (DESIGN.md principle 8, "Degrade cheaply"). An outcome carries one only
/// when the reply rests on such a source, and the reply says so in words
/// too; a host that has something better (a translator, an encyclopedia)
/// can read it and step in.
///
/// An open set of lower-case, hyphenated names, like [`Reason`]; the
/// constants are the ones the built-in skills use.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fallback(Cow<'static, str>);

impl Fallback {
    /// A translation from the word-pair lexicon, because the concept store
    /// has no word for it (docs/spec/knowledge.md, translate).
    pub const WORD_PAIRS: Fallback = Fallback(Cow::Borrowed("word-pairs"));
    /// A phrase translated word by word, because it is neither a word nor a
    /// stock phrase; labelled approximate, and the turn is `Unhandled`.
    pub const WORD_BY_WORD: Fallback = Fallback(Cow::Borrowed("word-by-word"));
    /// What a word's claims say about it (its class, what it is part of),
    /// because there is no definition in the conversation's language.
    pub const RELATIONS: Fallback = Fallback(Cow::Borrowed("relations"));
    /// The definition in the other language, given when asked for because
    /// there is none in the conversation's language.
    pub const OTHER_LANGUAGE: Fallback = Fallback(Cow::Borrowed("other-language"));

    /// A fallback by name.
    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    /// The name used in outcome lines and on the wire.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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

/// One stage's decision, for `talos explain` and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    /// The stage that decided: `normalise`, `language`, `safety`, `rules`,
    /// `slots`, `dispatch`, `render`, `reflector`, `fallback`, `random`,
    /// `state`.
    pub stage: &'static str,
    /// What it decided and why, in one line.
    pub note: String,
}

/// Stage-by-stage decisions, in pipeline order, returned when the turn asks
/// for them. Building it costs nothing when it is not asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    /// The steps, in order.
    pub steps: Vec<TraceStep>,
    /// Slots the manifest marks internal: they steer a handler and are left
    /// out of [`Outcome::slots`].
    pub internal_slots: Slots,
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
    /// What was extracted, typed, without the slots the manifest marks
    /// internal; a handler may fill in defaults it applied.
    pub slots: Slots,
    /// Confidence of the intent recognition (not of any fact): 1.0 for a rule.
    pub confidence: f32,
    /// Why the turn is `Unhandled`, when it is; for a reflector turn, the
    /// sentence shape (`Deflected`) or the guard (`Unhandled`), with no
    /// intent (docs/spec/reflector.md 2.2).
    pub reason: Option<Reason>,
    /// A question is open after this turn: Talos asked back (or the user
    /// hesitated and the question stays open), and the next message of
    /// the conversation is offered to it first.
    pub pending: bool,
    /// The slot Talos asked for, when this turn asked back.
    pub asks: Option<String>,
    /// True only when an external skill used a model.
    pub generated: bool,
    /// The cheaper source the reply rests on, when the best one had
    /// nothing (DESIGN.md principle 8); `None` when the best source
    /// answered, or nothing did.
    pub fallback: Option<Fallback>,
    /// The trace, when the turn asked for one.
    pub trace: Option<Trace>,
}

impl Outcome {
    /// The outcome line of transcripts and golden files, without the arrow:
    /// `status · intent · slot=value slot=value` when an intent was
    /// recognised, `status · reason` when none was, and the bare status when
    /// there is neither. Slots are in name order; `extra` adds slots (the
    /// internal ones, when a golden file asks for them). A turn that asked
    /// back is `asks · intent · slot` (docs/spec/README.md). A reply that
    /// rests on a fallback source ends with `· fallback <name>`.
    #[must_use]
    pub fn line(&self, extra: Option<&Slots>) -> String {
        if let (Some(slot), Some(intent)) = (&self.asks, &self.intent) {
            return format!("asks · {intent} · {slot}");
        }
        let mut line = self.status.as_str().to_owned();
        match (&self.intent, &self.reason) {
            (Some(intent), _) => {
                line.push_str(" · ");
                line.push_str(intent.as_str());
                let mut slots = self.slots.clone();
                if let Some(extra) = extra {
                    slots.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                for (i, (name, value)) in slots.iter().enumerate() {
                    line.push_str(if i == 0 { " · " } else { " " });
                    line.push_str(name);
                    line.push('=');
                    line.push_str(&value.to_string());
                }
            }
            (None, Some(reason)) => {
                line.push_str(" · ");
                line.push_str(reason.as_str());
            }
            (None, None) => {}
        }
        if let Some(fallback) = &self.fallback {
            line.push_str(" · fallback ");
            line.push_str(fallback.as_str());
        }
        line
    }
}
