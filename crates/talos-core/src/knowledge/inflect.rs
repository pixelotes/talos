//! Inflected forms (docs/spec/knowledge.md 2.1, step 1; KNOWLEDGE.md
//! section 7: "rules plus exception lists maintained in the project").
//!
//! The **rules** are here, as tables, and run both ways: [`analyses`] reads
//! a typed form back to the lemmas it may come from ("guitarras" →
//! *guitarra*, plural; "llovía" → *llover*, imperfect), and [`plural`]
//! writes a lemma's plural. They are deliberately regular: a candidate lemma
//! only counts when a lexeme with that lemma and a fitting part of speech
//! exists, so a rule that over-generates costs a lookup, not a wrong answer.
//!
//! The **exceptions** are data, not code: a pack's `form` table lists the
//! forms no rule produces ("fish" plural of *fish*, "llueve" of *llover*),
//! with their features. A vocabulary pack extends them by writing rows
//! (the wordnets' exception lists, Slice C), never by changing this file.
//!
//! Everything here works on the match form (lower case, no accents;
//! [`key`]), except [`plural`], which writes the lemma's own spelling.

use crate::Lang;
use crate::normalise::fold;
use crate::user::Correction;

/// The match form of a word or phrase, as forms are stored and looked up:
/// lower case, no accents, single spaces, no punctuation at either end.
#[must_use]
pub fn key(text: &str) -> String {
    Correction::key(text)
}

/// Which parts of speech a rule applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Nouns and adjectives (number, and Spanish gender).
    Nominal,
    /// Adjectives only (English comparison).
    Adjective,
    /// Verbs.
    Verb,
}

impl Class {
    /// Whether a lexeme with this part of speech (`lexeme.pos`) can carry
    /// the rule's features. A lexeme with no part of speech (a word the
    /// user taught) takes the nominal rules only.
    pub(crate) fn admits(self, pos: Option<&str>) -> bool {
        match (self, pos) {
            (Class::Verb, None) => false,
            (_, None) => true,
            (Class::Nominal, Some(p)) => p == "n" || p == "adj",
            (Class::Adjective, Some(p)) => p == "adj",
            (Class::Verb, Some(p)) => p == "v",
        }
    }
}

/// One way a typed form may have been made: from `lemma` (match form), with
/// `features` (comma-separated: `pl`, `f`, `pres,3sg`, `ger`…), by a rule
/// for `class`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Analysis {
    pub(crate) lemma: String,
    pub(crate) features: &'static str,
    pub(crate) class: Class,
}

/// English endings shared by the noun plural and the verb's third person:
/// (ending, replacement).
const EN_S: &[(&str, &str)] = &[
    ("ies", "y"),
    ("ves", "f"),
    ("ves", "fe"),
    ("ses", "s"),
    ("xes", "x"),
    ("zes", "z"),
    ("ches", "ch"),
    ("shes", "sh"),
    ("oes", "o"),
    ("s", ""),
];

