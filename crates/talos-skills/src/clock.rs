//! `clock`: the time, the date, weekdays and spans (docs/spec/basics.md).
//!
//! Everything is computed from the injected `now` in the conversation's own
//! local time; time in other places is `timezones`. Wording lives in the
//! manifest. What this file formats are quantities, as the number formatter
//! does: dates (names of weekdays and months), times, and spans of time
//! ("27 años, 2 meses y 13 días").

use talos_core::civil::{Date, Weekday, easter, is_leap_year};
use talos_core::extract::{self, TimeOfDay};
use talos_core::normalise::normalise;
use talos_core::{
    Call, Choice, Context, Expectation, Lang, Message, Problem, Skill, SkillResult, Value,
};

/// The `clock` handler.
#[derive(Debug, Default)]
pub(crate) struct Clock;

// --- Names and formats ---------------------------------------------------------

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

fn weekday_name(lang: Lang, day: Weekday) -> &'static str {
    let index = usize::from(day.days_from_monday());
    match lang {
        Lang::Es => WEEKDAYS_ES[index],
        Lang::En => WEEKDAYS_EN[index],
    }
}

fn month_name(lang: Lang, month: u8) -> &'static str {
    let index = usize::from(month.saturating_sub(1)).min(11);
    match lang {
        Lang::Es => MONTHS_ES[index],
        Lang::En => MONTHS_EN[index],
    }
}

/// "14 de marzo" / "14 March".
fn short_date(lang: Lang, date: Date) -> String {
    match lang {
        Lang::Es => format!("{} de {}", date.day(), month_name(lang, date.month())),
        Lang::En => format!("{} {}", date.day(), month_name(lang, date.month())),
    }
}

/// "14 de marzo de 2027" / "14 March 2027": always with the year.
fn written_date(lang: Lang, date: Date) -> String {
    match lang {
        Lang::Es => format!("{} de {}", short_date(lang, date), date.year()),
        Lang::En => format!("{} {}", short_date(lang, date), date.year()),
    }
}

/// "domingo, 14 de marzo de 2027" / "Sunday 14 March 2027".
fn full_date(lang: Lang, date: Date) -> String {
    let weekday = weekday_name(lang, date.weekday());
    match lang {
        Lang::Es => format!("{weekday}, {}", written_date(lang, date)),
        Lang::En => format!("{weekday} {}", written_date(lang, date)),
    }
}

/// A time of day by the language's convention (24-hour unless `twelve`):
/// "18:42", "1:05" (Spanish writes no zero before hours 1 to 9), "6:42 pm",
/// "6:42 PM". Also whether the hour as written is one, which Spanish says in
/// the singular ("Es la 1:05").
fn clock_time(lang: Lang, minute_of_day: u32, twelve: bool) -> (String, bool) {
    let (hour, minute) = (minute_of_day / 60 % 24, minute_of_day % 60);
    if twelve {
        let shown = if hour % 12 == 0 { 12 } else { hour % 12 };
        let suffix = match (lang, hour < 12) {
            (Lang::Es, true) => "am",
            (Lang::Es, false) => "pm",
            (Lang::En, true) => "AM",
            (Lang::En, false) => "PM",
        };
        return (format!("{shown}:{minute:02} {suffix}"), shown == 1);
    }
    let text = match lang {
        Lang::Es if (1..10).contains(&hour) => format!("{hour}:{minute:02}"),
        _ => format!("{hour:02}:{minute:02}"),
    };
    (text, hour == 1)
}

/// A unit of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Year,
    Month,
    Day,
    Hour,
    Minute,
}

impl Unit {
    fn name(self, lang: Lang, many: bool) -> &'static str {
        match (lang, self, many) {
            (Lang::Es, Unit::Year, false) => "año",
            (Lang::Es, Unit::Year, true) => "años",
            (Lang::Es, Unit::Month, false) => "mes",
            (Lang::Es, Unit::Month, true) => "meses",
            (Lang::Es, Unit::Day, false) => "día",
            (Lang::Es, Unit::Day, true) => "días",
            (Lang::Es, Unit::Hour, false) => "hora",
            (Lang::Es, Unit::Hour, true) => "horas",
            (Lang::Es, Unit::Minute, false) => "minuto",
            (Lang::Es, Unit::Minute, true) => "minutos",
            (Lang::En, Unit::Year, false) => "year",
            (Lang::En, Unit::Year, true) => "years",
            (Lang::En, Unit::Month, false) => "month",
            (Lang::En, Unit::Month, true) => "months",
            (Lang::En, Unit::Day, false) => "day",
            (Lang::En, Unit::Day, true) => "days",
            (Lang::En, Unit::Hour, false) => "hour",
            (Lang::En, Unit::Hour, true) => "hours",
            (Lang::En, Unit::Minute, false) => "minute",
            (Lang::En, Unit::Minute, true) => "minutes",
        }
    }

    /// The short form of outcome lines: `3 h`.
    fn short(self) -> &'static str {
        match self {
            Unit::Year => "y",
            Unit::Month => "mo",
            Unit::Day => "d",
            Unit::Hour => "h",
            Unit::Minute => "min",
        }
    }
}

