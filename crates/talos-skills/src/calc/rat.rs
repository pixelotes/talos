//! Exact rational numbers over [`Big`], with the roundings and the number
//! formatting the calculator shows (docs/spec/basics.md, `calc`).
//!
//! Every constructor and operation checks the size of its result, so no
//! input can make the calculator run away: a value whose numerator or
//! denominator would exceed [`MAX_BITS`] is [`MathError::TooLarge`].

use std::cmp::Ordering;

use talos_core::{Lang, Number};

use super::big::Big;

/// Largest numerator or denominator, in bits (about 10^2400).
pub(crate) const MAX_BITS: usize = 8000;
const MAX_BITS_U64: u64 = 8000;

/// The decimal exponent beyond which a result is refused (10^1000).
const MAX_EXP10: u64 = 1000;

/// Why a calculation has no result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MathError {
    /// A value too large (or too precise) to compute.
    TooLarge,
    /// Division by zero; `zero_over_zero` when the dividend was zero too.
    DivZero { zero_over_zero: bool },
    /// A power with a fractional exponent beyond roots.
    Unsupported,
}

/// An exact fraction, always in lowest terms with a positive denominator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rat {
    neg: bool,
    num: Big,
    den: Big,
}

impl Rat {
    pub(crate) fn zero() -> Rat {
        Rat {
            neg: false,
            num: Big::zero(),
            den: Big::one(),
        }
    }

    pub(crate) fn from_int(value: i64) -> Rat {
        Rat {
            neg: value < 0,
            num: Big::from_u64(value.unsigned_abs()),
            den: Big::one(),
        }
    }

    pub(crate) fn from_number(number: Number) -> Rat {
        Rat {
            neg: number.units() < 0,
            num: Big::from_u128(number.units().unsigned_abs()),
            den: Big::pow10(u64::from(number.scale())),
        }
        .reduced()
    }

    /// `neg`, `num / den`, reduced; checks the sizes and the divisor.
    pub(crate) fn new(neg: bool, num: Big, den: Big) -> Result<Rat, MathError> {
        if den.is_zero() {
            return Err(MathError::DivZero {
                zero_over_zero: num.is_zero(),
            });
        }
        if num.bits() > 2 * MAX_BITS || den.bits() > 2 * MAX_BITS {
            return Err(MathError::TooLarge);
        }
        let reduced = Rat { neg, num, den }.reduced();
        if reduced.num.bits() > MAX_BITS || reduced.den.bits() > MAX_BITS {
            return Err(MathError::TooLarge);
        }
        Ok(reduced)
    }

    fn reduced(mut self) -> Rat {
        if self.num.is_zero() {
            return Rat::zero();
        }
        if !self.den.is_one() {
            let g = self.num.gcd(&self.den);
            if !g.is_one()
                && let (Some((num, _)), Some((den, _))) = (self.num.divrem(&g), self.den.divrem(&g))
            {
                self.num = num;
                self.den = den;
            }
        }
        self
    }

