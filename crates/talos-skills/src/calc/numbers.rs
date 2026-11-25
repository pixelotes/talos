//! Reading numbers written in digits, by the conventions of the turn's
//! language (docs/spec/basics.md, "Numbers in and out"), at any length up to
//! 100 digits, and writing an input back as it was meant.

use talos_core::Lang;

use super::rat::{Rat, Shown, group, show_significant};

/// Most digits an input number may have.
pub(crate) const MAX_INPUT_DIGITS: usize = 100;

/// A number read from digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Read {
    pub(crate) value: Rat,
    /// The separators allowed more than one reading, so the reply shows the
    /// one that was taken ("he leído 1.000 como mil").
    pub(crate) unforced: bool,
}

/// Why digits could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// Separators mixed or misplaced for the language (`1,500,000` in
    /// Spanish).
    Malformed,
    /// More than 100 digits, or an exponent beyond reason.
    TooLarge,
}

/// Reads `raw` (digits, `.` and `,` separators, an optional `e<exponent>`),
/// already lower-cased.
///
/// Spanish groups with a dot and writes a decimal comma; English the
/// reverse. Either mark is accepted as a decimal point when the reading is
/// unambiguous; a lone separator followed by exactly three digits is a
/// thousands separator in the language that groups with it.
pub(crate) fn read_digits(raw: &str, lang: Lang) -> Result<Read, ReadError> {
    let (mantissa, exp) = match raw.split_once('e') {
        Some((m, e)) => {
            let exp: i64 = e.parse().map_err(|_| ReadError::TooLarge)?;
            (m, exp)
        }
        None => (raw, 0),
    };
    let edge_digit = |c: char| c.is_ascii_digit();
    if !mantissa.starts_with(edge_digit)
        || !mantissa.ends_with(edge_digit)
        || !mantissa.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return Err(ReadError::Malformed);
    }
    let (group, decimal) = match lang {
        Lang::Es => ('.', ','),
        Lang::En => (',', '.'),
    };
    let grouped = |s: &str, sep: char| {
        let mut parts = s.split(sep);
        let first = parts.next().unwrap_or("");
        (1..=3).contains(&first.len()) && parts.all(|p| p.len() == 3)
    };
    // A lone "1,000" looks like a group: say how it was read.
    let looks_like_group =
        |int: &str, frac: &str| frac.len() == 3 && int.len() <= 3 && !int.starts_with('0');
    let (int, frac, unforced): (String, String, bool) =
        if mantissa.contains(group) && mantissa.contains(decimal) {
            let (int, frac) = mantissa.rsplit_once(decimal).ok_or(ReadError::Malformed)?;
            if frac.contains(group) || !grouped(int, group) {
                return Err(ReadError::Malformed);
            }
            (int.replace(group, ""), frac.to_owned(), false)
        } else if mantissa.contains(group) {
            let count = mantissa.matches(group).count();
            let after = mantissa.rsplit(group).next().unwrap_or("");
            if count == 1 && after.len() != 3 {
                let (int, frac) = mantissa.split_once(group).ok_or(ReadError::Malformed)?;
                // English "1,5" is leniently 1.5, and says so; Spanish "1.5"
                // cannot be anything else.
                (int.to_owned(), frac.to_owned(), lang == Lang::En)
            } else if grouped(mantissa, group) {
                (
                    mantissa.replace(group, ""),
                    String::new(),
                    count == 1 && lang == Lang::Es,
                )
            } else {
                return Err(ReadError::Malformed);
            }
        } else if mantissa.contains(decimal) {
            if mantissa.matches(decimal).count() > 1 {
                return Err(ReadError::Malformed);
            }
            let (int, frac) = mantissa.split_once(decimal).ok_or(ReadError::Malformed)?;
            let unforced = looks_like_group(int, frac);
            (int.to_owned(), frac.to_owned(), unforced)
        } else {
            (mantissa.to_owned(), String::new(), false)
        };
    if int.len() + frac.len() > MAX_INPUT_DIGITS {
        return Err(ReadError::TooLarge);
    }
    let digits = format!("{int}{frac}");
    let scale = i64::try_from(frac.len()).map_err(|_| ReadError::TooLarge)?;
    let value = Rat::from_decimal(false, &digits, exp - scale).map_err(|_| ReadError::TooLarge)?;
    Ok(Read { value, unforced })
}