/// A span as its non-zero parts, largest unit first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span(Vec<(u32, Unit)>);

impl Span {
    fn new(parts: [(u32, Unit); 3]) -> Span {
        Span(
            parts
                .into_iter()
                .filter(|(n, _)| *n > 0)
                .collect(),
        )
    }

    /// A year or more is given as years, months and days; less, in days.
    fn between(from: Date, to: Date) -> Span {
        let (years, months, days) = from.calendar_span(to);
        if years >= 1 {
            Span::new([(years, Unit::Year), (months, Unit::Month), (days, Unit::Day)])
        } else {
            let total = u32::try_from(from.days_until(to)).unwrap_or(0);
            Span::new([(total, Unit::Day), (0, Unit::Day), (0, Unit::Day)])
        }
    }

    /// Minutes as days, hours and minutes.
    fn minutes(total: i64) -> Span {
        let total = u32::try_from(total.max(0)).unwrap_or(0);
        Span::new([
            (total / 1_440, Unit::Day),
            (total % 1_440 / 60, Unit::Hour),
            (total % 60, Unit::Minute),
        ])
    }

    /// A single unit with the value one ("1 día"): Spanish says "Falta".
    fn is_one(&self) -> bool {
        matches!(self.0.as_slice(), [(1, _)])
    }

    /// "27 años, 2 meses y 13 días" / "27 years, 2 months and 13 days".
    fn text(&self, lang: Lang) -> String {
        let parts: Vec<String> = self
            .0
            .iter()
            .map(|(n, unit)| format!("{n} {}", unit.name(lang, *n != 1)))
            .collect();
        let and = match lang {
            Lang::Es => "y",
            Lang::En => "and",
        };
        match parts.as_slice() {
            [] => String::new(),
            [only] => only.clone(),
            [init @ .., last] => format!("{} {and} {last}", init.join(", ")),
        }
    }

