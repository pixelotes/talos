//! `calc.convert`: a quantity from one unit to another (docs/spec/basics.md).

use talos_core::extract;
use talos_core::normalise::fold;
use talos_core::{Call, Context, Expectation, Lang, Message, SkillResult, Units, Value};

use super::numbers::{ReadError, canonical_number, echo_number, read_digits};
use super::rat::{Rat, show_places};
use super::units::{
    Category, Resolved, Temperature, Unit, assumed_scale, binary_twin, convert_linear,
    convert_temperature, resolve, uses_average_calendar,
};

/// An amount as the user wrote it.
struct Amount {
    value: Rat,
    /// The reading to show when the digits were ambiguous.
    unforced_raw: Option<String>,
}

enum AmountError {
    Unreadable(String),
    TooLarge,
}

/// Reads the `amount` slot (a term): digits, an exponent ("1e30"), or number
/// words ("una", "un millón", "a").
fn parse_amount(text: &str, lang: Lang) -> Result<Amount, AmountError> {
    let folded = fold(&text.trim().to_lowercase());
    let (negative, body) = match folded.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, folded.strip_prefix('+').unwrap_or(&folded).trim()),
    };
    let body = body.strip_suffix(" de").unwrap_or(body);
    let signed = |value: Rat| if negative { value.neg() } else { value };
    if body.starts_with(|c: char| c.is_ascii_digit()) {
        return match read_digits(body, lang) {
            Ok(read) => Ok(Amount {
                unforced_raw: read.unforced.then(|| body.to_owned()),
                value: signed(read.value),
            }),
            Err(ReadError::Malformed) => Err(AmountError::Unreadable(text.trim().to_owned())),
            Err(ReadError::TooLarge) => Err(AmountError::TooLarge),
        };
    }
    match extract::number(body, lang) {
        Ok(n) => Ok(Amount {
            value: signed(Rat::from_number(n)),
            unforced_raw: None,
        }),
        Err(_) => Err(AmountError::Unreadable(text.trim().to_owned())),
    }
}

fn unable(id: &'static str) -> SkillResult {
    SkillResult::Unable(Message::new(id))
}

/// Asks for the unit to convert to (docs/spec/basics.md, `calc.convert`).
fn ask_target(id: &'static str) -> SkillResult {
    SkillResult::Ask(Expectation::slot("to"), Message::new(id))
}

/// The reply for a unit that is not supported, if it is not.
fn unsupported(raw: &str, resolved: Resolved) -> Option<SkillResult> {
    match resolved {
        Resolved::Currency => Some(unable("currency")),
        Resolved::Area => Some(unable("area")),
        Resolved::DataRate => Some(unable("rate")),
        Resolved::OtherCategory => Some(unable("other_category")),
        Resolved::Unknown => Some(SkillResult::Unable(
            Message::new("unknown_unit").var("unit", raw.trim()),
        )),
        Resolved::Unit(_) | Resolved::BareDegrees => None,
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().chain(chars).collect()
    })
}

fn shown(value: &Rat, lang: Lang) -> String {
    let s = show_places(value, 2, lang, "-");
    if s.approx {
        format!("≈ {}", s.text)
    } else {
        s.text
    }
}

/// The scale word of a temperature unit: "Celsius", "Fahrenheit", "Kelvin".
fn scale_word(unit: &Unit, lang: Lang) -> &'static str {
    unit.name(lang, true).rsplit(' ').next().unwrap_or("")
}

/// A conversion request, its units resolved.
struct Request {
    amount: Amount,
    from: &'static Unit,
    to: &'static Unit,
    /// The scale taken for a bare "degrees".
    assumed: Option<&'static Unit>,
}

/// Reads the slots and resolves the units, or says why it cannot.
fn request(call: &Call<'_>) -> Result<Request, SkillResult> {
    let lang = call.lang;
    let text = |name: &str| call.slots.get(name).and_then(Value::as_text);
    let (Some(from_raw), Some(to_raw)) = (text("from"), text("to")) else {
        return Err(match text("from").map(resolve) {
            Some(Resolved::BareDegrees) => ask_target("degrees_target"),
            Some(_) => ask_target("convert_target"),
            None => unable("convert_usage"),
        });
    };
    let amount = match text("amount") {
        None => Amount {
            value: Rat::from_int(1),
            unforced_raw: None,
        },
        Some(t) => match parse_amount(t, lang) {
            Ok(a) => a,
            Err(AmountError::Unreadable(raw)) => {
                return Err(SkillResult::Unable(
                    Message::new("convert_bad_number").var("raw", raw),
                ));
            }
            Err(AmountError::TooLarge) => return Err(unable("convert_too_large")),
        },
    };
    let (from_resolved, to_resolved) = (resolve(from_raw), resolve(to_raw));
    // "3 pm en Tokio": neither side is a unit, so it is not ours.
    if matches!(
        (from_resolved, to_resolved),
        (Resolved::Unknown, Resolved::Unknown)
    ) {
        return Err(SkillResult::Decline);
    }
    for (raw, resolved) in [(from_raw, from_resolved), (to_raw, to_resolved)] {
        if let Some(reply) = unsupported(raw, resolved) {
            return Err(reply);
        }
    }
    match (from_resolved, to_resolved) {
        (Resolved::Unit(from), Resolved::Unit(to)) => Ok(Request {
            amount,
            from,
            to,
            assumed: None,
        }),
        (Resolved::BareDegrees, Resolved::Unit(to)) => match assumed_scale(to) {
            Some(scale) => Ok(Request {
                amount,
                from: scale,
                to,
                assumed: Some(scale),
            }),
            None => Err(SkillResult::Unable(
                Message::new("unknown_unit").var("unit", from_raw.trim()),
            )),
        },
        // A bare "degrees" as the target: ask which scale.
        _ => Err(ask_target("degrees_target")),
    }
}

