//! Slots for a request the classifier recognised (docs/SKILLS.md section
//! 3): a guess has no rule captures, so a slot whose manifest declares cue
//! words ([`Guess`]) is looked for next to them and read by the slot's own
//! extractor. Conservative: a value the extractor cannot read, or two
//! readings that disagree, give nothing, and the skill asks as it would
//! after a rule.

use std::ops::Range;

use super::slot;
use crate::manifest::{Guess, SlotDecl};
use crate::normalise::{Normalised, normalise};
use crate::value::Value;
use crate::{Lang, Timestamp};

/// Characters that may surround a word without being part of it.
const EDGES: &[char] = &[
    '.', ',', ';', ':', '!', '?', '¡', '¿', '"', '\'', '«', '»', '(', ')', '…',
];

/// How many words a value before a cue may have.
const BEFORE_WORDS: usize = 3;

/// Words a value before a cue never starts with.
const ARTICLES: &[&str] = &[
    "a", "an", "the", "un", "una", "el", "la", "los", "las", "unos", "unas",
];

/// Closing courtesies that are not part of a trailing value.
const COURTESIES: &[&[&str]] = &[
    &["por", "favor"],
    &["porfa"],
    &["porfavor"],
    &["gracias"],
    &["please"],
    &["thanks"],
    &["thank", "you"],
];