    /// The compact form of outcome lines: `3 h`, `2 h 30 min`.
    fn compact(&self) -> String {
        self.0
            .iter()
            .map(|(n, unit)| format!("{n} {}", unit.short()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

// --- Reading days ----------------------------------------------------------------

/// How a well-known day falls in the year.
#[derive(Debug, Clone, Copy)]
enum Rule {
    Fixed(u8, u8),
    Easter,
}

struct KnownDay {
    /// The match forms it is called by, in both languages, with no article.
    keys: &'static [&'static str],
    rule: Rule,
    es: &'static str,
    en: &'static str,
}

/// The fixed list of docs/spec/basics.md (`clock.until`).
const KNOWN_DAYS: &[KnownDay] = &[
    KnownDay {
        keys: &["ano nuevo", "new years day", "new year"],
        rule: Rule::Fixed(1, 1),
        es: "Año Nuevo",
        en: "New Year's Day",
    },
    KnownDay {
        keys: &["reyes", "dia de reyes", "epiphany", "three kings day"],
        rule: Rule::Fixed(1, 6),
        es: "Reyes",
        en: "Epiphany",
    },
    KnownDay {
        keys: &[
            "san valentin",
            "dia de san valentin",
            "valentines day",
            "valentines",
        ],
        rule: Rule::Fixed(2, 14),
        es: "San Valentín",
        en: "Valentine's Day",
    },
    KnownDay {
        keys: &["halloween"],
        rule: Rule::Fixed(10, 31),
        es: "Halloween",
        en: "Halloween",
    },
    KnownDay {
        keys: &["nochebuena", "christmas eve"],
        rule: Rule::Fixed(12, 24),
        es: "Nochebuena",
        en: "Christmas Eve",
    },
    KnownDay {
        keys: &["navidad", "dia de navidad", "christmas", "christmas day"],
        rule: Rule::Fixed(12, 25),
        es: "Navidad",
        en: "Christmas",
    },
    KnownDay {
        keys: &["nochevieja", "new years eve"],
        rule: Rule::Fixed(12, 31),
        es: "Nochevieja",
        en: "New Year's Eve",
    },
    KnownDay {
        keys: &[
            "pascua",
            "domingo de pascua",
            "domingo de resurreccion",
            "easter",
            "easter sunday",
        ],
        rule: Rule::Easter,
        es: "el Domingo de Resurrección",
        en: "Easter Sunday",
    },
];

impl KnownDay {
    fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Es => self.es,
            Lang::En => self.en,
        }
    }

    /// The date in `year`, if it has one.
    fn in_year(&self, year: i32) -> Option<Date> {
        match self.rule {
            Rule::Fixed(month, day) => Date::new(year, month, day),
            Rule::Easter => Some(easter(year)),
        }
    }

    /// The next occurrence on or after `today`, or with `since` the most
    /// recent one on or before it.
    fn occurrence(&self, today: Date, since: bool) -> Option<Date> {
        let years = today.year() - 2..=today.year() + 2;
        if since {
            years
                .rev()
                .filter_map(|y| self.in_year(y))
                .find(|d| *d <= today)
        } else {
            years.filter_map(|y| self.in_year(y)).find(|d| *d >= today)
        }
    }
}

/// Words that open a day phrase without being part of it.
const LEAD_WORDS: &[&str] = &[
    "el", "la", "los", "las", "para", "hasta", "desde", "a", "the", "on", "for", "until", "till",
    "since", "by",
];

const EDGE: &[char] = &['.', ',', ';', ':', '!', '?', '¡', '¿', '"', '\''];

/// The phrase without leading articles and prepositions or the apostrophes
/// of "New Year's", for looking a day up by name.
fn lookup_key(folded: &str) -> String {
    let words: Vec<&str> = folded
        .split_whitespace()
        .map(|w| w.trim_matches(EDGE))
        .filter(|w| !w.is_empty())
        .collect();
    let skip = words.iter().take_while(|w| LEAD_WORDS.contains(w)).count();
    words[skip..].join(" ").replace('\'', "")
}

/// A weekday name alone, in either language.
fn bare_weekday(key: &str) -> Option<Weekday> {
    let spanish = ["lunes", "martes", "miercoles", "jueves", "viernes", "sabado", "domingo"];
    let english = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    spanish
        .iter()
        .position(|n| *n == key)
        .or_else(|| english.iter().position(|n| *n == key))
        .map(|i| Weekday::ALL[i])
}

/// A day phrase that was read.
struct Resolved {
    date: Date,
    /// The well-known day it named, for the reply.
    known: Option<&'static KnownDay>,
    /// The weekday, when the phrase was just a weekday name.
    bare: Option<Weekday>,
}

/// Why a phrase did not become a date.
enum Bad {
    Unreadable,
    /// "31 de abril": the user's words without the article, and the year
    /// when the problem is a 29 February in a year that is not leap.
    NoSuch { what: String, leap: Option<i32> },
    /// English `3/4`: both readings.
    Ambiguous(Date, Date),
}

/// The year of a "29 February" in a year that has none, from the match form.
fn leap_trouble(folded: &str) -> Option<i32> {
    let tokens: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let february = tokens.iter().any(|t| matches!(*t, "febrero" | "february"))
        || matches!(tokens.get(..2), Some(["29", "2" | "02"] | ["2" | "02", "29"]));
    let year = tokens
        .iter()
        .find(|t| t.len() == 4 && t.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|t| t.parse::<i32>().ok())?;
    (tokens.contains(&"29") && february && !is_leap_year(year)).then_some(year)
}

/// The user's phrase for a date that does not exist, with no article.
fn without_lead(original: &str) -> String {
    let words: Vec<&str> = original.split_whitespace().collect();
    let skip = words
        .iter()
        .take_while(|w| LEAD_WORDS.contains(&normalise(w).folded.as_str()))
        .count();
    words[skip..].join(" ").trim_matches(EDGE).to_owned()
}

/// Reads a phrase that names a day: a well-known day, or whatever
/// [`extract::date`] reads. With `since` it looks back.
fn resolve(text: &str, lang: Lang, today: Date, since: bool) -> Result<Resolved, Bad> {
    let folded = normalise(text).folded;
    let key = lookup_key(&folded);
    if let Some(known) = KNOWN_DAYS.iter().find(|d| d.keys.contains(&key.as_str())) {
        let date = known.occurrence(today, since).ok_or(Bad::Unreadable)?;
        return Ok(Resolved {
            date,
            known: Some(known),
            bare: None,
        });
    }
    // The extractor reads "el día 3", not a bare "el 3" in Spanish.
    let phrase = if lang == Lang::Es
        && (1..=2).contains(&key.len())
        && key.bytes().all(|b| b.is_ascii_digit())
    {
        format!("dia {key}")
    } else {
        folded.clone()
    };
    let read = if since {
        extract::date_since(&phrase, lang, today)
    } else {
        extract::date(&phrase, lang, today)
    };
    match read {
        Ok(Value::Date { date, .. }) => Ok(Resolved {
            date,
            known: None,
            bare: bare_weekday(&key),
        }),
        Ok(_) | Err(Problem::Unreadable) => Err(Bad::Unreadable),
        Err(Problem::DoesNotExist) => Err(Bad::NoSuch {
            what: without_lead(text),
            leap: leap_trouble(&folded),
        }),
        Err(Problem::Ambiguous(readings)) => {
            let dates: Vec<Date> = readings.iter().filter_map(Value::as_date).collect();
            match dates.as_slice() {
                [a, b] => Err(Bad::Ambiguous(*a, *b)),
                _ => Err(Bad::Unreadable),
            }
        }
    }
}

/// One reading of an ambiguous date, as an option of the question: it fills
/// the slot with the ISO date, which reads the same in both languages, and
/// is named by its day and month in either ("4 March", "March 4", "4 de
/// marzo").
fn reading(date: Date) -> Choice {
    let (es, en) = (short_date(Lang::Es, date), short_date(Lang::En, date));
    let us = format!("{} {}", month_name(Lang::En, date.month()), date.day());
    Choice::new(date.to_string(), [en, us, es])
}

impl Bad {
    /// The reply for it, for the date in `slot`; `unreadable` is the hint
    /// of the asking intent. Two readings are asked about (conversation.md
    /// 3.4).
    fn result(self, lang: Lang, slot: &str, unreadable: &'static str) -> SkillResult {
        match self {
            Bad::Unreadable => SkillResult::Unable(Message::new(unreadable)),
            Bad::NoSuch {
                what,
                leap: Some(year),
            } => SkillResult::Answer(
                Message::new("no_such_leap")
                    .var("what", what)
                    .var("year", year.to_string()),
            ),
            Bad::NoSuch { what, leap: None } => {
                SkillResult::Answer(Message::new("no_such_date").var("what", what))
            }
            Bad::Ambiguous(first, second) => SkillResult::Ask(
                Expectation::choice(slot, vec![reading(first), reading(second)]),
                Message::new("ambiguous")
                    .var("first", short_date(lang, first))
                    .var("second", short_date(lang, second)),
            ),
        }
    }
}

fn slot_text<'a>(call: &'a Call<'_>, name: &str) -> Option<&'a str> {
    call.slots.get(name).and_then(Value::as_text)
}

/// Whether an unreadable date phrase names a place instead ("qué día es en
/// Tokio", "what's the date in Sydney"): that is `timezones.time_in`, so the
/// rule declines it.
fn names_a_place(text: &str) -> bool {
    let folded = normalise(text).folded;
    let words: Vec<&str> = folded.split_whitespace().collect();
    let words = match words.as_slice() {
        ["hoy" | "today", rest @ ..] => rest,
        all => all,
    };
    match words {
        ["en" | "in", next, ..] => {
            !folded.chars().any(|c| c.is_ascii_digit())
                && !matches!(
                    *next,
                    "un" | "una"
                        | "unos"
                        | "unas"
                        | "a"
                        | "an"
                        | "formato"
                        | "format"
                        | "iso"
                        | "medio"
                        | "media"
                        | "half"
                )
        }
        _ => false,
    }
}

fn dated(date: Date) -> Value {
    Value::Date {
        date,
        year_given: true,
    }
}

// --- The intents ---------------------------------------------------------------

impl Clock {
    fn now(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let Some(seconds) = cx.now().local_seconds_of_day() else {
            return SkillResult::Unable(Message::new("no_clock"));
        };
        let lang = call.lang;
        let minute = i64::from(seconds / 60);
        let twelve = slot_text(call, "format") == Some("12");
        let at = |total: i64| clock_time(lang, u32::try_from(total.rem_euclid(1_440)).unwrap_or(0), twelve);
        let Some(offset) = slot_text(call, "offset") else {
            let (time, one) = at(minute);
            let id = match minute {
                0 => "time_midnight",
                720 => "time_noon",
                _ if one => "time_one",
                _ => "time",
            };
            return SkillResult::Answer(Message::new(id).var("time", time));
        };
        let Ok(minutes) = extract::duration_minutes(&normalise(offset).folded, lang) else {
            return SkillResult::Unable(Message::new("bad_offset"));
        };
        let span = Span::minutes(minutes);
        cx.report("offset", Value::Text(span.compact()));
        let total = minute + minutes;
        let (time, one) = at(total);
        let id = match (total.div_euclid(1_440), one) {
            (0, false) => "offset",
            (0, true) => "offset_one",
            (1, false) => "offset_tomorrow",
            (1, true) => "offset_tomorrow_one",
            (_, false) => "offset_later",
            (_, true) => "offset_later_one",
        };
        let date = cx.now().date_or_utc().add_days(total.div_euclid(1_440));
        SkillResult::Answer(
            Message::new(id)
                .var("span", span.text(lang))
                .var("time", time)
                .var("date", full_date(lang, date)),
        )
    }

