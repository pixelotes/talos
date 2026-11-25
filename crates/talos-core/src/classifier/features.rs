//! Features of a message: character n-grams inside each word, whole words
//! and pairs of adjacent words, read from the match form (lower case,
//! accents folded). The trainer and the engine both call [`extract`], so a
//! model always sees features made the way it was trained on.
//!
//! Before extraction the text is reduced to words:
//!
//! - letters are kept; a run of three or more of the same letter becomes
//!   one ("holaaa" is "hola");
//! - every run of digits becomes one `0`, so "2d6" and "3d8" share their
//!   features (`0d0`) and a number never becomes vocabulary;
//! - sentence punctuation (`¿?¡!.,;:` quotes, brackets of speech) separates
//!   words and is dropped;
//! - any other symbol (`+ * / ^ % = $ €`, emoji) is a word of its own.

use std::borrow::Cow;

/// Which features a model uses; recorded in the model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureConfig {
    /// Shortest character n-gram.
    pub min_n: u8,
    /// Longest character n-gram.
    pub max_n: u8,
    /// Whole words as features.
    pub words: bool,
    /// Pairs of adjacent words as features.
    pub word_pairs: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            min_n: 2,
            max_n: 5,
            words: true,
            word_pairs: true,
        }
    }
}

/// Marks a whole-word feature; it cannot occur in a prepared text.
const WORD: char = '\u{1}';
/// Marks a word-pair feature.
const PAIR: char = '\u{2}';

/// The feature that stands for every word of a message the model does not
/// know, counted once per such word.
pub const UNKNOWN_WORD: &str = "\u{3}";

/// Whether a feature is a whole word.
#[must_use]
pub fn is_word(feature: &str) -> bool {
    feature.starts_with(WORD)
}

/// Only this much of a message is read: features of a long text past this
/// point add cost and nothing a short request needs.
const MAX_CHARS: usize = 400;

/// Sentence punctuation: separates words, is not a feature.
fn is_separator(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(
            c,
            '¿' | '?'
                | '¡'
                | '!'
                | '.'
                | ','
                | ';'
                | ':'
                | '"'
                | '\''
                | '`'
                | '«'
                | '»'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '…'
                | '_'
                | '|'
                | '~'
        )
}

/// The words of a match-form text, joined by single spaces with a space at
/// each end: `" que hora es "`.
#[must_use]
pub fn prepare(folded: &str) -> String {
    let mut out = String::with_capacity(folded.len().min(MAX_CHARS) + 2);
    out.push(' ');
    // The last character pushed, and how many times in a row.
    let mut last = ' ';
    let mut run = 0usize;
    for c in folded.chars().take(MAX_CHARS) {
        let c = if c.is_numeric() { '0' } else { c };
        if is_separator(c) {
            if last != ' ' {
                out.push(' ');
                last = ' ';
            }
            continue;
        }
        if c.is_alphanumeric() {
            if c == last {
                run += 1;
                // Digits collapse at once; letters from the third repeat.
                if c == '0' || run >= 2 {
                    if run == 2 && c != '0' {
                        // "aaa": drop the second "a" already written.
                        out.pop();
                    }
                    continue;
                }
            } else {
                run = 0;
            }
            out.push(c);
            last = c;
        } else {
            // A symbol is a word of its own.
            if last != ' ' {
                out.push(' ');
            }
            out.push(c);
            out.push(' ');
            last = ' ';
            run = 0;
        }
    }
    if last != ' ' {
        out.push(' ');
    }
    out
}

/// How many words a match-form text has, as [`extract`] splits it.
#[must_use]
pub fn word_count(folded: &str) -> usize {
    prepare(folded).split(' ').filter(|w| !w.is_empty()).count()
}

