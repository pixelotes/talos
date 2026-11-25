//! `calc`: arithmetic, percentages and unit conversion (docs/spec/basics.md).
//!
//! Calculation is exact: numbers are rational (`rat`, over the in-tree
//! integers of `big`), so `0.1 + 0.2` is `0.3` and `1/3*3` is `1`, and
//! rounding happens only when a value is shown. Roots that do not terminate
//! are computed to 40 digits and marked approximate. Nothing here reads the
//! clock, the environment or any global.
//!
//! - `expr`: tokenizer, parser and evaluator of `calc.arithmetic`;
//! - `percent`: the shapes of `calc.percent`;
//! - `convert` and `units`: `calc.convert` and its tables;
//! - `numbers`: reading digits by the language's conventions.

mod big;
mod convert;
mod expr;
mod numbers;
mod percent;
mod rat;
mod units;

use talos_core::{Call, Context, Lang, Message, Skill, SkillResult, Value};

use expr::{Failure, Parsed};
use rat::{MathError, show_significant};

/// The `calc` handler.
#[derive(Debug, Default)]
pub(crate) struct Calc;

/// Every reply id the handler can return.
const REPLIES: &[&str] = &[
    "value",
    "approx",
    "value_read",
    "approx_read",
    "value_reads",
    "approx_reads",
    "div_zero",
    "zero_over_zero",
    "no_real_root",
    "too_large",
    "unsupported_power",
    "arithmetic_usage",
    "incomplete",
    "implicit_multiplication",
    "bad_number",
    "pct_of",
    "pct_is",
    "pct_raise",
    "pct_lower",
    "pct_discount",
    "pct_vat",
    "pct_up",
    "pct_down",
    "pct_same",
    "pct_whole",
    "pct_tip",
    "pct_whole_zero",
    "pct_from_zero",
    "pct_percent_zero",
    "pct_usage",
    "pct_needs_rate",
    "pct_bad_number",
    "pct_too_large",
    "convert",
    "convert_read",
    "convert_degrees",
    "convert_average",
    "convert_binary",
    "mismatch",
    "below_zero",
    "unknown_unit",
    "currency",
    "area",
    "rate",
    "other_category",
    "convert_usage",
    "convert_target",
    "degrees_target",
    "convert_bad_number",
    "convert_too_large",
];

