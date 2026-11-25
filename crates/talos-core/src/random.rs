//! The injected random source (docs/spec/conversation.md 5.3,
//! docs/spec/basics.md "Randomness").
//!
//! A turn's draws are a pure function of the configured seed, the
//! conversation, the turn number and the normalised text: replaying a
//! transcript rolls the same dice, and no turn consumes a stream that
//! another turn shares. Nothing is seeded from system entropy.
//!
//! Handlers draw only through [`Random`], reached from
//! [`Context::random`](crate::Context::random). Its methods speak the
//! natural terms of the spec's `Given: draw = …` lines (a face, `heads`, a
//! 1-based option, a fortune entry), so a golden file can fix a turn's draws
//! with a [`Draw`] script instead of a seed. A turn that draws nothing never
//! even seeds the generator.

use std::collections::VecDeque;
use std::fmt;

use crate::normalise;
use crate::value::Number;

/// A source of uniformly distributed 64-bit words.
pub trait RandomSource {
    /// The next word.
    fn next_u64(&mut self) -> u64;
}

/// `SplitMix64` (Steele, Lea and Flood, 2014): tiny, fast, statistically
/// adequate for dice and picks, and identical on every platform.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator started from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl RandomSource for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// A uniform integer in `0..n` (Lemire's multiply-and-reject). `n > 0`.
fn below(source: &mut impl RandomSource, n: u64) -> u64 {
    let threshold = n.wrapping_neg() % n;
    loop {
        let product = u128::from(source.next_u64()) * u128::from(n);
        let low = u64::try_from(product & u128::from(u64::MAX)).unwrap_or(0);
        if low >= threshold {
            return u64::try_from(product >> 64).unwrap_or(0);
        }
    }
}

/// A uniform integer in `low..=high`.
fn between(source: &mut impl RandomSource, low: i64, high: i64) -> i64 {
    let span = i128::from(high) - i128::from(low) + 1;
    match u64::try_from(span) {
        Ok(span) => {
            let offset = i128::from(below(source, span));
            i64::try_from(i128::from(low) + offset).unwrap_or(low)
        }
        // The whole i64 range.
        Err(_) => i64::from_ne_bytes(source.next_u64().to_ne_bytes()),
    }
}

/// The two faces of a coin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Coin {
    /// Heads ("cara").
    Heads,
    /// Tails ("cruz").
    Tails,
}

/// One scripted outcome, in the natural terms of the spec's
/// `Given: draw = …` lines (docs/spec/basics.md, "Randomness").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Draw {
    /// A value: a die's face or a random number (`4`, `-2`, `1.73`).
    Value(Number),
    /// A coin's face (`heads`, `tails`).
    Coin(Coin),
    /// A 1-based option of a pick (`option 2`).
    Option(usize),
    /// A fortune, by a fragment of its text (`entry "early bird"`), matched
    /// ignoring case and accents.
    Entry(String),
}

impl fmt::Display for Draw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Draw::Value(n) => n.fmt(f),
            Draw::Coin(Coin::Heads) => f.write_str("heads"),
            Draw::Coin(Coin::Tails) => f.write_str("tails"),
            Draw::Option(n) => write!(f, "option {n}"),
            Draw::Entry(key) => write!(f, "entry {key:?}"),
        }
    }
}

impl Draw {
    /// Parses a comma-separated list of draws: `3, 5`, `heads`,
    /// `option 2`, `entry "a caballo regalado"`.
    ///
    /// # Errors
    ///
    /// A message naming the item that is not a draw.
    pub fn parse_list(text: &str) -> Result<Vec<Draw>, String> {
        let mut draws = Vec::new();
        let mut rest = text.trim();
        while !rest.is_empty() {
            let (item, after) = if let Some(quoted) = rest.strip_prefix("entry \"") {
                let end = quoted
                    .find('"')
                    .ok_or_else(|| format!("unclosed quote in `{rest}`"))?;
                (Draw::Entry(quoted[..end].to_owned()), &quoted[end + 1..])
            } else {
                let end = rest.find(',').unwrap_or(rest.len());
                (Draw::parse_one(rest[..end].trim())?, &rest[end..])
            };
            draws.push(item);
            rest = after.trim_start();
            if let Some(after_comma) = rest.strip_prefix(',') {
                rest = after_comma.trim_start();
            } else if !rest.is_empty() {
                return Err(format!("expected `,` before `{rest}`"));
            }
        }
        Ok(draws)
    }

