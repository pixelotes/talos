//! The Talos engine.
//!
//! Responsible for the turn, the explicit pipeline, NLU, dialog and rendering
//! (DESIGN.md section 2). Synchronous, with no async runtime, HTTP stack or
//! network dependency, so it stays embeddable and its tests fast. It knows no
//! concrete skill: skills are registered as a manifest plus a handler.
//!
//! The one entry point is [`Engine::respond`]; the pipeline it runs is
//! written out in order in that function (ADR 0004).

mod engine;
mod error;
pub mod fallback;
mod lang;
pub mod language;
pub mod manifest;
pub mod normalise;
mod outcome;
pub mod render;
pub mod rules;
pub mod skill;
mod turn;

pub use engine::{Config, Engine};
pub use error::Error;
pub use lang::{Lang, UnknownLang};
pub use manifest::Manifest;
pub use outcome::{IntentId, Outcome, Reason, Slots, Status, Trace, TraceStep};
pub use skill::{Call, Expectation, Message, Registration, Skill, SkillError, SkillResult};
pub use turn::{ConvId, Place, Timestamp, Turn};
