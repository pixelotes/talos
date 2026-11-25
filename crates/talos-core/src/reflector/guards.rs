//! The guards decided in code (docs/spec/reflector.md 4.4–4.6): long or
//! pasted text, other languages, and noise. The guards written as
//! patterns (`generative`, `unknown-fact`, `not-yet`, `advice`, `cannot`)
//! are rules in the scripts.

use crate::language;
use crate::normalise::Normalised;

/// More words than this is long text (reflector.md 4.6).
pub(crate) const LONG_TEXT_WORDS: usize = 60;

/// Long or pasted: over [`LONG_TEXT_WORDS`] words, or several lines.
pub(crate) fn is_long_text(message: &Normalised) -> bool {
    message.original.trim().contains(['\n', '\r'])
        || message.original.split_whitespace().count() > LONG_TEXT_WORDS
}

/// Words that belong to a language other than Spanish and English and are
/// not words of either: Catalan, Galician, Basque, Portuguese, French,
/// German and Italian (reflector.md 4.5). Written as the user would, with
/// accents; matched against the normalised text (lower case, accents kept).
const FOREIGN_WORDS: &[&str] = &[
    // Catalan.
    "quina",
    "quin",
    "quins",
    "quines",
    "bon",
    "bona",
    "bones",
    "gràcies",
    "adéu",
    "molt",
    "molta",
    "molts",
    "moltes",
    "també",
    "això",
    "perquè",
    "avui",
    "demà",
    "nosaltres",
    "vosaltres",
    "estic",
    "tinc",
    "vull",
    "amb",
    "què",
    "però",
    "més",
    "és",
    "sóc",
    "fer",
    "si us plau",
    // Galician.
    "grazas",
    "moito",
    "moita",
    "vostede",
    "unha",
    "isto",
    "noite",
    "mañá",
    "hoxe",
    "xa",
    "boas",
    // Portuguese.
    "obrigado",
    "obrigada",
    "olá",
    "você",
    "voce",
    "ajuda",
    "não",
    "nao",
    "muito",
    "muita",
    "bom",
    "boa",
    "tudo",
    "bem",
    "estou",
    "eu",
    "agora",
    "hoje",
    "amanhã",
    "tchau",
    "então",
    "fazer",
    "quero",
    "isso",
    "meu",
    "minha",
    "nós",
    "ele",
    "onde",
    "falar",
    // French.
    "bonjour",
    "bonsoir",
    "merci",
    "beaucoup",
    "quelle",
    "quel",
    "heure",
    "est-ce",
    "c'est",
    "je",
    "suis",
    "vous",
    "nous",
    "oui",
    "très",
    "avec",
    "il",
    "elle",
    "pas",
    "qui",
    "quoi",
    "pourquoi",
    "aujourd'hui",
    "s'il",
    "plaît",
    "salut",
    "ça",
    "êtes",
    "parlez",
    // German.
    "wie",
    "spät",
    "ist",
    "ich",
    "nicht",
    "danke",
    "bitte",
    "guten",
    "und",
    "ein",
    "eine",
    "mit",
    "auf",
    "wo",
    "warum",
    "heute",
    "tschüss",
    "sprechen",
    "zeit",
    "uhr",
    "sehr",
    "nein",
    "kannst",
    "bist",
    "habe",
    "haben",
    "morgen",
    "abend",
    "geht",
    "dir",
    "mir",
    // Italian.
    "grazie",
    "mille",
    "buongiorno",
    "buonasera",
    "prego",
    "sono",
    "sei",
    "perché",
    "molto",
    "anche",
    "questo",
    "quello",
    "ancora",
    "oggi",
    "domani",
    "voglio",
    "posso",
    "parlare",
    "della",
    "delle",
    "degli",
    "nel",
    "nella",
    "gli",
    "stai",
    // Basque.
    "kaixo",
    "eskerrik",
    "asko",
    "zer",
    "ordu",
    "egun",
    "bai",
    "ez",
    "naiz",
    "dut",
    "zara",
    "mesedez",
    "gabon",
];

/// Letters neither Spanish nor English writes.
fn is_foreign_letter(c: char) -> bool {
    c.is_alphabetic() && !c.is_ascii() && !matches!(c, 'á' | 'é' | 'í' | 'ó' | 'ú' | 'ü' | 'ñ')
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
}

