//! Stage 7 stub: the honest `Unhandled` replies (docs/spec/conversation.md
//! section 4). The reflector replaces the catch-all reply in Slice D; until
//! then every unrecognised message gets the same plain sentence.

use crate::{Lang, Reason};

/// The fixed reply for an `Unhandled` turn with no skill message of its own.
#[must_use]
pub fn reply(lang: Lang, reason: &Reason) -> &'static str {
    let empty = *reason == Reason::EMPTY;
    let failed = *reason == Reason::FAILED;
    match lang {
        Lang::Es if empty => "No he recibido ningún texto.",
        Lang::En if empty => "I didn't receive any text.",
        Lang::Es if failed => "Algo ha fallado y no puedo responder a eso.",
        Lang::En if failed => "Something went wrong and I can't answer that.",
        Lang::Es => "No sé hacer eso.",
        Lang::En => "I can't do that.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_reply_in_every_language() {
        let reasons = [
            Reason::EMPTY,
            Reason::CATCH_ALL,
            Reason::CANNOT,
            Reason::CANNOT_ASK,
            Reason::FAILED,
            Reason::new("opinion"),
        ];
        for lang in Lang::ALL {
            for reason in &reasons {
                assert_ne!(reply(lang, reason), "");
            }
        }
        assert_eq!(reply(Lang::Es, &Reason::CATCH_ALL), "No sé hacer eso.");
        assert_eq!(reply(Lang::En, &Reason::CATCH_ALL), "I can't do that.");
    }
}