    fn date(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let lang = call.lang;
        let today = cx.now().date_or_utc();
        if let Some(part) = slot_text(call, "part") {
            return match normalise(part).folded.as_str() {
                "ano" | "year" => SkillResult::Answer(
                    Message::new("date_year").var("year", today.year().to_string()),
                ),
                _ => SkillResult::Answer(
                    Message::new("date_month").var("month", month_name(lang, today.month())),
                ),
            };
        }
        let date = match slot_text(call, "day") {
            None => today,
            Some(text) => match resolve(text, lang, today, false) {
                Ok(found) => {
                    cx.report("day", dated(found.date));
                    found.date
                }
                Err(Bad::Unreadable) if names_a_place(text) => return SkillResult::Decline,
                Err(bad) => return bad.result(lang, "day", "bad_date"),
            },
        };
        if slot_text(call, "format") == Some("iso") {
            return SkillResult::Answer(Message::new("date_iso").var("iso", date.to_string()));
        }
        let id = match today.days_until(date) {
            0 if slot_text(call, "form") == Some("stand") => "date_stand",
            0 => "date_today",
            1 => "date_tomorrow",
            -1 => "date_yesterday",
            d if d > 0 => "date_future",
            _ => "date_past",
        };
        SkillResult::Answer(
            Message::new(id)
                .var("full", full_date(lang, date))
                .var("date", written_date(lang, date)),
        )
    }

