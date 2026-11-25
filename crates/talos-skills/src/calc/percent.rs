//! `calc.percent`: the shapes of percentage question (docs/spec/basics.md).
//! The rule that matched fixes the `shape` slot; the numbers arrive typed.

use std::cmp::Ordering;

use talos_core::normalise::fold;
use talos_core::{Call, Context, Expectation, Lang, Message, SkillResult, Value};

use super::numbers::{canonical_number, echo_number};
use super::rat::{MathError, Rat, show_places};

/// The slots of the shape, read.
struct Inputs<'a> {
    call: &'a Call<'a>,
}

/// A slot that was given, was not, or could not be read.
enum Slot {
    Missing,
    Bad(String),
    Value(Rat),
}

impl Inputs<'_> {
    fn get(&self, name: &str) -> Slot {
        match self.call.slots.get(name) {
            None => Slot::Missing,
            Some(Value::Number(n)) => Slot::Value(Rat::from_number(*n)),
            Some(other) => Slot::Bad(other.to_string()),
        }
    }

    fn text(&self, name: &str) -> Option<String> {
        self.call
            .slots
            .get(name)
            .and_then(Value::as_text)
            .map(|t| fold(&t.to_lowercase()))
    }
}

/// A currency the amount carried, to put back on the result.
#[derive(Clone, Copy)]
struct Currency(char);

impl Currency {
    fn from_slots(inputs: &Inputs<'_>) -> Option<Currency> {
        let word = inputs.text("sym").or_else(|| inputs.text("unit"))?;
        match word.trim() {
            "€" | "euro" | "euros" | "eur" => Some(Currency('€')),
            "$" | "dolar" | "dolares" | "dollar" | "dollars" | "usd" => Some(Currency('$')),
            "£" | "pound" | "pounds" | "gbp" => Some(Currency('£')),
            _ => None,
        }
    }

    /// Spanish puts the symbol after the amount ("40 €"), English before
    /// ("$40").
    fn write(self, number: &str, lang: Lang) -> String {
        match lang {
            Lang::Es => format!("{number} {}", self.0),
            Lang::En => match number.strip_prefix('-') {
                Some(rest) => format!("-{}{rest}", self.0),
                None => format!("{}{number}", self.0),
            },
        }
    }
}

/// A result with at most two decimals, `≈` first when rounded.
fn result(value: &Rat, lang: Lang, currency: Option<Currency>) -> String {
    let s = show_places(value, 2, lang, "-");
    let text = currency.map_or(s.text.clone(), |c| c.write(&s.text, lang));
    if s.approx { format!("≈ {text}") } else { text }
}

fn plain(value: &Rat, lang: Lang) -> String {
    echo_number(value, lang, "-").text
}

fn hundred() -> Rat {
    Rat::from_int(100)
}

/// `value * percent / 100`.
fn of(value: &Rat, percent: &Rat) -> Result<Rat, MathError> {
    value.mul(percent)?.div(&hundred())
}

fn unable(id: &'static str) -> SkillResult {
    SkillResult::Unable(Message::new(id))
}

pub(super) fn handle(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    // Zero divisors are answered where they arise: what is left is size.
    run(cx, call).unwrap_or_else(|_| unable("pct_too_large"))
}

/// Reads a slot the shape needs, or asks for it: `$missing` is the
/// question when it is absent; a number that cannot be read is asked for
/// again (docs/spec/conversation.md 2.1).
macro_rules! need {
    ($inputs:expr, $name:expr, $missing:expr) => {
        match $inputs.get($name) {
            Slot::Value(v) => v,
            Slot::Missing => {
                return Ok(SkillResult::Ask(
                    Expectation::slot($name),
                    Message::new($missing),
                ));
            }
            Slot::Bad(raw) => {
                return Ok(SkillResult::Ask(
                    Expectation::slot($name),
                    Message::new("pct_bad_number").var("raw", raw),
                ));
            }
        }
    };
}

