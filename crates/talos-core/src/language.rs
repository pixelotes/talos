//! Stage 1: which language a turn is in (ADR 0015,
//! docs/spec/conversation.md section 1).
//!
//! Steps 1, 2 and 4 of ADR 0015: the caller's hint, then words and
//! characters that belong to only one language, then the conversation's
//! language (or the configured default on a fresh conversation). The
//! classifier (step 3) arrives in v0.2.
//!
//! The result is sticky per conversation, with hysteresis:
//!
//! - a conversation whose language was **settled** (by a hint or by
//!   evidence) switches only on clear evidence for the other language: at
//!   least [`SWITCH_WORDS`] marker words of it, and more than of the current
//!   one. "gracias" after English turns, or "hello" after Spanish ones,
//!   switches nothing;
//! - a conversation whose language only **defaulted** (no evidence yet)
//!   takes the first one-sided evidence it sees.

use crate::Lang;

/// Marker words of the other language needed to switch a settled
/// conversation: one word is never enough (conversation.md 1.2).
pub const SWITCH_WORDS: usize = 2;

/// How firmly a conversation holds its language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Settled by a hint or by evidence: switches only on clear evidence.
    Settled,
    /// Taken from the default with no evidence: the next evidence decides.
    Defaulted,
}

/// The language evidence in one message: marker words of each language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Evidence {
    /// Spanish marker words (a word on the Spanish list, or containing a
    /// letter only Spanish uses), plus one for `¿`/`¡` when no word is.
    pub es: usize,
    /// English marker words.
    pub en: usize,
}

impl Evidence {
    /// The score of one language.
    #[must_use]
    pub const fn of(self, lang: Lang) -> usize {
        match lang {
            Lang::Es => self.es,
            Lang::En => self.en,
        }
    }

    /// The language with more evidence, if one has more.
    #[must_use]
    pub fn leader(self) -> Option<Lang> {
        match self.es.cmp(&self.en) {
            std::cmp::Ordering::Greater => Some(Lang::Es),
            std::cmp::Ordering::Less => Some(Lang::En),
            std::cmp::Ordering::Equal => None,
        }
    }
}

/// How the language of a turn was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decided {
    /// The caller said so.
    Hint,
    /// A conversation with no settled language: the evidence decided.
    Evidence(Evidence),
    /// Clear evidence switched a settled conversation.
    Switched(Evidence),
    /// The conversation kept its settled language.
    Kept(Evidence),
    /// No evidence either way and nothing settled: the default (or the
    /// earlier default) applies.
    Default(Evidence),
}

impl Decided {
    /// How firmly the conversation holds the language afterwards.
    #[must_use]
    pub const fn hold(self) -> Hold {
        match self {
            Decided::Default(_) => Hold::Defaulted,
            Decided::Hint | Decided::Evidence(_) | Decided::Switched(_) | Decided::Kept(_) => {
                Hold::Settled
            }
        }
    }
}

/// Function words and greetings that occur in Spanish and not in English.
/// Accent-less spellings are included: their absence is not evidence.
const ES_WORDS: &[&str] = &[
    "el",
    "la",
    "los",
    "las",
    "lo",
    "le",
    "les",
    "del",
    "al",
    "de",
    "que",
    "qué",
    "como",
    "cómo",
    "cual",
    "cuál",
    "cuando",
    "cuándo",
    "donde",
    "dónde",
    "quien",
    "quién",
    "por",
    "para",
    "con",
    "una",
    "un",
    "unos",
    "unas",
    "es",
    "está",
    "esta",
    "estoy",
    "eres",
    "soy",
    "en",
    "y",
    "o",
    "pero",
    "muy",
    "mi",
    "mis",
    "tu",
    "tus",
    "su",
    "sus",
    "hola",
    "buenas",
    "buenos",
    "días",
    "dias",
    "tardes",
    "noches",
    "gracias",
    "adiós",
    "adios",
    "sí",
    "dime",
    "hazme",
    "escribe",
    "escríbeme",
    "escribeme",
];

/// Function words and greetings that occur in English and not in Spanish.
const EN_WORDS: &[&str] = &[
    "the",
    "what",
    "what's",
    "whats",
    "is",
    "are",
    "was",
    "were",
    "i",
    "i'm",
    "you",
    "your",
    "my",
    "how",
    "who",
    "where",
    "when",
    "why",
    "which",
    "it",
    "it's",
    "this",
    "that",
    "of",
    "to",
    "and",
    "with",
    "for",
    "from",
    "do",
    "does",
    "can",
    "please",
    "hello",
    "hi",
    "hiya",
    "good",
    "morning",
    "afternoon",
    "evening",
    "night",
    "thanks",
    "thank",
    "write",
    "tell",
    "give",
];

/// Characters that only Spanish uses among the two.
fn is_es_char(c: char) -> bool {
    matches!(c, 'ñ' | 'á' | 'é' | 'í' | 'ó' | 'ú' | 'ü')
}

/// Counts the marker words of each language in normalised text (accents
/// kept: they are evidence, though their absence is not).
#[must_use]
pub fn evidence(text: &str) -> Evidence {
    let mut found = Evidence::default();
    for word in text
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
    {
        let word = unstretch(word);
        let word = word.as_str();
        if ES_WORDS.contains(&word) || word.chars().any(is_es_char) {
            found.es += 1;
        }
        if EN_WORDS.contains(&word) || word.ends_with("n't") {
            found.en += 1;
        }
    }
    if found.es == 0 && text.contains(['¿', '¡']) {
        found.es = 1;
    }
    found
}

