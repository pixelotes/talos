//! `random`: dice, coins, picking among options and random numbers
//! (docs/spec/basics.md). Every draw comes from `cx.random()`, one per die,
//! coin, pick and number, in order.
//!
//! Inside this module `Random` is the skill's handler; the source of draws
//! is only ever reached through the context, so core's type is not named
//! here (the tests import it under another name).

use talos_core::extract;
use talos_core::normalise;
use talos_core::{Call, Context, Lang, Message, Number, Skill, SkillResult, Value};

/// The `random` handler.
#[derive(Debug, Default)]
pub(crate) struct Random;

/// Most dice (and coins, and numbers) in one request.
const MAX_COUNT: i64 = 100;
/// Fewest and most sides of a die.
const MIN_SIDES: i64 = 2;
const MAX_SIDES: i64 = 1000;
/// Largest modifier of a roll; beyond it the request is not a plain roll.
const MAX_MODIFIER: i64 = 1_000_000;
/// Most dice written out one by one.
const LISTED: i64 = 10;
/// Largest magnitude of a bound of `random.number`: 10^15.
const MAX_BOUND: i128 = 1_000_000_000_000_000;
/// Decimals of a number drawn from a range with decimals.
const PLACES: u8 = 2;
/// Most options a pick considers.
const MAX_OPTIONS: usize = 1000;
/// Redraws of one duplicate before a distinct draw takes the next free value.
const MAX_REDRAWS: u32 = 10_000;

/// What a slot holds, as the handler needs it.
enum Slot<T> {
    Absent,
    Value(T),
    Bad,
}

/// A whole-number slot.
fn whole(call: &Call<'_>, name: &str) -> Slot<i64> {
    match call.slots.get(name) {
        None => Slot::Absent,
        Some(Value::Number(n)) => n.to_i64().map_or(Slot::Bad, Slot::Value),
        Some(_) => Slot::Bad,
    }
}

/// A slot's whole number, or `default` when absent; `None` when unreadable.
fn whole_or(call: &Call<'_>, name: &str, default: i64) -> Option<i64> {
    match whole(call, name) {
        Slot::Absent => Some(default),
        Slot::Value(n) => Some(n),
        Slot::Bad => None,
    }
}

fn int_text(n: i64, lang: Lang) -> String {
    Number::from_int(n).format(lang)
}

/// A word the skill's manifest holds (`word_*` replies), so that no wording
/// is written in code. Falls back to the id outside an engine.
fn word(cx: &Context<'_>, lang: Lang, id: &str) -> String {
    cx.skills()
        .iter()
        .find(|m| m.id == "random")
        .and_then(|m| m.replies.get(id))
        .and_then(|r| r.get(lang).first())
        .cloned()
        .unwrap_or_else(|| id.to_owned())
}

/// "a", "a and b", "a, b and c".
fn join(items: &[String], and: &str) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} {and} {last}", init.join(", ")),
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

