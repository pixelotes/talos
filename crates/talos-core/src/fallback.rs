//! The engine's own fixed replies: for an `Unhandled` turn that neither a
//! skill nor the reflector worded (an empty message, a failure), and for
//! the dialog moves that belong to no skill (docs/spec/conversation.md
//! sections 3, 4 and 6). Everything else that nothing answered goes to the
//! reflector (`crate::reflector`), whose scripts hold the catch-all and the
//! guards' replies.

use crate::{Lang, Reason};

/// The fixed reply for an engine-level `Unhandled` turn.
#[must_use]
pub fn reply(lang: Lang, reason: &Reason) -> &'static str {
    let empty = *reason == Reason::EMPTY;
    let failed = *reason == Reason::FAILED;
    match lang {
        Lang::Es if empty => "No he recibido ningún texto.",
        Lang::En if empty => "I didn't get any text.",
        Lang::Es if failed => "Algo ha fallado y no puedo responder a eso.",
        Lang::En if failed => "Something went wrong and I can't answer that.",
        Lang::Es => "No sé hacer eso.",
        Lang::En => "I can't do that.",
    }
}

/// A dialog move the engine answers itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// An open question was cancelled.
    Cancelled,
    /// An acknowledgement with nothing to act on, or a cancellation word
    /// with nothing open.
    Acknowledged,
    /// A bare yes or no with no question open.
    NothingPending,
    /// A question got no usable answer twice and is dropped.
    GaveUp,
    /// The prefix put before a question asked a second time.
    NotUnderstood,
}

/// The fixed reply for a dialog move.
#[must_use]
pub fn dialog(lang: Lang, kind: Move) -> &'static str {
    match (lang, kind) {
        (Lang::Es, Move::Cancelled) => "Vale, lo dejo.",
        (Lang::En, Move::Cancelled) => "Okay, dropped.",
        (Lang::Es, Move::Acknowledged) => "Vale.",
        (Lang::En, Move::Acknowledged) => "Okay.",
        (Lang::Es, Move::NothingPending) => "No tengo ninguna pregunta pendiente.",
        (Lang::En, Move::NothingPending) => "There's no open question to answer.",
        (Lang::Es, Move::GaveUp) => "No he conseguido entenderlo, así que lo dejo.",
        (Lang::En, Move::GaveUp) => "I couldn't work that out, so I'll leave it.",
        (Lang::Es, Move::NotUnderstood) => "No te he entendido.",
        (Lang::En, Move::NotUnderstood) => "I didn't catch that.",
    }
}

/// The reply to a hesitation while a question is open; `nth` counts the
/// turns since the question (0 for the first), so the wording moves on
/// (docs/spec/conversation.md 3.7).
#[must_use]
pub fn hedge(lang: Lang, nth: u32) -> &'static str {
    let replies: [&str; 3] = match lang {
        Lang::Es => ["Tómate tu tiempo.", "Vale.", "Aquí sigo."],
        Lang::En => ["Take your time.", "Okay.", "I'm still here."],
    };
    replies[usize::try_from(nth).unwrap_or(0) % replies.len()]
}

/// The reply to a command no installed skill declares; `help` is the name
/// of the command that lists them, when one is installed (ADR 0016).
#[must_use]
pub fn unknown_command(lang: Lang, name: &str, help: Option<&str>) -> String {
    match (lang, help) {
        (Lang::Es, Some(help)) => {
            format!("No conozco el comando {name}. Escribe {help} para ver los que hay.")
        }
        (Lang::En, Some(help)) => {
            format!("I don't know the command {name}. Type {help} to see the ones there are.")
        }
        (Lang::Es, None) => format!("No conozco el comando {name}."),
        (Lang::En, None) => format!("I don't know the command {name}."),
    }
}

/// The reply to an argument given to a command that takes none.
#[must_use]
pub fn command_argument(lang: Lang, name: &str) -> String {
    match lang {
        Lang::Es => format!("El comando {name} no lleva nada detrás."),
        Lang::En => format!("The command {name} takes nothing after it."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_reply_in_every_language() {
        let reasons = [Reason::EMPTY, Reason::CANNOT, Reason::FAILED];
        let moves = [
            Move::Cancelled,
            Move::Acknowledged,
            Move::NothingPending,
            Move::GaveUp,
            Move::NotUnderstood,
        ];
        for lang in Lang::ALL {
            for reason in &reasons {
                assert_ne!(reply(lang, reason), "");
            }
            for kind in moves {
                assert_ne!(dialog(lang, kind), "");
            }
            assert_ne!(hedge(lang, 7), "");
        }
        assert_eq!(reply(Lang::En, &Reason::EMPTY), "I didn't get any text.");
        assert_eq!(
            unknown_command(Lang::Es, "/clac", Some("/ayuda")),
            "No conozco el comando /clac. Escribe /ayuda para ver los que hay."
        );
        assert_eq!(hedge(Lang::Es, 0), "Tómate tu tiempo.");
    }
}