/// Collapses a run of three or more equal letters to one, so a stretched
/// word ("holaaa", "hiii") is looked up as itself. Neither language spells
/// a word with three equal letters in a row.
fn unstretch(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(word.len());
    let mut i = 0;
    while i < chars.len() {
        let run = chars[i..].iter().take_while(|&&c| c == chars[i]).count();
        let keep = if run >= 3 { 1 } else { run };
        out.extend(std::iter::repeat_n(chars[i], keep));
        i += run;
    }
    out
}

/// Decides the language of a turn from its hint, its normalised text and
/// the conversation's current language.
#[must_use]
pub fn decide(
    hint: Option<Lang>,
    text: &str,
    current: Option<(Lang, Hold)>,
    default: Lang,
) -> (Lang, Decided) {
    if let Some(lang) = hint {
        return (lang, Decided::Hint);
    }
    let found = evidence(text);
    match current {
        Some((lang, Hold::Settled)) => {
            let other = lang.other();
            if found.of(other) >= SWITCH_WORDS && found.of(other) > found.of(lang) {
                (other, Decided::Switched(found))
            } else {
                (lang, Decided::Kept(found))
            }
        }
        Some((_, Hold::Defaulted)) | None => {
            if let Some(lang) = found.leader() {
                (lang, Decided::Evidence(found))
            } else {
                let lang = current.map_or(default, |(lang, _)| lang);
                (lang, Decided::Default(found))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(text: &str, default: Lang) -> Lang {
        decide(None, text, None, default).0
    }

    fn after(lang: Lang, text: &str) -> (Lang, Decided) {
        decide(None, text, Some((lang, Hold::Settled)), Lang::Es)
    }

    #[test]
    fn the_hint_wins() {
        assert_eq!(
            decide(
                Some(Lang::En),
                "hola",
                Some((Lang::Es, Hold::Settled)),
                Lang::Es
            ),
            (Lang::En, Decided::Hint)
        );
    }

    #[test]
    fn unambiguous_words_decide_a_fresh_conversation() {
        assert_eq!(fresh("hola", Lang::En), Lang::Es);
        assert_eq!(fresh("que hora es", Lang::En), Lang::Es);
        assert_eq!(fresh("hello", Lang::Es), Lang::En);
        assert_eq!(fresh("what time is it", Lang::Es), Lang::En);
        assert_eq!(fresh("don't", Lang::Es), Lang::En);
        assert_eq!(fresh("¿mañana?", Lang::En), Lang::Es);
        assert_eq!(fresh("escríbeme un poema", Lang::En), Lang::Es);
        assert_eq!(fresh("¿?", Lang::En), Lang::Es);
    }

    #[test]
    fn stretched_words_count_as_themselves() {
        assert_eq!(unstretch("holaaaa"), "hola");
        assert_eq!(unstretch("hiii"), "hi");
        assert_eq!(unstretch("hello"), "hello");
        assert_eq!(fresh("hiii!!!", Lang::Es), Lang::En);
        assert_eq!(fresh("holaaa", Lang::En), Lang::Es);
    }

    #[test]
    fn no_evidence_or_a_tie_takes_the_default() {
        assert_eq!(
            decide(None, "ok", None, Lang::En),
            (Lang::En, Decided::Default(Evidence::default()))
        );
        assert_eq!(fresh("hey", Lang::Es), Lang::Es);
        assert_eq!(fresh("hey", Lang::En), Lang::En);
        assert_eq!(fresh("asdkjh qwe", Lang::Es), Lang::Es);
        // More English than Spanish.
        assert_eq!(fresh("hola, what time is it", Lang::Es), Lang::En);
    }

    #[test]
    fn one_word_never_switches_a_settled_conversation() {
        // conversation.md 1.2 and 1.3.
        assert_eq!(after(Lang::En, "gracias").0, Lang::En);
        assert_eq!(after(Lang::En, "hola").0, Lang::En);
        assert_eq!(after(Lang::Es, "thanks").0, Lang::Es);
        assert_eq!(after(Lang::Es, "ok").0, Lang::Es);
        assert_eq!(after(Lang::Es, "chess").0, Lang::Es);
        // Mixed: the grammar's language has more markers.
        assert_eq!(after(Lang::Es, "qué hora es in tokyo").0, Lang::Es);
        assert_eq!(after(Lang::En, "what's the hora in tokyo").0, Lang::En);
    }

    #[test]
    fn clear_evidence_switches() {
        assert!(matches!(
            after(Lang::Es, "and what's the time in london?"),
            (Lang::En, Decided::Switched(_))
        ));
        assert!(matches!(
            after(Lang::En, "y en berlín, por favor"),
            (Lang::Es, Decided::Switched(_))
        ));
    }

    #[test]
    fn a_defaulted_conversation_takes_the_first_evidence() {
        let defaulted = Some((Lang::En, Hold::Defaulted));
        assert_eq!(decide(None, "hola", defaulted, Lang::En).0, Lang::Es);
        assert_eq!(
            decide(None, "ok", defaulted, Lang::Es),
            (Lang::En, Decided::Default(Evidence::default()))
        );
        assert_eq!(
            Decided::Default(Evidence::default()).hold(),
            Hold::Defaulted
        );
        assert_eq!(Decided::Hint.hold(), Hold::Settled);
    }
}
