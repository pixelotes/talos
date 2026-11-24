//! The two languages Talos speaks (ADR 0011).

use std::fmt;
use std::str::FromStr;

/// A conversation language. Nothing in the engine assumes exactly two, but
/// the first major version ships Spanish and English only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {
    /// Spanish (peninsular).
    Es,
    /// English.
    En,
}

impl Lang {
    /// Every supported language, in a fixed order.
    pub const ALL: [Lang; 2] = [Lang::Es, Lang::En];

    /// The ISO 639-1 code: `es` or `en`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    /// The other language: the one whose rules are tried second (ADR 0011).
    #[must_use]
    pub const fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }

    /// Position in [`Lang::ALL`], for per-language tables.
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        match self {
            Lang::Es => 0,
            Lang::En => 1,
        }
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// The text was not `es` or `en`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLang(pub String);

impl fmt::Display for UnknownLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown language `{}` (expected `es` or `en`)", self.0)
    }
}

impl std::error::Error for UnknownLang {}

impl FromStr for Lang {
    type Err = UnknownLang;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "es" => Ok(Lang::Es),
            "en" => Ok(Lang::En),
            _ => Err(UnknownLang(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codes() {
        assert_eq!("es".parse(), Ok(Lang::Es));
        assert_eq!(" EN ".parse(), Ok(Lang::En));
        assert!("fr".parse::<Lang>().is_err());
    }

    #[test]
    fn other_is_an_involution() {
        for lang in Lang::ALL {
            assert_eq!(lang.other().other(), lang);
            assert_ne!(lang.other(), lang);
            assert_eq!(Lang::ALL[lang.index()], lang);
        }
    }
}
