//! `timezones`: the current time in a place, and the difference in hours
//! between two places (docs/spec/state.md, `timezones`).
//!
//! Places are read from `term` slots with the place table the skill shares
//! with `preferences` (`crate::places`: cities and regions in both
//! languages, each with its zone's rule) and `talos_core::extract::place`
//! (references such as "mi ciudad", and offsets such as "UTC+2").
//!
//! The little phrases that go inside a reply's variables are written here
//! per language, as the clock skill writes weekday names: a time with its
//! Spanish article ("son las 13:42"), the day note ("(ya es lunes)"), a
//! difference ("1 hora y media"). Everything around them is in the manifest.

use talos_core::{
    Call, Choice, Context, Date, Expectation, Lang, Message, PlaceRef, Skill, SkillResult, Value,
};

use crate::places::{
    CITIES, Found, MAX_PLACES, NOTICE, Option_, Spot, city_spot, pieces, resolve,
    zone_name, zone_rule,
};

/// The `timezones` handler.
pub(crate) struct Timezones;

const WEEKDAYS_ES: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const WEEKDAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS_ES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The user's remembered place (`preferences`), when the place table knows
/// it. A name that is unknown or ambiguous here gives none, and the caller's
/// place is used instead.
fn remembered(cx: &Context<'_>, lang: Lang, now: i64) -> Option<Spot> {
    let place = cx.preferences().place()?;
    match resolve(place, lang, now) {
        Found::One(spot) => Some(spot),
        Found::Spain(_) => city_spot(&CITIES[0], lang, false),
        _ => None,
    }
}

/// The reference place (state.md, precedence): the remembered one, else
/// the caller's.
fn reference(cx: &Context<'_>, lang: Lang, now: i64) -> Option<Spot> {
    if let Some(spot) = remembered(cx, lang, now) {
        return Some(spot);
    }
    let place = cx.place()?.as_str();
    if let Some(rule) = zone_rule(place) {
        return Some(Spot {
            name: zone_name(place, lang),
            report: place.to_owned(),
            rule,
        });
    }
    match resolve(place, lang, now) {
        Found::One(spot) => Some(spot),
        Found::Spain(_) => city_spot(&CITIES[0], lang, false),
        _ => None,
    }
}

// --- Writing times and differences --------------------------------------------------

fn weekday(lang: Lang, date: Date) -> &'static str {
    let i = usize::from(date.weekday().days_from_monday());
    match lang {
        Lang::Es => WEEKDAYS_ES[i],
        Lang::En => WEEKDAYS_EN[i],
    }
}

/// "28 de marzo" / "28 March".
fn day_month(lang: Lang, date: Date) -> String {
    let m = usize::from(date.month() - 1);
    match lang {
        Lang::Es => format!("{} de {}", date.day(), MONTHS_ES[m]),
        Lang::En => format!("{} {}", date.day(), MONTHS_EN[m]),
    }
}

/// "lunes, 15 de marzo" / "Monday 15 March".
fn full_day(lang: Lang, date: Date) -> String {
    match lang {
        Lang::Es => format!("{}, {}", weekday(lang, date), day_month(lang, date)),
        Lang::En => format!("{} {}", weekday(lang, date), day_month(lang, date)),
    }
}

/// The time at a place, as `HH:MM`, in two forms: with the Spanish verb
/// and article ("son las 13:42", "es la 01:42") and with the article only
/// ("las 13:42"). English has neither.
fn times(lang: Lang, minute: u32) -> (String, String) {
    let (h, m) = (minute / 60, minute % 60);
    let hhmm = format!("{h:02}:{m:02}");
    match lang {
        Lang::En => (hhmm.clone(), hhmm),
        Lang::Es if h == 1 => (format!("es la {hhmm}"), format!("la {hhmm}")),
        Lang::Es => (format!("son las {hhmm}"), format!("las {hhmm}")),
    }
}

/// " (ya es lunes)" when the place is a day ahead of the reference date,
/// " (todavía es domingo)" when behind, nothing on the same day.
fn day_note(lang: Lang, date: Date, reference: Date) -> String {
    let day = weekday(lang, date);
    match (date.cmp(&reference), lang) {
        (std::cmp::Ordering::Equal, _) => String::new(),
        (std::cmp::Ordering::Greater, Lang::Es) => format!(" (ya es {day})"),
        (std::cmp::Ordering::Greater, Lang::En) => format!(" (already {day})"),
        (std::cmp::Ordering::Less, Lang::Es) => format!(" (todavía es {day})"),
        (std::cmp::Ordering::Less, Lang::En) => format!(" (still {day})"),
    }
}

