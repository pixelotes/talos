//! Typed slot values (docs/SKILLS.md section 3, docs/spec/conversation.md
//! section 2).
//!
//! Rule captures are turned into these by the extractors in
//! [`extract`](crate::extract) before a handler sees them, so a handler
//! never parses text.

use std::collections::BTreeMap;
use std::fmt;

use crate::Lang;
use crate::civil::Date;

/// What was extracted from a message, by slot name. Ordered, so outcomes
/// compare and print deterministically.
pub type Slots = BTreeMap<String, Value>;

/// The type a manifest declares for a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotType {
    /// A word or phrase taken as content, in the user's own spelling.
    Term,
    /// A number, in digits or words.
    Number,
    /// A calendar date, absolute or relative to the turn's `now`.
    Date,
}

impl SlotType {
    /// The name used in manifests.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            SlotType::Term => "term",
            SlotType::Number => "number",
            SlotType::Date => "date",
        }
    }

    /// The type a manifest names, if it is one this version extracts.
    #[must_use]
    pub fn parse(name: &str) -> Option<SlotType> {
        match name {
            "term" => Some(SlotType::Term),
            "number" => Some(SlotType::Number),
            "date" => Some(SlotType::Date),
            _ => None,
        }
    }
}

/// An exact decimal number: `units / 10^scale`, with no trailing zeros in
/// the fraction. Exact, so `0.1 + 0.2` stays a question for the calculator
/// and not for floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Number {
    units: i128,
    scale: u8,
}

/// The largest number of digits a [`Number`] holds, so that every value fits
/// in its `i128` with room for arithmetic.
pub(crate) const MAX_DIGITS: usize = 30;

impl Number {
    /// A whole number.
    #[must_use]
    pub const fn from_int(value: i64) -> Number {
        Number {
            units: value as i128,
            scale: 0,
        }
    }

    /// `units / 10^scale`, normalised. `None` if the scale exceeds
    /// [`MAX_DIGITS`].
    #[must_use]
    pub fn new(mut units: i128, mut scale: u8) -> Option<Number> {
        if usize::from(scale) > MAX_DIGITS {
            return None;
        }
        while scale > 0 && units % 10 == 0 {
            units /= 10;
            scale -= 1;
        }
        Some(Number { units, scale })
    }

    /// The value scaled to an integer: `units` in `units / 10^scale`.
    #[must_use]
    pub const fn units(self) -> i128 {
        self.units
    }

    /// Digits after the decimal point.
    #[must_use]
    pub const fn scale(self) -> u8 {
        self.scale
    }

    /// Whether the number has no fractional part.
    #[must_use]
    pub const fn is_integer(self) -> bool {
        self.scale == 0
    }

    /// The value as an `i64`, if it is a whole number that fits.
    #[must_use]
    pub fn to_i64(self) -> Option<i64> {
        if self.is_integer() {
            i64::try_from(self.units).ok()
        } else {
            None
        }
    }

    /// The value negated.
    #[must_use]
    pub const fn neg(self) -> Number {
        Number {
            units: -self.units,
            scale: self.scale,
        }
    }

    /// The number multiplied by `factor`, if it still fits.
    #[must_use]
    pub fn times(self, factor: Number) -> Option<Number> {
        let units = self.units.checked_mul(factor.units)?;
        Number::new(units, self.scale.checked_add(factor.scale)?)
    }

    /// The sum, if it fits.
    #[must_use]
    pub fn plus(self, other: Number) -> Option<Number> {
        let scale = self.scale.max(other.scale);
        let a = self
            .units
            .checked_mul(10_i128.checked_pow(u32::from(scale - self.scale))?)?;
        let b = other
            .units
            .checked_mul(10_i128.checked_pow(u32::from(scale - other.scale))?)?;
        Number::new(a.checked_add(b)?, scale)
    }

    /// The integer and fraction digits of the absolute value.
    fn digits(self) -> (String, String) {
        let all = self.units.unsigned_abs().to_string();
        let scale = usize::from(self.scale);
        if all.len() > scale {
            let (int, frac) = all.split_at(all.len() - scale);
            (int.to_owned(), frac.to_owned())
        } else {
            ("0".to_owned(), format!("{all:0>scale$}"))
        }
    }

    /// The number written by the conventions of `lang` (docs/spec/basics.md,
    /// "Numbers in and out"): Spanish writes a decimal comma and groups
    /// thousands with a dot from five integer digits up (`1234`, `12.345`);
    /// English writes a decimal point and groups with commas from four up
    /// (`1,234`).
    #[must_use]
    pub fn format(self, lang: Lang) -> String {
        let (int, frac) = self.digits();
        let (group, decimal, from) = match lang {
            Lang::Es => ('.', ',', 5),
            Lang::En => (',', '.', 4),
        };
        let mut out = String::new();
        if self.units < 0 {
            out.push('-');
        }
        if int.len() >= from {
            for (i, c) in int.chars().enumerate() {
                if i > 0 && (int.len() - i) % 3 == 0 {
                    out.push(group);
                }
                out.push(c);
            }
        } else {
            out.push_str(&int);
        }
        if !frac.is_empty() {
            out.push(decimal);
            out.push_str(&frac);
        }
        out
    }
}