fn run(cx: &mut Context<'_>, call: &Call<'_>) -> Result<SkillResult, MathError> {
    let lang = call.lang;
    let inputs = Inputs { call };
    let answer = |message: Message| Ok(SkillResult::Answer(message));
    match inputs.text("shape").as_deref() {
        Some("of") => {
            let percent = need!(inputs, "percent", "pct_ask_percent");
            let amount = need!(inputs, "amount", "pct_ask_amount");
            let value = of(&amount, &percent)?;
            answer(
                Message::new("pct_of")
                    .var("percent", plain(&percent, lang))
                    .var("amount", plain(&amount, lang))
                    .var("result", result(&value, lang, None)),
            )
        }
        Some("part_of_whole") => {
            let part = need!(inputs, "part", "pct_ask_number");
            let whole = need!(inputs, "whole", "pct_ask_number");
            if whole.is_zero() {
                return answer(Message::new("pct_whole_zero"));
            }
            let percent = part.div(&whole)?.mul(&hundred())?;
            answer(
                Message::new("pct_is")
                    .var("part", plain(&part, lang))
                    .var("percent", result(&percent, lang, None))
                    .var("whole", plain(&whole, lang)),
            )
        }
        Some("adjust") => adjust(cx, &inputs),
        Some("change") => {
            let from = need!(inputs, "from", "pct_ask_number");
            let to = need!(inputs, "to", "pct_ask_number");
            if from.is_zero() {
                return answer(Message::new("pct_from_zero"));
            }
            let percent = to.sub(&from)?.div(&from.abs())?.mul(&hundred())?;
            let (id, magnitude) = match percent.cmp_value(&Rat::zero()) {
                Ordering::Greater => ("pct_up", percent),
                Ordering::Less => ("pct_down", percent.abs()),
                Ordering::Equal => ("pct_same", percent),
            };
            answer(Message::new(id).var("percent", result(&magnitude, lang, None)))
        }
        Some("whole") => {
            let part = need!(inputs, "part", "pct_ask_number");
            let percent = need!(inputs, "percent", "pct_ask_percent");
            if percent.is_zero() {
                return answer(Message::new("pct_percent_zero"));
            }
            let whole = part.mul(&hundred())?.div(&percent)?;
            answer(
                Message::new("pct_whole")
                    .var("part", plain(&part, lang))
                    .var("percent", plain(&percent, lang))
                    .var("result", result(&whole, lang, None)),
            )
        }
        Some("tip") => {
            let percent = need!(inputs, "percent", "pct_ask_percent");
            let amount = need!(inputs, "amount", "pct_ask_amount");
            let tip = of(&amount, &percent)?;
            let total = amount.add(&tip)?;
            answer(
                Message::new("pct_tip")
                    .var("tip", result(&tip, lang, None))
                    .var("total", result(&total, lang, None)),
            )
        }
        _ => Ok(SkillResult::Decline),
    }
}

/// Raising or lowering an amount by a percentage, with the sign of the
/// change reported in the `percent` slot.
fn adjust(cx: &mut Context<'_>, inputs: &Inputs<'_>) -> Result<SkillResult, MathError> {
    let lang = inputs.call.lang;
    let amount = need!(inputs, "amount", "pct_ask_amount");
    let percent = need!(inputs, "percent", "pct_needs_rate");
    let op = inputs.text("op").unwrap_or_default();
    let discount = op == "discount";
    let lowers = discount
        || matches!(
            op.as_str(),
            "menos" | "-" | "minus" | "baja" | "reduce" | "disminuye" | "rebaja" | "decrease"
                | "lower" | "cut"
        );
    let vat = !lowers && inputs.text("tax").is_some();
    let id = match (lowers, discount, vat) {
        (true, true, _) => "pct_discount",
        (true, false, _) => "pct_lower",
        (false, _, true) => "pct_vat",
        (false, _, false) => "pct_raise",
    };
    let change = of(&amount, &percent)?;
    let value = if lowers {
        amount.sub(&change)?
    } else {
        amount.add(&change)?
    };
    let sign = if lowers { "-" } else { "+" };
    cx.report(
        "percent",
        Value::Text(format!("{sign}{}", canonical_number(&percent))),
    );
    let currency = Currency::from_slots(inputs);
    let amount_text = {
        let n = plain(&amount, lang);
        currency.map_or(n.clone(), |c| c.write(&n, lang))
    };
    Ok(SkillResult::Answer(
        Message::new(id)
            .var("percent", plain(&percent, lang))
            .var("amount", amount_text)
            .var("result", result(&value, lang, currency)),
    ))
}
