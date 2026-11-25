//! What the `talos` front ends share (docs/API.md, ADR 0010): the clock they
//! read before each turn, the outcome's wire form, and, behind the `server`
//! feature, the HTTP server. Each front end is a thin shell around
//! `Engine::respond`; nothing here decides anything about a turn.

pub mod clock;
#[cfg(feature = "server")]
pub mod server;
pub mod wire;

/// The version every front end reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