    fn weekday(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let lang = call.lang;
        let today = cx.now().date_or_utc();
        let date = match slot_text(call, "date") {
            None if slot_text(call, "birth").is_some() => {
                return SkillResult::Unable(Message::new("unknown_birth"));
            }
            None => today,
            Some(text) => match resolve(text, lang, today, false) {
                Ok(found) => {
                    cx.report("date", dated(found.date));
                    found.date
                }
                Err(Bad::Unreadable) if names_a_place(text) => return SkillResult::Decline,
                Err(bad) => return bad.result(lang, "date", "bad_date"),
            },
        };
        let id = match today.days_until(date) {
            0 => "weekday_today",
            1 => "weekday_tomorrow",
            -1 => "weekday_yesterday",
            d if d > 0 => "weekday_future",
            // Before the Gregorian reform of 1582 the calendar was another one.
            _ if date.year() < 1583 => "weekday_past_proleptic",
            _ => "weekday_past",
        };
        SkillResult::Answer(
            Message::new(id)
                .var("weekday", weekday_name(lang, date.weekday()))
                .var("short", short_date(lang, date))
                .var("date", written_date(lang, date)),
        )
    }

    fn until(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let lang = call.lang;
        let Some(target) = slot_text(call, "target") else {
            return SkillResult::Ask(Expectation::slot("target"), Message::new("ask_until"));
        };
        let since = slot_text(call, "dir") == Some("since");
        let today = cx.now().date_or_utc();
        let folded = normalise(target).folded;
        if let Some((day, at)) = time_target(&folded, lang) {
            return Self::until_time(cx, call, since, day, at);
        }
        let found = match resolve(target, lang, today, since) {
            Ok(found) => found,
            // "mi cumpleaños": a day only the user knows, so it is asked.
            Err(Bad::Unreadable)
                if folded
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| matches!(w, "cumpleanos" | "cumple" | "birthday")) =>
            {
                return SkillResult::Ask(Expectation::slot("target"), Message::new("ask_birthday"));
            }
            Err(bad) => return bad.result(lang, "target", "usage_until"),
        };
        let date = found.date;
        cx.report("target", dated(date));
        let diff = today.days_until(date);
        let full = full_date(lang, date);
        if let Some(weekday) = found.bare
            && !since
            && diff == 7
            && today.weekday() == weekday
        {
            return SkillResult::Answer(
                Message::new("until_weekday_today")
                    .var("weekday", weekday_name(lang, weekday))
                    .var("span", Span::between(today, date).text(lang))
                    .var("date", written_date(lang, date)),
            );
        }
        if diff == 0 {
            return SkillResult::Answer(Message::new("until_today").var("full", full));
        }
        let name = found.known.map(|k| k.name(lang));
        let (span, id) = if diff > 0 {
            let span = Span::between(today, date);
            let id = match (name.is_some(), span.is_one()) {
                (false, false) => "ahead",
                (false, true) => "ahead_one",
                (true, false) => "ahead_named",
                (true, true) => "ahead_named_one",
            };
            (span, id)
        } else if since {
            let id = if name.is_some() { "since_named" } else { "since" };
            (Span::between(date, today), id)
        } else {
            (Span::between(date, today), "ago")
        };
        let mut message = Message::new(id)
            .var("span", span.text(lang))
            .var("full", full);
        if let Some(name) = name {
            message = message.var("name", name);
        }
        SkillResult::Answer(message)
    }

    /// How long until a time of day, on an explicit day or the next time it
    /// comes round.
    fn until_time(
        cx: &mut Context<'_>,
        call: &Call<'_>,
        since: bool,
        day: Option<i64>,
        at: TimeOfDay,
    ) -> SkillResult {
        let lang = call.lang;
        if since {
            return SkillResult::Unable(Message::new("since_time"));
        }
        let Some(seconds) = cx.now().local_seconds_of_day() else {
            return SkillResult::Unable(Message::new("no_clock"));
        };
        let today = cx.now().date_or_utc();
        let now = i64::from(seconds / 60);
        let (days, minute) = if let Some(days) = day {
            (days, i64::from(at.hour) * 60 + i64::from(at.minute))
        } else {
            let (days, minute) = at.next_after(u32::try_from(now).unwrap_or(0));
            (i64::from(days), i64::from(minute))
        };
        let date = today.add_days(days);
        cx.report(
            "target",
            Value::Text(format!("{date} {:02}:{:02}", minute / 60, minute % 60)),
        );
        let remaining = days * 1_440 + minute - now;
        let (time, _) = clock_time(lang, u32::try_from(minute).unwrap_or(0), false);
        if remaining < 0 {
            return SkillResult::Answer(
                Message::new("ago").var("span", Span::minutes(-remaining).text(lang)),
            );
        }
        if remaining == 0 {
            return SkillResult::Answer(
                Message::new("until_today").var("full", full_date(lang, date)),
            );
        }
        let span = Span::minutes(remaining);
        let one = span.is_one();
        let id = match (days, one) {
            (0, false) => "time_ahead",
            (0, true) => "time_ahead_one",
            (1, false) => "time_ahead_tomorrow",
            (1, true) => "time_ahead_tomorrow_one",
            (_, false) => "time_ahead_date",
            (_, true) => "time_ahead_date_one",
        };
        SkillResult::Answer(
            Message::new(id)
                .var("span", span.text(lang))
                .var("time", time)
                .var("full", full_date(lang, date)),
        )
    }
}

