//! Arbitrary-precision unsigned integers, just enough for exact rational
//! arithmetic: little-endian `u32` limbs, schoolbook algorithms, no
//! dependencies. Sizes are bounded by the callers (`rat.rs` caps every
//! value), so the quadratic algorithms are fast enough.

use std::cmp::Ordering;
use std::fmt::Write as _;

/// An unsigned integer. No trailing zero limbs; zero is the empty vector.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Big(Vec<u32>);

const BASE: u64 = 1 << 32;
/// The largest power of ten that fits a limb: 10^9.
const CHUNK: u32 = 1_000_000_000;

fn limb(value: u64) -> u32 {
    u32::try_from(value & 0xFFFF_FFFF).unwrap_or(0)
}

impl Big {
    pub(crate) const fn zero() -> Big {
        Big(Vec::new())
    }

    pub(crate) fn one() -> Big {
        Big::from_u64(1)
    }

    pub(crate) fn from_u64(value: u64) -> Big {
        let mut limbs = vec![limb(value), limb(value >> 32)];
        trim(&mut limbs);
        Big(limbs)
    }

    pub(crate) fn from_u128(value: u128) -> Big {
        let mut limbs = Vec::with_capacity(4);
        let mut rest = value;
        while rest > 0 {
            limbs.push(limb(u64::try_from(rest & 0xFFFF_FFFF).unwrap_or(0)));
            rest >>= 32;
        }
        Big(limbs)
    }