/// A difference of `minutes` (positive) in words: "8 horas", "1 hora y
/// media", "5 hours 30 minutes".
fn span(lang: Lang, minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match lang {
        Lang::Es => {
            let hours = if h == 1 { "1 hora".to_owned() } else { format!("{h} horas") };
            match (h, m) {
                (_, 0) => hours,
                (0, 30) => "media hora".to_owned(),
                (_, 30) => format!("{hours} y media"),
                (0, _) => format!("{m} minutos"),
                _ => format!("{hours} y {m} minutos"),
            }
        }
        Lang::En => {
            let hours = if h == 1 { "1 hour".to_owned() } else { format!("{h} hours") };
            match (h, m) {
                (_, 0) => hours,
                (0, _) => format!("{m} minutes"),
                _ => format!("{hours} {m} minutes"),
            }
        }
    }
}

/// What the difference will be after a change, said briefly: "serán 7",
/// "será 1", "serán 4 horas y media"; "7", "4 hours 30 minutes".
fn after(lang: Lang, minutes: i64, verb: bool) -> String {
    let short = if minutes % 60 == 0 {
        (minutes / 60).to_string()
    } else {
        span(lang, minutes)
    };
    match lang {
        Lang::Es if verb && minutes <= 90 => format!("será {short}"),
        Lang::Es if verb => format!("serán {short}"),
        _ => short,
    }
}

// --- The handler ---------------------------------------------------------------------

fn slot_text<'a>(call: &'a Call<'_>, name: &str) -> Option<&'a str> {
    call.slots
        .get(name)
        .and_then(Value::as_text)
        .filter(|t| !t.trim().is_empty())
}

/// The local date where the user is: the reference place's, else the
/// turn's own wall clock, else UTC's.
fn reference_date(cx: &Context<'_>, lang: Lang, now: i64) -> Date {
    reference(cx, lang, now).map_or_else(
        || {
            cx.now()
                .local_date()
                .unwrap_or_else(|| Date::from_days_since_epoch(now.div_euclid(86_400)))
        },
        |spot| spot.local(now).0,
    )
}

fn ask_choice(slot: &str, name: &str, options: &[Option_]) -> SkillResult {
    let choices: Vec<Choice> = options
        .iter()
        .map(|o| Choice::new(o.value.clone(), o.names.iter()))
        .collect();
    let message = match options {
        [a, b] => Message::new("which")
            .var("name", name)
            .var("a", &a.region)
            .var("b", &b.region),
        [a, b, c, ..] => Message::new("which3")
            .var("name", name)
            .var("a", &a.region)
            .var("b", &b.region)
            .var("c", &c.region),
        _ => return SkillResult::Ask(Expectation::slot(slot), Message::new("ask_place")),
    };
    SkillResult::Ask(Expectation::choice(slot, choices), message)
}

/// The choice value for one piece of a list, with the other pieces kept.
fn replace_piece(parts: &[String], index: usize, options: &mut [Option_]) {
    for option in options {
        let mut all: Vec<String> = parts.to_vec();
        all[index].clone_from(&option.value);
        option.value = all.join(" y ");
    }
}