/// A target that is a time of day, alone or on a named day ("mañana a las
/// 5", "tomorrow afternoon"): the explicit day offset, if any, and the time.
fn time_target(folded: &str, lang: Lang) -> Option<(Option<i64>, TimeOfDay)> {
    if let Ok(at) = extract::time_of_day(folded, lang) {
        return Some((None, at));
    }
    let words: Vec<&str> = folded.split_whitespace().collect();
    let (days, rest): (i64, &[&str]) = match (lang, words.as_slice()) {
        (Lang::Es, ["hoy", rest @ ..]) | (Lang::En, ["today", rest @ ..]) => (0, rest),
        (Lang::Es, ["manana", rest @ ..]) | (Lang::En, ["tomorrow", rest @ ..]) => (1, rest),
        (Lang::Es, ["pasado", "manana", rest @ ..])
        | (Lang::En, ["the", "day", "after", "tomorrow", rest @ ..]) => (2, rest),
        _ => return None,
    };
    if rest.is_empty() {
        return None;
    }
    let at = extract::time_of_day(&rest.join(" "), lang).ok()?;
    Some((Some(days), at))
}

impl Skill for Clock {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn replies(&self) -> &'static [&'static str] {
        &[
            "time",
            "time_one",
            "time_midnight",
            "time_noon",
            "offset",
            "offset_one",
            "offset_tomorrow",
            "offset_tomorrow_one",
            "offset_later",
            "offset_later_one",
            "no_clock",
            "bad_offset",
            "date_today",
            "date_stand",
            "date_tomorrow",
            "date_yesterday",
            "date_future",
            "date_past",
            "date_year",
            "date_month",
            "date_iso",
            "no_such_date",
            "no_such_leap",
            "bad_date",
            "ambiguous",
            "weekday_today",
            "weekday_tomorrow",
            "weekday_yesterday",
            "weekday_future",
            "weekday_past",
            "weekday_past_proleptic",
            "unknown_birth",
            "ahead",
            "ahead_one",
            "ahead_named",
            "ahead_named_one",
            "until_today",
            "until_weekday_today",
            "since",
            "since_named",
            "ago",
            "time_ahead",
            "time_ahead_one",
            "time_ahead_tomorrow",
            "time_ahead_tomorrow_one",
            "time_ahead_date",
            "time_ahead_date_one",
            "usage_until",
            "ask_until",
            "ask_birthday",
            "since_time",
        ]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "clock.now" => Self::now(cx, call),
            "clock.date" => Self::date(cx, call),
            "clock.weekday" => Self::weekday(cx, call),
            "clock.until" => Self::until(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{IntentId, Random, Slots, Timestamp};

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    /// A local instant in UTC+1.
    fn at(day: Date, hour: u32, minute: u32) -> Timestamp {
        Timestamp::from_local(day, hour * 3_600 + minute * 60, 3_600)
    }

    fn run(
        intent: &str,
        lang: Lang,
        slots: &[(&str, &str)],
        now: Timestamp,
    ) -> SkillResult {
        let intent = IntentId(intent.to_owned());
        let slots: Slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::Text((*v).to_owned())))
            .collect();
        let mut cx = Context::standalone(now, Random::seeded(0));
        let call = Call {
            intent: &intent,
            lang,
            slots: &slots,
            text: "x",
        };
        Clock.handle(&mut cx, &call)
    }

    fn message(result: &SkillResult) -> &Message {
        match result {
            SkillResult::Answer(m) | SkillResult::Unable(m) | SkillResult::Ask(_, m) => m,
            other => panic!("{other:?}"),
        }
    }

    fn id(result: &SkillResult) -> &'static str {
        message(result).id
    }

    fn var(result: &SkillResult, name: &str) -> String {
        message(result)
            .vars
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn times_follow_the_languages_convention() {
        let time = |lang, minute, twelve| clock_time(lang, minute, twelve);
        assert_eq!(time(Lang::Es, 18 * 60 + 42, false), ("18:42".into(), false));
        assert_eq!(time(Lang::Es, 65, false), ("1:05".into(), true));
        assert_eq!(time(Lang::Es, 13 * 60 + 7, false), ("13:07".into(), false));
        assert_eq!(time(Lang::Es, 0, false), ("00:00".into(), false));
        assert_eq!(time(Lang::En, 2 * 60 + 42, false), ("02:42".into(), false));
        assert_eq!(time(Lang::En, 18 * 60 + 42, true), ("6:42 PM".into(), false));
        assert_eq!(time(Lang::En, 0, true), ("12:00 AM".into(), false));
        assert_eq!(time(Lang::Es, 13 * 60 + 5, true), ("1:05 pm".into(), true));
    }

    #[test]
    fn spans_are_written_in_the_language() {
        let span = Span::between(date(2000, 1, 1), date(2027, 3, 14));
        assert_eq!(span.text(Lang::Es), "27 años, 2 meses y 13 días");
        assert_eq!(span.text(Lang::En), "27 years, 2 months and 13 days");
        assert_eq!(span.compact(), "27 y 2 mo 13 d");
        assert!(!span.is_one());
        let day = Span::between(date(2027, 3, 14), date(2027, 3, 15));
        assert!(day.is_one());
        assert_eq!(day.text(Lang::Es), "1 día");
        // Under a year it is days, even across a leap day.
        let long = Span::between(date(2027, 3, 14), date(2028, 3, 13));
        assert_eq!(long.text(Lang::En), "365 days");
        assert_eq!(Span::minutes(90).text(Lang::Es), "1 hora y 30 minutos");
        assert_eq!(Span::minutes(1_500).text(Lang::En), "1 day and 1 hour");
        assert_eq!(Span::minutes(1_500).compact(), "1 d 1 h");
    }

    #[test]
    fn well_known_days_take_the_next_or_last_occurrence() {
        let christmas = KNOWN_DAYS.iter().find(|d| d.es == "Navidad").unwrap();
        let today = date(2027, 12, 25);
        assert_eq!(christmas.occurrence(today, false), Some(today));
        assert_eq!(christmas.occurrence(today, true), Some(today));
        assert_eq!(
            christmas.occurrence(date(2027, 12, 26), false),
            Some(date(2028, 12, 25))
        );
        assert_eq!(
            christmas.occurrence(date(2027, 12, 24), true),
            Some(date(2026, 12, 25))
        );
        let easter = KNOWN_DAYS.iter().find(|d| d.en == "Easter Sunday").unwrap();
        // Easter 2027 has passed on 29 March: the next is 2028's.
        assert_eq!(
            easter.occurrence(date(2027, 3, 29), false),
            Some(date(2028, 4, 16))
        );
        assert_eq!(
            easter.occurrence(date(2027, 3, 27), true),
            Some(date(2026, 4, 5))
        );
    }

    #[test]
    fn days_are_resolved_around_the_month_ends() {
        let resolved = |text: &str, lang, today, since| {
            resolve(text, lang, today, since)
                .ok()
                .map(|r| r.date.to_string())
        };
        let eve = date(2028, 2, 28);
        let cases = [
            ("mañana", Lang::Es, eve, false, "2028-02-29"),
            ("el 29 de febrero", Lang::Es, eve, false, "2028-02-29"),
            ("1 de marzo", Lang::Es, eve, false, "2028-03-01"),
            ("tomorrow", Lang::En, date(2027, 12, 31), false, "2028-01-01"),
            ("yesterday", Lang::En, date(2028, 1, 1), false, "2027-12-31"),
            // A bare Spanish day number is the next such day.
            ("el 31", Lang::Es, date(2027, 4, 15), false, "2027-05-31"),
            ("el 30", Lang::Es, date(2027, 3, 14), false, "2027-03-30"),
            // Looking back, the last 31st before 15 April is in March.
            ("the 31st", Lang::En, date(2027, 4, 15), true, "2027-03-31"),
        ];
        for (text, lang, today, since, expected) in cases {
            assert_eq!(
                resolved(text, lang, today, since).as_deref(),
                Some(expected),
                "{text}"
            );
        }
        assert!(matches!(
            resolve("el 31 de abril", Lang::Es, eve, false),
            Err(Bad::NoSuch { leap: None, .. })
        ));
        assert!(matches!(
            resolve("29 de febrero de 2100", Lang::Es, eve, false),
            Err(Bad::NoSuch {
                leap: Some(2100),
                ..
            })
        ));
        assert!(matches!(
            resolve("3/4", Lang::En, eve, false),
            Err(Bad::Ambiguous(..))
        ));
        assert!(matches!(
            resolve("zzz", Lang::En, eve, false),
            Err(Bad::Unreadable)
        ));
    }

    #[test]
    fn leap_trouble_needs_a_29_february_in_a_common_year() {
        assert_eq!(leap_trouble("el 29 de febrero de 2027"), Some(2027));
        assert_eq!(leap_trouble("february 29, 1900"), Some(1900));
        assert_eq!(leap_trouble("29/2/2027"), Some(2027));
        assert_eq!(leap_trouble("2/29/2027"), Some(2027));
        assert_eq!(leap_trouble("el 29 de febrero de 2028"), None);
        assert_eq!(leap_trouble("el 29 de febrero de 2000"), None);
        assert_eq!(leap_trouble("el 30 de febrero de 2027"), None);
        assert_eq!(leap_trouble("el 29 de febrero"), None);
    }

    #[test]
    fn the_clock_reads_the_local_wall_time() {
        let now = at(date(2027, 3, 14), 18, 42);
        let result = run("clock.now", Lang::Es, &[], now);
        assert_eq!(id(&result), "time");
        assert_eq!(var(&result, "time"), "18:42");
        // Without a local offset the clock cannot say.
        let result = run("clock.now", Lang::Es, &[], Timestamp::from_unix(0));
        assert!(matches!(result, SkillResult::Unable(_)));
        assert_eq!(id(&result), "no_clock");
        // 00:00 and 12:00 are marked; the seconds are dropped.
        let day = date(2027, 3, 14);
        let result = run(
            "clock.now",
            Lang::En,
            &[],
            Timestamp::from_local(day, 59, 3_600),
        );
        assert_eq!(id(&result), "time_midnight");
        let noon = Timestamp::from_local(day, 12 * 3_600 + 59, 3_600);
        let result = run("clock.now", Lang::En, &[], noon);
        assert_eq!(id(&result), "time_noon");
    }

    #[test]
    fn an_offset_marks_the_day_it_lands_on() {
        let now = at(date(2027, 12, 31), 23, 30);
        let result = run("clock.now", Lang::En, &[("offset", "in 1 hour")], now);
        assert_eq!(id(&result), "offset_tomorrow");
        assert_eq!(var(&result, "time"), "00:30");
        let result = run("clock.now", Lang::En, &[("offset", "in 48 hours")], now);
        assert_eq!(id(&result), "offset_later");
        assert_eq!(var(&result, "date"), "Sunday 2 January 2028");
        let early = at(date(2027, 3, 14), 0, 5);
        let result = run("clock.now", Lang::En, &[("offset", "in 90 minutes")], early);
        assert_eq!(id(&result), "offset_one");
        assert_eq!(var(&result, "time"), "01:35");
        let result = run("clock.now", Lang::En, &[("offset", "in a while")], now);
        assert_eq!(id(&result), "bad_offset");
    }

    #[test]
    fn dates_name_their_tense() {
        let now = at(date(2027, 3, 14), 18, 42);
        let ids = [
            ("hoy", "date_today"),
            ("mañana", "date_tomorrow"),
            ("ayer", "date_yesterday"),
            ("pasado mañana", "date_future"),
            ("anteayer", "date_past"),
            ("el 31 de abril", "no_such_date"),
        ];
        for (day, expected) in ids {
            let result = run("clock.date", Lang::Es, &[("day", day)], now);
            assert_eq!(id(&result), expected, "{day}");
        }
        let slots = [("day", "mañana"), ("format", "iso")];
        let result = run("clock.date", Lang::Es, &slots, now);
        assert_eq!(var(&result, "iso"), "2027-03-15");
        let result = run("clock.date", Lang::Es, &[("part", "año")], now);
        assert_eq!(var(&result, "year"), "2027");
    }

    #[test]
    fn a_weekday_comes_with_its_date_and_calendar() {
        let now = at(date(2027, 3, 14), 18, 42);
        let result = run("clock.weekday", Lang::En, &[("date", "July 20, 1969")], now);
        assert_eq!(id(&result), "weekday_past");
        assert_eq!(var(&result, "weekday"), "Sunday");
        let result = run("clock.weekday", Lang::En, &[("date", "1 January 1500")], now);
        assert_eq!(id(&result), "weekday_past_proleptic");
        assert_eq!(var(&result, "weekday"), "Monday");
        let result = run("clock.weekday", Lang::En, &[("birth", "yes")], now);
        assert_eq!(id(&result), "unknown_birth");
        assert!(matches!(result, SkillResult::Unable(_)));
    }

    #[test]
    fn until_counts_days_by_the_calendar_and_times_by_the_minute() {
        let now = at(date(2027, 3, 14), 18, 42);
        let until = |target: &str, dir: &[(&str, &str)]| {
            let mut slots = vec![("target", target)];
            slots.extend_from_slice(dir);
            run("clock.until", Lang::En, &slots, now)
        };
        let since = [("dir", "since")];
        assert_eq!(id(&until("Christmas", &[])), "ahead_named");
        assert_eq!(var(&until("Christmas", &[]), "span"), "286 days");
        assert_eq!(id(&until("tomorrow", &[])), "ahead_one");
        assert_eq!(id(&until("Sunday", &[])), "until_weekday_today");
        assert_eq!(id(&until("yesterday", &[])), "ago");
        assert_eq!(id(&until("yesterday", &since)), "since");
        assert_eq!(id(&until("Christmas", &since)), "since_named");
        assert_eq!(id(&until("today", &[])), "until_today");
        assert_eq!(id(&until("7pm", &[])), "time_ahead");
        assert_eq!(var(&until("7pm", &[]), "span"), "18 minutes");
        assert_eq!(id(&until("6:43 pm", &[])), "time_ahead_one");
        assert_eq!(id(&until("6:42 pm", &[])), "time_ahead_tomorrow_one");
        assert_eq!(var(&until("6:42 pm", &[]), "span"), "1 day");
        assert_eq!(id(&until("9pm", &since)), "since_time");
        assert_eq!(id(&until("my birthday", &[])), "ask_birthday");
        assert_eq!(id(&until("the match", &[])), "usage_until");
        assert_eq!(id(&until("3/4", &[])), "ambiguous");
        // A missing target is asked for.
        let result = run("clock.until", Lang::En, &[], now);
        assert!(matches!(&result, SkillResult::Ask(e, _) if e.slot == "target"));
        assert_eq!(id(&result), "ask_until");
    }

    #[test]
    fn declines_intents_it_does_not_own_and_lists_every_reply_once() {
        let result = run("clock.nope", Lang::Es, &[], Timestamp::from_unix(0));
        assert_eq!(result, SkillResult::Decline);
        assert_eq!(Clock.id(), "clock");
        let mut replies = Clock.replies().to_vec();
        replies.sort_unstable();
        replies.dedup();
        assert_eq!(replies.len(), Clock.replies().len());
    }
}
