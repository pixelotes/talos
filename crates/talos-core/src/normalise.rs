//! Stage 0: normalisation (DESIGN.md section 4).
//!
//! Produces three aligned forms of a message:
//!
//! - `original`, exactly as received, for terms that must keep the user's
//!   spelling;
//! - `text`: NFKC, lower case, typographic variants mapped, whitespace
//!   collapsed. Accents are kept, because they are language evidence;
//! - `folded`, the **match form**: `text` with diacritics removed from Latin
//!   letters (`qué` → `que`, `mañana` → `manana`). Rules and extractors match
//!   this form, so "que hora es" and "qué hora es" are the same request
//!   (docs/spec/conversation.md section 2), and patterns are written without
//!   accents.
//!
//! `text` and `folded` have the same characters one for one, and every
//! character remembers the span of the original it came from, so a capture
//! in the match form can be read back in the user's own spelling.

use std::ops::Range;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{decompose_canonical, is_combining_mark};

/// Where one character of the normalised forms came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Origin {
    /// Byte offset of the character in `text`.
    text: usize,
    /// Byte offset of the character in `folded`.
    folded: usize,
    /// The span of `original` it was derived from.
    original: (usize, usize),
}

/// A message in its three forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalised {
    /// Exactly as received.
    pub original: String,
    /// NFKC, lower-cased, typographic variants mapped, whitespace collapsed;
    /// accents kept.
    pub text: String,
    /// The match form: `text` without diacritics on Latin letters.
    pub folded: String,
    /// One entry per character of `text` (and of `folded`).
    origins: Vec<Origin>,
}

impl Normalised {
    /// True when nothing but whitespace (or invisible characters) was sent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The characters whose `folded` offsets fall in `range`.
    fn origins_in(&self, range: &Range<usize>) -> &[Origin] {
        let start = self.origins.partition_point(|o| o.folded < range.start);
        let end = self.origins.partition_point(|o| o.folded < range.end);
        &self.origins[start..end]
    }

    /// The part of the original message that a span of the match form came
    /// from, e.g. a rule capture read back in the user's own spelling.
    #[must_use]
    pub fn original_of(&self, folded: Range<usize>) -> &str {
        match self.origins_in(&folded) {
            [] => "",
            [first, .., last] => &self.original[first.original.0..last.original.1],
            [only] => &self.original[only.original.0..only.original.1],
        }
    }

    /// The part of `text` (accents kept) that a span of the match form
    /// covers.
    #[must_use]
    pub fn text_of(&self, folded: Range<usize>) -> &str {
        let origins = self.origins_in(&folded);
        let Some(first) = origins.first() else {
            return "";
        };
        let end = self
            .origins
            .get(self.origins.partition_point(|o| o.folded < folded.end))
            .map_or(self.text.len(), |o| o.text);
        &self.text[first.text..end]
    }
}

/// The match-form equivalent of one character: a Latin letter with
/// diacritics becomes its base letter; anything else is kept.
#[must_use]
pub fn fold_char(c: char) -> char {
    if c.is_ascii() {
        return c;
    }
    let mut base = None;
    let mut marks_only = true;
    decompose_canonical(c, |d| match base {
        None => base = Some(d),
        Some(_) => marks_only &= is_combining_mark(d),
    });
    match base {
        Some(b) if b != c && b.is_ascii_alphabetic() && marks_only => b,
        _ => c,
    }
}

/// The match form of already lower-cased text.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