/// The words of the match form, as byte ranges, split at spaces and
/// hyphens ("20-sided" is two words), edge punctuation left out.
fn words(folded: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    let mut push = |s: usize, e: usize| {
        let word = &folded[s..e];
        let lead = word.len() - word.trim_start_matches(EDGES).len();
        let trimmed = word.trim_matches(EDGES);
        if !trimmed.is_empty() {
            out.push(s + lead..s + lead + trimmed.len());
        }
    };
    for (i, c) in folded.char_indices() {
        if c.is_whitespace() || c == '-' {
            if let Some(s) = start.take() {
                push(s, i);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        push(s, folded.len());
    }
    out
}

/// Where a cue of several words starts, if it does at word `at`; returns
/// the number of words it covers.
fn cue_at(text: &str, words: &[Range<usize>], at: usize, cue: &str) -> Option<usize> {
    let parts: Vec<&str> = cue.split_whitespace().collect();
    let all = parts
        .iter()
        .enumerate()
        .all(|(k, part)| words.get(at + k).is_some_and(|w| &text[w.clone()] == *part));
    (!parts.is_empty() && all).then_some(parts.len())
}

/// The end of the message's words with closing courtesies dropped.
fn without_courtesies(text: &str, words: &[Range<usize>]) -> usize {
    let mut n = words.len();
    'outer: loop {
        for courtesy in COURTESIES {
            let k = courtesy.len();
            if n > k
                && courtesy
                    .iter()
                    .zip(&words[n - k..n])
                    .all(|(c, w)| &text[w.clone()] == *c)
            {
                n -= k;
                continue 'outer;
            }
        }
        return n;
    }
}

/// The slot's value as found next to its cues in `message`, if it is found
/// in exactly one reading.
#[must_use]
pub fn guess(decl: &SlotDecl, message: &Normalised, lang: Lang, now: Timestamp) -> Option<Value> {
    let Guess {
        before,
        after,
        words: keywords,
    } = &decl.guess;
    let text = message.folded.as_str();
    let words = words(text);
    let read = |range: Range<usize>| -> Option<Value> {
        let folded = &text[range.clone()];
        let original = message.original_of(range);
        let value = slot(decl, original, folded, lang, now);
        (!matches!(value, Value::Invalid { .. })).then_some(value)
    };
    let mut found: Vec<Value> = Vec::new();
    let end = without_courtesies(text, &words);
    for at in 0..words.len() {
        for cue in before {
            if cue_at(text, &words, at, cue).is_some() {
                // Longest first ("twenty one"), never from an article ("a
                // 12-sided one" is 12, not "a 12").
                let value = (1..=BEFORE_WORDS.min(at))
                    .rev()
                    .filter(|&n| !ARTICLES.contains(&&text[words[at - n].clone()]))
                    .find_map(|n| read(words[at - n].start..words[at - 1].end));
                found.extend(value);
            }
        }
        for cue in after {
            if let Some(n) = cue_at(text, &words, at, cue)
                && at + n < end
            {
                found.extend(read(words[at + n].start..words[end - 1].end));
            }
        }
    }
    let named: Vec<&String> = keywords
        .iter()
        .filter(|(_, cues)| {
            (0..words.len()).any(|at| cues.iter().any(|c| cue_at(text, &words, at, c).is_some()))
        })
        .map(|(value, _)| value)
        .collect();
    if let [value] = named.as_slice() {
        let n = normalise(value);
        let value = slot(decl, &n.original, &n.folded, lang, now);
        if !matches!(value, Value::Invalid { .. }) {
            found.push(value);
        }
    }
    let first = found.first()?;
    found.iter().all(|v| v == first).then(|| first.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::Number;
    use crate::value::SlotType;

    fn decl(kind: SlotType, guess: Guess) -> SlotDecl {
        SlotDecl {
            name: "x".to_owned(),
            kind,
            required: false,
            repeated: false,
            internal: false,
            options: Vec::new(),
            guess,
        }
    }

    fn cues(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn run(decl: &SlotDecl, text: &str, lang: Lang) -> Option<String> {
        guess(decl, &normalise(text), lang, Timestamp::from_unix(0)).map(|v| v.to_string())
    }

    #[test]
    fn a_number_just_before_its_cue() {
        let count = decl(
            SlotType::Number,
            Guess {
                before: cues(&["dados", "dice"]),
                ..Guess::default()
            },
        );
        assert_eq!(
            run(&count, "oye, lanza tres dados", Lang::Es).as_deref(),
            Some("3")
        );
        assert_eq!(
            run(&count, "throw twenty one dice!", Lang::En).as_deref(),
            Some("21")
        );
        assert_eq!(
            run(&count, "roll 4 dice please", Lang::En).as_deref(),
            Some("4")
        );
        // No cue, a cue with nothing readable before it, or two readings.
        assert_eq!(run(&count, "lanza un dado", Lang::Es), None);
        assert_eq!(run(&count, "los dados", Lang::Es), None);
        assert_eq!(run(&count, "dos dados y tres dados", Lang::Es), None);
        assert_eq!(
            run(&count, "dos dados o dos dados", Lang::Es).as_deref(),
            Some("2")
        );
        let sides = decl(
            SlotType::Number,
            Guess {
                before: cues(&["caras", "sided", "sides"]),
                ..Guess::default()
            },
        );
        assert_eq!(
            run(&sides, "échame un dado de veinte caras", Lang::Es).as_deref(),
            Some("20")
        );
        assert_eq!(
            run(&sides, "a 12-sided one", Lang::En).as_deref(),
            Some("12")
        );
        assert_eq!(
            run(&sides, "a die with 8 sides", Lang::En),
            Some(Number::from_int(8).to_string())
        );
    }

    #[test]
    fn a_trailing_value_after_its_cue() {
        let options = SlotDecl {
            repeated: true,
            ..decl(
                SlotType::Term,
                Guess {
                    after: cues(&["entre", "between"]),
                    ..Guess::default()
                },
            )
        };
        assert_eq!(
            run(
                &options,
                "decide tú entre Pizza o sushi, por favor",
                Lang::Es
            )
            .as_deref(),
            Some("Pizza|sushi")
        );
        assert_eq!(run(&options, "elige entre", Lang::Es), None);
        let place = decl(
            SlotType::Place,
            Guess {
                after: cues(&["en", "in"]),
                ..Guess::default()
            },
        );
        assert_eq!(
            run(&place, "resido en Nueva York", Lang::Es).as_deref(),
            Some("Nueva York")
        );
        assert_eq!(run(&place, "resido en algún sitio", Lang::Es), None);
        let name = decl(
            SlotType::Term,
            Guess {
                after: cues(&["call me", "llamame"]),
                ..Guess::default()
            },
        );
        assert_eq!(
            run(&name, "you can call me Sam", Lang::En).as_deref(),
            Some("Sam")
        );
    }

    #[test]
    fn a_value_named_by_its_words_alone() {
        let key = decl(
            SlotType::Term,
            Guess {
                words: BTreeMap::from([
                    ("name".to_owned(), cues(&["nombre", "name"])),
                    ("place".to_owned(), cues(&["ciudad", "where i live"])),
                ]),
                ..Guess::default()
            },
        );
        assert_eq!(
            run(&key, "¿recuerdas mi nombre?", Lang::Es).as_deref(),
            Some("name")
        );
        assert_eq!(
            run(&key, "tell me where I live", Lang::En).as_deref(),
            Some("place")
        );
        assert_eq!(run(&key, "mi nombre y mi ciudad", Lang::Es), None);
        assert_eq!(run(&key, "nada", Lang::Es), None);
    }
}
