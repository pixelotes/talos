//! Durations, datetimes and date ranges (docs/spec/conversation.md 2.2 and
//! 2.3). The text is the match form of a capture.

use super::{date, half, number, time_of_day, words};
use crate::civil::Date;
use crate::value::{Number, Problem, Value};
use crate::{Lang, Timestamp};

/// The seconds in a unit word, or `Err(months)` for a calendar unit. Units
/// of both languages are understood in either: a loanword is content
/// (conversation.md 1.4).
fn unit(word: &str) -> Option<Result<i64, i64>> {
    Some(match word {
        "segundo" | "segundos" | "seg" | "segs" | "s" | "second" | "seconds" | "sec" | "secs" => {
            Ok(1)
        }
        "minuto" | "minutos" | "min" | "mins" | "minute" | "minutes" => Ok(60),
        "hora" | "horas" | "h" | "hour" | "hours" | "hr" | "hrs" => Ok(3_600),
        "dia" | "dias" | "day" | "days" => Ok(86_400),
        "semana" | "semanas" | "week" | "weeks" => Ok(604_800),
        "fortnight" | "fortnights" => Ok(1_209_600),
        "mes" | "meses" | "month" | "months" => Err(1),
        "ano" | "anos" | "year" | "years" => Err(12),
        _ => return None,
    })
}

/// Plural unit words, which need a number before them.
const PLURAL_UNITS: &[&str] = &[
    "segundos",
    "segs",
    "seconds",
    "secs",
    "minutos",
    "mins",
    "minutes",
    "horas",
    "hours",
    "hrs",
    "dias",
    "days",
    "semanas",
    "weeks",
    "fortnights",
    "meses",
    "months",
    "anos",
    "years",
];

/// Words that make a quantity vague: "unas horas", "a few minutes".
const VAGUE: &[&str] = &[
    "unas", "unos", "algunas", "algunos", "varias", "varios", "pocas", "pocos", "muchas", "muchos",
    "some", "few", "several", "many",
];

/// The amount said before a unit: "un", "media", "tres cuartos", "un par",
/// "a couple", "90". Connectors ("de", "of", "an") have been dropped.
fn amount(words: &[&str], lang: Lang) -> Result<Number, Problem> {
    if words.iter().any(|w| VAGUE.contains(w)) {
        return Err(Problem::Unreadable);
    }
    match words {
        [] | ["un" | "una" | "a" | "an" | "one"] => Ok(Number::from_int(1)),
        ["media" | "medio" | "half"] => Ok(half()),
        ["un", "par"] | ["a", "couple"] | ["couple" | "par"] => Ok(Number::from_int(2)),
        _ => {
            let text = words.join(" ");
            number(&text, lang).or_else(|_| number(&text, lang.other()))
        }
    }
}

/// A compact token: "1h30", "1h30m", "90min", "45m", "30s", "30seg", "2h".
fn compact(token: &str) -> Option<i64> {
    let digits_end = token.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    let n: i64 = token[..digits_end].parse().ok()?;
    let rest = &token[digits_end..];
    if let Some(after) = rest.strip_prefix('h') {
        let minutes = after.strip_suffix('m').unwrap_or(after);
        let minutes: i64 = if minutes.is_empty() {
            0
        } else if minutes.len() <= 2 && minutes.bytes().all(|b| b.is_ascii_digit()) {
            minutes.parse().ok().filter(|m| *m < 60)?
        } else {
            return None;
        };
        return Some(n * 3_600 + minutes * 60);
    }
    match rest {
        "min" | "mins" | "m" => Some(n * 60),
        "s" | "seg" | "segs" | "sec" | "secs" => Some(n),
        "d" => Some(n * 86_400),
        _ => None,
    }
}

/// "1:30" as hours and minutes, in seconds.
fn clock_hours(token: &str) -> Option<i64> {
    let (h, m) = token.split_once(':')?;
    let ok = |s: &str, max: usize| {
        !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit())
    };
    if !ok(h, 3) || m.len() != 2 || !ok(m, 2) {
        return None;
    }
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    (m < 60).then_some(h * 3_600 + m * 60)
}

/// The longest duration read: about a century.
const MAX_SECONDS: i64 = 100 * 366 * 86_400;