impl Random {
    fn dice(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let unable = |id| SkillResult::Unable(Message::new(id));
        match call.slots.get("kind").and_then(Value::as_text) {
            Some("mixed") => return unable("dice_mixed"),
            Some("unsupported") => return unable("dice_unsupported"),
            _ => {}
        }
        let plural = call.slots.contains_key("plural");
        let (Some(count), Some(sides)) = (
            whole_or(call, "count", if plural { 2 } else { 1 }),
            whole_or(call, "sides", 6),
        ) else {
            return unable("dice_usage");
        };
        cx.report("count", Value::Number(Number::from_int(count)));
        cx.report("sides", Value::Number(Number::from_int(sides)));

        let given = |name| !matches!(whole(call, name), Slot::Absent);
        let (Some(plus), Some(minus)) = (whole_or(call, "modifier", 0), whole_or(call, "minus", 0))
        else {
            return unable("dice_usage");
        };
        if plus.abs() > MAX_MODIFIER || minus.abs() > MAX_MODIFIER {
            return unable("dice_usage");
        }
        let modifier = plus - minus;
        if given("modifier") || given("minus") {
            cx.report("modifier", Value::Number(Number::from_int(modifier)));
        }

        if count < 1 {
            return unable("dice_zero");
        }
        if count > MAX_COUNT {
            return unable("dice_count_max");
        }
        if sides < MIN_SIDES {
            return unable("dice_sides_min");
        }
        if sides > MAX_SIDES {
            return unable("dice_sides_max");
        }

        let faces: Vec<i64> = (0..count).map(|_| cx.random().int(1, sides)).collect();
        let total = faces.iter().sum::<i64>() + modifier;
        let lang = call.lang;
        let text = |n| int_text(n, lang);
        let listed: Vec<String> = faces.iter().map(|&f| text(f)).collect();
        let faces_text = join(&listed, &word(cx, lang, "word_and"));
        let suffix = match modifier.signum() {
            0 => "",
            1 => "_plus",
            _ => "_minus",
        };
        let message = if count == 1 {
            match suffix {
                "" => Message::new("dice_one"),
                "_plus" => Message::new("dice_one_plus"),
                _ => Message::new("dice_one_minus"),
            }
            .var("face", text(faces[0]))
        } else if count > LISTED {
            Message::new("dice_total")
        } else if plural && modifier == 0 && count == 2 && !given("count") {
            Message::new("dice_the").var("faces", faces_text)
        } else {
            match suffix {
                "" => Message::new("dice_list"),
                "_plus" => Message::new("dice_list_plus"),
                _ => Message::new("dice_list_minus"),
            }
            .var("faces", faces_text)
        };
        SkillResult::Answer(
            message
                .var("modifier", text(modifier.abs()))
                .var("total", text(total)),
        )
    }

    fn coin(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let unable = |id| SkillResult::Unable(Message::new(id));
        let Some(count) = whole_or(call, "count", 1) else {
            return unable("coin_usage");
        };
        cx.report("count", Value::Number(Number::from_int(count)));
        if count < 1 {
            return unable("coin_usage");
        }
        if count > MAX_COUNT {
            return unable("coin_max");
        }
        let flips: Vec<bool> = (0..count)
            .map(|_| cx.random().coin() == talos_core::Coin::Heads)
            .collect();
        let lang = call.lang;
        if count == 1 {
            return SkillResult::Answer(Message::new(if flips[0] {
                "coin_heads"
            } else {
                "coin_tails"
            }));
        }
        if count > LISTED {
            let heads = flips.iter().filter(|&&h| h).count();
            let tails = flips.len() - heads;
            return SkillResult::Answer(
                Message::new("coin_counts")
                    .var("heads", heads.to_string())
                    .var("tails", tails.to_string()),
            );
        }
        let (heads, tails) = (word(cx, lang, "word_heads"), word(cx, lang, "word_tails"));
        let names: Vec<String> = flips
            .iter()
            .map(|&h| if h { heads.clone() } else { tails.clone() })
            .collect();
        SkillResult::Answer(Message::new("coin_list").var("faces", capitalise(&names.join(", "))))
    }

    fn pick(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let unable = |id| SkillResult::Unable(Message::new(id));
        let Some(Value::List { items, uncertain }) = call.slots.get("options") else {
            return unable("pick_usage");
        };
        if items.is_empty() {
            return unable("pick_usage");
        }
        if items.len() > MAX_OPTIONS {
            return unable("pick_many");
        }
        if let [only] = items.as_slice() {
            // "dos camisetas", "two shirts": a quantity, not an option.
            let mut words = only.split_whitespace();
            let quantity = words.next().is_some_and(|first| {
                words.next().is_some()
                    && extract::number(&normalise::fold(&first.to_lowercase()), call.lang).is_ok()
            });
            if quantity {
                return unable("pick_usage");
            }
            return SkillResult::Answer(Message::new("pick_one").var("option", only.clone()));
        }
        let chosen = &items[cx.random().pick(items.len()).min(items.len() - 1)];
        let message = if *uncertain {
            let and = word(cx, call.lang, "word_and");
            Message::new("pick_uncertain").var("options", join(items, &and))
        } else {
            Message::new("pick_chosen")
        };
        SkillResult::Answer(message.var("option", chosen.clone()))
    }

