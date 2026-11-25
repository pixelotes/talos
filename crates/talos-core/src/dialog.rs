//! Stage 2's vocabulary (docs/spec/conversation.md section 3, DESIGN.md
//! section 4): the small dialog moves that belong to no skill —
//! cancellations, hesitations, acknowledgements — the framing around an
//! answer to an open question, and the shape of a command line (ADR 0016).
//! Everything here reads the match form (lower case, no accents).

use std::ops::Range;

use crate::normalise::Normalised;

/// Punctuation and symbols around words that carry nothing here.
fn bare(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || ",.;:!?¡¿\"'()…-".contains(c))
}

/// The message as single-spaced words without punctuation: "déjalo, da
/// igual." is `dejalo da igual`.
fn plain(folded: &str) -> String {
    folded
        .split(|c: char| c.is_whitespace() || ",.;:!?¡¿\"()…".contains(c))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Phrases that cancel an open question (3.5), and the "none of them"
/// that declines a choice (2.7).
const CANCEL: &[&str] = &[
    "dejalo",
    "dejalo estar",
    "dejalo ya",
    "olvidalo",
    "olvidalo ya",
    "no importa",
    "da igual",
    "da lo mismo",
    "nada",
    "nada nada",
    "cancela",
    "cancelalo",
    "cancelar",
    "paso",
    "mejor no",
    "no te preocupes",
    "lo que sea",
    "ninguna",
    "ninguno",
    "ni idea",
    "never mind",
    "nevermind",
    "forget it",
    "forget about it",
    "drop it",
    "cancel",
    "cancel that",
    "leave it",
    "skip it",
    "doesn't matter",
    "doesnt matter",
    "it doesn't matter",
    "it doesnt matter",
    "none",
    "none of them",
    "no idea",
    "whatever",
];

/// Words that may sit next to a cancellation without changing it: "no,
/// déjalo", "bah, da igual", "oh, never mind".
const CANCEL_COMPANIONS: &[&str] = &[
    "no", "bah", "bueno", "pues", "oh", "ah", "nah", "vale", "ok",
];

/// Whether the message cancels an open question: a cancellation phrase,
/// alone or with others ("déjalo, da igual") and a few companions ("no,
/// déjalo").
#[must_use]
pub(crate) fn is_cancel(folded: &str) -> bool {
    let parts: Vec<String> = folded
        .split([',', ';', '.', '!'])
        .map(plain)
        .filter(|p| !p.is_empty())
        .collect();
    !parts.is_empty()
        && parts.iter().any(|p| CANCEL.contains(&p.as_str()))
        && parts
            .iter()
            .all(|p| CANCEL.contains(&p.as_str()) || CANCEL_COMPANIONS.contains(&p.as_str()))
}

/// Hesitations and requests for time (3.7).
const HEDGES: &[&str] = &[
    "a ver",
    "vamos a ver",
    "espera",
    "espera un momento",
    "un momento",
    "un segundo",
    "un sec",
    "dejame pensar",
    "deja que piense",
    "dejame que piense",
    "pues",
    "este",
    "bueno",
    "wait",
    "wait a moment",
    "let me think",
    "hold on",
    "one moment",
    "one sec",
    "just a sec",
    "just a moment",
    "well",
];

/// Whether a word is a hum: "mmm", "hmm", "uh", "um", "eh", "erm".
fn hum(word: &str) -> bool {
    let shapes: [&[&str]; 6] = [
        &["m"],
        &["h", "m"],
        &["u", "h"],
        &["u", "m"],
        &["e", "h"],
        &["e", "r", "m"],
    ];
    let mut groups: Vec<(char, usize)> = Vec::new();
    for c in word.chars() {
        match groups.last_mut() {
            Some((last, n)) if *last == c => *n += 1,
            _ => groups.push((c, 1)),
        }
    }
    let letters: Vec<String> = groups.iter().map(|(c, _)| c.to_string()).collect();
    let letters: Vec<&str> = letters.iter().map(String::as_str).collect();
    !word.is_empty() && shapes.contains(&letters.as_slice())
}

/// Whether the message is a hesitation: "mmm", "a ver", "espera", "hmm",
/// "wait", "let me think", "hold on", possibly with a hum before it.
#[must_use]
pub(crate) fn is_hedge(folded: &str) -> bool {
    let text = plain(folded);
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    let rest: Vec<&str> = words.iter().copied().skip_while(|w| hum(w)).collect();
    if words.is_empty() {
        return false;
    }
    rest.is_empty() || HEDGES.contains(&rest.join(" ").as_str())
}

/// Acknowledgements (1.2): answered minimally, never with a question.
const ACKS: &[&str] = &[
    "ok",
    "okay",
    "okey",
    "oki",
    "vale",
    "vale vale",
    "genial",
    "perfecto",
    "de acuerdo",
    "entendido",
    "muy bien",
    "estupendo",
    "guay",
    "great",
    "cool",
    "alright",
    "all right",
    "got it",
    "fine",
    "nice",
    "perfect",
    "👍",
    "👌",
    "🆗",
];

/// Whether the message is a bare acknowledgement: "ok", "vale", "genial",
/// "👍".
#[must_use]
pub(crate) fn is_ack(folded: &str) -> bool {
    let text = plain(folded);
    ACKS.contains(&text.as_str())
        || (!text.is_empty() && text.chars().all(|c| "👍👌🆗".contains(c)))
}

/// Whether the message begins with an acknowledgement word ("vale,
/// gracias", "ok thanks").
#[must_use]
pub(crate) fn starts_with_ack(folded: &str) -> bool {
    plain(folded)
        .split(' ')
        .next()
        .is_some_and(|first| ACKS.contains(&first))
}

/// Words that frame an answer at its start: "pues Tokio", "no, en Osaka",
/// "perdona, Osaka", "sorry, I meant Osaka" (3.2).
const LEADS: &[&[&str]] = &[
    &["pues"],
    &["bueno"],
    &["vale"],
    &["ok"],
    &["no"],
    &["perdona"],
    &["perdon"],
    &["quiero", "decir"],
    &["me", "refiero", "a"],
    &["digo"],
    &["oh"],
    &["ah"],
    &["well"],
    &["sorry"],
    &["i", "meant"],
    &["i", "mean"],
    &["oops"],
];

/// Prepositions before a free-text answer: "a millas", "to miles", "entre
/// pizza y sushi". Not dropped before a typed value, where they mean
/// something ("in 3 days").
const PREPOSITIONS: &[&[&str]] = &[
    &["a"],
    &["al"],
    &["en"],
    &["entre"],
    &["in"],
    &["into"],
    &["to"],
    &["between"],
];

/// Words that frame an answer at its end: "Tokio, anda", "in Tokyo
/// please", "London, thanks".
const TAILS: &[&[&str]] = &[
    &["por", "favor"],
    &["porfa"],
    &["gracias"],
    &["anda"],
    &["please"],
    &["thanks"],
    &["thank", "you"],
];

/// The part of the message that is the answer itself, as a byte range of
/// the match form: framing words at either end removed (3.2). The whole
/// message if nothing is left. With `prepositions`, a leading preposition
/// goes too (for a free-text slot).
#[must_use]
pub(crate) fn frame(message: &Normalised, prepositions: bool) -> Range<usize> {
    let folded = message.folded.as_str();
    // Words with their byte ranges, punctuation trimmed.
    let mut words: Vec<(Range<usize>, &str)> = Vec::new();
    let mut offset = 0;
    for piece in folded.split(' ') {
        let word = bare(piece);
        if !word.is_empty() {
            let start = offset + piece.find(word).unwrap_or(0);
            words.push((start..start + word.len(), word));
        }
        offset += piece.len() + 1;
    }
    let texts: Vec<&str> = words.iter().map(|(_, w)| *w).collect();
    let (mut from, mut to) = (0, texts.len());
    'lead: loop {
        let preps: &[&[&str]] = if prepositions { PREPOSITIONS } else { &[] };
        for lead in LEADS.iter().chain(preps) {
            if to - from > lead.len() && texts[from..from + lead.len()] == **lead {
                from += lead.len();
                continue 'lead;
            }
        }
        break;
    }
    'tail: loop {
        for tail in TAILS {
            if to - from > tail.len() && texts[to - tail.len()..to] == **tail {
                to -= tail.len();
                continue 'tail;
            }
        }
        break;
    }
    match (
        words.get(from),
        to.checked_sub(1).and_then(|last| words.get(last)),
    ) {
        (Some((first, _)), Some((last, _))) if from < to => first.start..last.end,
        _ => 0..folded.len(),
    }
}