/// The usual counterpart of a unit in the other system, by id, as an alias
/// [`resolve`] reads (docs/spec/basics.md, calc.convert): for metric users,
/// what an imperial unit converts to; for imperial users, the reverse. A
/// unit already in the user's system, or in neither, has none.
fn counterpart(from: &Unit, units: Units) -> Option<&'static str> {
    let to = match (units, from.id) {
        (Units::Metric, "in") => "cm",
        (Units::Metric, "ft" | "yd") => "m",
        (Units::Metric, "mi") => "km",
        (Units::Metric, "oz") => "g",
        (Units::Metric, "lb") => "kg",
        (Units::Metric, "floz") => "ml",
        (Units::Metric, "gal") => "l",
        (Units::Metric, "mph") => "km/h",
        (Units::Metric, "°F") => "c",
        (Units::Imperial, "mm" | "cm") => "in",
        (Units::Imperial, "m") => "ft",
        (Units::Imperial, "km") => "mi",
        (Units::Imperial, "mg" | "g") => "oz",
        (Units::Imperial, "kg" | "t") => "lb",
        (Units::Imperial, "ml" | "cl") => "floz",
        (Units::Imperial, "l") => "gal",
        (Units::Imperial, "km/h" | "m/s") => "mph",
        (Units::Imperial, "°C" | "K") => "f",
        _ => return None,
    };
    Some(to)
}

pub(super) fn handle(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    // No target named: the remembered units choose one (docs/spec/state.md).
    let mut slots;
    let mut call = *call;
    if !call.slots.contains_key("to")
        && let Some(units) = cx.preferences().units()
        && let Some(Resolved::Unit(from)) = call.slots.get("from").and_then(Value::as_text).map(resolve)
        && let Some(to) = counterpart(from, units)
    {
        slots = call.slots.clone();
        slots.insert("to".to_owned(), Value::Text(to.to_owned()));
        call = Call {
            slots: &slots,
            ..call
        };
    }
    let call = &call;
    let request = match request(call) {
        Ok(request) => request,
        Err(reply) => return reply,
    };
    cx.report("amount", Value::Text(canonical_number(&request.amount.value)));
    cx.report("from", Value::Text(request.from.id.to_owned()));
    cx.report("to", Value::Text(request.to.id.to_owned()));
    answer(&request, call.lang)
}

fn answer(request: &Request, lang: Lang) -> SkillResult {
    let Request {
        amount,
        from,
        to,
        assumed,
    } = request;
    if from.category != to.category {
        return SkillResult::Answer(
            Message::new("mismatch")
                .var("from_name", capitalised(from.name(lang, true)))
                .var("from_cat", from.category.name(lang))
                .var("to_name", to.name(lang, true))
                .var("to_cat", to.category.name(lang)),
        );
    }
    let result = if from.category == Category::Temperature {
        match convert_temperature(&amount.value, from, to) {
            Some(Temperature::Value(v)) => v,
            Some(Temperature::BelowZero) => {
                return SkillResult::Answer(
                    Message::new("below_zero")
                        .var("amount", echo_number(&amount.value, lang, "-").text)
                        .var("from", from.display(lang, true, false))
                        .var("to_name", capitalised(scale_word(to, lang))),
                );
            }
            None => return unable("convert_too_large"),
        }
    } else if let Some(v) = convert_linear(&amount.value, from, to) {
        v
    } else {
        return unable("convert_too_large");
    };

    let one = Rat::from_int(1);
    let named = from.is_bit_based() || to.is_bit_based();
    let message = |id: &'static str| {
        Message::new(id)
            .var("amount", echo_number(&amount.value, lang, "-").text)
            .var("from", from.display(lang, amount.value != one, named))
            .var("result", shown(&result, lang))
            .var("to", to.display(lang, result != one, named))
    };

    // One note at most: what was read, then what was assumed, then the
    // calendar average, then the binary twin.
    if let Some(raw) = &amount.unforced_raw {
        let read = echo_number(&amount.value, lang, "-").text;
        return SkillResult::Answer(
            message("convert_read")
                .var("raw", raw.clone())
                .var("read", read),
        );
    }
    if let Some(scale) = assumed {
        return SkillResult::Answer(
            message("convert_degrees").var("degrees", scale_word(scale, lang)),
        );
    }
    if uses_average_calendar(from, to) {
        return SkillResult::Answer(message("convert_average"));
    }
    if from != to
        && let (Some(from_b), Some(to_b)) = (binary_twin(from), binary_twin(to))
        && let Some(twin) = convert_linear(&amount.value, from_b, to_b)
    {
        return SkillResult::Answer(
            message("convert_binary")
                .var("from_b", from_b.display(lang, amount.value != one, false))
                .var("result_b", shown(&twin, lang))
                .var("to_b", to_b.display(lang, twin != one, false)),
        );
    }
    SkillResult::Answer(message("convert"))
}