    fn number(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let unable = |id| SkillResult::Unable(Message::new(id));
        if call.slots.contains_key("incomplete") {
            return unable("number_incomplete");
        }
        let (low, high) = match (bound(call, "low", 1), bound(call, "high", 100)) {
            (Ok(low), Ok(high)) => (low, high),
            (Err(failure), _) | (_, Err(failure)) => return unable(failure.reply()),
        };
        let places = if low.is_integer() && high.is_integer() {
            0
        } else {
            PLACES
        };
        let (Some(a), Some(b)) = (units(low, places), units(high, places)) else {
            return unable("number_usage");
        };
        let (lo, hi) = (a.min(b), a.max(b));
        let reversed = a > b;
        let shown = |n: i64| number_text(n, places, call.lang);
        cx.report("low", Value::Number(Number::new(i128::from(lo), places).unwrap_or(low)));
        cx.report("high", Value::Number(Number::new(i128::from(hi), places).unwrap_or(high)));
        let distinct = call.slots.contains_key("distinct");
        if distinct {
            cx.report("distinct", Value::Text("yes".to_owned()));
        }

        let Some(count) = whole_or(call, "count", 1) else {
            return unable("number_usage");
        };
        if count < 1 {
            return unable("number_usage");
        }
        if count > MAX_COUNT {
            return unable("number_count_max");
        }
        let span = hi - lo + 1;
        if distinct && count > span {
            return SkillResult::Answer(
                Message::new("number_impossible")
                    .var("count", int_text(count, call.lang))
                    .var("available", int_text(span, call.lang)),
            );
        }

        let mut drawn: Vec<i64> = Vec::new();
        for _ in 0..count {
            let mut value = draw(cx, lo, hi, places);
            if distinct {
                let mut redraws = 0;
                while drawn.contains(&value) && redraws < MAX_REDRAWS {
                    value = draw(cx, lo, hi, places);
                    redraws += 1;
                }
                if drawn.contains(&value) {
                    // Practically unreachable; keeps the loop bounded: the
                    // next value of the range nobody has taken yet.
                    value = (1..=count)
                        .map(|k| lo + (value - lo + k).rem_euclid(span))
                        .find(|v| !drawn.contains(v))
                        .unwrap_or(value);
                }
            }
            drawn.push(value);
        }
        // Decimal ranges separate with ";": a decimal comma is a comma.
        let separator = if places == 0 { ", " } else { "; " };
        let numbers = drawn.iter().map(|&n| shown(n)).collect::<Vec<_>>().join(separator);
        let message = if reversed {
            Message::new("number_range")
                .var("low", shown(lo))
                .var("high", shown(hi))
        } else if lo == hi {
            Message::new("number_single")
        } else {
            Message::new("number_one")
        };
        SkillResult::Answer(message.var("numbers", numbers))
    }
}

/// Why a bound cannot be used.
enum Failure {
    TooBig,
    Unreadable,
}

impl Failure {
    const fn reply(&self) -> &'static str {
        match self {
            Failure::TooBig => "number_big",
            Failure::Unreadable => "number_usage",
        }
    }
}