/// A message read as a command line: `/name the rest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandLine {
    /// The name, slash included, in the match form (`/calc`).
    pub(crate) name: String,
    /// The name as a byte range of the match form.
    pub(crate) name_range: Range<usize>,
    /// The rest of the line, trimmed, as a byte range of the match form;
    /// empty when nothing follows the name.
    pub(crate) argument: Range<usize>,
}

/// The command a message is, if its first character is `/` (ADR 0016).
#[must_use]
pub(crate) fn command_line(message: &Normalised) -> Option<CommandLine> {
    let folded = message.folded.as_str();
    if !folded.starts_with('/') {
        return None;
    }
    let end = folded.find(char::is_whitespace).unwrap_or(folded.len());
    let rest = &folded[end..];
    let start = end + (rest.len() - rest.trim_start().len());
    let stop = start + rest.trim().len();
    Some(CommandLine {
        name: folded[..end].to_owned(),
        name_range: 0..end,
        argument: start..stop.max(start),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalise::normalise;

    fn folded(text: &str) -> String {
        normalise(text).folded
    }

    #[test]
    fn cancellations() {
        for yes in [
            "déjalo",
            "dejalo",
            "olvídalo",
            "no importa",
            "da igual",
            "nada, nada",
            "cancela",
            "paso",
            "Déjalo, da igual",
            "no, déjalo",
            "never mind",
            "Nevermind.",
            "forget it",
            "cancel that",
            "doesn't matter",
            "none",
            "ni idea",
        ] {
            assert!(is_cancel(&folded(yes)), "{yes}");
        }
        for no in [
            "no",
            "olvida mi nombre",
            "forget my name",
            "nada más que decir",
            "",
        ] {
            assert!(!is_cancel(&folded(no)), "{no}");
        }
    }

    #[test]
    fn hedges_and_acknowledgements() {
        for yes in [
            "mmm",
            "Hmm...",
            "a ver",
            "espera",
            "déjame pensar",
            "uh",
            "hold on",
            "mmm, a ver",
        ] {
            assert!(is_hedge(&folded(yes)), "{yes}");
        }
        for no in ["Tokio", "mamá", "help", ""] {
            assert!(!is_hedge(&folded(no)), "{no}");
        }
        for yes in ["ok", "Vale.", "genial", "👍", "okay!"] {
            assert!(is_ack(&folded(yes)), "{yes}");
        }
        for no in ["sí", "vale, gracias", "ok qué hora es"] {
            assert!(!is_ack(&folded(no)), "{no}");
        }
    }

    fn framed(text: &str) -> String {
        let message = normalise(text);
        message.original_of(frame(&message, false)).to_owned()
    }

    #[test]
    fn framing_is_removed_from_an_answer() {
        assert_eq!(framed("Tokio"), "Tokio");
        assert_eq!(framed("pues Tokio"), "Tokio");
        assert_eq!(framed("Tokio, anda"), "Tokio");
        assert_eq!(framed("tokyo please"), "tokyo");
        assert_eq!(framed("London, thanks"), "London");
        assert_eq!(framed("no, en Osaka"), "en Osaka");
        assert_eq!(framed("sorry, I meant Osaka"), "Osaka");
        assert_eq!(framed("por favor"), "por favor");
    }

    #[test]
    fn command_lines() {
        let line = command_line(&normalise("/Calc  1 + 2 ")).unwrap();
        assert_eq!(line.name, "/calc");
        assert_eq!(line.argument, 6..11);
        let line = command_line(&normalise("/dado")).unwrap();
        assert!(line.argument.is_empty());
        assert_eq!(command_line(&normalise("calc 1 + 2")), None);
    }
}