impl Skill for Calc {
    fn id(&self) -> &'static str {
        "calc"
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "calc.arithmetic" => arithmetic(cx, call),
            "calc.percent" => percent::handle(cx, call),
            "calc.convert" => convert::handle(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

fn unable(id: &'static str) -> SkillResult {
    SkillResult::Unable(Message::new(id))
}

/// Evaluates the `expression` slot and writes the answer.
fn arithmetic(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let Some(Value::Text(text)) = call.slots.get("expression") else {
        return unable("arithmetic_usage");
    };
    let parsed = match expr::parse(text, call.lang) {
        Ok(parsed) => parsed,
        Err(failure) => {
            cx.report("expression", Value::Text(expr::repair_parens(text)));
            return failed(failure, call.lang);
        }
    };
    cx.report("expression", Value::Text(parsed.canonical.clone()));
    match parsed.value() {
        Ok(value) => answer(&parsed, &value, call.lang),
        Err(failure) => failed(failure, call.lang),
    }
}

/// The reply for an expression that has no value.
fn failed(failure: Failure, lang: Lang) -> SkillResult {
    match failure {
        Failure::NotMath => SkillResult::Decline,
        Failure::Incomplete => unable("incomplete"),
        Failure::ImplicitMul => unable("implicit_multiplication"),
        Failure::BadNumber(raw) => SkillResult::Unable(Message::new("bad_number").var("raw", raw)),
        Failure::TooBig | Failure::Math(MathError::TooLarge) => unable("too_large"),
        Failure::Math(MathError::DivZero { zero_over_zero }) => {
            SkillResult::Answer(Message::new(if zero_over_zero {
                "zero_over_zero"
            } else {
                "div_zero"
            }))
        }
        Failure::Math(MathError::Unsupported) => unable("unsupported_power"),
        Failure::NoRealRoot(radicand) => SkillResult::Answer(
            Message::new("no_real_root")
                .var("radicand", show_significant(&radicand, 10, false, lang, "-").text),
        ),
    }
}

fn answer(done: &Parsed, value: &expr::Val, lang: Lang) -> SkillResult {
    let shown = show_significant(
        &value.value,
        10,
        value.approx,
        lang,
        "\u{2212}",
    );
    let readings: Vec<String> = done
        .notes
        .iter()
        .map(|note| {
            let read = numbers::echo_number(&note.value, lang, "-").text;
            format!("{} = {read}", note.raw)
        })
        .collect();
    let (id, extra) = match (shown.approx, done.notes.as_slice()) {
        (false, []) => ("value", None),
        (true, []) => ("approx", None),
        (approx, [one]) => {
            let read = numbers::echo_number(&one.value, lang, "-").text;
            (
                if approx { "approx_read" } else { "value_read" },
                Some(vec![("raw", one.raw.clone()), ("read", read)]),
            )
        }
        (approx, _) => (
            if approx { "approx_reads" } else { "value_reads" },
            Some(vec![("readings", readings.join("; "))]),
        ),
    };
    let mut message = Message::new(id)
        .var("expr", done.echo.clone())
        .var("result", shown.text);
    for (name, value) in extra.into_iter().flatten() {
        message = message.var(name, value);
    }
    SkillResult::Answer(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{IntentId, Number, Random, Timestamp};

    fn call(intent: &str, lang: Lang, slots: &[(&str, Value)]) -> SkillResult {
        let intent = IntentId(intent.to_owned());
        let slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let mut cx = Context::standalone(Timestamp::from_unix(0), Random::seeded(0));
        Calc.handle(
            &mut cx,
            &Call {
                intent: &intent,
                lang,
                slots: &slots,
                text: "",
            },
        )
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn number(units: i128, scale: u8) -> Value {
        Value::Number(Number::new(units, scale).unwrap())
    }

    /// `(kind, reply id, variables)`, kind being A(nswer) or U(nable).
    fn shape(result: &SkillResult) -> (char, &'static str, Vec<(String, String)>) {
        let (kind, message) = match result {
            SkillResult::Answer(m) => ('A', m),
            SkillResult::Unable(m) => ('U', m),
            other => panic!("unexpected {other:?}"),
        };
        (
            kind,
            message.id,
            message.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        )
    }

    fn reply(result: &SkillResult, var: &str) -> String {
        let (_, _, vars) = shape(result);
        vars.into_iter()
            .find(|(k, _)| k == var)
            .map_or_else(String::new, |(_, v)| v)
    }

    fn percent(lang: Lang, slots: &[(&str, Value)]) -> SkillResult {
        call("calc.percent", lang, slots)
    }

    fn convert(lang: Lang, amount: Option<&str>, from: &str, to: Option<&str>) -> SkillResult {
        let mut slots = vec![("from", text(from))];
        if let Some(a) = amount {
            slots.push(("amount", text(a)));
        }
        if let Some(t) = to {
            slots.push(("to", text(t)));
        }
        call("calc.convert", lang, &slots)
    }

    #[test]
    fn arithmetic_answers_declines_and_hints() {
        let ok = call("calc.arithmetic", Lang::En, &[("expression", text("0.1+0.2"))]);
        assert_eq!(shape(&ok).1, "value");
        assert_eq!(reply(&ok, "result"), "0.3");
        let approx = call("calc.arithmetic", Lang::Es, &[("expression", text("2/3"))]);
        assert_eq!(shape(&approx).1, "approx");
        assert_eq!(reply(&approx, "result"), "0,6666666667");
        let read = call("calc.arithmetic", Lang::Es, &[("expression", text("1.000+1"))]);
        assert_eq!(shape(&read).1, "value_read");
        assert_eq!(reply(&read, "raw"), "1.000");
        let reads = call("calc.arithmetic", Lang::Es, &[("expression", text("1.000+2,000"))]);
        assert_eq!(shape(&reads).1, "value_reads");
        let approx_read = call("calc.arithmetic", Lang::En, &[("expression", text("1,5/3"))]);
        assert_eq!(shape(&approx_read).1, "value_read");
        let zero = call("calc.arithmetic", Lang::En, &[("expression", text("1/0"))]);
        assert_eq!(shape(&zero), ('A', "div_zero", vec![]));
        let none = call("calc.arithmetic", Lang::En, &[]);
        assert_eq!(shape(&none).1, "arithmetic_usage");
        assert_eq!(
            call("calc.arithmetic", Lang::En, &[("expression", text("hello"))]),
            SkillResult::Decline
        );
        assert_eq!(
            call("calc.arithmetic", Lang::En, &[("expression", text("5%+2"))]),
            SkillResult::Decline
        );
        assert_eq!(call("calc.other", Lang::En, &[]), SkillResult::Decline);
        let negative_root = call("calc.arithmetic", Lang::En, &[("expression", text("sqrt(-9)"))]);
        assert_eq!(shape(&negative_root).1, "no_real_root");
        assert_eq!(reply(&negative_root, "radicand"), "-9");
    }

    #[test]
    fn percentages_in_each_shape() {
        let of = percent(
            Lang::Es,
            &[("shape", text("of")), ("percent", number(125, 1)), ("amount", number(80, 0))],
        );
        assert_eq!(reply(&of, "percent"), "12,5");
        assert_eq!(reply(&of, "result"), "10");
        let rounded = percent(
            Lang::Es,
            &[("shape", text("of")), ("percent", number(15, 0)), ("amount", number(9999, 2))],
        );
        assert_eq!(reply(&rounded, "result"), "≈ 15");
        let third = percent(
            Lang::En,
            &[("shape", text("part_of_whole")), ("part", number(1, 0)), ("whole", number(3, 0))],
        );
        assert_eq!(reply(&third, "percent"), "≈ 33.33");
        let zero = percent(
            Lang::En,
            &[("shape", text("part_of_whole")), ("part", number(1, 0)), ("whole", number(0, 0))],
        );
        assert_eq!(shape(&zero), ('A', "pct_whole_zero", vec![]));
        let up = percent(
            Lang::En,
            &[("shape", text("change")), ("from", number(50, 0)), ("to", number(65, 0))],
        );
        assert_eq!((shape(&up).1, reply(&up, "percent")), ("pct_up", "30".to_owned()));
        let down = percent(
            Lang::En,
            &[("shape", text("change")), ("from", number(80, 0)), ("to", number(60, 0))],
        );
        assert_eq!(shape(&down).1, "pct_down");
        let from_zero = percent(
            Lang::En,
            &[("shape", text("change")), ("from", number(0, 0)), ("to", number(5, 0))],
        );
        assert_eq!(shape(&from_zero).1, "pct_from_zero");
        let whole = percent(
            Lang::En,
            &[("shape", text("whole")), ("part", number(20, 0)), ("percent", number(40, 0))],
        );
        assert_eq!(reply(&whole, "result"), "50");
        let tip = percent(
            Lang::Es,
            &[("shape", text("tip")), ("percent", number(10, 0)), ("amount", number(45, 0))],
        );
        assert_eq!((reply(&tip, "tip"), reply(&tip, "total")), ("4,5".to_owned(), "49,5".to_owned()));
        let too_big = percent(
            Lang::En,
            &[
                ("shape", text("of")),
                ("percent", number(10_i128.pow(29), 0)),
                ("amount", number(10_i128.pow(29), 0)),
            ],
        );
        assert_eq!(shape(&too_big).0, 'A', "30-digit inputs are fine: {too_big:?}");
    }

    #[test]
    fn percentages_carry_currency_and_say_which_way() {
        let es = percent(
            Lang::Es,
            &[
                ("shape", text("adjust")),
                ("op", text("discount")),
                ("percent", number(20, 0)),
                ("amount", number(50, 0)),
                ("unit", text("euros")),
            ],
        );
        assert_eq!(shape(&es).1, "pct_discount");
        assert_eq!((reply(&es, "amount"), reply(&es, "result")), ("50 €".to_owned(), "40 €".to_owned()));
        let en = percent(
            Lang::En,
            &[
                ("shape", text("adjust")),
                ("op", text("minus")),
                ("percent", number(150, 0)),
                ("amount", number(10, 0)),
                ("sym", text("$")),
            ],
        );
        assert_eq!(shape(&en).1, "pct_lower");
        assert_eq!(reply(&en, "result"), "-$5");
        let vat = percent(
            Lang::Es,
            &[
                ("shape", text("adjust")),
                ("op", text("mas")),
                ("tax", text("IVA")),
                ("percent", number(21, 0)),
                ("amount", number(100, 0)),
            ],
        );
        assert_eq!(shape(&vat).1, "pct_vat");
        assert_eq!(reply(&vat, "result"), "121");
    }

    #[test]
    fn percentages_missing_or_unreadable_get_a_hint() {
        let no_rate = percent(Lang::Es, &[("shape", text("adjust")), ("amount", number(100, 0))]);
        assert_eq!(shape(&no_rate), ('U', "pct_needs_rate", vec![]));
        let no_amount = percent(Lang::En, &[("shape", text("of")), ("percent", number(50, 0))]);
        assert_eq!(shape(&no_amount), ('U', "pct_usage", vec![]));
        let bad = percent(
            Lang::En,
            &[
                ("shape", text("of")),
                ("percent", number(5, 0)),
                (
                    "amount",
                    Value::Invalid {
                        raw: "1,500,000".to_owned(),
                        problem: talos_core::Problem::Unreadable,
                    },
                ),
            ],
        );
        assert_eq!(shape(&bad).1, "pct_bad_number");
        assert_eq!(percent(Lang::En, &[("percent", number(5, 0))]), SkillResult::Decline);
    }

    #[test]
    fn conversions_round_say_so_and_keep_exact_results_exact() {
        let miles = convert(Lang::En, Some("10"), "miles", Some("km"));
        assert_eq!(reply(&miles, "result"), "≈ 16.09");
        let inches = convert(Lang::Es, Some("5"), "pulgadas", Some("cm"));
        assert_eq!((shape(&inches).1, reply(&inches, "result")), ("convert", "12,7".to_owned()));
        let tiny = convert(Lang::Es, Some("1"), "mm", Some("km"));
        assert_eq!(reply(&tiny, "result"), "0,000001");
        let default_amount = convert(Lang::En, None, "pound", Some("oz"));
        assert_eq!((reply(&default_amount, "amount"), reply(&default_amount, "result")), ("1".to_owned(), "16".to_owned()));
        let words = convert(Lang::Es, Some("una"), "hora", Some("segundos"));
        assert_eq!(reply(&words, "result"), "3600");
        let thousand = convert(Lang::Es, Some("1.000"), "m", Some("km"));
        assert_eq!(shape(&thousand).1, "convert_read");
        let sci = convert(Lang::En, Some("1e30"), "km", Some("mm"));
        assert_eq!(reply(&sci, "result"), "1 × 10^36");
        let negative = convert(Lang::En, Some("-40"), "c", Some("f"));
        assert_eq!(reply(&negative, "result"), "-40");
    }

    #[test]
    fn conversions_that_cannot_be_done_say_why() {
        assert_eq!(shape(&convert(Lang::En, Some("5"), "flurbos", Some("m"))).1, "unknown_unit");
        assert_eq!(shape(&convert(Lang::En, Some("5"), "euros", Some("usd"))).1, "currency");
        assert_eq!(
            convert(Lang::Es, Some("3"), "pm", Some("tokio")),
            SkillResult::Decline,
            "two unknown words are not a conversion"
        );
        assert_eq!(
            shape(&convert(Lang::En, Some("1e999"), "km", Some("mm"))).1,
            "convert_too_large",
            "a result beyond 10^1000 is refused"
        );
        assert_eq!(shape(&convert(Lang::En, Some("5"), "m2", Some("ft2"))).1, "area");
        assert_eq!(shape(&convert(Lang::En, Some("5"), "mbps", Some("mb/s"))).1, "rate");
        assert_eq!(shape(&convert(Lang::En, Some("5"), "watts", Some("hp"))).1, "other_category");
        assert_eq!(shape(&convert(Lang::En, Some("5"), "kg", Some("m"))).1, "mismatch");
        assert_eq!(shape(&convert(Lang::En, Some("-300"), "c", Some("k"))).1, "below_zero");
        assert_eq!(shape(&convert(Lang::En, Some("5"), "km", None)).1, "convert_target");
        assert_eq!(shape(&convert(Lang::En, Some("20"), "degrees", None)).1, "degrees_target");
        assert_eq!(shape(&convert(Lang::En, Some("20"), "km", Some("degrees"))).1, "degrees_target");
        assert_eq!(shape(&convert(Lang::Es, Some("1,500,000"), "km", Some("m"))).1, "convert_bad_number");
        assert_eq!(shape(&convert(Lang::En, Some(&"9".repeat(101)), "km", Some("m"))).1, "convert_too_large");
        assert_eq!(shape(&convert(Lang::En, Some("1e1200"), "km", Some("m"))).1, "convert_too_large");
        assert_eq!(call("calc.convert", Lang::En, &[]), SkillResult::Unable(Message::new("convert_usage")));
    }

    #[test]
    fn conversion_notes_in_order_of_importance() {
        assert_eq!(shape(&convert(Lang::En, Some("70"), "degrees", Some("celsius"))).1, "convert_degrees");
        assert_eq!(shape(&convert(Lang::En, Some("2"), "years", Some("days"))).1, "convert_average");
        assert_eq!(shape(&convert(Lang::En, Some("2"), "years", Some("months"))).1, "convert");
        assert_eq!(shape(&convert(Lang::En, Some("2"), "gb", Some("mb"))).1, "convert_binary");
        assert_eq!(shape(&convert(Lang::En, Some("2"), "gb", Some("gb"))).1, "convert");
        let bits = convert(Lang::En, Some("8"), "Mb", Some("MB"));
        assert_eq!(reply(&bits, "from"), "Mb (megabits)");
        assert_eq!(reply(&bits, "to"), "MB (megabytes)");
        let lower = convert(Lang::En, Some("8"), "mb", Some("MB"));
        assert_eq!(reply(&lower, "result"), "8", "a lower-case b is bytes");
    }
}
