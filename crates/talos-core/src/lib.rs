//! The Talos engine.
//!
//! Responsible for the turn, the explicit pipeline, NLU, dialog and rendering
//! (DESIGN.md section 2). Synchronous, with no async runtime, HTTP stack or
//! network dependency, so it stays embeddable and its tests fast. It knows no
//! concrete skill: skills are registered as a manifest plus a handler.
//!
//! The one entry point is [`Engine::respond`]; the pipeline it runs is
//! written out in order in one function (ADR 0004). The contract for skill
//! authors ([`Skill`], [`Context`], [`Call`], [`SkillResult`]) is described
//! in `docs/SKILL_AUTHORING.md`.

pub mod check;
pub mod civil;
pub mod classifier;
mod dialog;
mod engine;
mod error;
pub mod extract;
pub mod fallback;
pub mod golden;
mod lang;
pub mod language;
pub mod manifest;
pub mod normalise;
mod outcome;
mod patterns;
pub mod random;
mod reflector;
pub mod render;
pub mod rules;
mod safety;
pub mod skill;
mod state;
mod turn;
mod value;

pub use civil::{Date, Weekday};
pub use engine::{Config, Engine};
pub use error::Error;
pub use lang::{Lang, UnknownLang};
pub use manifest::Manifest;
pub use outcome::{IntentId, Outcome, Reason, Status, Trace, TraceStep};
pub use random::{Coin, Draw, Random};
pub use safety::{DEFAULT_EMERGENCY, Emergency, SPAIN};
pub use skill::{
    Call, Choice, Context, Expectation, Expects, Message, Preferences, Registration, Skill,
    SkillError, SkillResult,
};
pub use state::ConversationState;
pub use turn::{ConvId, Place, Timestamp, Turn};
pub use value::{Number, PlaceRef, Problem, SlotType, Slots, Value};