/// A message in another language: more marker words of other languages
/// than of Spanish and English, and at least one.
pub(crate) fn is_other_language(message: &Normalised) -> bool {
    let text = &message.text;
    let mut foreign = 0;
    let mut total = 0;
    for word in words(text) {
        total += 1;
        if FOREIGN_WORDS.contains(&word) || word.chars().any(is_foreign_letter) {
            foreign += 1;
        }
    }
    // Phrases of two words.
    foreign += FOREIGN_WORDS
        .iter()
        .filter(|w| w.contains(' ') && text.contains(*w))
        .count();
    if foreign == 0 {
        return false;
    }
    let evidence = language::evidence(text);
    let native = evidence.es + evidence.en;
    foreign > native && (foreign >= 2 || total <= 3)
}

/// A word that is no word: keyboard mash, a run of one letter, no vowel.
fn is_mash(word: &str) -> bool {
    const KEYBOARD: [&str; 22] = [
        "qwe", "asd", "sdf", "dfg", "fgh", "ghj", "hjk", "jkl", "zxc", "xcv", "cvb", "vbn", "lkj",
        "kjh", "jhg", "hgf", "gfd", "fds", "dsa", "ñlk", "poi", "mnb",
    ];
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 2 {
        return false;
    }
    let vowel = |c: &char| "aeiouyáéíóúü".contains(*c);
    if !letters.iter().any(vowel) {
        // "mmm", "hmm", "pff", "shh" are interjections; "zzz", "jjjj",
        // "ñlkjh" are not words.
        return !letters.iter().all(|c| "mhpfs".contains(*c));
    }
    if letters.iter().all(|c| *c == letters[0]) {
        return true;
    }
    let mut run = 0;
    for c in &letters {
        run = if vowel(c) { 0 } else { run + 1 };
        if run >= 5 {
            return true;
        }
    }
    KEYBOARD.iter().any(|k| word.contains(k))
}

/// Noise (reflector.md 4.4): nothing but punctuation, digits, symbols or
/// emoji; a single letter; or mostly words that are no words. Run after
/// the rules, so laughter ("jajaja", 😂) and fillers ("mmm") are taken by
/// their shapes first.
pub(crate) fn is_noise(message: &Normalised) -> bool {
    let text = &message.text;
    if !text.chars().any(char::is_alphabetic) {
        return true;
    }
    let all: Vec<&str> = words(text).collect();
    if all.is_empty() || (all.len() == 1 && all[0].chars().count() == 1) {
        return true;
    }
    let mash = all.iter().filter(|w| is_mash(w)).count();
    mash * 2 >= all.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalise::normalise;

    #[test]
    fn other_languages() {
        for text in [
            "bon dia, quina hora és?",
            "obrigado pela ajuda",
            "wie spät ist es?",
            "grazie mille",
            "bonjour, quelle heure est-il",
            "eskerrik asko",
            "こんにちは",
        ] {
            assert!(is_other_language(&normalise(text)), "{text}");
        }
        for text in [
            "creo que el teletrabajo ha venido para quedarse",
            "I love rainy Sundays",
            "I said merci to the waiter and he smiled",
            "qué fuerte",
            "café con leche",
        ] {
            assert!(!is_other_language(&normalise(text)), "{text}");
        }
    }

    #[test]
    fn noise() {
        for text in [
            "ñlkjh",
            "k",
            "?",
            ".",
            "jjjjjjj",
            "4567",
            "asdkjh qwe",
            "🙃",
        ] {
            assert!(is_noise(&normalise(text)), "{text}");
        }
        for text in [
            "no sé",
            "lo de ayer con lo otro",
            "I see",
            "the thing",
            "mmm hola",
        ] {
            assert!(!is_noise(&normalise(text)), "{text}");
        }
    }

    #[test]
    fn long_text() {
        assert!(is_long_text(&normalise("una línea\notra línea")));
        assert!(is_long_text(&normalise(&"palabra ".repeat(61))));
        assert!(!is_long_text(&normalise(&"palabra ".repeat(60))));
    }
}