    /// `digits` (integer and fraction digits joined) scaled by
    /// `10^exp10`: a decimal literal, as `from_decimal("15", 1)` = 1.5.
    pub(crate) fn from_decimal(neg: bool, digits: &str, exp10: i64) -> Result<Rat, MathError> {
        let units = Big::from_dec(digits).ok_or(MathError::TooLarge)?;
        if exp10.unsigned_abs() > 1100 {
            return Err(MathError::TooLarge);
        }
        let scale = Big::pow10(exp10.unsigned_abs());
        let rat = if exp10 >= 0 {
            Rat::new(neg, units.mul(&scale), Big::one())?
        } else {
            Rat::new(neg, units, scale)?
        };
        rat.within_limit()
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    pub(crate) fn is_neg(&self) -> bool {
        self.neg
    }

    pub(crate) fn is_integer(&self) -> bool {
        self.den.is_one()
    }

    pub(crate) fn neg(&self) -> Rat {
        Rat {
            neg: !self.neg && !self.num.is_zero(),
            num: self.num.clone(),
            den: self.den.clone(),
        }
    }

    pub(crate) fn abs(&self) -> Rat {
        Rat {
            neg: false,
            num: self.num.clone(),
            den: self.den.clone(),
        }
    }

    /// Refuses a value beyond 10^1000.
    pub(crate) fn within_limit(self) -> Result<Rat, MathError> {
        let limit = Big::pow10(MAX_EXP10).mul(&self.den);
        if self.num > limit {
            Err(MathError::TooLarge)
        } else {
            Ok(self)
        }
    }

    pub(crate) fn add(&self, other: &Rat) -> Result<Rat, MathError> {
        let (a, b) = (self.num.mul(&other.den), other.num.mul(&self.den));
        let den = self.den.mul(&other.den);
        let (neg, num) = match (self.neg, other.neg) {
            (x, y) if x == y => (x, a.add(&b)),
            (x, _) => {
                if a >= b {
                    (x, a.sub(&b))
                } else {
                    (!x, b.sub(&a))
                }
            }
        };
        Rat::new(neg, num, den)
    }

    pub(crate) fn sub(&self, other: &Rat) -> Result<Rat, MathError> {
        self.add(&other.neg())
    }

    pub(crate) fn mul(&self, other: &Rat) -> Result<Rat, MathError> {
        Rat::new(
            self.neg != other.neg,
            self.num.mul(&other.num),
            self.den.mul(&other.den),
        )
    }

    pub(crate) fn div(&self, other: &Rat) -> Result<Rat, MathError> {
        if other.is_zero() {
            return Err(MathError::DivZero {
                zero_over_zero: self.is_zero(),
            });
        }
        Rat::new(
            self.neg != other.neg,
            self.num.mul(&other.den),
            self.den.mul(&other.num),
        )
    }

    /// The exact power with an integer exponent (`exp` must be an integer).
    /// `0^0` is 1, as on every calculator.
    pub(crate) fn powi(&self, exp: &Rat) -> Result<Rat, MathError> {
        if !exp.is_integer() {
            return Err(MathError::Unsupported);
        }
        if exp.is_zero() {
            return Ok(Rat::from_int(1));
        }
        if self.is_zero() {
            return if exp.neg {
                Err(MathError::DivZero {
                    zero_over_zero: false,
                })
            } else {
                Ok(Rat::zero())
            };
        }
        let odd = !exp.num.is_even();
        if self.num.is_one() && self.den.is_one() {
            return Ok(Rat::from_int(if self.neg && odd { -1 } else { 1 }));
        }
        let e = exp.num.to_u64().ok_or(MathError::TooLarge)?;
        let size = self.num.bits().max(self.den.bits()).saturating_sub(1);
        let estimate = e.saturating_mul(u64::try_from(size).unwrap_or(u64::MAX));
        if estimate > MAX_BITS_U64 {
            return Err(MathError::TooLarge);
        }
        let (num, den) = (self.num.pow(e), self.den.pow(e));
        let neg = self.neg && odd;
        if exp.neg {
            Rat::new(neg, den, num)
        } else {
            Rat::new(neg, num, den)
        }
    }

    pub(crate) fn cmp_value(&self, other: &Rat) -> Ordering {
        match (self.neg, other.neg) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (neg, _) => {
                let ord = self
                    .num
                    .mul(&other.den)
                    .cmp(&other.num.mul(&self.den));
                if neg { ord.reverse() } else { ord }
            }
        }
    }

    /// The `n`th root (2 or 3) of a non-negative value, to 40 significant
    /// digits when not exact. The flag says whether it is exact.
    pub(crate) fn root(&self, n: u32) -> Result<(Rat, bool), MathError> {
        let take = |x: &Big| if n == 2 { x.isqrt() } else { x.icbrt() };
        let power = |x: &Big| if n == 2 { x.mul(x) } else { x.mul(x).mul(x) };
        let (rn, rd) = (take(&self.num), take(&self.den));
        if power(&rn) == self.num && power(&rd) == self.den {
            return Ok((Rat::new(false, rn, rd)?, true));
        }
        // floor(root(num * 10^(n k) / den)) / 10^k, with k large enough for
        // 40 significant digits.
        let magnitude = i64::try_from(self.num.to_dec().len()).unwrap_or(i64::MAX)
            - i64::try_from(self.den.to_dec().len()).unwrap_or(0);
        let k = u64::try_from((40 - magnitude / i64::from(n)).max(0)).unwrap_or(0);
        let scaled = self.num.mul(&Big::pow10(k * u64::from(n)));
        let (q, _) = scaled.divrem(&self.den).ok_or(MathError::TooLarge)?;
        Ok((Rat::new(false, take(&q), Big::pow10(k))?, false))
    }