    fn parse_one(item: &str) -> Result<Draw, String> {
        match item {
            "heads" => return Ok(Draw::Coin(Coin::Heads)),
            "tails" => return Ok(Draw::Coin(Coin::Tails)),
            _ => {}
        }
        if let Some(n) = item.strip_prefix("option ") {
            return n
                .trim()
                .parse()
                .ok()
                .filter(|&n| n > 0)
                .map(Draw::Option)
                .ok_or_else(|| format!("`{item}`: options are numbered from 1"));
        }
        parse_canonical_number(item)
            .map(Draw::Value)
            .ok_or_else(|| {
                format!("`{item}` is not a draw (a number, heads, tails, option N or entry \"…\")")
            })
    }
}

/// A number written canonically: optional `-`, digits, optional `.` and
/// digits.
fn parse_canonical_number(text: &str) -> Option<Number> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let valid = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() || !valid(int) || !valid(frac) || int.len() + frac.len() > 30 {
        return None;
    }
    let units: i128 = format!("{int}{frac}").parse().ok()?;
    let n = Number::new(units, u8::try_from(frac.len()).ok()?)?;
    Some(if negative { n.neg() } else { n })
}

/// Where draws come from.
#[derive(Debug)]
enum Source {
    /// Not drawn from yet: the seed, kept until the first draw.
    Pending(u64),
    /// Drawing.
    Live(SplitMix64),
    /// A golden file's script.
    Script {
        draws: VecDeque<Draw>,
        errors: Vec<String>,
    },
}

/// What happened to a script during a turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptReport {
    /// Scripted draws the turn did not consume.
    pub unused: Vec<Draw>,
    /// Draws that did not fit the script: the wrong kind, out of range, or
    /// more draws than scripted.
    pub errors: Vec<String>,
}

/// A turn's random source, as handlers use it. Every method is one draw.
#[derive(Debug)]
pub struct Random {
    source: Source,
    draws: u32,
}