/// Spanish verb endings by conjugation, regular verbs only: (ending in the
/// match form, infinitive ending, features). The future and the
/// conditional, which add to the whole infinitive, are [`ES_FUTURE`]. Where
/// two persons share an ending the first listed names it.
const ES_VERB: &[(&str, &str, &str)] = &[
    // -ar
    ("o", "ar", "pres,1sg"),
    ("as", "ar", "pres,2sg"),
    ("a", "ar", "pres,3sg"),
    ("amos", "ar", "pres,1pl"),
    ("ais", "ar", "pres,2pl"),
    ("an", "ar", "pres,3pl"),
    ("e", "ar", "pret,1sg"),
    ("aste", "ar", "pret,2sg"),
    ("o", "ar", "pret,3sg"),
    ("asteis", "ar", "pret,2pl"),
    ("aron", "ar", "pret,3pl"),
    ("aba", "ar", "impf,3sg"),
    ("abas", "ar", "impf,2sg"),
    ("abamos", "ar", "impf,1pl"),
    ("abais", "ar", "impf,2pl"),
    ("aban", "ar", "impf,3pl"),
    ("es", "ar", "subj,2sg"),
    ("emos", "ar", "subj,1pl"),
    ("eis", "ar", "subj,2pl"),
    ("en", "ar", "subj,3pl"),
    ("ando", "ar", "ger"),
    ("ado", "ar", "part"),
    ("ada", "ar", "part,f"),
    ("ados", "ar", "part,pl"),
    ("adas", "ar", "part,f,pl"),
    // -er
    ("o", "er", "pres,1sg"),
    ("es", "er", "pres,2sg"),
    ("e", "er", "pres,3sg"),
    ("emos", "er", "pres,1pl"),
    ("eis", "er", "pres,2pl"),
    ("en", "er", "pres,3pl"),
    ("i", "er", "pret,1sg"),
    ("iste", "er", "pret,2sg"),
    ("io", "er", "pret,3sg"),
    ("imos", "er", "pret,1pl"),
    ("isteis", "er", "pret,2pl"),
    ("ieron", "er", "pret,3pl"),
    ("ia", "er", "impf,3sg"),
    ("ias", "er", "impf,2sg"),
    ("iamos", "er", "impf,1pl"),
    ("iais", "er", "impf,2pl"),
    ("ian", "er", "impf,3pl"),
    ("a", "er", "subj,3sg"),
    ("as", "er", "subj,2sg"),
    ("amos", "er", "subj,1pl"),
    ("ais", "er", "subj,2pl"),
    ("an", "er", "subj,3pl"),
    ("iendo", "er", "ger"),
    ("ido", "er", "part"),
    ("ida", "er", "part,f"),
    ("idos", "er", "part,pl"),
    ("idas", "er", "part,f,pl"),
    // -ir
    ("o", "ir", "pres,1sg"),
    ("es", "ir", "pres,2sg"),
    ("e", "ir", "pres,3sg"),
    ("imos", "ir", "pres,1pl"),
    ("is", "ir", "pres,2pl"),
    ("en", "ir", "pres,3pl"),
    ("i", "ir", "pret,1sg"),
    ("iste", "ir", "pret,2sg"),
    ("io", "ir", "pret,3sg"),
    ("isteis", "ir", "pret,2pl"),
    ("ieron", "ir", "pret,3pl"),
    ("ia", "ir", "impf,3sg"),
    ("ias", "ir", "impf,2sg"),
    ("iamos", "ir", "impf,1pl"),
    ("iais", "ir", "impf,2pl"),
    ("ian", "ir", "impf,3pl"),
    ("a", "ir", "subj,3sg"),
    ("as", "ir", "subj,2sg"),
    ("amos", "ir", "subj,1pl"),
    ("ais", "ir", "subj,2pl"),
    ("an", "ir", "subj,3pl"),
    ("iendo", "ir", "ger"),
    ("ido", "ir", "part"),
    ("ida", "ir", "part,f"),
    ("idos", "ir", "part,pl"),
    ("idas", "ir", "part,f,pl"),
];

/// Endings of the future and the conditional, added to the infinitive.
const ES_FUTURE: &[(&str, &str)] = &[
    ("e", "fut,1sg"),
    ("as", "fut,2sg"),
    ("a", "fut,3sg"),
    ("emos", "fut,1pl"),
    ("eis", "fut,2pl"),
    ("an", "fut,3pl"),
    ("ia", "cond,3sg"),
    ("ias", "cond,2sg"),
    ("iamos", "cond,1pl"),
    ("iais", "cond,2pl"),
    ("ian", "cond,3pl"),
];

/// Spanish nominal endings: (ending, replacement, features).
const ES_NOMINAL: &[(&str, &str, &str)] = &[
    ("ces", "z", "pl"),
    ("es", "", "pl"),
    ("s", "", "pl"),
    ("oras", "or", "f,pl"),
    ("ora", "or", "f"),
    ("as", "o", "f,pl"),
    ("a", "o", "f"),
];

/// Words that end the head of a Spanish noun phrase: in "cajas de
/// resonancia" only "cajas" is inflected.
const ES_PREPOSITIONS: &[&str] = &[
    "de", "del", "a", "al", "en", "con", "para", "por", "sin", "sobre", "entre",
];

/// The shortest lemma a rule may produce, in characters.
const MIN_LEMMA: usize = 2;

fn strip<'a>(word: &'a str, ending: &str) -> Option<&'a str> {
    word.strip_suffix(ending)
        .filter(|stem| stem.chars().count() >= MIN_LEMMA - 1 && !stem.is_empty())
}

/// "runn" → "run": the stem with a doubled final consonant undone.
fn undouble(stem: &str) -> Option<&str> {
    let mut chars = stem.chars().rev();
    let (a, b) = (chars.next()?, chars.next()?);
    (a == b && !"aeiouy".contains(a) && a.is_ascii_alphabetic()).then(|| &stem[..stem.len() - 1])
}