/// A bound of the range: the slot, or `default` when absent. Larger than
/// 10^15 in magnitude, or too long to read, is "too big".
fn bound(call: &Call<'_>, name: &str, default: i64) -> Result<Number, Failure> {
    match call.slots.get(name) {
        None => Ok(Number::from_int(default)),
        Some(Value::Number(n)) => {
            let integer_part = n.units() / 10_i128.pow(u32::from(n.scale()));
            if integer_part.abs() > MAX_BOUND {
                Err(Failure::TooBig)
            } else {
                Ok(*n)
            }
        }
        Some(Value::Invalid { raw, .. }) => {
            let digits = raw.chars().filter(char::is_ascii_digit).count();
            if raw.contains('^') || digits > 15 {
                Err(Failure::TooBig)
            } else {
                Err(Failure::Unreadable)
            }
        }
        Some(_) => Err(Failure::Unreadable),
    }
}

/// `n` in units of `10^-places`; `None` if it has more decimals.
fn units(n: Number, places: u8) -> Option<i64> {
    let factor = 10_i128.checked_pow(u32::from(places.checked_sub(n.scale())?))?;
    i64::try_from(n.units().checked_mul(factor)?).ok()
}

/// One draw of a number in `lo..=hi` (units of `10^-places`).
fn draw(cx: &mut Context<'_>, lo: i64, hi: i64, places: u8) -> i64 {
    if places == 0 {
        cx.random().int(lo, hi)
    } else {
        cx.random().scaled(lo, hi, places)
    }
}

/// A number given in units of `10^-places`, with all its decimals ("1,50").
fn number_text(units: i64, places: u8, lang: Lang) -> String {
    if places == 0 {
        return int_text(units, lang);
    }
    let sign = if units < 0 { "-" } else { "" };
    let scale = 10_i64.pow(u32::from(places));
    let magnitude = units.unsigned_abs();
    let int = i64::try_from(magnitude / scale.unsigned_abs()).unwrap_or(0);
    let frac = magnitude % scale.unsigned_abs();
    let separator = Number::new(1, 1)
        .map(|n| n.format(lang))
        .and_then(|s| s.chars().nth(1))
        .unwrap_or('.');
    let width = usize::from(places);
    format!("{sign}{}{separator}{frac:0width$}", int_text(int, lang))
}