/// Reads an amount of time: "5 minutos", "hora y media", "media hora", "un
/// cuarto de hora", "tres cuartos de hora", "una hora y cuarto", "90
/// minutos", "1h30", "1:30 h", "45 min", "30 seg", "quince días", "un par
/// de horas"; "an hour and a half", "half an hour", "a quarter of an hour",
/// "1h 30m", "30 secs", "a fortnight", "a couple of hours". A leading "dentro
/// de"/"en"/"durante"/"in"/"for" is dropped. Months and years are kept as
/// calendar months; "quince días" stays fifteen days.
///
/// # Errors
///
/// [`Problem::Unreadable`] for anything else, including a vague quantity
/// ("unas horas", "un rato", "a while", "some time"): a missing slot, not
/// a guess (docs/spec/conversation.md 2.2).
pub fn duration(text: &str, lang: Lang) -> Result<Value, Problem> {
    let all = words(text);
    let tokens: &[&str] = match all.as_slice() {
        ["dentro", "de", rest @ ..] | ["en" | "durante" | "in" | "for" | "during", rest @ ..] => {
            rest
        }
        _ => &all,
    };
    let (mut months, mut seconds) = (Number::from_int(0), Number::from_int(0));
    let mut pending: Vec<&str> = Vec::new();
    let mut last: Option<Result<i64, i64>> = None;
    let mut i = 0;
    let add = |total: Number, amount: Number, per: i64| {
        total
            .plus(
                amount
                    .times(Number::from_int(per))
                    .ok_or(Problem::Unreadable)?,
            )
            .ok_or(Problem::Unreadable)
    };
    while i < tokens.len() {
        let word = tokens[i];
        i += 1;
        if pending.is_empty()
            && let Some(s) = compact(word)
        {
            seconds = add(seconds, Number::from_int(s), 1)?;
            last = Some(Ok(60));
            continue;
        }
        if pending.is_empty()
            && let Some(s) = clock_hours(word)
            && tokens.get(i).is_some_and(|u| unit(u) == Some(Ok(3_600)))
        {
            seconds = add(seconds, Number::from_int(s), 1)?;
            last = Some(Ok(3_600));
            i += 1;
            continue;
        }
        if let Some(u) = unit(word) {
            while let [.., "de" | "of" | "an" | "a"] = pending.as_slice() {
                if pending.len() == 1 && matches!(pending[0], "a" | "an") {
                    break;
                }
                pending.pop();
            }
            if let ["a" | "an", rest @ ..] = pending.as_slice()
                && !rest.is_empty()
            {
                pending.remove(0);
            }
            if pending.is_empty() && PLURAL_UNITS.contains(&word) {
                // "horas" alone: some hours, a vague quantity.
                return Err(Problem::Unreadable);
            }
            let n = amount(&pending, lang)?;
            match u {
                Ok(per) => seconds = add(seconds, n, per)?,
                Err(per) => months = add(months, n, per)?,
            }
            last = Some(u);
            pending.clear();
            continue;
        }
        if pending.is_empty() && last.is_some() && matches!(word, "y" | "and") {
            continue;
        }
        pending.push(word);
    }
    // "hora y media", "an hour and a half", "una hora y cuarto".
    if !pending.is_empty() {
        let fraction = match pending.as_slice() {
            ["media" | "medio" | "half"] | ["a", "half"] => half(),
            ["cuarto"] | ["a", "quarter"] => Number::new(25, 2).ok_or(Problem::Unreadable)?,
            _ => return Err(Problem::Unreadable),
        };
        match last {
            Some(Ok(per)) => seconds = add(seconds, fraction, per)?,
            Some(Err(per)) => months = add(months, fraction, per)?,
            None => return Err(Problem::Unreadable),
        }
    }
    if last.is_none() {
        return Err(Problem::Unreadable);
    }
    let months = months.to_i64().ok_or(Problem::Unreadable)?;
    let seconds = seconds.to_i64().ok_or(Problem::Unreadable)?;
    if months < 0 || seconds < 0 || months + seconds == 0 || seconds > MAX_SECONDS || months > 1_200
    {
        return Err(Problem::Unreadable);
    }
    Ok(Value::Duration { months, seconds })
}

/// The turn's local date and minute of the day (UTC when the offset is not
/// known).
fn local_now(now: Timestamp) -> (Date, u32) {
    let seconds = now
        .local_seconds_of_day()
        .unwrap_or_else(|| u32::try_from(now.unix_seconds.rem_euclid(86_400)).unwrap_or(0));
    (now.date_or_utc(), seconds / 60)
}

/// A time of day read from `text`, on `day` when one was named, else at
/// its next occurrence after `now_minute`.
fn at(day: Option<Date>, today: Date, now_minute: u32, text: &str, lang: Lang) -> Option<Value> {
    let time = time_of_day(text, lang).ok()?;
    let said = u32::from(time.hour) * 60 + u32::from(time.minute);
    let (ahead, next) = time.next_after(now_minute);
    let (date, minute_of_day) = match day {
        None => (today.add_days(i64::from(ahead)), next),
        // "hoy a las 7" at 18:42 is 19:00; "hoy a las 9" is this morning.
        Some(day) if day == today && ahead == 0 => (day, next),
        // Another day: a bare hour is read as said.
        Some(day) => (day, said),
    };
    Some(Value::DateTime {
        date,
        hour: u8::try_from(minute_of_day / 60).ok()?,
        minute: u8::try_from(minute_of_day % 60).ok()?,
    })
}

