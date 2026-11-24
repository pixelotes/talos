//! Stage 1: which language a turn is in (ADR 0015).
//!
//! Slice A implements steps 1, 2 and 4: the caller's hint, then words and
//! characters that belong to only one language, then the configured default.
//! The classifier (step 3) and per-conversation stickiness come later.

use crate::Lang;

/// How the language of a turn was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decided {
    /// The caller said so.
    Hint,
    /// Markers of one language and none of the other: the counts found.
    Markers {
        /// Spanish markers in the message.
        es: usize,
        /// English markers in the message.
        en: usize,
    },
    /// No decisive evidence (none, or markers of both): the default applies.
    Default {
        /// Spanish markers in the message.
        es: usize,
        /// English markers in the message.
        en: usize,
    },
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
    matches!(c, 'ñ' | '¿' | '¡' | 'á' | 'é' | 'í' | 'ó' | 'ú' | 'ü')
}

/// Counts the markers of each language in normalised text.
#[must_use]
pub fn markers(text: &str) -> (usize, usize) {
    let mut es = text.chars().filter(|&c| is_es_char(c)).count();
    let mut en = 0;
    for word in text
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
    {
        let word = unstretch(word);
        let word = word.as_str();
        if ES_WORDS.contains(&word) {
            es += 1;
        }
        if EN_WORDS.contains(&word) || word.ends_with("n't") {
            en += 1;
        }
    }
    (es, en)
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

/// Decides the language of a turn from its hint and its normalised text.
#[must_use]
pub fn detect(hint: Option<Lang>, text: &str, default: Lang) -> (Lang, Decided) {
    if let Some(lang) = hint {
        return (lang, Decided::Hint);
    }
    match markers(text) {
        (es, 0) if es > 0 => (Lang::Es, Decided::Markers { es, en: 0 }),
        (0, en) if en > 0 => (Lang::En, Decided::Markers { es: 0, en }),
        (es, en) => (default, Decided::Default { es, en }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(text: &str) -> Lang {
        detect(None, text, Lang::Es).0
    }

    fn lang_default_en(text: &str) -> Lang {
        detect(None, text, Lang::En).0
    }

    #[test]
    fn the_hint_wins() {
        assert_eq!(
            detect(Some(Lang::En), "hola", Lang::Es),
            (Lang::En, Decided::Hint)
        );
    }

    #[test]
    fn unambiguous_words_decide() {
        assert_eq!(lang_default_en("hola"), Lang::Es);
        assert_eq!(lang_default_en("que hora es"), Lang::Es);
        assert_eq!(lang("hello"), Lang::En);
        assert_eq!(lang("what time is it"), Lang::En);
        assert_eq!(lang("don't"), Lang::En);
    }

    #[test]
    fn stretched_words_count_as_themselves() {
        assert_eq!(unstretch("holaaaa"), "hola");
        assert_eq!(unstretch("hiii"), "hi");
        assert_eq!(unstretch("hello"), "hello");
        assert_eq!(lang("hiii!!!"), Lang::En);
        assert_eq!(lang_default_en("holaaa"), Lang::Es);
    }

    #[test]
    fn spanish_characters_decide() {
        assert_eq!(lang_default_en("¿mañana?"), Lang::Es);
        assert_eq!(lang_default_en("escríbeme un poema"), Lang::Es);
    }

    #[test]
    fn no_evidence_or_mixed_evidence_falls_back_to_the_default() {
        assert_eq!(
            detect(None, "ok", Lang::En),
            (Lang::En, Decided::Default { es: 0, en: 0 })
        );
        assert_eq!(lang("hey"), Lang::Es);
        assert_eq!(lang_default_en("hey"), Lang::En);
        assert_eq!(lang_default_en("hola, what time is it"), Lang::En);
        assert_eq!(lang("asdkjh qwe"), Lang::Es);
    }
}