impl Random {
    /// A source seeded for one turn. The generator starts only on the first
    /// draw.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self {
            source: Source::Pending(seed),
            draws: 0,
        }
    }

    /// A source that replays `draws`, for golden files and handler tests.
    #[must_use]
    pub fn scripted(draws: Vec<Draw>) -> Self {
        Self {
            source: Source::Script {
                draws: draws.into(),
                errors: Vec::new(),
            },
            draws: 0,
        }
    }

    /// How many draws this turn made.
    #[must_use]
    pub const fn draws(&self) -> u32 {
        self.draws
    }

    /// What became of the script, if this source replays one.
    #[must_use]
    pub fn into_report(self) -> Option<ScriptReport> {
        match self.source {
            Source::Script { draws, errors } => Some(ScriptReport {
                unused: draws.into(),
                errors,
            }),
            Source::Pending(_) | Source::Live(_) => None,
        }
    }

    /// The generator, started if this is the first draw; `None` when a
    /// script is replayed.
    fn generator(&mut self) -> Option<&mut SplitMix64> {
        self.draws += 1;
        if let Source::Pending(seed) = self.source {
            self.source = Source::Live(SplitMix64::new(seed));
        }
        match &mut self.source {
            Source::Live(generator) => Some(generator),
            Source::Pending(_) | Source::Script { .. } => None,
        }
    }

    /// The next scripted draw, or an error recorded. Only for scripts.
    fn scripted_draw(&mut self, wanted: &str) -> Option<Draw> {
        let Source::Script { draws, errors } = &mut self.source else {
            return None;
        };
        let next = draws.pop_front();
        if next.is_none() {
            errors.push(format!(
                "a {wanted} was drawn but the script has no more draws"
            ));
        }
        next
    }

    fn script_error(&mut self, message: String) {
        if let Source::Script { errors, .. } = &mut self.source {
            errors.push(message);
        }
    }

    /// A whole number in `low..=high` (in either order): a die's face, a
    /// random number. Scripted as a plain number (`4`).
    pub fn int(&mut self, low: i64, high: i64) -> i64 {
        let (low, high) = (low.min(high), low.max(high));
        if let Some(generator) = self.generator() {
            return between(generator, low, high);
        }
        match self.scripted_draw("number") {
            Some(Draw::Value(n)) => match n.to_i64() {
                Some(v) if (low..=high).contains(&v) => v,
                _ => {
                    self.script_error(format!(
                        "scripted {n} is not a whole number in {low}..={high}"
                    ));
                    low
                }
            },
            Some(other) => {
                self.script_error(format!("scripted `{other}` where a number was drawn"));
                low
            }
            None => low,
        }
    }

    /// A number with `places` decimals in `low..=high`, both given in units
    /// of `10^-places`; the result is in the same units. Scripted as a
    /// decimal (`1.73` with `places = 2` gives 173).
    pub fn scaled(&mut self, low: i64, high: i64, places: u8) -> i64 {
        let (low, high) = (low.min(high), low.max(high));
        if let Some(generator) = self.generator() {
            return between(generator, low, high);
        }
        match self.scripted_draw("number") {
            Some(Draw::Value(n)) if n.scale() <= places => {
                let factor = 10_i128.pow(u32::from(places - n.scale()));
                match i64::try_from(n.units() * factor) {
                    Ok(v) if (low..=high).contains(&v) => v,
                    _ => {
                        self.script_error(format!("scripted {n} is out of range"));
                        low
                    }
                }
            }
            Some(other) => {
                self.script_error(format!(
                    "scripted `{other}` where a number with {places} decimals was drawn"
                ));
                low
            }
            None => low,
        }
    }

    /// A coin flip. Scripted as `heads` or `tails`.
    pub fn coin(&mut self) -> Coin {
        if let Some(generator) = self.generator() {
            return if generator.next_u64() >> 63 == 0 {
                Coin::Heads
            } else {
                Coin::Tails
            };
        }
        match self.scripted_draw("coin") {
            Some(Draw::Coin(coin)) => coin,
            Some(other) => {
                self.script_error(format!("scripted `{other}` where a coin was flipped"));
                Coin::Heads
            }
            None => Coin::Heads,
        }
    }

    /// One of `count` options, as a 0-based index; `0` when `count` is 0.
    /// Scripted as `option N`, 1-based.
    pub fn pick(&mut self, count: usize) -> usize {
        let Ok(n) = u64::try_from(count) else {
            return 0;
        };
        if let Some(generator) = self.generator() {
            return if n == 0 {
                0
            } else {
                usize::try_from(below(generator, n)).unwrap_or(0)
            };
        }
        match self.scripted_draw("pick") {
            Some(Draw::Option(k)) if (1..=count).contains(&k) => k - 1,
            Some(other) => {
                self.script_error(format!("scripted `{other}` for a pick among {count}"));
                0
            }
            None => 0,
        }
    }

    /// One entry of a bundled list, as an index into `entries`. Scripted as
    /// `entry "fragment"`: the first entry containing the fragment,
    /// ignoring case and accents.
    pub fn entry(&mut self, entries: &[&str]) -> usize {
        if let Some(generator) = self.generator() {
            return match u64::try_from(entries.len()) {
                Ok(n) if n > 0 => usize::try_from(below(generator, n)).unwrap_or(0),
                _ => 0,
            };
        }
        match self.scripted_draw("entry") {
            Some(Draw::Entry(key)) => {
                let key = normalise::fold(&key.to_lowercase());
                if let Some(index) = entries
                    .iter()
                    .position(|e| normalise::fold(&e.to_lowercase()).contains(&key))
                {
                    index
                } else {
                    self.script_error(format!("no entry contains {key:?}"));
                    0
                }
            }
            Some(other) => {
                self.script_error(format!("scripted `{other}` where an entry was drawn"));
                0
            }
            None => 0,
        }
    }
}

/// 64-bit FNV-1a, fed in pieces: tiny, and stable across platforms and
/// releases (unlike `std`'s hasher).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fnv(u64);

impl Fnv {
    pub(crate) const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub(crate) fn bytes(mut self, bytes: &[u8]) -> Self {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self
    }

    pub(crate) const fn finish(self) -> u64 {
        self.0
    }
}