/// Normalises a message.
///
/// In order: Unicode NFKC (which also folds superscripts, full-width forms,
/// ligatures, the ellipsis and no-break spaces), lower-casing, typographic
/// variants (curly quotes, dashes, `×`, `÷`), removal of invisible format
/// characters, and whitespace collapsed to single spaces and trimmed. The
/// match form then folds diacritics.
#[must_use]
pub fn normalise(original: &str) -> Normalised {
    // Each base character with its combining marks is normalised as a unit,
    // so every output character knows which original span it came from.
    let mut mapped: Vec<(char, (usize, usize))> = Vec::with_capacity(original.len());
    let mut chars = original.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let mut end = start + c.len_utf8();
        while let Some(&(i, next)) = chars.peek() {
            if !is_combining_mark(next) {
                break;
            }
            end = i + next.len_utf8();
            chars.next();
        }
        for composed in original[start..end].nfkc() {
            for lower in composed.to_lowercase() {
                let out = match typographic(lower) {
                    Some(replacement) => replacement,
                    None if is_invisible(lower) => continue,
                    None if lower.is_whitespace() || lower.is_control() => ' ',
                    None => lower,
                };
                mapped.push((out, (start, end)));
            }
        }
    }

    let mut text = String::with_capacity(mapped.len());
    let mut folded = String::with_capacity(mapped.len());
    let mut origins = Vec::with_capacity(mapped.len());
    let mut pending_space = false;
    for (c, span) in mapped {
        if c == ' ' {
            pending_space = !text.is_empty();
            continue;
        }
        if pending_space {
            origins.push(Origin {
                text: text.len(),
                folded: folded.len(),
                original: span,
            });
            text.push(' ');
            folded.push(' ');
            pending_space = false;
        }
        origins.push(Origin {
            text: text.len(),
            folded: folded.len(),
            original: span,
        });
        text.push(c);
        folded.push(fold_char(c));
    }
    Normalised {
        original: original.to_owned(),
        text,
        folded,
        origins,
    }
}

/// The plain equivalent of a typographic variant, if `c` is one.
fn typographic(c: char) -> Option<char> {
    Some(match c {
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2032}' | '`' | '\u{00B4}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{2033}' | '\u{00AB}'
        | '\u{00BB}' => '"',
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
        | '\u{2212}' => '-',
        '\u{00D7}' | '\u{22C5}' => '*',
        '\u{00F7}' | '\u{2215}' => '/',
        _ => return None,
    })
}

/// Zero-width and other format characters that carry no text.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> String {
        normalise(s).text
    }

    #[test]
    fn keeps_the_original() {
        let out = normalise("  ¡HOLA!  ");
        assert_eq!(out.original, "  ¡HOLA!  ");
        assert_eq!(out.text, "¡hola!");
    }

    #[test]
    fn folds_case_and_keeps_accents() {
        assert_eq!(n("¿QUÉ Hora ES?"), "¿qué hora es?");
        assert_eq!(n("ÑANDÚ"), "ñandú");
    }

    #[test]
    fn composes_decomposed_accents() {
        // "e" + combining acute becomes the single code point "é".
        assert_eq!(n("que\u{301}"), "qué");
        assert_eq!(normalise("que\u{301}").folded, "que");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(n("buenas \t\n  tardes\u{00A0}talos"), "buenas tardes talos");
        assert_eq!(n(" \t\n "), "");
        assert!(normalise("\u{200B}").is_empty());
    }

    #[test]
    fn maps_typographic_variants() {
        assert_eq!(n("what\u{2019}s"), "what's");
        assert_eq!(n("\u{201C}hi\u{201D} «hola»"), "\"hi\" \"hola\"");
        assert_eq!(n("3 × 4 ÷ 2 − 1"), "3 * 4 / 2 - 1");
        assert_eq!(n("x² …"), "x2 ...");
        assert_eq!(n("ＨＯＬＡ"), "hola");
    }

    #[test]
    fn the_match_form_drops_diacritics_only() {
        let out = normalise("¿Qué tal, MAÑANA? Pingüino ß ø 3×4");
        assert_eq!(out.text, "¿qué tal, mañana? pingüino ß ø 3*4");
        assert_eq!(out.folded, "¿que tal, manana? pinguino ß ø 3*4");
        assert_eq!(out.text.chars().count(), out.folded.chars().count());
        assert_eq!(fold("àéîõüç"), "aeiouc");
    }

    #[test]
    fn spans_of_the_match_form_read_back_the_original() {
        let out = normalise("Elige:  Té,   CAFÉ o Agua!");
        let start = out.folded.find("te,").unwrap();
        let end = out.folded.find('!').unwrap();
        assert_eq!(out.original_of(start..end), "Té,   CAFÉ o Agua");
        assert_eq!(out.text_of(start..end), "té, café o agua");
        assert_eq!(out.original_of(0..0), "");
        let decomposed = normalise("cafe\u{301} solo");
        let end = decomposed.folded.find(' ').unwrap();
        assert_eq!(decomposed.original_of(0..end), "cafe\u{301}");
    }
}