/// An input number written back: every digit it had; only a number with
/// more than 30 digits that is exactly `d × 10^k` is written that way.
pub(crate) fn echo_number(value: &Rat, lang: Lang, minus: &str) -> Shown {
    let Some(places) = terminating_places(value) else {
        return show_significant(value, 10, false, lang, minus);
    };
    let fixed = value.round_places(places);
    if fixed.int.len() > 30 && value.round_sig(10).exact {
        return show_significant(value, 10, false, lang, minus);
    }
    let mut text = String::new();
    if value.is_neg() {
        text.push_str(minus);
    }
    text.push_str(&group(&fixed.int, lang));
    if !fixed.frac.is_empty() {
        text.push(if lang == Lang::Es { ',' } else { '.' });
        text.push_str(&fixed.frac);
    }
    Shown {
        text,
        approx: false,
    }
}

/// How many decimals `value` has, when it has finitely many.
fn terminating_places(value: &Rat) -> Option<u64> {
    value.decimal_places()
}

/// A number with a decimal point and no grouping: `1500`, `1.5`.
pub(crate) fn canonical_number(n: &Rat) -> String {
    let Some(places) = n.decimal_places() else {
        // Number words and digits are always terminating decimals; kept
        // total anyway.
        let sig = n.round_sig(10);
        return format!("{}{}e{}", if sig.neg { "-" } else { "" }, sig.digits, sig.exp);
    };
    let fixed = n.round_places(places);
    let sign = if fixed.neg { "-" } else { "" };
    if fixed.frac.is_empty() {
        format!("{sign}{}", fixed.int)
    } else {
        format!("{sign}{}.{}", fixed.int, fixed.frac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(raw: &str, lang: Lang) -> Option<(String, bool)> {
        read_digits(raw, lang).ok().map(|r| {
            (
                echo_number(&r.value, Lang::En, "-").text.replace(',', ""),
                r.unforced,
            )
        })
    }

    #[test]
    fn spanish_and_english_conventions() {
        let table = [
            ("1.000", Lang::Es, Some(("1000", true))),
            ("1,000", Lang::Es, Some(("1", true))),
            ("1,000", Lang::En, Some(("1000", false))),
            ("1.000", Lang::En, Some(("1", true))),
            ("1,5", Lang::En, Some(("1.5", true))),
            ("1,5", Lang::Es, Some(("1.5", false))),
            ("3.5", Lang::Es, Some(("3.5", false))),
            ("1.000,50", Lang::Es, Some(("1000.5", false))),
            ("1,000.50", Lang::En, Some(("1000.5", false))),
            ("1.500.000", Lang::Es, Some(("1500000", false))),
            ("0,125", Lang::Es, Some(("0.125", false))),
            ("1,500,000", Lang::Es, None),
            ("1.000,50", Lang::En, None),
            ("12.34.56", Lang::En, None),
            ("1.", Lang::En, None),
            (".5", Lang::En, None),
        ];
        for (raw, lang, want) in table {
            let got = read(raw, lang);
            let want = want.map(|(v, u)| (v.to_owned(), u));
            assert_eq!(got, want, "{raw} in {lang:?}");
        }
    }

    #[test]
    fn exponents_and_length_limits() {
        let big = read_digits("1e30", Lang::En).unwrap();
        assert_eq!(echo_number(&big.value, Lang::En, "-").text, "1 × 10^30");
        assert_eq!(
            echo_number(&read_digits("2e3", Lang::En).unwrap().value, Lang::En, "-").text,
            "2,000"
        );
        let hundred = "9".repeat(100);
        assert!(read_digits(&hundred, Lang::En).is_ok());
        assert_eq!(
            read_digits(&"9".repeat(101), Lang::En),
            Err(ReadError::TooLarge)
        );
        assert_eq!(read_digits("1e99999", Lang::En), Err(ReadError::TooLarge));
        assert_eq!(read_digits("1e", Lang::En), Err(ReadError::TooLarge));
    }

    #[test]
    fn long_decimals_are_echoed_in_full() {
        let pi = "3.14159265358979323846264338327950288";
        let r = read_digits(pi, Lang::En).unwrap();
        assert_eq!(echo_number(&r.value, Lang::En, "-").text, pi);
    }
}
