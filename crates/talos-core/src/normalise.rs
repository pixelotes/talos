//! Stage 0: normalisation (DESIGN.md section 4).
//!
//! Produces the form every later stage matches against. The original text is
//! kept next to it for skills that need it verbatim.

use unicode_normalization::UnicodeNormalization;

/// A message in two forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalised {
    /// Exactly as received.
    pub original: String,
    /// NFKC, lower-cased, typographic variants mapped, whitespace collapsed.
    pub text: String,
}

impl Normalised {
    /// True when nothing but whitespace (or invisible characters) was sent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Normalises a message.
///
/// In order: Unicode NFKC (which also folds superscripts, full-width forms,
/// ligatures, the ellipsis and no-break spaces), lower-casing, typographic
/// variants (curly quotes, dashes, `×`, `÷`), removal of invisible format
/// characters, and whitespace collapsed to single spaces and trimmed.
/// Accents are kept: they are evidence of language.
#[must_use]
pub fn normalise(original: &str) -> Normalised {
    let folded = original.nfkc().collect::<String>().to_lowercase();
    let mut mapped = String::with_capacity(folded.len());
    for c in folded.chars() {
        match typographic(c) {
            Some(replacement) => mapped.push(replacement),
            None if is_invisible(c) => {}
            None if c.is_whitespace() || c.is_control() => mapped.push(' '),
            None => mapped.push(c),
        }
    }
    let text = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    Normalised {
        original: original.to_owned(),
        text,
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
}