impl Skill for Random {
    fn id(&self) -> &'static str {
        "random"
    }

    fn replies(&self) -> &'static [&'static str] {
        &[
            "dice_one",
            "dice_one_plus",
            "dice_one_minus",
            "dice_list",
            "dice_list_plus",
            "dice_list_minus",
            "dice_the",
            "dice_total",
            "dice_zero",
            "dice_sides_min",
            "dice_sides_max",
            "dice_count_max",
            "dice_mixed",
            "dice_unsupported",
            "dice_usage",
            "coin_heads",
            "coin_tails",
            "coin_list",
            "coin_counts",
            "coin_max",
            "coin_usage",
            "pick_chosen",
            "pick_uncertain",
            "pick_one",
            "pick_many",
            "pick_usage",
            "number_one",
            "number_range",
            "number_single",
            "number_impossible",
            "number_big",
            "number_count_max",
            "number_usage",
            "number_incomplete",
            "word_heads",
            "word_tails",
            "word_and",
        ]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "random.dice" => Self::dice(cx, call),
            "random.coin" => Self::coin(cx, call),
            "random.pick" => Self::pick(cx, call),
            "random.number" => Self::number(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{Draw, IntentId, Random as Source, Slots, Timestamp};

    fn run(intent: &str, slots: &[(&str, Value)], draws: &str) -> (SkillResult, usize) {
        let intent = IntentId(intent.to_owned());
        let slots: Slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let script = Draw::parse_list(draws).unwrap();
        let mut cx = Context::standalone(Timestamp::from_unix(0), Source::scripted(script));
        let call = Call {
            intent: &intent,
            lang: Lang::En,
            slots: &slots,
            text: "x",
        };
        let result = Random.handle(&mut cx, &call);
        let unused = cx.random().draws();
        (result, unused as usize)
    }

    fn n(v: i64) -> Value {
        Value::Number(Number::from_int(v))
    }

    fn id(result: &SkillResult) -> &'static str {
        match result {
            SkillResult::Answer(m) | SkillResult::Unable(m) => m.id,
            _ => "other",
        }
    }

    #[test]
    fn odd_dice_never_panic_and_draw_nothing() {
        for (slots, reply) in [
            (vec![("count", n(0))], "dice_zero"),
            (vec![("count", n(101))], "dice_count_max"),
            (vec![("sides", n(1))], "dice_sides_min"),
            (vec![("sides", n(-4))], "dice_sides_min"),
            (vec![("sides", n(1001))], "dice_sides_max"),
            (vec![("count", n(i64::MAX))], "dice_count_max"),
            (vec![("modifier", n(i64::MAX))], "dice_usage"),
        ] {
            let (result, draws) = run("random.dice", &slots, "");
            assert_eq!(id(&result), reply);
            assert_eq!(draws, 0);
        }
    }

    #[test]
    fn dice_draw_once_per_die() {
        let (result, draws) = run("random.dice", &[("count", n(3)), ("sides", n(8))], "5, 2, 8");
        assert_eq!(id(&result), "dice_list");
        assert_eq!(draws, 3);
    }

    #[test]
    fn a_reversed_or_degenerate_range_is_handled() {
        let (result, _) = run("random.number", &[("low", n(10)), ("high", n(1))], "4");
        assert_eq!(id(&result), "number_range");
        let (result, _) = run("random.number", &[("low", n(5)), ("high", n(5))], "5");
        assert_eq!(id(&result), "number_single");
        let (result, draws) = run(
            "random.number",
            &[
                ("low", n(1)),
                ("high", n(5)),
                ("count", n(10)),
                ("distinct", Value::Text("yes".to_owned())),
            ],
            "",
        );
        assert_eq!(id(&result), "number_impossible");
        assert_eq!(draws, 0);
    }

    #[test]
    fn distinct_numbers_always_terminate() {
        // A source that keeps repeating the same value cannot loop forever.
        let script = vec!["1"; 20_000].join(",");
        let (result, _) = run(
            "random.number",
            &[
                ("low", n(1)),
                ("high", n(3)),
                ("count", n(3)),
                ("distinct", Value::Text("yes".to_owned())),
            ],
            &script,
        );
        assert_eq!(id(&result), "number_one");
    }

    #[test]
    fn huge_bounds_and_counts_are_refused() {
        let big = Value::Number(Number::new(10_i128.pow(20), 0).unwrap());
        let (result, draws) = run("random.number", &[("high", big)], "");
        assert_eq!(id(&result), "number_big");
        assert_eq!(draws, 0);
        let (result, _) = run("random.number", &[("count", n(150))], "");
        assert_eq!(id(&result), "number_count_max");
        let (result, _) = run("random.coin", &[("count", n(500))], "");
        assert_eq!(id(&result), "coin_max");
    }

    #[test]
    fn picks_need_options() {
        let list = |items: &[&str]| Value::List {
            items: items.iter().map(|s| (*s).to_owned()).collect(),
            uncertain: false,
        };
        let (result, _) = run("random.pick", &[], "");
        assert_eq!(id(&result), "pick_usage");
        let (result, draws) = run("random.pick", &[("options", list(&["tea"]))], "");
        assert_eq!(id(&result), "pick_one");
        assert_eq!(draws, 0);
        let (result, _) = run("random.pick", &[("options", list(&["two shirts"]))], "");
        assert_eq!(id(&result), "pick_usage");
        let (result, draws) = run(
            "random.pick",
            &[("options", list(&["tea", "coffee"]))],
            "option 2",
        );
        assert_eq!(id(&result), "pick_chosen");
        assert_eq!(draws, 1);
    }

    #[test]
    fn decimals_keep_two_places() {
        assert_eq!(number_text(173, 2, Lang::Es), "1,73");
        assert_eq!(number_text(150, 2, Lang::En), "1.50");
        assert_eq!(number_text(-50, 2, Lang::En), "-0.50");
    }
}