    /// Rounds `|self|` (non-zero) to `sig` significant digits, half up.
    pub(crate) fn round_sig(&self, sig: usize) -> Sig {
        let digits_of = |b: &Big| i64::try_from(b.to_dec().len()).unwrap_or(i64::MAX);
        let est = digits_of(&self.num) - digits_of(&self.den);
        let at_least = |e: i64| {
            let p = Big::pow10(e.unsigned_abs());
            if e >= 0 {
                self.num >= self.den.mul(&p)
            } else {
                self.num.mul(&p) >= self.den
            }
        };
        let e10 = if at_least(est) { est } else { est - 1 };
        let mut shift = i64::try_from(sig).unwrap_or(10) - 1 - e10;
        let scale = Big::pow10(shift.unsigned_abs());
        let (numer, denom) = if shift >= 0 {
            (self.num.mul(&scale), self.den.clone())
        } else {
            (self.num.clone(), self.den.mul(&scale))
        };
        let (mut quotient, rest) = numer.divrem(&denom).unwrap_or((Big::zero(), Big::zero()));
        let exact = rest.is_zero();
        if rest.shl(1) >= denom {
            quotient = quotient.add(&Big::one());
            if quotient.to_dec().len() > sig {
                quotient = Big::pow10(u64::try_from(sig).unwrap_or(10) - 1);
                shift -= 1;
            }
        }
        Sig {
            neg: self.neg,
            digits: quotient.to_dec(),
            exp: -shift,
            exact,
        }
    }

    /// Rounds `|self|` to `places` decimals, half up.
    pub(crate) fn round_places(&self, places: u64) -> Fixed {
        let numer = self.num.mul(&Big::pow10(places));
        let (mut quotient, rest) = numer.divrem(&self.den).unwrap_or((Big::zero(), Big::zero()));
        let exact = rest.is_zero();
        if rest.shl(1) >= self.den {
            quotient = quotient.add(&Big::one());
        }
        let all = quotient.to_dec();
        let places = usize::try_from(places).unwrap_or(0);
        let padded = if all.len() <= places {
            format!("{}{all}", "0".repeat(places + 1 - all.len()))
        } else {
            all
        };
        let (int, frac) = padded.split_at(padded.len() - places);
        Fixed {
            neg: self.neg,
            int: int.to_owned(),
            frac: frac.trim_end_matches('0').to_owned(),
            exact,
        }
    }

    /// `(p, q)` when the value is `p / q` with `q` 2 or 3 and a small `p`.
    pub(crate) fn small_fraction(&self) -> Option<(i64, u32)> {
        let q = u32::try_from(self.den.to_u64()?).ok().filter(|q| matches!(q, 2 | 3))?;
        let p = i64::try_from(self.num.to_u64()?).ok().filter(|p| *p <= 1000)?;
        Some((if self.neg { -p } else { p }, q))
    }

    /// How many decimals the value has, when it has finitely many.
    pub(crate) fn decimal_places(&self) -> Option<u64> {
        let (mut twos, mut fives) = (0_u64, 0_u64);
        let mut rest = self.den.clone();
        while rest.is_even() && !rest.is_zero() {
            rest = rest.shr(1);
            twos += 1;
        }
        loop {
            let (q, r) = rest.divrem_small(5);
            if r != 0 {
                break;
            }
            rest = q;
            fives += 1;
        }
        rest.is_one().then_some(twos.max(fives))
    }

    /// The integer digits of `|self|` (the value must be an integer).
    pub(crate) fn int_digits(&self) -> String {
        self.num.to_dec()
    }
}

/// A value rounded to a number of significant digits:
/// `±digits × 10^exp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sig {
    pub(crate) neg: bool,
    pub(crate) digits: String,
    pub(crate) exp: i64,
    pub(crate) exact: bool,
}

impl Sig {
    /// `floor(log10 |value|)` of the rounded value.
    pub(crate) fn e10(&self) -> i64 {
        self.exp + i64::try_from(self.digits.len()).unwrap_or(0) - 1
    }
}

/// A value rounded to a number of decimals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fixed {
    pub(crate) neg: bool,
    pub(crate) int: String,
    /// Fraction digits, trailing zeros dropped.
    pub(crate) frac: String,
    pub(crate) exact: bool,
}

impl Fixed {
    pub(crate) fn is_zero(&self) -> bool {
        self.int.bytes().all(|b| b == b'0') && self.frac.is_empty()
    }
}