impl Timezones {
    fn time_in(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let lang = call.lang;
        let now = cx.now().unix_seconds;
        if !Self::continues(cx, call) {
            return SkillResult::Decline;
        }
        if slot_text(call, "form") == Some("convert") {
            return SkillResult::Unable(Message::new("convert"));
        }
        let Some(text) = slot_text(call, "place") else {
            return SkillResult::Ask(Expectation::slot("place"), Message::new("ask_place"));
        };
        let parts = pieces(text);
        if parts.len() > MAX_PLACES {
            return SkillResult::Ask(Expectation::slot("place"), Message::new("too_many"));
        }
        let spots = match Self::places(cx, call, &parts, now) {
            Ok(Ok(spots)) => spots,
            Ok(Err(spain)) => {
                let reference = reference_date(cx, lang, now);
                return Self::spain(cx, lang, now, reference, spain);
            }
            Err(result) => return result,
        };
        let reference = reference_date(cx, lang, now);
        let reported: Vec<String> = spots.iter().map(|s| s.report.clone()).collect();
        match reported.as_slice() {
            [one] => cx.report("place", Value::Place(PlaceRef::Name(one.clone()))),
            _ => cx.report(
                "place",
                Value::List {
                    items: reported,
                    uncertain: false,
                },
            ),
        }
        let locals: Vec<(Date, u32)> = spots.iter().map(|s| s.local(now)).collect();
        if let ([spot], [(date, minute)]) = (spots.as_slice(), locals.as_slice())
            && slot_text(call, "what") == Some("date")
        {
            let id = match date.cmp(&reference) {
                std::cmp::Ordering::Equal => "date",
                std::cmp::Ordering::Greater => "date_ahead",
                std::cmp::Ordering::Less => "date_behind",
            };
            let (clock, at) = times(lang, *minute);
            return SkillResult::Answer(
                Message::new(id)
                    .var("place", &spot.name)
                    .var("date", full_day(lang, *date))
                    .var("clock", clock)
                    .var("at", at),
            );
        }
        let all_same = locals.windows(2).all(|w| w[0] == w[1]);
        let letters = ["a", "b", "c"];
        let mut message = Message::new(match (spots.len(), all_same) {
            (1, _) => "time",
            (2, true) => "two_same",
            (2, false) => "two",
            (_, true) => "three_same",
            (_, false) => "three",
        });
        for (i, (spot, (date, minute))) in spots.iter().zip(&locals).enumerate() {
            let (clock, at) = times(lang, *minute);
            let note = day_note(lang, *date, reference);
            if i == 0 {
                message = message
                    .var("place", &spot.name)
                    .var("clock", &clock)
                    .var("at", &at)
                    .var("note", &note);
            }
            let l = letters[i.min(2)];
            message = message
                .var(l, &spot.name)
                .var(format!("clock_{l}"), clock)
                .var(format!("at_{l}"), at)
                .var(format!("note_{l}"), note);
        }
        SkillResult::Answer(message)
    }

