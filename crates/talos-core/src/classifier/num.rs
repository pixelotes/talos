//! `exp` and `ln` written with `+`, `-`, `*`, `/` only, so that training and
//! inference give bit-identical results on every platform. The standard
//! library's versions call the platform's maths library, which IEEE 754
//! does not require to round the same way everywhere; the four basic
//! operations and the square root it does.

/// ln 2, split so that `k * LN2_HI` is exact for the `k` that occur.
const LN2_HI: f64 = 6.931_471_803_691_238e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;
const LN2: f64 = std::f64::consts::LN_2;

/// e^x. Accurate to a few units in the last place over the range the
/// classifier uses; saturates to 0 below −700 and to e^700 above 700.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "k is within ±1010 after clamping"
)]
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let x = x.clamp(-700.0, 700.0);
    // x = k ln 2 + r with |r| <= ln 2 / 2.
    let k = (x / LN2).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    // Taylor series of e^r; 15 terms reach f64 precision for |r| < 0.35.
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..=15 {
        term = term * r / f64::from(n);
        sum += term;
    }
    // 2^k, built from its bits: k + 1023 is in 1..=2046 after clamping.
    let two_k = f64::from_bits(((k as i64 + 1023) as u64) << 52);
    sum * two_k
}

/// The natural logarithm of a positive, finite `x`; `-inf` for 0 and NaN
/// for a negative number.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "the exponent field is 11 bits"
)]
pub fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    // Subnormals are scaled into the normal range first.
    let (x, extra) = if x < f64::MIN_POSITIVE {
        (x * 2f64.powi(54), -54)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023 + extra;
    // m in [1, 2).
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > std::f64::consts::SQRT_2 {
        m /= 2.0;
        e += 1;
    }
    // ln m = 2 atanh(s), s = (m - 1) / (m + 1), |s| < 0.172.
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut power = s;
    let mut sum = 0.0;
    for n in 0..20 {
        sum += power / f64::from(2 * n + 1);
        power *= s2;
    }
    #[allow(clippy::cast_precision_loss, reason = "e is within ±1100")]
    let e = e as f64;
    2.0 * sum + e * LN2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-14 * b.abs().max(1.0)
    }

    #[test]
    fn exp_matches_the_standard_library() {
        for i in -400..=400 {
            let x = f64::from(i) * 0.137;
            assert!(close(exp(x), x.exp()), "{x}: {} {}", exp(x), x.exp());
        }
        assert!(exp(-1000.0) < 1e-300);
        assert!(exp(f64::NAN).is_nan());
        assert!(close(exp(0.0), 1.0));
    }

    #[test]
    fn ln_matches_the_standard_library() {
        for i in 1..=2000 {
            let x = f64::from(i) * 0.731;
            assert!(close(ln(x), x.ln()), "{x}: {} {}", ln(x), x.ln());
        }
        assert!(close(ln(1e-310), 1e-310f64.ln()));
        assert!(close(ln(1e300), 1e300f64.ln()));
        assert!(ln(0.0).is_infinite());
        assert!(ln(-1.0).is_nan());
        assert!(close(ln(1.0), 0.0));
    }
}