/// The features of a match-form text with their counts, sorted by feature
/// bytes.
#[must_use]
pub fn extract(folded: &str, config: FeatureConfig) -> Vec<(String, u32)> {
    let prepared = prepare(folded);
    let mut all: Vec<Cow<'_, str>> = Vec::with_capacity(prepared.len() * 4);
    let words: Vec<&str> = prepared.split(' ').filter(|w| !w.is_empty()).collect();
    let min_n = usize::from(config.min_n.max(1));
    let max_n = usize::from(config.max_n.max(config.min_n).max(1));
    // Character n-grams of each word with its two spaces: " hora ".
    let mut start = 0;
    for word in &words {
        // `prepared` is " w1 w2 … ", so each word sits between two spaces.
        let at = prepared[start..].find(word).map_or(start, |i| start + i);
        let padded = &prepared[at - 1..=at + word.len()];
        start = at + word.len();
        let bounds: Vec<usize> = padded
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(padded.len()))
            .collect();
        let chars = bounds.len() - 1;
        for n in min_n..=max_n.min(chars) {
            for i in 0..=chars - n {
                all.push(Cow::Borrowed(&padded[bounds[i]..bounds[i + n]]));
            }
        }
        if config.words {
            all.push(Cow::Owned(format!("{WORD}{word}")));
        }
    }
    if config.word_pairs {
        for pair in words.windows(2) {
            all.push(Cow::Owned(format!("{PAIR}{} {}", pair[0], pair[1])));
        }
    }
    all.sort_unstable();
    let mut counted: Vec<(String, u32)> = Vec::with_capacity(all.len());
    for feature in all {
        match counted.last_mut() {
            Some((last, count)) if *last == *feature => *count += 1,
            _ => counted.push((feature.into_owned(), 1)),
        }
    }
    counted
}

/// A feature as a person reads it in a trace: `w:hora`, `p:que hora`, or
/// the n-gram with its spaces shown as `_`.
#[must_use]
pub fn display(feature: &str) -> String {
    if feature == UNKNOWN_WORD {
        "w:?".to_owned()
    } else if let Some(word) = feature.strip_prefix(WORD) {
        format!("w:{word}")
    } else if let Some(pair) = feature.strip_prefix(PAIR) {
        format!("p:{pair}")
    } else {
        feature.replace(' ', "_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_text_is_words_between_single_spaces() {
        assert_eq!(prepare("¿que hora es?"), " que hora es ");
        assert_eq!(prepare("holaaaa!!!"), " hola ");
        assert_eq!(prepare("llamar"), " llamar ");
        assert_eq!(prepare("2d6 y 3d8+2"), " 0d0 y 0d0 + 0 ");
        assert_eq!(prepare("100 km/h"), " 0 km / h ");
        assert_eq!(prepare("15%"), " 0 % ");
        assert_eq!(prepare("what's up"), " what s up ");
        assert_eq!(prepare(""), " ");
        assert_eq!(prepare("   "), " ");
        assert_eq!(prepare("a\u{1}b"), " a b ");
        assert_eq!(word_count("¿qué hora es?"), 3);
        assert_eq!(word_count("2+2"), 3);
    }

    #[test]
    fn features_are_ngrams_words_and_pairs() {
        let config = FeatureConfig {
            min_n: 2,
            max_n: 3,
            words: true,
            word_pairs: true,
        };
        let names: Vec<String> = extract("la la hora", config)
            .iter()
            .map(|(f, c)| format!("{}×{c}", display(f)))
            .collect();
        assert!(names.contains(&"_l×2".to_owned()), "{names:?}");
        assert!(names.contains(&"la_×2".to_owned()), "{names:?}");
        assert!(names.contains(&"w:la×2".to_owned()), "{names:?}");
        assert!(names.contains(&"p:la hora×1".to_owned()), "{names:?}");
        assert!(names.contains(&"ra_×1".to_owned()), "{names:?}");
        // Never across a word boundary.
        assert!(!names.iter().any(|n| n.starts_with("a_h")), "{names:?}");
        // Sorted, distinct.
        let raw = extract("la la hora", config);
        assert!(raw.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn non_ascii_text_is_cut_on_character_boundaries() {
        let features = extract("ñandú 😂😂 日本語", FeatureConfig::default());
        assert_ne!(features.len(), 0);
        assert_eq!(extract("", FeatureConfig::default()).len(), 0);
    }
}