    /// The places of a time question, read: their spots, or Spain's name
    /// when Spain alone was asked about (`Ok(Err(…))`), or what to say
    /// instead (`Err`).
    #[allow(clippy::type_complexity, reason = "three outcomes, said above")]
    fn places(
        cx: &Context<'_>,
        call: &Call<'_>,
        parts: &[String],
        now: i64,
    ) -> Result<Result<Vec<Spot>, String>, SkillResult> {
        let lang = call.lang;
        let ask = |id: &'static str| SkillResult::Ask(Expectation::slot("place"), Message::new(id));
        let mut spots = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            match resolve(part, lang, now) {
                Found::One(spot) => spots.push(spot),
                Found::Spain(name) if parts.len() == 1 => return Ok(Err(name)),
                Found::Spain(_) => spots.extend(city_spot(&CITIES[0], lang, false)),
                Found::Home => spots.push(remembered(cx, lang, now).ok_or_else(|| ask("ask_home"))?),
                Found::Here => spots.push(reference(cx, lang, now).ok_or(SkillResult::Decline)?),
                Found::There => return Err(ask("ask_place")),
                Found::Several(country) => {
                    return Err(SkillResult::Ask(
                        Expectation::slot("place"),
                        Message::new("ask_country").var("country", country),
                    ));
                }
                Found::Choose(name, mut options) => {
                    replace_piece(parts, index, &mut options);
                    return Err(ask_choice("place", &name, &options));
                }
                Found::Unknown(_)
                    if slot_text(call, "follow").is_none() && slot_text(call, "loose").is_none() =>
                {
                    return Err(SkillResult::Unable(Message::new("unknown")));
                }
                Found::Unknown(_) | Found::Unreadable => return Err(SkillResult::Decline),
            }
        }
        Ok(Ok(spots))
    }

    /// Spain: the peninsula and the Canary Islands.
    fn spain(cx: &mut Context<'_>, lang: Lang, now: i64, reference: Date, name: String) -> SkillResult {
        cx.report("place", Value::Place(PlaceRef::Name(name)));
        let mut message = Message::new("spain");
        for (l, zone) in [("a", "Europe/Madrid"), ("b", "Atlantic/Canary")] {
            let Some(rule) = zone_rule(zone) else {
                return SkillResult::Unable(Message::new("unknown"));
            };
            let spot = Spot {
                name: String::new(),
                report: String::new(),
                rule,
            };
            let (date, minute) = spot.local(now);
            let (clock, at) = times(lang, minute);
            message = message
                .var(format!("clock_{l}"), clock)
                .var(format!("at_{l}"), at)
                .var(format!("note_{l}"), day_note(lang, date, reference));
        }
        SkillResult::Answer(message)
    }

    /// A follow-up ("¿y en Londres?") continues only a question of this
    /// skill.
    fn continues(cx: &Context<'_>, call: &Call<'_>) -> bool {
        if slot_text(call, "follow").is_none() {
            return true;
        }
        let last = cx.conversation().last_intent().map(talos_core::IntentId::as_str);
        match call.intent.as_str() {
            "timezones.difference" => last == Some("timezones.difference"),
            _ => last.is_some_and(|i| i.starts_with("timezones.")),
        }
    }

    /// Reads one side of a difference: a spot, or what to say instead.
    fn side(
        cx: &Context<'_>,
        call: &Call<'_>,
        slot: &str,
        text: Option<&str>,
        now: i64,
    ) -> Result<Spot, SkillResult> {
        let lang = call.lang;
        let missing = |id: &'static str| SkillResult::Ask(Expectation::slot(slot), Message::new(id));
        let Some(text) = text else {
            return match reference(cx, lang, now) {
                Some(place) => Ok(place),
                None => Err(missing(if slot == "to" { "ask_to" } else { "ask_from" })),
            };
        };
        match resolve(text, lang, now) {
            Found::One(place) => Ok(place),
            Found::Spain(name) => {
                let mut madrid = city_spot(&CITIES[0], lang, false).ok_or(SkillResult::Decline)?;
                madrid.report = name;
                Ok(madrid)
            }
            Found::Home => remembered(cx, lang, now).ok_or_else(|| missing("ask_home")),
            Found::Here | Found::There => reference(cx, lang, now).ok_or_else(|| missing("ask_from_here")),
            Found::Several(country) => Err(SkillResult::Ask(
                Expectation::slot(slot),
                Message::new("ask_country_city").var("country", country),
            )),
            Found::Choose(name, options) => Err(ask_choice(slot, &name, &options)),
            // A phrasing that only says "diferencia" ("qué diferencia hay
            // entre un sapo y una rana") is `loose`: a name that is no place
            // means it was not about time at all.
            Found::Unknown(name)
                if ["form", "follow", "loose"]
                    .iter()
                    .all(|s| slot_text(call, s).is_none()) =>
            {
                Err(SkillResult::Unable(Message::new("unknown_named").var("name", name)))
            }
            Found::Unknown(_) | Found::Unreadable => Err(SkillResult::Decline),
        }
    }

    fn difference(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let lang = call.lang;
        let now = cx.now().unix_seconds;
        if !Self::continues(cx, call) {
            return SkillResult::Decline;
        }
        let (from, to) = (slot_text(call, "from"), slot_text(call, "to"));
        let (from_parts, to_parts) = (from.map(pieces), to.map(pieces));
        let (n_from, n_to) = (
            from_parts.as_ref().map_or(0, Vec::len),
            to_parts.as_ref().map_or(0, Vec::len),
        );
        // Two places in `to` are the pair (an answer to "which two?", whatever
        // `from` held); any more, or a list in `from`, is too many.
        if n_to > 2 || (n_from > 1 && n_to != 2) {
            return SkillResult::Ask(Expectation::slot("to"), Message::new("ask_pair"));
        }
        let (from, to): (Option<String>, Option<String>) = match to_parts {
            Some(parts) if parts.len() == 2 => (Some(parts[0].clone()), Some(parts[1].clone())),
            _ => (from.map(str::to_owned), to.map(str::to_owned)),
        };
        let Some(to) = to else {
            return SkillResult::Ask(Expectation::slot("to"), Message::new("ask_to"));
        };
        let b = match Self::side(cx, call, "to", Some(&to), now) {
            Ok(spot) => spot,
            Err(result) => return result,
        };
        let a = match Self::side(cx, call, "from", from.as_deref(), now) {
            Ok(spot) => spot,
            Err(result) => return result,
        };
        cx.report("from", Value::Place(PlaceRef::Name(a.report.clone())));
        cx.report("to", Value::Place(PlaceRef::Name(b.report.clone())));
        if a == b {
            return SkillResult::Answer(Message::new("same_place"));
        }
        let gap = |t: i64| i64::from(b.offset(t) - a.offset(t)) / 60;
        let now_gap = gap(now);
        let base = Message::new("")
            .var("a", &a.name)
            .var("b", &b.name)
            .var("diff", span(lang, now_gap.abs()));
        if let Some(form) = slot_text(call, "form") {
            let id = match (form, now_gap.signum()) {
                ("same", 0) => "yes_same",
                ("same" | "ahead" | "behind", 1) if form != "ahead" => "no_ahead",
                ("same" | "ahead" | "behind", -1) if form != "behind" => "no_behind",
                ("ahead", 1) => "yes_ahead",
                ("behind", -1) => "yes_behind",
                _ => "no_same",
            };
            return SkillResult::Answer(Message { id, ..base });
        }
        SkillResult::Answer(Self::coming(lang, &a, &b, now, base))
    }

    /// The difference now, with the coming changes of summer time that
    /// change it (within [`NOTICE`]).
    fn coming(lang: Lang, a: &Spot, b: &Spot, now: i64, base: Message) -> Message {
        let gap = |t: i64| i64::from(b.offset(t) - a.offset(t)) / 60;
        let now_gap = gap(now);
        let mut changes: Vec<(i64, &Spot)> = a
            .rule
            .changes(now, now + NOTICE)
            .into_iter()
            .map(|(t, _)| (t, a))
            .chain(b.rule.changes(now, now + NOTICE).into_iter().map(|(t, _)| (t, b)))
            .collect();
        changes.sort_by_key(|(t, _)| *t);
        let mut steps: Vec<(Date, i64)> = Vec::new();
        let mut last = now_gap;
        for (t, spot) in changes {
            let next = gap(t);
            if next != last {
                steps.push((spot.local(t).0, next));
                last = next;
            }
        }
        let side = |g: i64| match g.signum() {
            1 => "ahead",
            -1 => "behind",
            _ => "same",
        };
        let id: &'static str = match (side(now_gap), steps.as_slice()) {
            (now_side, []) => now_side,
            ("ahead", [(_, n), (_, m), ..]) if *n > 0 && *m > 0 => "ahead_then2",
            ("behind", [(_, n), (_, m), ..]) if *n < 0 && *m < 0 => "behind_then2",
            ("ahead", [(_, n), ..]) => match side(*n) {
                "ahead" => "ahead_then",
                "behind" => "ahead_then_behind",
                _ => "ahead_then_same",
            },
            ("behind", [(_, n), ..]) => match side(*n) {
                "behind" => "behind_then",
                "ahead" => "behind_then_ahead",
                _ => "behind_then_same",
            },
            (_, [(_, n), ..]) => {
                if *n > 0 {
                    "same_then_ahead"
                } else {
                    "same_then_behind"
                }
            }
        };
        let mut message = Message { id, ..base };
        if let Some((date, n)) = steps.first() {
            let flips = side(*n) != side(now_gap);
            message = message
                .var("when", day_month(lang, *date))
                .var("after", if flips { span(lang, n.abs()) } else { after(lang, n.abs(), true) });
        }
        if let Some((date, n)) = steps.get(1) {
            message = message
                .var("when2", day_month(lang, *date))
                .var("after2", after(lang, n.abs(), false));
        }
        message
    }
}