/// Reads a date, a time of day, or both, against the turn's `now`:
/// "mañana por la tarde", "las 3 y media", "el lunes a las 5 de la tarde",
/// "hoy a las 15:30", "esta noche", "dentro de 2 horas"; "tomorrow morning",
/// "half past three", "next Friday at 9 pm", "tonight", "in 20 minutes".
///
/// A phrase that names only a day is a [`Value::Date`] (read as
/// [`date`](super::date) reads it); otherwise a [`Value::DateTime`]. A bare
/// hour today is the next time that reading occurs ("las 5" at 18:42 is
/// 05:00 tomorrow); on another day it is read as said. A day part alone is
/// read at its representative time (morning 09:00, afternoon 17:00, night
/// 21:00, small hours 03:00).
///
/// # Errors
///
/// As [`date`](super::date) when the phrase is a date (ambiguous `3/4` in
/// English, "31 de abril"); [`Problem::Unreadable`] otherwise.
pub fn datetime(text: &str, lang: Lang, now: Timestamp) -> Result<Value, Problem> {
    let (today, now_minute) = local_now(now);
    match date(text, lang, today) {
        Err(Problem::Unreadable) => {}
        found => return found,
    }
    let tokens = words(text);
    if matches!(tokens.as_slice(), ["dentro", "de", ..] | ["en" | "in", ..])
        && let Ok(Value::Duration { months, seconds }) = duration(text, lang)
    {
        let day = today.add_months(months).ok_or(Problem::Unreadable)?;
        let total = i64::from(now_minute) * 60 + seconds;
        let date = day.add_days(total.div_euclid(86_400));
        let minute = total.rem_euclid(86_400) / 60;
        return Ok(Value::DateTime {
            date,
            hour: u8::try_from(minute / 60).map_err(|_| Problem::Unreadable)?,
            minute: u8::try_from(minute % 60).map_err(|_| Problem::Unreadable)?,
        });
    }
    // "esta tarde", "this afternoon", "tonight" name today.
    let today_named = matches!(tokens.as_slice(), ["esta" | "this", _] | ["tonight"]);
    if let Some(found) = at(today_named.then_some(today), today, now_minute, text, lang) {
        return Ok(found);
    }
    for split in 1..tokens.len() {
        let (left, right) = (tokens[..split].join(" "), tokens[split..].join(" "));
        for (day_text, time_text) in [(&left, &right), (&right, &left)] {
            if let Ok(Value::Date { date: day, .. }) = date(day_text, lang, today)
                && let Some(found) = at(Some(day), today, now_minute, time_text, lang)
            {
                return Ok(found);
            }
        }
    }
    Err(Problem::Unreadable)
}

/// The Monday of the week `date` is in.
fn monday(date: Date) -> Date {
    date.add_days(-i64::from(date.weekday().days_from_monday()))
}

