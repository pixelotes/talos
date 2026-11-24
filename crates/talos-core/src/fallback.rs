//! Stage 7 stub: the honest `Unhandled` replies (docs/spec/conversation.md
//! section 4). The reflector replaces the not-recognised reply in Slice D;
//! until then every unrecognised message gets the same plain sentence.

use crate::{Lang, Reason};

/// The fixed reply for an `Unhandled` turn with no skill message of its own.
#[must_use]
pub fn reply(lang: Lang, reason: Reason) -> &'static str {
    match (lang, reason) {
        (Lang::Es, Reason::Empty) => "No he recibido ningún texto.",
        (Lang::En, Reason::Empty) => "I didn't receive any text.",
        (Lang::Es, Reason::Failed) => "Algo ha fallado y no puedo responder a eso.",
        (Lang::En, Reason::Failed) => "Something went wrong and I can't answer that.",
        (Lang::Es, Reason::NotRecognised | Reason::Unable | Reason::CannotAsk) => {
            "No sé hacer eso."
        }
        (Lang::En, Reason::NotRecognised | Reason::Unable | Reason::CannotAsk) => {
            "I can't do that."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_reply_in_every_language() {
        let reasons = [
            Reason::Empty,
            Reason::NotRecognised,
            Reason::Unable,
            Reason::CannotAsk,
            Reason::Failed,
        ];
        for lang in Lang::ALL {
            for reason in reasons {
                assert_ne!(reply(lang, reason), "");
            }
        }
        assert_eq!(reply(Lang::Es, Reason::NotRecognised), "No sé hacer eso.");
        assert_eq!(reply(Lang::En, Reason::NotRecognised), "I can't do that.");
    }
}