/// A shown number and whether it was rounded (the reply then says `≈`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) text: String,
    pub(crate) approx: bool,
}

/// Groups integer digits as the language does (basics.md, "Numbers in and
/// out"): Spanish from five digits up with a dot, English from four with a
/// comma.
pub(crate) fn group(int: &str, lang: Lang) -> String {
    let (sep, from) = match lang {
        Lang::Es => ('.', 5),
        Lang::En => (',', 4),
    };
    if int.len() < from {
        return int.to_owned();
    }
    let mut out = String::with_capacity(int.len() + int.len() / 3);
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn decimal_mark(lang: Lang) -> char {
    match lang {
        Lang::Es => ',',
        Lang::En => '.',
    }
}

/// `digits × 10^exp` written in full, trailing zeros of `digits` dropped.
fn plain(neg: bool, digits: &str, exp: i64, lang: Lang, minus: &str) -> String {
    let (digits, exp) = {
        let trimmed = digits.trim_end_matches('0');
        let dropped = i64::try_from(digits.len() - trimmed.len()).unwrap_or(0);
        if trimmed.is_empty() {
            ("0", 0)
        } else {
            (trimmed, exp + dropped)
        }
    };
    let len = i64::try_from(digits.len()).unwrap_or(0);
    let (int, frac) = if exp >= 0 {
        (
            format!("{digits}{}", "0".repeat(usize::try_from(exp).unwrap_or(0))),
            String::new(),
        )
    } else if len > -exp {
        let at = usize::try_from(len + exp).unwrap_or(0);
        (digits[..at].to_owned(), digits[at..].to_owned())
    } else {
        (
            "0".to_owned(),
            format!(
                "{}{digits}",
                "0".repeat(usize::try_from(-exp - len).unwrap_or(0))
            ),
        )
    };
    let mut out = String::new();
    if neg {
        out.push_str(minus);
    }
    out.push_str(&group(&int, lang));
    if !frac.is_empty() {
        out.push(decimal_mark(lang));
        out.push_str(&frac);
    }
    out
}

/// `d,ddd × 10^e`.
fn scientific(neg: bool, digits: &str, exp: i64, lang: Lang, minus: &str) -> String {
    let trimmed = digits.trim_end_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    let e10 = exp + i64::try_from(digits.len()).unwrap_or(0) - 1;
    let mut out = String::new();
    if neg {
        out.push_str(minus);
    }
    out.push_str(&trimmed[..1]);
    if trimmed.len() > 1 {
        out.push(decimal_mark(lang));
        out.push_str(&trimmed[1..]);
    }
    out.push_str(" × 10^");
    out.push_str(&e10.to_string());
    out
}

/// Shows a value at most `sig` significant digits long: whole numbers up to
/// 30 digits exactly, plain decimals when the magnitude is moderate,
/// scientific notation otherwise. `approx_input` forces `≈` for values that
/// were already approximations.
pub(crate) fn show_significant(
    value: &Rat,
    sig: usize,
    approx_input: bool,
    lang: Lang,
    minus: &str,
) -> Shown {
    if value.is_zero() {
        return Shown {
            text: "0".to_owned(),
            approx: approx_input,
        };
    }
    if value.is_integer() && !approx_input {
        let digits = value.int_digits();
        if digits.len() <= 30 {
            return Shown {
                text: format!(
                    "{}{}",
                    if value.is_neg() { minus } else { "" },
                    group(&digits, lang)
                ),
                approx: false,
            };
        }
    }
    let r = value.round_sig(sig);
    let e10 = r.e10();
    let text = if (-15..30).contains(&e10) {
        plain(r.neg, &r.digits, r.exp, lang, minus)
    } else {
        scientific(r.neg, &r.digits, r.exp, lang, minus)
    };
    Shown {
        text,
        approx: approx_input || !r.exact,
    }
}

/// Shows a value rounded to `places` decimals, trailing zeros dropped (the
/// percentage and conversion results). A non-zero value that would show as
/// zero keeps four significant digits instead. Beyond 30 integer digits
/// the value is written in scientific notation.
pub(crate) fn show_places(value: &Rat, places: u64, lang: Lang, minus: &str) -> Shown {
    if value.is_zero() {
        return Shown {
            text: "0".to_owned(),
            approx: false,
        };
    }
    let fixed = value.round_places(places);
    if fixed.is_zero() {
        let r = value.round_sig(4);
        return Shown {
            text: plain(r.neg, &r.digits, r.exp, lang, minus),
            approx: !r.exact,
        };
    }
    if fixed.int.len() > 30 {
        return show_significant(value, 10, false, lang, minus);
    }
    let mut text = String::new();
    if fixed.neg {
        text.push_str(minus);
    }
    text.push_str(&group(&fixed.int, lang));
    if !fixed.frac.is_empty() {
        text.push(decimal_mark(lang));
        text.push_str(&fixed.frac);
    }
    Shown {
        text,
        approx: !fixed.exact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Rat {
        let neg = text.starts_with('-');
        let body = text.trim_start_matches('-');
        match body.split_once('.') {
            Some((i, f)) => Rat::from_decimal(neg, &format!("{i}{f}"), -i64::try_from(f.len()).unwrap()).unwrap(),
            None => Rat::from_decimal(neg, body, 0).unwrap(),
        }
    }

    fn frac(n: i64, d: i64) -> Rat {
        Rat::from_int(n).div(&Rat::from_int(d)).unwrap()
    }

    fn en(r: &Rat) -> String {
        show_significant(r, 10, false, Lang::En, "-").text
    }

    #[test]
    fn decimals_add_exactly() {
        assert_eq!(dec("0.1").add(&dec("0.2")).unwrap(), dec("0.3"));
        assert_eq!(frac(1, 3).mul(&Rat::from_int(3)).unwrap(), Rat::from_int(1));
        assert_eq!(dec("-1.5").add(&dec("1.5")).unwrap(), Rat::zero());
        assert_eq!(dec("0.3").sub(&dec("0.1")).unwrap(), dec("0.2"));
        assert_eq!(dec("2.5").mul(&dec("-4")).unwrap(), Rat::from_int(-10));
        assert_eq!(en(&dec("12.50")), "12.5");
    }

    #[test]
    fn division_by_zero_says_whether_it_was_zero_over_zero() {
        assert_eq!(
            Rat::from_int(5).div(&Rat::zero()),
            Err(MathError::DivZero {
                zero_over_zero: false
            })
        );
        assert_eq!(
            Rat::zero().div(&Rat::zero()),
            Err(MathError::DivZero {
                zero_over_zero: true
            })
        );
        assert!(Rat::zero().powi(&Rat::from_int(-1)).is_err());
    }

    #[test]
    fn powers_are_exact_and_bounded() {
        let two = Rat::from_int(2);
        assert_eq!(
            en(&two.powi(&Rat::from_int(64)).unwrap()),
            "18,446,744,073,709,551,616"
        );
        assert_eq!(two.powi(&Rat::from_int(-2)).unwrap(), frac(1, 4));
        assert_eq!(Rat::from_int(-2).powi(&Rat::from_int(3)).unwrap(), Rat::from_int(-8));
        assert_eq!(Rat::from_int(-2).powi(&Rat::from_int(2)).unwrap(), Rat::from_int(4));
        assert_eq!(Rat::zero().powi(&Rat::zero()).unwrap(), Rat::from_int(1));
        assert_eq!(
            Rat::from_int(9).powi(&Rat::from_int(387_420_489)),
            Err(MathError::TooLarge)
        );
        assert_eq!(
            two.powi(&dec("0.5")),
            Err(MathError::Unsupported),
            "fractional exponents are the caller's business"
        );
        assert_eq!(
            Rat::from_int(1).powi(&Rat::from_int(1_000_000_007)).unwrap(),
            Rat::from_int(1)
        );
        assert_eq!(
            Rat::from_int(-1).powi(&Rat::from_int(1_000_000_007)).unwrap(),
            Rat::from_int(-1)
        );
        let huge = Rat::from_decimal(false, "1", 1000).unwrap();
        assert!(huge.mul(&Rat::from_int(10)).unwrap().within_limit().is_err());
    }

    #[test]
    fn ten_significant_digits_round_half_up_and_mark_inexact() {
        let third = show_significant(&frac(10, 3), 10, false, Lang::En, "-");
        assert_eq!((third.text.as_str(), third.approx), ("3.333333333", true));
        let two_thirds = show_significant(&frac(2, 3), 10, false, Lang::En, "-");
        assert_eq!(two_thirds.text, "0.6666666667");
        let exact = show_significant(&frac(5, 2), 10, false, Lang::Es, "-");
        assert_eq!((exact.text.as_str(), exact.approx), ("2,5", false));
        let long = show_significant(&frac(1, 7), 10, false, Lang::En, "-");
        assert_eq!(long.text, "0.1428571429");
        // A carry that adds a digit: 9.9999999999 rounds to 10.
        let carry = show_significant(&dec("9.99999999999"), 10, false, Lang::En, "-");
        assert_eq!((carry.text.as_str(), carry.approx), ("10", true));
        let negative = show_significant(&frac(-1, 3), 10, false, Lang::En, "\u{2212}");
        assert_eq!(negative.text, "\u{2212}0.3333333333");
    }

    #[test]
    fn big_values_go_scientific_above_thirty_digits() {
        let p200 = Rat::from_int(2).powi(&Rat::from_int(200)).unwrap();
        let shown = show_significant(&p200, 10, false, Lang::En, "-");
        assert_eq!(
            (shown.text.as_str(), shown.approx),
            ("1.606938044 × 10^60", true)
        );
        let exact = Rat::from_int(10).powi(&Rat::from_int(400)).unwrap();
        let shown = show_significant(&exact, 10, false, Lang::En, "-");
        assert_eq!((shown.text.as_str(), shown.approx), ("1 × 10^400", false));
        let thirty = Rat::from_int(10).powi(&Rat::from_int(29)).unwrap();
        assert_eq!(en(&thirty).len(), 30 + 9, "30 digits and 9 commas");
        let tiny = Rat::from_int(1)
            .div(&Rat::from_int(10).powi(&Rat::from_int(40)).unwrap())
            .unwrap();
        assert_eq!(en(&tiny), "1 × 10^-40");
        let spanish = show_significant(&p200, 10, false, Lang::Es, "-");
        assert_eq!(spanish.text, "1,606938044 × 10^60");
    }

    #[test]
    fn roots_are_exact_when_they_can_be() {
        let (r, exact) = Rat::from_int(144).root(2).unwrap();
        assert!(exact);
        assert_eq!(r, Rat::from_int(12));
        let (r, exact) = dec("0.25").root(2).unwrap();
        assert!(exact);
        assert_eq!(r, dec("0.5"));
        let (r, exact) = Rat::from_int(2).root(2).unwrap();
        assert!(!exact);
        assert_eq!(show_significant(&r, 10, true, Lang::En, "-").text, "1.414213562");
        let (r, exact) = Rat::from_int(27).root(3).unwrap();
        assert!(exact);
        assert_eq!(r, Rat::from_int(3));
        let (r, _) = Rat::from_int(2).root(3).unwrap();
        assert_eq!(show_significant(&r, 10, true, Lang::En, "-").text, "1.25992105");
    }

    #[test]
    fn places_round_half_up_and_keep_tiny_values_visible() {
        let es = |r: &Rat| show_places(r, 2, Lang::Es, "-");
        assert_eq!(es(&dec("14.9985")).text, "15");
        assert!(es(&dec("14.9985")).approx);
        assert_eq!(es(&dec("730.485")).text, "730,49");
        assert_eq!(es(&dec("0.000001")).text, "0,000001");
        assert!(!es(&dec("0.000001")).approx);
        assert_eq!(es(&frac(1, 3).mul(&dec("0.001")).unwrap()).text, "0,0003333");
        assert_eq!(es(&dec("-0.001")).text, "-0,001");
        assert_eq!(es(&dec("12345.5")).text, "12.345,5");
        assert_eq!(es(&dec("-12")).text, "-12");
        assert_eq!(es(&Rat::zero()).text, "0");
    }

    #[test]
    fn comparison_orders_signs_and_magnitudes() {
        assert_eq!(frac(1, 3).cmp_value(&dec("0.33")), Ordering::Greater);
        assert_eq!(frac(-1, 3).cmp_value(&dec("-0.33")), Ordering::Less);
        assert_eq!(dec("-5").cmp_value(&dec("2")), Ordering::Less);
        assert_eq!(dec("2").cmp_value(&dec("2.0")), Ordering::Equal);
    }

    #[test]
    fn number_conversion_keeps_exact_values() {
        let n = Number::new(15, 1).unwrap();
        assert_eq!(Rat::from_number(n), dec("1.5"));
        assert_eq!(Rat::from_number(Number::from_int(-7)), Rat::from_int(-7));
    }
}