fn english_word(word: &str, out: &mut Vec<(String, &'static str, Class)>) {
    for (ending, replacement) in EN_S {
        if let Some(stem) = strip(word, ending) {
            if *ending == "s" && (stem.ends_with('s') || stem.ends_with('u')) {
                continue;
            }
            let lemma = format!("{stem}{replacement}");
            out.push((lemma.clone(), "pl", Class::Nominal));
            out.push((lemma, "pres,3sg", Class::Verb));
        }
    }
    if let Some(stem) = strip(word, "men") {
        out.push((format!("{stem}man"), "pl", Class::Nominal));
    }
    let mut verb = |ending: &str, features: &'static str| {
        if let Some(stem) = strip(word, ending) {
            out.push((stem.to_owned(), features, Class::Verb));
            out.push((format!("{stem}e"), features, Class::Verb));
            if let Some(single) = undouble(stem) {
                out.push((single.to_owned(), features, Class::Verb));
            }
            if ending == "ing"
                && let Some(base) = stem.strip_suffix('y')
            {
                out.push((format!("{base}ie"), features, Class::Verb));
            }
        }
    };
    verb("ing", "ger");
    verb("ed", "past");
    if let Some(stem) = strip(word, "ied") {
        out.push((format!("{stem}y"), "past", Class::Verb));
    }
    for (ending, features) in [("est", "sup"), ("er", "cmp")] {
        if let Some(stem) = strip(word, ending) {
            out.push((stem.to_owned(), features, Class::Adjective));
            out.push((format!("{stem}e"), features, Class::Adjective));
            if let Some(single) = undouble(stem) {
                out.push((single.to_owned(), features, Class::Adjective));
            }
        }
    }
    for (ending, features) in [("iest", "sup"), ("ier", "cmp")] {
        if let Some(stem) = strip(word, ending) {
            out.push((format!("{stem}y"), features, Class::Adjective));
        }
    }
}