impl Skill for Timezones {
    fn id(&self) -> &'static str {
        "timezones"
    }

    fn replies(&self) -> &'static [&'static str] {
        &[
            "time",
            "two",
            "two_same",
            "three",
            "three_same",
            "spain",
            "date",
            "date_ahead",
            "date_behind",
            "ask_place",
            "ask_home",
            "ask_country",
            "too_many",
            "unknown",
            "convert",
            "which",
            "which3",
            "ahead",
            "ahead_then",
            "ahead_then2",
            "ahead_then_same",
            "ahead_then_behind",
            "behind",
            "behind_then",
            "behind_then2",
            "behind_then_same",
            "behind_then_ahead",
            "same",
            "same_then_ahead",
            "same_then_behind",
            "same_place",
            "yes_ahead",
            "yes_behind",
            "yes_same",
            "no_ahead",
            "no_behind",
            "no_same",
            "ask_to",
            "ask_from",
            "ask_from_here",
            "ask_pair",
            "ask_country_city",
            "unknown_named",
        ]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "timezones.time_in" => Self::time_in(cx, call),
            "timezones.difference" => Self::difference(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_and_afters() {
        assert_eq!(span(Lang::Es, 90), "1 hora y media");
        assert_eq!(span(Lang::Es, 330), "5 horas y media");
        assert_eq!(span(Lang::En, 330), "5 hours 30 minutes");
        assert_eq!(span(Lang::En, 60), "1 hour");
        assert_eq!(span(Lang::Es, 345), "5 horas y 45 minutos");
        assert_eq!(after(Lang::Es, 420, true), "serán 7");
        assert_eq!(after(Lang::Es, 60, true), "será 1");
        assert_eq!(after(Lang::En, 270, true), "4 hours 30 minutes");
    }
}