/// The seed of one turn: the configured seed, the conversation, the turn
/// number and the normalised text, and nothing else.
#[must_use]
pub(crate) fn turn_seed(config_seed: u64, conversation: &str, turn: u32, text: &str) -> u64 {
    // Separators keep ("ab", "c") and ("a", "bc") apart.
    Fnv::new()
        .bytes(&config_seed.to_le_bytes())
        .bytes(conversation.as_bytes())
        .bytes(&[0xff])
        .bytes(&turn.to_le_bytes())
        .bytes(text.as_bytes())
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_is_the_published_sequence() {
        // Reference values for seed 1234567 from the algorithm's paper/code.
        let mut g = SplitMix64::new(1_234_567);
        assert_eq!(g.next_u64(), 6_457_827_717_110_365_317);
        assert_eq!(g.next_u64(), 3_203_168_211_198_807_973);
    }

    #[test]
    fn draws_stay_in_range_and_are_reproducible() {
        let mut a = Random::seeded(7);
        let mut b = Random::seeded(7);
        for _ in 0..1_000 {
            let face = a.int(1, 6);
            assert!((1..=6).contains(&face));
            assert_eq!(face, b.int(6, 1));
        }
        assert_eq!(a.draws(), 1_000);
        let mut seen = [false; 3];
        for _ in 0..200 {
            seen[a.pick(3)] = true;
        }
        assert_eq!(seen, [true; 3]);
        assert!((-50..=-40).contains(&a.int(-50, -40)));
        assert!((100..=200).contains(&a.scaled(100, 200, 2)));
        let _ = a.int(i64::MIN, i64::MAX);
        assert_eq!(a.pick(0), 0);
    }

    #[test]
    fn an_unused_source_never_starts() {
        let r = Random::seeded(1);
        assert_eq!(r.draws(), 0);
        assert!(matches!(r.source, Source::Pending(1)));
    }

    #[test]
    fn scripts_replay_natural_terms() {
        let script = Draw::parse_list("3, 5, heads, option 2, entry \"Early Bird\", 1.73").unwrap();
        let mut r = Random::scripted(script);
        assert_eq!(r.int(1, 6), 3);
        assert_eq!(r.int(1, 6), 5);
        assert_eq!(r.coin(), Coin::Heads);
        assert_eq!(r.pick(3), 1);
        assert_eq!(r.entry(&["A stitch in time", "The early bird…"]), 1);
        assert_eq!(r.scaled(100, 200, 2), 173);
        assert_eq!(r.into_report(), Some(ScriptReport::default()));
    }

    #[test]
    fn script_mismatches_are_reported_not_panicked() {
        let mut r = Random::scripted(Draw::parse_list("7, tails").unwrap());
        assert_eq!(r.int(1, 6), 1);
        assert_eq!(r.pick(2), 0);
        assert_eq!(r.coin(), Coin::Heads);
        let report = r.into_report().unwrap();
        assert_eq!(report.errors.len(), 3, "{report:?}");
        let mut r = Random::scripted(Draw::parse_list("1, 2").unwrap());
        r.int(1, 6);
        assert_eq!(
            r.into_report().unwrap().unused,
            [Draw::Value(Number::from_int(2))]
        );
    }

    #[test]
    fn script_syntax_errors() {
        assert!(Draw::parse_list("option 0").is_err());
        assert!(Draw::parse_list("12 heads and 8 tails").is_err());
        assert!(Draw::parse_list("entry \"open").is_err());
        assert_eq!(Draw::parse_list("").unwrap(), []);
        assert_eq!(
            Draw::parse_list("-2").unwrap(),
            [Draw::Value(Number::from_int(-2))]
        );
    }

    #[test]
    fn the_turn_seed_depends_on_every_input() {
        let base = turn_seed(0, "c", 1, "tira un dado");
        assert_eq!(base, turn_seed(0, "c", 1, "tira un dado"));
        assert_ne!(base, turn_seed(1, "c", 1, "tira un dado"));
        assert_ne!(base, turn_seed(0, "d", 1, "tira un dado"));
        assert_ne!(base, turn_seed(0, "c", 2, "tira un dado"));
        assert_ne!(base, turn_seed(0, "c", 1, "tira dos dados"));
    }
}