    pub(crate) fn to_u64(&self) -> Option<u64> {
        match self.0.as_slice() {
            [] => Some(0),
            [a] => Some(u64::from(*a)),
            [a, b] => Some(u64::from(*a) | (u64::from(*b) << 32)),
            _ => None,
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn is_one(&self) -> bool {
        self.0 == [1]
    }

    pub(crate) fn is_even(&self) -> bool {
        self.0.first().is_none_or(|l| l % 2 == 0)
    }

    /// Number of significant bits (zero has none).
    pub(crate) fn bits(&self) -> usize {
        match self.0.last() {
            None => 0,
            Some(top) => self.0.len() * 32 - top.leading_zeros() as usize,
        }
    }

    fn bit(&self, index: usize) -> bool {
        self.0
            .get(index / 32)
            .is_some_and(|l| (l >> (index % 32)) & 1 == 1)
    }

    pub(crate) fn add(&self, other: &Big) -> Big {
        let (long, short) = if self.0.len() >= other.0.len() {
            (&self.0, &other.0)
        } else {
            (&other.0, &self.0)
        };
        let mut out = Vec::with_capacity(long.len() + 1);
        let mut carry = 0_u64;
        for (i, &l) in long.iter().enumerate() {
            let sum = u64::from(l) + u64::from(short.get(i).copied().unwrap_or(0)) + carry;
            out.push(limb(sum));
            carry = sum / BASE;
        }
        if carry > 0 {
            out.push(limb(carry));
        }
        Big(out)
    }

    /// `self - other`; zero when `other` is larger (callers compare first).
    pub(crate) fn sub(&self, other: &Big) -> Big {
        let mut out = self.clone();
        out.sub_assign(other);
        out
    }

    fn sub_assign(&mut self, other: &Big) {
        if *self <= *other {
            self.0.clear();
            return;
        }
        let mut borrow = 0_u64;
        for i in 0..self.0.len() {
            let sub = u64::from(other.0.get(i).copied().unwrap_or(0)) + borrow;
            let cur = u64::from(self.0[i]);
            if cur >= sub {
                self.0[i] = limb(cur - sub);
                borrow = 0;
            } else {
                self.0[i] = limb(cur + BASE - sub);
                borrow = 1;
            }
        }
        trim(&mut self.0);
    }

    pub(crate) fn mul(&self, other: &Big) -> Big {
        if self.is_zero() || other.is_zero() {
            return Big::zero();
        }
        let mut out = vec![0_u32; self.0.len() + other.0.len()];
        for (i, &a) in self.0.iter().enumerate() {
            let mut carry = 0_u64;
            for (j, &b) in other.0.iter().enumerate() {
                let cur = u64::from(out[i + j]) + u64::from(a) * u64::from(b) + carry;
                out[i + j] = limb(cur);
                carry = cur / BASE;
            }
            let mut k = i + other.0.len();
            while carry > 0 {
                let cur = u64::from(out[k]) + carry;
                out[k] = limb(cur);
                carry = cur / BASE;
                k += 1;
            }
        }
        trim(&mut out);
        Big(out)
    }

    pub(crate) fn mul_small(&self, factor: u32) -> Big {
        let mut out = Vec::with_capacity(self.0.len() + 1);
        let mut carry = 0_u64;
        for &l in &self.0 {
            let cur = u64::from(l) * u64::from(factor) + carry;
            out.push(limb(cur));
            carry = cur / BASE;
        }
        if carry > 0 {
            out.push(limb(carry));
        }
        trim(&mut out);
        Big(out)
    }

    /// Quotient and remainder by a non-zero limb.
    pub(crate) fn divrem_small(&self, divisor: u32) -> (Big, u32) {
        let mut out = vec![0_u32; self.0.len()];
        let mut rem = 0_u64;
        for i in (0..self.0.len()).rev() {
            let cur = rem * BASE + u64::from(self.0[i]);
            out[i] = limb(cur / u64::from(divisor));
            rem = cur % u64::from(divisor);
        }
        trim(&mut out);
        (Big(out), limb(rem))
    }

    pub(crate) fn shl(&self, bits: usize) -> Big {
        if self.is_zero() {
            return Big::zero();
        }
        let (limbs, rest) = (bits / 32, bits % 32);
        let mut out = vec![0_u32; limbs];
        let mut carry = 0_u32;
        for &l in &self.0 {
            if rest == 0 {
                out.push(l);
            } else {
                out.push((l << rest) | carry);
                carry = l >> (32 - rest);
            }
        }
        if carry > 0 {
            out.push(carry);
        }
        Big(out)
    }

    pub(crate) fn shr(&self, bits: usize) -> Big {
        let (limbs, rest) = (bits / 32, bits % 32);
        if limbs >= self.0.len() {
            return Big::zero();
        }
        let mut out = Vec::with_capacity(self.0.len() - limbs);
        for i in limbs..self.0.len() {
            let low = self.0[i] >> rest;
            let high = if rest == 0 {
                0
            } else {
                self.0.get(i + 1).map_or(0, |h| h << (32 - rest))
            };
            out.push(low | high);
        }
        trim(&mut out);
        Big(out)
    }

    /// Shifts left by one bit and sets the lowest bit.
    fn shl1_with(&mut self, low: bool) {
        let mut carry = u32::from(low);
        for l in &mut self.0 {
            let next = *l >> 31;
            *l = (*l << 1) | carry;
            carry = next;
        }
        if carry > 0 {
            self.0.push(carry);
        }
    }

    /// Quotient and remainder; `None` when `divisor` is zero.
    pub(crate) fn divrem(&self, divisor: &Big) -> Option<(Big, Big)> {
        if divisor.is_zero() {
            return None;
        }
        if self < divisor {
            return Some((Big::zero(), self.clone()));
        }
        if let [d] = divisor.0.as_slice() {
            let (q, r) = self.divrem_small(*d);
            return Some((q, Big::from_u64(u64::from(r))));
        }
        let mut quotient = vec![0_u32; self.0.len()];
        let mut rem = Big::zero();
        for i in (0..self.bits()).rev() {
            rem.shl1_with(self.bit(i));
            if rem >= *divisor {
                rem.sub_assign(divisor);
                quotient[i / 32] |= 1 << (i % 32);
            }
        }
        trim(&mut quotient);
        Some((Big(quotient), rem))
    }

    /// `self` to the power `exp`, by repeated squaring. The caller bounds
    /// the size of the result.
    pub(crate) fn pow(&self, mut exp: u64) -> Big {
        let mut result = Big::one();
        let mut base = self.clone();
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.mul(&base);
            }
            exp >>= 1;
            if exp > 0 {
                base = base.mul(&base);
            }
        }
        result
    }

    pub(crate) fn pow10(exp: u64) -> Big {
        Big::from_u64(10).pow(exp)
    }

    /// Greatest common divisor, by the binary algorithm (no division).
    pub(crate) fn gcd(&self, other: &Big) -> Big {
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        let mut a = self.clone();
        let mut b = other.clone();
        let shift = a.trailing_zeros().min(b.trailing_zeros());
        a = a.shr(a.trailing_zeros());
        loop {
            b = b.shr(b.trailing_zeros());
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            b.sub_assign(&a);
            if b.is_zero() {
                break;
            }
        }
        a.shl(shift)
    }

    fn trailing_zeros(&self) -> usize {
        let mut count = 0;
        for &l in &self.0 {
            if l == 0 {
                count += 32;
            } else {
                return count + l.trailing_zeros() as usize;
            }
        }
        count
    }

    /// Floor of the square root.
    pub(crate) fn isqrt(&self) -> Big {
        if self.is_zero() {
            return Big::zero();
        }
        let mut x = Big::one().shl(self.bits().div_ceil(2));
        loop {
            let Some((q, _)) = self.divrem(&x) else {
                return x;
            };
            let y = x.add(&q).shr(1);
            if y >= x {
                return x;
            }
            x = y;
        }
    }