/// The canonical form used in outcome lines and golden files: a decimal
/// point, no grouping (`1500`, `1.5`, `-3`).
impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (int, frac) = self.digits();
        let sign = if self.units < 0 { "-" } else { "" };
        if frac.is_empty() {
            write!(f, "{sign}{int}")
        } else {
            write!(f, "{sign}{int}.{frac}")
        }
    }
}

/// Why a capture could not become a value of its slot's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Not a value of the type at all, or malformed (`1.500,25` in an
    /// English turn).
    Unreadable,
    /// More than one reading; the candidates, in order (English `3/4`).
    Ambiguous(Vec<Value>),
    /// Well formed but naming nothing real ("31 de abril").
    DoesNotExist,
}

/// A typed slot value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A term: text in the user's original spelling, case and accents.
    Text(String),
    /// A number.
    Number(Number),
    /// A date, resolved against the turn's `now`.
    Date {
        /// The date.
        date: Date,
        /// Whether the user gave the year. Without one the date is the next
        /// occurrence on or after today; a skill that means the most recent
        /// one ("since") moves it back a year itself.
        year_given: bool,
    },
    /// A repeated term: a list of options in the user's spelling, repeats
    /// (ignoring case) removed.
    List {
        /// The items, in order.
        items: Vec<String>,
        /// True when the list mixed "y"/"and" with "o"/"or", so the split
        /// may not be what the user meant and the reply should show it.
        uncertain: bool,
    },
    /// A capture that could not be read as its slot's type.
    Invalid {
        /// The text captured, in the user's spelling.
        raw: String,
        /// What is wrong with it.
        problem: Problem,
    },
}

impl Value {
    /// The text of a [`Value::Text`].
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The number of a [`Value::Number`].
    #[must_use]
    pub fn as_number(&self) -> Option<Number> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The date of a [`Value::Date`].
    #[must_use]
    pub fn as_date(&self) -> Option<Date> {
        match self {
            Value::Date { date, .. } => Some(*date),
            _ => None,
        }
    }

    /// The items of a [`Value::List`].
    #[must_use]
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::List { items, .. } => Some(items),
            _ => None,
        }
    }
}

/// The canonical form used in outcome lines and golden files: numbers with a
/// decimal point, dates in ISO 8601, list items joined by `|`, an invalid
/// value as the text captured.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(text) | Value::Invalid { raw: text, .. } => f.write_str(text),
            Value::Number(n) => n.fmt(f),
            Value::Date { date, .. } => date.fmt(f),
            Value::List { items, .. } => f.write_str(&items.join("|")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(units: i128, scale: u8) -> Number {
        Number::new(units, scale).unwrap()
    }

    #[test]
    fn numbers_are_normalised_and_print_canonically() {
        assert_eq!(n(15_000, 4), n(15, 1));
        assert_eq!(n(15, 1).to_string(), "1.5");
        assert_eq!(n(-3, 0).to_string(), "-3");
        assert_eq!(n(5, 3).to_string(), "0.005");
        assert_eq!(n(-5, 1).to_string(), "-0.5");
        assert_eq!(Number::from_int(1500).to_i64(), Some(1500));
        assert_eq!(n(15, 1).to_i64(), None);
        assert_eq!(n(15, 1).plus(n(25, 2)), Some(n(175, 2)));
        assert_eq!(
            n(25, 1).times(Number::from_int(1_000_000)),
            Some(n(2_500_000, 0))
        );
    }

    #[test]
    fn numbers_are_written_by_the_languages_conventions() {
        let cases = [
            (n(1234, 0), "1234", "1,234"),
            (n(12_345, 0), "12.345", "12,345"),
            (n(123_456_789, 2), "1.234.567,89", "1,234,567.89"),
            (n(-15, 1), "-1,5", "-1.5"),
            (n(999, 0), "999", "999"),
        ];
        for (number, es, en) in cases {
            assert_eq!(number.format(Lang::Es), es);
            assert_eq!(number.format(Lang::En), en);
        }
    }

    #[test]
    fn values_print_canonically() {
        let date = Date::new(2027, 4, 3).unwrap();
        assert_eq!(
            Value::Date {
                date,
                year_given: false
            }
            .to_string(),
            "2027-04-03"
        );
        let list = Value::List {
            items: vec!["té".to_owned(), "café".to_owned()],
            uncertain: false,
        };
        assert_eq!(list.to_string(), "té|café");
        assert_eq!(list.as_list().map(<[String]>::len), Some(2));
        assert_eq!(SlotType::parse("number"), Some(SlotType::Number));
        assert_eq!(SlotType::parse("place"), None);
    }
}
