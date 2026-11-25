//! The engine's own fixed replies for an `Unhandled` turn that neither a
//! skill nor the reflector worded: an empty message, a failure, a skill
//! that wanted to ask back (docs/spec/conversation.md section 4). Everything
//! else that nothing answered goes to the reflector (`crate::reflector`),
//! whose scripts hold the catch-all and the guards' replies.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_reply_in_every_language() {
        let reasons = [
            Reason::EMPTY,
            Reason::CANNOT,
            Reason::CANNOT_ASK,
            Reason::FAILED,
        ];
        for lang in Lang::ALL {
            for reason in &reasons {
                assert_ne!(reply(lang, reason), "");
            }
        }
        assert_eq!(reply(Lang::En, &Reason::EMPTY), "I didn't get any text.");
        assert_eq!(reply(Lang::Es, &Reason::CANNOT_ASK), "No sé hacer eso.");
    }
}