fn spanish_nominal(word: &str, out: &mut Vec<(String, &'static str, Class)>) {
    for (ending, replacement, features) in ES_NOMINAL {
        if let Some(stem) = strip(word, ending) {
            out.push((format!("{stem}{replacement}"), features, Class::Nominal));
        }
    }
}

fn spanish_verb(word: &str, out: &mut Vec<(String, &'static str, Class)>) {
    for (ending, infinitive, features) in ES_VERB {
        if let Some(stem) = strip(word, ending) {
            out.push((format!("{stem}{infinitive}"), features, Class::Verb));
        }
    }
    for (ending, features) in ES_FUTURE {
        if let Some(stem) = word.strip_suffix(ending)
            && ["ar", "er", "ir"].iter().any(|e| stem.ends_with(e))
            && stem.len() > 2
        {
            out.push((stem.to_owned(), features, Class::Verb));
        }
    }
}

/// The ways a typed word or phrase (in the match form) may be an inflected
/// form of a lemma, by the rules of `lang`; the form itself is not among
/// them. An English phrase inflects its last word ("string instruments");
/// a Spanish one, every word before its first preposition ("guitarras
/// eléctricas", "cajas de resonancia").
pub(crate) fn analyses(key: &str, lang: Lang) -> Vec<Analysis> {
    let words: Vec<&str> = key.split(' ').filter(|w| !w.is_empty()).collect();
    let Some((last, init)) = words.split_last() else {
        return Vec::new();
    };
    let mut found: Vec<(String, &'static str, Class)> = Vec::new();
    match lang {
        Lang::En => {
            let mut one = Vec::new();
            english_word(last, &mut one);
            let prefix = init.join(" ");
            for (lemma, features, class) in one {
                let lemma = if prefix.is_empty() {
                    lemma
                } else {
                    format!("{prefix} {lemma}")
                };
                found.push((lemma, features, class));
            }
        }
        Lang::Es => {
            let head_len = words
                .iter()
                .position(|w| ES_PREPOSITIONS.contains(w))
                .unwrap_or(words.len());
            if head_len == 1 || words.len() == 1 {
                let mut one = Vec::new();
                spanish_nominal(words[0], &mut one);
                spanish_verb(words[0], &mut one);
                let rest = words[1..].join(" ");
                for (lemma, features, class) in one {
                    let lemma = if rest.is_empty() {
                        lemma
                    } else {
                        format!("{lemma} {rest}")
                    };
                    found.push((lemma, features, class));
                }
            } else if head_len > 1 {
                spanish_plural_phrase(&words, head_len, &mut found);
            }
        }
    }
    let mut out: Vec<Analysis> = Vec::with_capacity(found.len());
    for (lemma, features, class) in found {
        if lemma == key || lemma.chars().count() < MIN_LEMMA {
            continue;
        }
        let analysis = Analysis {
            lemma,
            features,
            class,
        };
        if !out.contains(&analysis) {
            out.push(analysis);
        }
    }
    out
}

/// A Spanish phrase whose head has several words: every head word ending
/// in `s` is read as a plural, together.
fn spanish_plural_phrase(
    words: &[&str],
    head_len: usize,
    out: &mut Vec<(String, &'static str, Class)>,
) {
    let mut options: Vec<Vec<String>> = Vec::with_capacity(head_len);
    for word in &words[..head_len] {
        if word.ends_with('s') {
            let mut one = Vec::new();
            spanish_nominal(word, &mut one);
            let stems: Vec<String> = one
                .into_iter()
                .filter(|(_, f, _)| *f == "pl")
                .map(|(l, _, _)| l)
                .collect();
            if stems.is_empty() {
                return;
            }
            options.push(stems);
        } else {
            options.push(vec![(*word).to_owned()]);
        }
    }
    let rest = words[head_len..].join(" ");
    let mut combos: Vec<Vec<String>> = vec![Vec::new()];
    for choice in options {
        combos = combos
            .into_iter()
            .flat_map(|prefix| {
                choice.iter().map(move |c| {
                    let mut next = prefix.clone();
                    next.push(c.clone());
                    next
                })
            })
            .collect();
    }
    for combo in combos {
        let mut lemma = combo.join(" ");
        if !rest.is_empty() {
            lemma.push(' ');
            lemma.push_str(&rest);
        }
        out.push((lemma, "pl", Class::Nominal));
    }
}

fn is_accented(c: char) -> bool {
    "áéíóú".contains(c)
}

fn deaccent(c: char) -> char {
    match c {
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' => 'o',
        'ú' => 'u',
        other => other,
    }
}

fn syllables(word: &str) -> usize {
    let mut count = 0;
    let mut in_vowel = false;
    for c in word.chars() {
        let vowel = "aeiouáéíóúü".contains(c.to_lowercase().next().unwrap_or(c));
        if vowel && !in_vowel {
            count += 1;
        }
        in_vowel = vowel;
    }
    count
}

fn spanish_plural_word(word: &str) -> String {
    let lower: Vec<char> = word.to_lowercase().chars().collect();
    let Some(&last) = lower.last() else {
        return word.to_owned();
    };
    let before = lower.len().checked_sub(2).map(|i| lower[i]);
    let chars: Vec<char> = word.chars().collect();
    let head = |n: usize| chars[..chars.len() - n].iter().collect::<String>();
    // A stressed last syllable in -n or -s loses its written accent.
    let unstressed_tail = |n: usize| {
        let mut out: String = chars[..chars.len() - n - 1].iter().collect();
        out.push(deaccent(chars[chars.len() - n - 1]));
        out.extend(&chars[chars.len() - n..]);
        out
    };
    let accented_before = before.is_some_and(is_accented);
    match last {
        'a' | 'e' | 'o' | 'i' | 'u' | 'á' | 'é' | 'ó' => format!("{word}s"),
        'z' => format!("{}ces", head(1)),
        's' | 'x' | 'n' if accented_before => format!("{}es", unstressed_tail(1)),
        's' | 'x' if syllables(word) > 1 => word.to_owned(),
        _ => format!("{word}es"),
    }
}

fn english_plural_word(word: &str) -> String {
    let lower = word.to_lowercase();
    if let Some(stem) = word.strip_suffix('y')
        && !lower
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| "aeiou".contains(c))
    {
        return format!("{stem}ies");
    }
    if ["s", "x", "z", "ch", "sh"]
        .iter()
        .any(|e| lower.ends_with(e))
    {
        return format!("{word}es");
    }
    format!("{word}s")
}

/// The regular plural of a noun or adjective lemma, in its own spelling:
/// "guitar" → "guitars", "violín" → "violines", "pez" → "peces",
/// "guitarra eléctrica" → "guitarras eléctricas", "caja de resonancia" →
/// "cajas de resonancia". A plural no rule gives is a pack's `form` row,
/// looked up first ([`super::Knowledge::form_of`]).
#[must_use]
pub fn plural(lemma: &str, lang: Lang) -> String {
    let words: Vec<&str> = lemma.split(' ').collect();
    match lang {
        Lang::En => {
            let (last, init) = words.split_last().unwrap_or((&"", &[]));
            let mut out = init.join(" ");
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&english_plural_word(last));
            out
        }
        Lang::Es => {
            let head_len = words
                .iter()
                .position(|w| ES_PREPOSITIONS.contains(&fold(&w.to_lowercase()).as_str()))
                .unwrap_or(words.len());
            words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    if i < head_len {
                        spanish_plural_word(w)
                    } else {
                        (*w).to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lemmas(word: &str, lang: Lang) -> Vec<(String, &'static str)> {
        analyses(&key(word), lang)
            .into_iter()
            .map(|a| (a.lemma, a.features))
            .collect()
    }

    fn reads(word: &str, lang: Lang, lemma: &str, features: &str) {
        assert!(
            lemmas(word, lang)
                .iter()
                .any(|(l, f)| l == lemma && *f == features),
            "{word} → {lemma} ({features}): {:?}",
            lemmas(word, lang)
        );
    }

    #[test]
    fn english_forms_read_back_to_their_lemma() {
        for (word, lemma, features) in [
            ("guitars", "guitar", "pl"),
            ("violins", "violin", "pl"),
            ("drums", "drum", "pl"),
            ("benches", "bench", "pl"),
            ("bodies", "body", "pl"),
            ("knives", "knife", "pl"),
            ("string instruments", "string instrument", "pl"),
            ("rains", "rain", "pres,3sg"),
            ("raining", "rain", "ger"),
            ("rained", "rain", "past"),
            ("running", "run", "ger"),
            ("baked", "bake", "past"),
            ("lying", "lie", "ger"),
            ("carried", "carry", "past"),
            ("colder", "cold", "cmp"),
            ("biggest", "big", "sup"),
            ("happier", "happy", "cmp"),
            ("women", "woman", "pl"),
        ] {
            reads(word, Lang::En, lemma, features);
        }
        assert!(lemmas("glass", Lang::En).iter().all(|(l, _)| l != "glas"));
        assert_eq!(lemmas("a", Lang::En).len(), 0);
    }

    #[test]
    fn spanish_forms_read_back_to_their_lemma() {
        for (word, lemma, features) in [
            ("guitarras", "guitarra", "pl"),
            ("violines", "violin", "pl"),
            ("cuerdas", "cuerda", "pl"),
            ("bancos", "banco", "pl"),
            ("peces", "pez", "pl"),
            ("nubes", "nube", "pl"),
            ("encantada", "encantado", "f"),
            ("guitarras eléctricas", "guitarra electrica", "pl"),
            ("cajas de resonancia", "caja de resonancia", "pl"),
            ("instrumentos de cuerda", "instrumento de cuerda", "pl"),
            ("seres vivos", "ser vivo", "pl"),
            ("llovía", "llover", "impf,3sg"),
            ("lloverá", "llover", "fut,3sg"),
            ("llovería", "llover", "cond,3sg"),
            ("lloviendo", "llover", "ger"),
            ("cantamos", "cantar", "pres,1pl"),
            ("cantaron", "cantar", "pret,3pl"),
            ("vivimos", "vivir", "pres,1pl"),
        ] {
            reads(word, Lang::Es, lemma, features);
        }
    }

    #[test]
    fn rules_apply_only_to_their_parts_of_speech() {
        assert!(Class::Nominal.admits(Some("n")));
        assert!(Class::Nominal.admits(Some("adj")));
        assert!(Class::Nominal.admits(None));
        assert!(!Class::Nominal.admits(Some("v")));
        assert!(Class::Verb.admits(Some("v")));
        assert!(!Class::Verb.admits(None));
        assert!(!Class::Adjective.admits(Some("n")));
    }

    #[test]
    fn plurals_are_written_in_the_lemmas_spelling() {
        for (lemma, lang, plural_form) in [
            ("guitar", Lang::En, "guitars"),
            ("bench", Lang::En, "benches"),
            ("body", Lang::En, "bodies"),
            ("day", Lang::En, "days"),
            ("string instrument", Lang::En, "string instruments"),
            ("guitarra", Lang::Es, "guitarras"),
            ("violín", Lang::Es, "violines"),
            ("pez", Lang::Es, "peces"),
            ("flor", Lang::Es, "flores"),
            ("mástil", Lang::Es, "mástiles"),
            ("francés", Lang::Es, "franceses"),
            ("virus", Lang::Es, "virus"),
            ("mes", Lang::Es, "meses"),
            ("guitarra eléctrica", Lang::Es, "guitarras eléctricas"),
            ("caja de resonancia", Lang::Es, "cajas de resonancia"),
            ("instrumento musical", Lang::Es, "instrumentos musicales"),
        ] {
            assert_eq!(plural(lemma, lang), plural_form, "{lemma}");
        }
    }

    #[test]
    fn the_match_form_drops_case_accents_and_end_punctuation() {
        assert_eq!(key("  ¿Violín? "), "violin");
        assert_eq!(key("Caja  de Resonancia"), "caja de resonancia");
    }
}
