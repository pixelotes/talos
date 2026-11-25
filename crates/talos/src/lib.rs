//! What the `talos` front ends share (docs/API.md, ADR 0010): the clock they
//! read before each turn, where the user's data and the packs are kept, the
//! outcome's wire form, `talos packs` (behind the `knowledge` feature) and,
//! behind the `server` feature, the HTTP server. Each front end is a thin shell around
//! `Engine::respond`; nothing here decides anything about a turn.

pub mod clock;
pub mod data;
#[cfg(feature = "knowledge")]
pub mod packs;
#[cfg(feature = "server")]
pub mod server;
pub mod wire;

/// The version every front end reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