    /// Floor of the cube root.
    pub(crate) fn icbrt(&self) -> Big {
        if self.is_zero() {
            return Big::zero();
        }
        let mut x = Big::one().shl(self.bits().div_ceil(3));
        loop {
            let Some((q, _)) = self.divrem(&x.mul(&x)) else {
                return x;
            };
            let y = x.mul_small(2).add(&q).divrem_small(3).0;
            if y >= x {
                return x;
            }
            x = y;
        }
    }

    /// Decimal digits, no sign.
    pub(crate) fn to_dec(&self) -> String {
        if self.is_zero() {
            return "0".to_owned();
        }
        let mut chunks = Vec::new();
        let mut rest = self.clone();
        while !rest.is_zero() {
            let (q, r) = rest.divrem_small(CHUNK);
            chunks.push(r);
            rest = q;
        }
        let mut out = String::new();
        for (i, chunk) in chunks.iter().rev().enumerate() {
            if i == 0 {
                out.push_str(&chunk.to_string());
            } else {
                let _ = write!(out, "{chunk:09}");
            }
        }
        out
    }

    /// Parses ASCII decimal digits; `None` for anything else or empty text.
    pub(crate) fn from_dec(digits: &str) -> Option<Big> {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut out = Big::zero();
        let mut start = 0;
        while start < digits.len() {
            let end = (start + 9).min(digits.len());
            let chunk: u32 = digits[start..end].parse().ok()?;
            let scale = 10_u32.checked_pow(u32::try_from(end - start).ok()?)?;
            out = out.mul_small(scale).add(&Big::from_u64(u64::from(chunk)));
            start = end;
        }
        Some(out)
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
}

fn trim(limbs: &mut Vec<u32>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(text: &str) -> Big {
        Big::from_dec(text).unwrap()
    }

    #[test]
    fn decimal_round_trip_and_arithmetic() {
        let a = big("123456789012345678901234567890");
        assert_eq!(a.to_dec(), "123456789012345678901234567890");
        assert_eq!(a.add(&big("1")).to_dec(), "123456789012345678901234567891");
        assert_eq!(a.sub(&big("890")).to_dec(), "123456789012345678901234567000");
        assert_eq!(a.mul(&Big::from_u64(10)).to_dec(), format!("{}0", a.to_dec()));
        assert_eq!(big("99999999999").mul(&big("99999999999")).to_dec(), "9999999999800000000001");
        assert_eq!(Big::zero().to_dec(), "0");
        assert_eq!(big("1000000000").to_dec(), "1000000000");
        assert!(Big::from_dec("12a").is_none());
        assert!(Big::from_dec("").is_none());
    }

    #[test]
    fn division_matches_multiplication() {
        let a = big("98765432109876543210987654321098765432109876543210");
        let b = big("12345678901234567890123");
        let (q, r) = a.divrem(&b).unwrap();
        assert!(r < b);
        assert_eq!(q.mul(&b).add(&r), a);
        assert!(a.divrem(&Big::zero()).is_none());
        let (q, r) = big("100").divrem(&big("7")).unwrap();
        assert_eq!((q.to_dec(), r.to_dec()), ("14".to_owned(), "2".to_owned()));
    }

    #[test]
    fn powers_roots_and_gcd() {
        assert_eq!(Big::from_u64(2).pow(64).to_dec(), "18446744073709551616");
        assert_eq!(Big::pow10(30).to_dec(), format!("1{}", "0".repeat(30)));
        assert_eq!(big("144").isqrt().to_dec(), "12");
        assert_eq!(big("145").isqrt().to_dec(), "12");
        assert_eq!(big("143").isqrt().to_dec(), "11");
        assert_eq!(big("27").icbrt().to_dec(), "3");
        assert_eq!(big("26").icbrt().to_dec(), "2");
        let p = Big::pow10(80);
        assert_eq!(p.mul(&p).isqrt(), p);
        assert_eq!(big("48").gcd(&big("180")).to_dec(), "12");
        assert_eq!(big("17").gcd(&big("5")).to_dec(), "1");
        assert_eq!(Big::zero().gcd(&big("9")).to_dec(), "9");
    }

    #[test]
    fn shifts_and_bits() {
        assert_eq!(big("1").shl(100).bits(), 101);
        assert_eq!(big("1").shl(100).shr(100), big("1"));
        assert_eq!(Big::from_u64(u64::MAX).to_u64(), Some(u64::MAX));
        assert!(big("18446744073709551616").to_u64().is_none());
        assert!(Big::from_u64(10).is_even());
        assert_eq!(Big::from_u128(1 << 100).bits(), 101);
    }
}