/// Reads a span of days: "hoy" is not one; "esta semana", "la semana que
/// viene", "la semana pasada", "este mes", "el mes que viene", "el finde",
/// "este fin de semana", "el finde que viene", "este año"; "this week",
/// "next week", "last week", "this month", "next month", "the weekend",
/// "next weekend", "this year". Weeks run Monday to Sunday
/// (docs/spec/basics.md, clock conventions); a weekend is its Saturday and
/// Sunday.
///
/// # Errors
///
/// [`Problem::Unreadable`] for anything that is not one of those spans.
pub fn daterange(text: &str, lang: Lang, today: Date) -> Result<Value, Problem> {
    let all = words(text);
    let tokens: Vec<&str> = all
        .iter()
        .copied()
        .skip_while(|w| {
            matches!(
                *w,
                "para" | "hasta" | "durante" | "for" | "during" | "until"
            )
        })
        .collect();
    // (unit, offset): unit 0 week, 1 weekend, 2 month, 3 year.
    let (unit, offset): (u8, i64) = match (lang, tokens.as_slice()) {
        (Lang::Es, ["esta", "semana"]) | (Lang::En, ["this", "week"]) => (0, 0),
        (
            Lang::Es,
            ["la", "semana", "que", "viene"]
            | ["la", "proxima", "semana"]
            | ["la", "semana", "proxima"]
            | ["semana", "que", "viene"],
        )
        | (Lang::En, ["next", "week"]) => (0, 1),
        (Lang::Es, ["la", "semana", "pasada"]) | (Lang::En, ["last", "week"]) => (0, -1),
        (
            Lang::Es,
            ["este" | "el", "finde"] | ["este" | "el", "fin", "de", "semana"] | ["finde"],
        )
        | (Lang::En, ["this" | "the", "weekend"] | ["weekend"]) => (1, 0),
        (
            Lang::Es,
            ["el", "finde", "que", "viene"]
            | ["el", "proximo", "finde"]
            | ["el", "proximo", "fin", "de", "semana"]
            | ["el", "fin", "de", "semana", "que", "viene"],
        )
        | (Lang::En, ["next", "weekend"]) => (1, 1),
        (Lang::Es, ["este", "mes"]) | (Lang::En, ["this", "month"]) => (2, 0),
        (Lang::Es, ["el", "mes", "que", "viene"] | ["el", "proximo", "mes"])
        | (Lang::En, ["next", "month"]) => (2, 1),
        (Lang::Es, ["el", "mes", "pasado"]) | (Lang::En, ["last", "month"]) => (2, -1),
        (Lang::Es, ["este", "ano"]) | (Lang::En, ["this", "year"]) => (3, 0),
        (Lang::Es, ["el", "ano", "que", "viene"] | ["el", "proximo", "ano"])
        | (Lang::En, ["next", "year"]) => (3, 1),
        (Lang::Es, ["el", "ano", "pasado"]) | (Lang::En, ["last", "year"]) => (3, -1),
        _ => return Err(Problem::Unreadable),
    };
    let (start, end) = match unit {
        0 | 1 => {
            let first = monday(today).add_days(7 * offset);
            if unit == 0 {
                (first, first.add_days(6))
            } else {
                (first.add_days(5), first.add_days(6))
            }
        }
        2 => {
            let first = Date::new(today.year(), today.month(), 1)
                .and_then(|d| d.add_months(offset))
                .ok_or(Problem::Unreadable)?;
            (first, first.end_of_month())
        }
        _ => {
            let year = today.year() + i32::try_from(offset).map_err(|_| Problem::Unreadable)?;
            (
                Date::new(year, 1, 1).ok_or(Problem::Unreadable)?,
                Date::new(year, 12, 31).ok_or(Problem::Unreadable)?,
            )
        }
    };
    Ok(Value::DateRange { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The match form, as a capture reaches an extractor.
    fn fold(text: &str) -> String {
        crate::normalise::normalise(text).folded
    }

    fn dur(text: &str, lang: Lang) -> Option<String> {
        duration(&fold(text), lang).ok().map(|v| v.to_string())
    }

    #[test]
    fn spanish_durations() {
        let cases = [
            ("5 minutos", "PT5M"),
            ("cinco minutos", "PT5M"),
            ("una hora", "PT1H"),
            ("hora y media", "PT1H30M"),
            ("media hora", "PT30M"),
            ("un cuarto de hora", "PT15M"),
            ("tres cuartos de hora", "PT45M"),
            ("una hora y cuarto", "PT1H15M"),
            ("dos horas y media", "PT2H30M"),
            ("90 minutos", "PT1H30M"),
            ("1h30", "PT1H30M"),
            ("1:30 h", "PT1H30M"),
            ("45 min", "PT45M"),
            ("30 seg", "PT30S"),
            ("tres días", "P3D"),
            ("una semana", "P7D"),
            ("quince dias", "P15D"),
            ("un par de horas", "PT2H"),
            ("dentro de 3 horas", "PT3H"),
            ("2 horas y 30 minutos", "PT2H30M"),
            ("un mes", "P1M"),
            ("año y medio", "P1Y6M"),
        ];
        for (text, expected) in cases {
            assert_eq!(dur(text, Lang::Es).as_deref(), Some(expected), "{text}");
        }
        for vague in [
            "un rato",
            "un ratito",
            "un momento",
            "unas horas",
            "un finde",
            "mañana",
            "horas",
            "",
        ] {
            assert_eq!(dur(vague, Lang::Es), None, "{vague}");
        }
    }

    #[test]
    fn english_durations() {
        let cases = [
            ("5 minutes", "PT5M"),
            ("five minutes", "PT5M"),
            ("an hour", "PT1H"),
            ("an hour and a half", "PT1H30M"),
            ("half an hour", "PT30M"),
            ("a quarter of an hour", "PT15M"),
            ("three quarters of an hour", "PT45M"),
            ("90 minutes", "PT1H30M"),
            ("1h30", "PT1H30M"),
            ("1h 30m", "PT1H30M"),
            ("45 min", "PT45M"),
            ("30 secs", "PT30S"),
            ("three days", "P3D"),
            ("a week", "P7D"),
            ("a fortnight", "P14D"),
            ("a couple of hours", "PT2H"),
            ("in 20 minutes", "PT20M"),
            ("two years", "P2Y"),
        ];
        for (text, expected) in cases {
            assert_eq!(dur(text, Lang::En).as_deref(), Some(expected), "{text}");
        }
        for vague in [
            "a while",
            "a bit",
            "some time",
            "some hours",
            "a few minutes",
        ] {
            assert_eq!(dur(vague, Lang::En), None, "{vague}");
        }
        // A loanword unit is content.
        assert_eq!(dur("five minutos", Lang::En).as_deref(), Some("PT5M"));
    }

    /// 2027-03-14 18:42 at UTC+1, a Sunday.
    fn now() -> Timestamp {
        Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600)
    }

    fn dt(text: &str, lang: Lang) -> Option<String> {
        datetime(&fold(text), lang, now())
            .ok()
            .map(|v| v.to_string())
    }

    #[test]
    fn spanish_datetimes() {
        let cases = [
            ("mañana por la tarde", "2027-03-15 17:00"),
            ("mañana por la mañana", "2027-03-15 09:00"),
            ("mañana por la noche", "2027-03-15 21:00"),
            ("esta tarde", "2027-03-14 17:00"),
            ("las 3 y media", "2027-03-15 03:30"),
            ("las 7", "2027-03-14 19:00"),
            ("las 5", "2027-03-15 05:00"),
            ("las cuatro menos cuarto de la tarde", "2027-03-15 15:45"),
            ("el lunes a las 5 de la tarde", "2027-03-15 17:00"),
            ("hoy a las 15:30", "2027-03-14 15:30"),
            ("a las 8 de la mañana", "2027-03-15 08:00"),
            ("mediodía", "2027-03-15 12:00"),
            ("medianoche", "2027-03-15 00:00"),
            ("dentro de 2 horas", "2027-03-14 20:42"),
            ("el 3/4", "2027-04-03"),
            ("mañana", "2027-03-15"),
            ("el viernes", "2027-03-19"),
        ];
        for (text, expected) in cases {
            assert_eq!(dt(text, Lang::Es).as_deref(), Some(expected), "{text}");
        }
        assert_eq!(dt("el finde", Lang::Es), None);
        assert_eq!(dt("un rato", Lang::Es), None);
    }

    #[test]
    fn english_datetimes() {
        let cases = [
            ("tomorrow afternoon", "2027-03-15 17:00"),
            ("tomorrow morning", "2027-03-15 09:00"),
            ("tonight", "2027-03-14 21:00"),
            ("half past three", "2027-03-15 03:30"),
            ("3:30 pm", "2027-03-15 15:30"),
            ("next friday at 9 pm", "2027-03-19 21:00"),
            ("at noon", "2027-03-15 12:00"),
            ("in 20 minutes", "2027-03-14 19:02"),
            ("next friday", "2027-03-19"),
        ];
        for (text, expected) in cases {
            assert_eq!(dt(text, Lang::En).as_deref(), Some(expected), "{text}");
        }
        assert!(matches!(
            datetime("3/4", Lang::En, now()),
            Err(Problem::Ambiguous(_))
        ));
        assert_eq!(dt("the weekend", Lang::En), None);
    }

    fn range(text: &str, lang: Lang) -> Option<String> {
        let today = now().date_or_utc();
        daterange(&fold(text), lang, today)
            .ok()
            .map(|v| v.to_string())
    }

    #[test]
    fn date_ranges() {
        // Sunday 14 March 2027: its week began on Monday the 8th.
        let cases = [
            ("esta semana", Lang::Es, "2027-03-08/2027-03-14"),
            ("la semana que viene", Lang::Es, "2027-03-15/2027-03-21"),
            ("el finde", Lang::Es, "2027-03-13/2027-03-14"),
            ("el mes que viene", Lang::Es, "2027-04-01/2027-04-30"),
            ("este año", Lang::Es, "2027-01-01/2027-12-31"),
            ("this week", Lang::En, "2027-03-08/2027-03-14"),
            ("next weekend", Lang::En, "2027-03-20/2027-03-21"),
            ("last month", Lang::En, "2027-02-01/2027-02-28"),
        ];
        for (text, lang, expected) in cases {
            assert_eq!(range(text, lang).as_deref(), Some(expected), "{text}");
        }
        assert_eq!(range("mañana", Lang::Es), None);
    }
}
