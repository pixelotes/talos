//! Capture extractors (DESIGN.md section 4, docs/spec/conversation.md
//! section 2): they turn the text a rule captured into a typed
//! [`Value`](crate::Value). Each is bilingual and tested on its own,
//! independently of any skill.
//!
//! [`number`] and [`date`] read the **match form** of a capture (lower
//! case, accents folded; see [`normalise`](crate::normalise)), so "mañana"
//! and "manana" are the same word. [`list`] reads the capture in the user's
//! **original** spelling, because a term keeps its case and accents.
//!
//! What is here is what the v0.1 skills need. Times of day, durations and
//! places are not yet extractors; full slot filling and asking back arrive
//! in v0.2.

use crate::Lang;
use crate::civil::{Date, Weekday, days_in_month};
use crate::value::{MAX_DIGITS, Number, Problem, Value};

/// Punctuation that may surround a captured value without being part of it.
const EDGE_PUNCTUATION: &[char] = &['.', ',', ';', ':', '!', '?', '¡', '¿', '"', '\'', '(', ')'];

/// Trims whitespace and surrounding punctuation.
fn trim_edges(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || EDGE_PUNCTUATION.contains(&c))
}

/// Splits text into words for the number and date extractors: on
/// whitespace and hyphens between letters, with edge punctuation removed.
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for word in trim_edges(text).split_whitespace() {
        let word = trim_edges(word);
        if word.is_empty() {
            continue;
        }
        // "twenty-one", but not "-3".
        if word.contains('-') && word.chars().all(|c| c.is_alphabetic() || c == '-') {
            out.extend(word.split('-').filter(|w| !w.is_empty()));
        } else {
            out.push(word);
        }
    }
    out
}

// --- Numbers -------------------------------------------------------------------

/// Reads digits with separators by the conventions of `lang`
/// (docs/spec/conversation.md 2.1): Spanish groups with a dot and writes a
/// decimal comma; English groups with a comma and writes a decimal point.
/// A lone separator followed by exactly three digits is a thousands
/// separator in the language that groups with it. Mixed or malformed
/// separators are never guessed at.
fn digits(text: &str, lang: Lang) -> Option<Number> {
    let (negative, body) = match text.strip_prefix(['-', '+']) {
        Some(rest) => (text.starts_with('-'), rest),
        None => (false, text),
    };
    if body.is_empty()
        || !body.starts_with(|c: char| c.is_ascii_digit())
        || !body.ends_with(|c: char| c.is_ascii_digit())
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ' '))
    {
        return None;
    }
    let (group, decimal) = match lang {
        Lang::Es => ('.', ','),
        Lang::En => (',', '.'),
    };
    let grouped = |s: &str, sep: char| -> bool {
        let mut parts = s.split(sep);
        let first = parts.next().unwrap_or("");
        (1..=3).contains(&first.len()) && parts.all(|p| p.len() == 3)
    };
    let (int, frac): (String, &str) = if body.contains(' ') {
        // "1 500": spaces group in both languages; no other separator.
        if body.contains(['.', ',']) || !grouped(body, ' ') {
            return None;
        }
        (body.replace(' ', ""), "")
    } else if body.contains(group) && body.contains(decimal) {
        let (int, frac) = body.rsplit_once(decimal)?;
        if frac.contains(group) || !grouped(int, group) {
            return None;
        }
        (int.replace(group, ""), frac)
    } else if body.contains(group) {
        let count = body.matches(group).count();
        let after = body.rsplit(group).next().unwrap_or("");
        if count == 1 && after.len() != 3 {
            // Spanish "1.5" is 1.5; English "1,5" is leniently 1.5.
            body.split_once(group).map(|(i, f)| (i.to_owned(), f))?
        } else if grouped(body, group) {
            (body.replace(group, ""), "")
        } else {
            return None;
        }
    } else if body.contains(decimal) {
        if body.matches(decimal).count() > 1 {
            return None;
        }
        body.split_once(decimal).map(|(i, f)| (i.to_owned(), f))?
    } else {
        (body.to_owned(), "")
    };
    if int.len() + frac.len() > MAX_DIGITS {
        return None;
    }
    let units: i128 = format!("{int}{frac}").parse().ok()?;
    let n = Number::new(units, u8::try_from(frac.len()).ok()?)?;
    Some(if negative { n.neg() } else { n })
}

/// The value of a number word below one hundred, or of a word for hundreds.
fn small_word(word: &str, lang: Lang) -> Option<i64> {
    const ES: &[(&str, i64)] = &[
        ("cero", 0),
        ("un", 1),
        ("uno", 1),
        ("una", 1),
        ("dos", 2),
        ("tres", 3),
        ("cuatro", 4),
        ("cinco", 5),
        ("seis", 6),
        ("siete", 7),
        ("ocho", 8),
        ("nueve", 9),
        ("diez", 10),
        ("once", 11),
        ("doce", 12),
        ("trece", 13),
        ("catorce", 14),
        ("quince", 15),
        ("dieciseis", 16),
        ("diecisiete", 17),
        ("dieciocho", 18),
        ("diecinueve", 19),
        ("veinte", 20),
        ("veintiun", 21),
        ("veintiuno", 21),
        ("veintiuna", 21),
        ("veintidos", 22),
        ("veintitres", 23),
        ("veinticuatro", 24),
        ("veinticinco", 25),
        ("veintiseis", 26),
        ("veintisiete", 27),
        ("veintiocho", 28),
        ("veintinueve", 29),
        ("treinta", 30),
        ("cuarenta", 40),
        ("cincuenta", 50),
        ("sesenta", 60),
        ("setenta", 70),
        ("ochenta", 80),
        ("noventa", 90),
        ("cien", 100),
        ("ciento", 100),
        ("doscientos", 200),
        ("doscientas", 200),
        ("trescientos", 300),
        ("trescientas", 300),
        ("cuatrocientos", 400),
        ("cuatrocientas", 400),
        ("quinientos", 500),
        ("quinientas", 500),
        ("seiscientos", 600),
        ("seiscientas", 600),
        ("setecientos", 700),
        ("setecientas", 700),
        ("ochocientos", 800),
        ("ochocientas", 800),
        ("novecientos", 900),
        ("novecientas", 900),
    ];
    const EN: &[(&str, i64)] = &[
        ("zero", 0),
        ("a", 1),
        ("an", 1),
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        ("twenty", 20),
        ("thirty", 30),
        ("forty", 40),
        ("fifty", 50),
        ("sixty", 60),
        ("seventy", 70),
        ("eighty", 80),
        ("ninety", 90),
    ];
    let table = match lang {
        Lang::Es => ES,
        Lang::En => EN,
    };
    table.iter().find(|(w, _)| *w == word).map(|&(_, v)| v)
}

/// A scale word: thousand, million, and the language's own "billion"
/// (Spanish "billón" is 10^12, English "billion" 10^9; basics.md).
fn scale_word(word: &str, lang: Lang) -> Option<i64> {
    match (lang, word) {
        (Lang::Es, "mil") | (Lang::En, "thousand" | "thousands") => Some(1_000),
        (Lang::Es, "millon" | "millones") | (Lang::En, "million" | "millions") => Some(1_000_000),
        (Lang::Es, "billon" | "billones") => Some(1_000_000_000_000),
        (Lang::En, "billion" | "billions") => Some(1_000_000_000),
        _ => None,
    }
}

/// Accumulates number words: `big` holds millions and up, `segment` the
/// thousands below the last million, `current` the group being read.
#[derive(Default)]
struct Words {
    big: Option<Number>,
    segment: Option<Number>,
    current: Option<Number>,
    /// The scale of the previous word, for "y medio" / "and a half".
    last_scale: Option<i64>,
}

/// One half, exactly.
fn half() -> Number {
    Number::new(5, 1).unwrap_or(Number::from_int(0))
}

fn add(a: Option<Number>, b: Number) -> Option<Number> {
    match a {
        Some(a) => a.plus(b),
        None => Some(b),
    }
}

impl Words {
    fn current_or_one(&self) -> Number {
        self.current.unwrap_or(Number::from_int(1))
    }

    /// Applies one word; `None` if it is not a number word in `lang`.
    fn word(&mut self, word: &str, lang: Lang) -> Option<()> {
        let half_after_scale = |this: &mut Words| -> Option<()> {
            match this.last_scale.take() {
                Some(scale) if scale >= 1_000_000 => {
                    this.big = add(this.big, half().times(Number::from_int(scale))?);
                }
                Some(scale) => {
                    this.segment = add(this.segment, half().times(Number::from_int(scale))?);
                }
                None => this.current = add(this.current, half()),
            }
            Some(())
        };
        if let Some(n) = digits(word, lang) {
            self.current = add(self.current, n);
        } else if let Some(v) = small_word(word, lang) {
            self.current = add(self.current, Number::from_int(v));
        } else if lang == Lang::En && matches!(word, "hundred" | "hundreds") {
            self.current = self.current_or_one().times(Number::from_int(100));
        } else if let Some(scale) = scale_word(word, lang) {
            let factor = Number::from_int(scale);
            if scale >= 1_000_000 {
                let group = match (self.segment, self.current) {
                    (None, None) => Number::from_int(1),
                    (s, c) => add(s, c.unwrap_or(Number::from_int(0)))?,
                };
                self.big = add(self.big, group.times(factor)?);
                self.segment = None;
            } else {
                self.segment = add(self.segment, self.current_or_one().times(factor)?);
            }
            self.current = None;
            self.last_scale = Some(scale);
            return Some(());
        } else {
            match (lang, word) {
                (Lang::Es, "medio" | "media") | (Lang::En, "half") => {
                    return half_after_scale(self);
                }
                (Lang::Es, "cuarto" | "cuartos") | (Lang::En, "quarter" | "quarters") => {
                    self.current = Some(self.current_or_one().times(Number::new(25, 2)?)?);
                }
                (Lang::Es, "docena" | "docenas") | (Lang::En, "dozen" | "dozens") => {
                    self.current = Some(self.current_or_one().times(Number::from_int(12))?);
                }
                _ => return None,
            }
        }
        self.last_scale = None;
        Some(())
    }

    fn total(&self) -> Option<Number> {
        let mut total = Number::from_int(0);
        for part in [self.big, self.segment, self.current].into_iter().flatten() {
            total = total.plus(part)?;
        }
        Some(total)
    }
}

/// Reads a number in digits, in words, or mixed ("3 mil", "2,5 millones",
/// "two and a half million"), by the conventions of the turn's language.
/// The text is the match form of a capture.
///
/// # Errors
///
/// [`Problem::Unreadable`] when the text is not a number, or its separators
/// are mixed or malformed for the language (`1,500,000` in Spanish).
pub fn number(text: &str, lang: Lang) -> Result<Number, Problem> {
    let text = trim_edges(text);
    if let Some(n) = digits(text, lang) {
        return Ok(n);
    }
    let mut tokens = words(text);
    let negative = match tokens.first() {
        Some(&"menos") => lang == Lang::Es,
        Some(&("minus" | "negative")) => lang == Lang::En,
        _ => false,
    };
    if negative {
        tokens.remove(0);
    }
    let mut acc = Words::default();
    let mut any = false;
    let mut i = 0;
    while i < tokens.len() {
        let word = tokens[i];
        // Connectors: "treinta y cinco", "one hundred and ten", "y medio",
        // "and a half".
        let connector = matches!((lang, word), (Lang::Es, "y") | (Lang::En, "and"));
        if connector && any && i + 1 < tokens.len() {
            i += 1;
            continue;
        }
        // "a half", "un cuarto" are handled word by word; "a" before "half"
        // must not count as one.
        if lang == Lang::En && word == "a" && tokens.get(i + 1) == Some(&"half") {
            i += 1;
            continue;
        }
        acc.word(word, lang).ok_or(Problem::Unreadable)?;
        any = true;
        i += 1;
    }
    if !any {
        return Err(Problem::Unreadable);
    }
    let total = acc.total().ok_or(Problem::Unreadable)?;
    Ok(if negative { total.neg() } else { total })
}

// --- Dates -------------------------------------------------------------------

fn weekday_word(word: &str, lang: Lang) -> Option<Weekday> {
    let names: [&str; 7] = match lang {
        Lang::Es => [
            "lunes",
            "martes",
            "miercoles",
            "jueves",
            "viernes",
            "sabado",
            "domingo",
        ],
        Lang::En => [
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ],
    };
    let index = names.iter().position(|n| *n == word)?;
    Some(Weekday::ALL[index])
}

fn month_word(word: &str, lang: Lang) -> Option<u8> {
    let index = match lang {
        Lang::Es => [
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
        ]
        .iter()
        .position(|n| *n == word)
        .or_else(|| (word == "setiembre").then_some(8)),
        Lang::En => [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ]
        .iter()
        .position(|n| *n == word || (n.len() > 4 && word.len() == 3 && n.starts_with(word)))
        .or_else(|| (word == "sept").then_some(8)),
    }?;
    u8::try_from(index + 1).ok()
}

/// A day of the month: digits, an English ordinal ("3rd"), or a number
/// word ("uno", "primero", "three").
fn day_word(word: &str, lang: Lang) -> Option<u8> {
    let bare = match lang {
        Lang::En => word
            .strip_suffix("st")
            .or_else(|| word.strip_suffix("nd"))
            .or_else(|| word.strip_suffix("rd"))
            .or_else(|| word.strip_suffix("th"))
            .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or(word),
        Lang::Es => word.strip_suffix('º').unwrap_or(word),
    };
    if bare.bytes().all(|b| b.is_ascii_digit()) && !bare.is_empty() && bare.len() <= 2 {
        return bare.parse().ok().filter(|d| (1..=31).contains(d));
    }
    let named = match (lang, word) {
        (Lang::Es, "primero") | (Lang::En, "first") => Some(1),
        (Lang::En, "second") => Some(2),
        (Lang::En, "third") => Some(3),
        _ => small_word(word, lang),
    };
    named
        .and_then(|d| u8::try_from(d).ok())
        .filter(|d| (1..=31).contains(d))
}

/// A year: four digits, or two meaning 20xx.
fn year_word(word: &str) -> Option<i32> {
    if !word.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match word.len() {
        2 => word.parse::<i32>().ok().map(|y| 2000 + y),
        4 => word.parse().ok(),
        _ => None,
    }
}

/// The next date on or after `today` with this month and day; for 29
/// February, the next leap year's.
fn next_occurrence(today: Date, month: u8, day: u8) -> Result<Date, Problem> {
    if day > days_in_month(2000, month) {
        return Err(Problem::DoesNotExist);
    }
    (today.year()..today.year() + 9)
        .filter_map(|year| Date::new(year, month, day))
        .find(|d| *d >= today)
        .ok_or(Problem::DoesNotExist)
}

/// A day and month (and maybe a year) as a value.
fn calendar(today: Date, day: u8, month: u8, year: Option<i32>) -> Result<Value, Problem> {
    match year {
        Some(year) => Date::new(year, month, day)
            .map(|date| Value::Date {
                date,
                year_given: true,
            })
            .ok_or(Problem::DoesNotExist),
        None => next_occurrence(today, month, day).map(|date| Value::Date {
            date,
            year_given: false,
        }),
    }
}

fn relative(date: Date) -> Value {
    Value::Date {
        date,
        year_given: false,
    }
}

/// Adds calendar months, keeping the day where the month has it and the
/// month's last day otherwise.
fn add_months(date: Date, months: i64) -> Option<Date> {
    let index = i64::from(date.year()) * 12 + i64::from(date.month()) - 1 + months;
    let year = i32::try_from(index.div_euclid(12)).ok()?;
    let month = u8::try_from(index.rem_euclid(12) + 1).ok()?;
    Date::new(year, month, date.day().min(days_in_month(year, month)))
}

/// "dentro de 3 días", "in two weeks", "hace un mes", "3 days ago".
fn offset(tokens: &[&str], lang: Lang, today: Date) -> Option<Date> {
    let (sign, rest): (i64, &[&str]) = match (lang, tokens) {
        (Lang::Es, ["dentro", "de", rest @ ..]) | (Lang::En, ["in", rest @ ..]) => (1, rest),
        (Lang::Es, ["hace", rest @ ..]) | (Lang::En, [rest @ .., "ago"]) => (-1, rest),
        _ => return None,
    };
    let (unit, amount) = rest.split_last()?;
    let n = number(&amount.join(" "), lang)
        .ok()?
        .to_i64()?
        .checked_mul(sign)?;
    match (lang, *unit) {
        (Lang::Es, "dia" | "dias") | (Lang::En, "day" | "days") => Some(today.add_days(n)),
        (Lang::Es, "semana" | "semanas") | (Lang::En, "week" | "weeks") => {
            Some(today.add_days(n.checked_mul(7)?))
        }
        (Lang::Es, "mes" | "meses") | (Lang::En, "month" | "months") => add_months(today, n),
        (Lang::Es, "ano" | "anos") | (Lang::En, "year" | "years") => {
            add_months(today, n.checked_mul(12)?)
        }
        _ => None,
    }
}

/// How a weekday name was qualified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    /// "el viernes", "next Friday": the first strictly after today.
    Next,
    /// "este viernes", "this Friday": the nearest from today on, today
    /// included.
    This,
    /// "el viernes pasado", "last Friday": the latest strictly before today.
    Last,
    /// "el viernes de la semana que viene", "Friday next week": that day of
    /// the Monday-to-Sunday week after this one.
    NextWeek,
}

fn weekday_phrase(tokens: &[&str], lang: Lang) -> Option<(Weekday, Which)> {
    use Which::{Last, Next, NextWeek, This};
    let day = |w: &str| weekday_word(w, lang);
    match lang {
        Lang::Es => match tokens {
            ["este" | "esta", w] => Some((day(w)?, This)),
            [w] | ["proximo" | "siguiente", w] | [w, "que", "viene"] | [w, "proximo"] => {
                Some((day(w)?, Next))
            }
            [w, "pasado"] | ["pasado", w] => Some((day(w)?, Last)),
            [w, "de", "la", "semana", "que", "viene"]
            | [w, "de", "la", "proxima", "semana"]
            | [w, "de", "la", "semana", "siguiente"] => Some((day(w)?, NextWeek)),
            _ => None,
        },
        Lang::En => match tokens {
            ["this", w] => Some((day(w)?, This)),
            [w] | ["next", w] => Some((day(w)?, Next)),
            ["last", w] | [w, "last"] => Some((day(w)?, Last)),
            [w, "next", "week"] => Some((day(w)?, NextWeek)),
            _ => None,
        },
    }
}

fn resolve_weekday(today: Date, day: Weekday, which: Which) -> Date {
    let today_index = i64::from(today.weekday().days_from_monday());
    let target = i64::from(day.days_from_monday());
    let ahead = (target - today_index).rem_euclid(7);
    match which {
        Which::This => today.add_days(ahead),
        Which::Next => today.add_days(if ahead == 0 { 7 } else { ahead }),
        Which::Last => {
            let back = (today_index - target).rem_euclid(7);
            today.add_days(-(if back == 0 { 7 } else { back }))
        }
        Which::NextWeek => today.add_days(7 - today_index + target),
    }
}

/// A numeric date: `3/4`, `3-4-2027`, `25/12/27`. Day first in Spanish; in
/// English ambiguous unless only one reading is a valid date.
fn numeric(word: &str, lang: Lang, today: Date) -> Option<Result<Value, Problem>> {
    let sep = if word.contains('/') { '/' } else { '-' };
    let parts: Vec<&str> = word.split(sep).collect();
    if !(2..=3).contains(&parts.len())
        || parts[..2]
            .iter()
            .any(|p| p.is_empty() || p.len() > 2 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let a: u8 = parts[0].parse().ok()?;
    let b: u8 = parts[1].parse().ok()?;
    let year = match parts.get(2) {
        Some(y) => Some(year_word(y)?),
        None => None,
    };
    let read = |day: u8, month: u8| -> Option<Value> {
        if !(1..=12).contains(&month) {
            return None;
        }
        calendar(today, day, month, year).ok()
    };
    Some(match lang {
        Lang::Es => read(a, b).ok_or(Problem::DoesNotExist),
        Lang::En => match (read(a, b), read(b, a)) {
            (Some(x), Some(y)) if x != y => Err(Problem::Ambiguous(vec![x, y])),
            (Some(x), _) | (None, Some(x)) => Ok(x),
            (None, None) => Err(Problem::DoesNotExist),
        },
    })
}

/// "3 de mayo (de 2027)", "el uno de enero", "March 3rd(, 2027)", "3 March
/// 2027", "the 3rd of March".
fn named_month(tokens: &[&str], lang: Lang, today: Date) -> Option<Result<Value, Problem>> {
    let (day, month, year) = match lang {
        Lang::Es => match tokens {
            [d, "de", m] => (day_word(d, lang)?, month_word(m, lang)?, None),
            [d, "de", m, "de" | "del", y] => (
                day_word(d, lang)?,
                month_word(m, lang)?,
                Some(year_word(y)?),
            ),
            _ => return None,
        },
        Lang::En => match tokens {
            [m, d] | [d, m] | [d, "of", m] if month_word(m, lang).is_some() => {
                (day_word(d, lang)?, month_word(m, lang)?, None)
            }
            [m, d, y] | [d, m, y] | [d, "of", m, y] if month_word(m, lang).is_some() => (
                day_word(d, lang)?,
                month_word(m, lang)?,
                Some(year_word(y)?),
            ),
            _ => return None,
        },
    };
    Some(calendar(today, day, month, year))
}

/// Reads a date relative to `today` (docs/spec/conversation.md 2.3): "hoy",
/// "mañana", "pasado mañana", "ayer"; "today", "tomorrow", "the day after
/// tomorrow"; weekdays with "este", "próximo", "que viene", "pasado",
/// "this", "next", "last"; "dentro de 3 días", "in two weeks", "hace un
/// mes"; "3 de mayo", "March 3rd"; numeric "3/5" (day first in Spanish,
/// asked about in English unless only one reading is valid); "el día 3",
/// "the 3rd"; "a finales de mes", "end of the month". A date without a
/// year is the next occurrence on or after today. The text is the match
/// form of a capture.
///
/// # Errors
///
/// [`Problem::DoesNotExist`] for "31 de abril"; [`Problem::Ambiguous`] with
/// both readings for an English `3/4`; [`Problem::Unreadable`] for anything
/// else, including "el finde"/"the weekend", which is not one date.
pub fn date(text: &str, lang: Lang, today: Date) -> Result<Value, Problem> {
    let all = words(text);
    // Leading articles and prepositions carry nothing.
    let skip = all
        .iter()
        .take_while(|w| match lang {
            Lang::Es => matches!(**w, "el" | "la" | "para" | "hasta" | "desde" | "a"),
            Lang::En => matches!(
                **w,
                "on" | "the" | "for" | "until" | "till" | "since" | "by"
            ),
        })
        .count();
    let tokens = &all[skip..];
    let day = |n: i64| Ok(relative(today.add_days(n)));
    match (lang, tokens) {
        (Lang::Es, ["hoy"]) | (Lang::En, ["today"]) => return day(0),
        (Lang::Es, ["manana"]) | (Lang::En, ["tomorrow"]) => return day(1),
        (Lang::Es, ["pasado", "manana"]) | (Lang::En, ["day", "after", "tomorrow"]) => {
            return day(2);
        }
        (Lang::Es, ["ayer"]) | (Lang::En, ["yesterday"]) => return day(-1),
        (Lang::Es, ["anteayer"] | ["antes", "de", "ayer"])
        | (Lang::En, ["day", "before", "yesterday"]) => return day(-2),
        (Lang::Es, ["finales" | "final" | "fin", "de", "mes"] | ["fin", "de", "este", "mes"])
        | (Lang::En, ["end", "of", "the" | "this", "month"]) => {
            return Ok(relative(today.end_of_month()));
        }
        (Lang::Es, ["dia", d]) | (Lang::En, [d]) if day_word(d, lang).is_some() => {
            let d = day_word(d, lang).unwrap_or(1);
            // The next month, from this one, that has that day.
            return (0..13)
                .filter_map(|m| add_months(Date::new(today.year(), today.month(), 1)?, m))
                .filter_map(|first| Date::new(first.year(), first.month(), d))
                .find(|date| *date >= today)
                .map(relative)
                .ok_or(Problem::DoesNotExist);
        }
        _ => {}
    }
    if let [word] = tokens
        && let Some(result) = numeric(word, lang, today)
    {
        return result;
    }
    if let Some(date) = offset(tokens, lang, today) {
        return Ok(relative(date));
    }
    if let Some((weekday, which)) = weekday_phrase(tokens, lang) {
        return Ok(relative(resolve_weekday(today, weekday, which)));
    }
    if let Some(result) = named_month(tokens, lang, today) {
        return result;
    }
    Err(Problem::Unreadable)
}

// --- Lists -------------------------------------------------------------------

/// Whether a word joins options, and which kind: `Some(true)` for "and"-type
/// ("y", "e", "and"), `Some(false)` for "or"-type ("o", "u", "or").
fn conjunction(word: &str) -> Option<bool> {
    match word.to_lowercase().as_str() {
        "y" | "e" | "and" => Some(true),
        "o" | "u" | "or" => Some(false),
        _ => None,
    }
}

/// Reads a list of options in the user's own spelling: "té, café o agua",
/// "tea, coffee or water", "rojo/verde", "Ana, Luis y Marta"
/// (docs/spec/basics.md, `random.pick`). Items are split at commas,
/// slashes and the conjunctions "y"/"e"/"o"/"u"/"and"/"or"; surrounding
/// punctuation is removed; repeats, ignoring case and accents, count once.
/// When the text mixes "and"- and "or"-type conjunctions the split may not
/// be what the user meant, and the list is marked uncertain. Text with no
/// separator is a one-item list. The text is the original spelling of a
/// capture.
#[must_use]
pub fn list(original: &str) -> Value {
    let text = trim_edges(original);
    let mut pieces: Vec<&str> = Vec::new();
    let (mut ands, mut ors) = (false, false);
    let mut start = 0;
    let is_break = |c: char| c.is_whitespace() || c == ',' || c == '/';
    let mut chars = text.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if c == ',' || c == '/' {
            pieces.push(&text[start..pos]);
            start = pos + c.len_utf8();
            continue;
        }
        if c.is_whitespace() {
            continue;
        }
        let mut end = pos + c.len_utf8();
        while let Some(&(next_pos, next)) = chars.peek() {
            if is_break(next) {
                break;
            }
            end = next_pos + next.len_utf8();
            chars.next();
        }
        if let Some(and) = conjunction(&text[pos..end]) {
            if and {
                ands = true;
            } else {
                ors = true;
            }
            pieces.push(&text[start..pos]);
            start = end;
        }
    }
    pieces.push(&text[start..]);

    let mut items: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for piece in pieces {
        let item = trim_edges(piece);
        if item.is_empty() {
            continue;
        }
        let key = crate::normalise::fold(&item.to_lowercase());
        if !seen.contains(&key) {
            seen.push(key);
            items.push(item.to_owned());
        }
    }
    Value::List {
        items,
        uncertain: ands && ors,
    }
}

/// Reads a single term: the capture in the user's spelling with surrounding
/// whitespace and punctuation removed.
#[must_use]
pub fn term(original: &str) -> Value {
    Value::Text(trim_edges(original).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(text: &str, lang: Lang) -> Option<String> {
        number(text, lang).ok().map(|n| n.to_string())
    }

    #[test]
    fn separators_follow_the_turns_language() {
        // The table of docs/spec/conversation.md 2.1.
        let table = [
            ("1.500", Some("1500"), Some("1.5")),
            ("1,500", Some("1.5"), Some("1500")),
            ("1.5", Some("1.5"), Some("1.5")),
            ("1,5", Some("1.5"), Some("1.5")),
            ("1 500", Some("1500"), Some("1500")),
            ("1.500.000", Some("1500000"), None),
            ("1,500,000", None, Some("1500000")),
            ("12.34.56", None, None),
            ("1.500,25", Some("1500.25"), None),
            ("1,500.25", None, Some("1500.25")),
            ("-3", Some("-3"), Some("-3")),
            ("250", Some("250"), Some("250")),
            ("0,05", Some("0.05"), Some("0.05")),
        ];
        for (text, es, en) in table {
            assert_eq!(num(text, Lang::Es).as_deref(), es, "es {text}");
            assert_eq!(num(text, Lang::En).as_deref(), en, "en {text}");
        }
    }

    #[test]
    fn spanish_number_words() {
        let cases = [
            ("tres", "3"),
            ("veintiuno", "21"),
            ("veintiun", "21"),
            ("treinta y cinco", "35"),
            ("cien", "100"),
            ("ciento diez", "110"),
            ("mil quinientos", "1500"),
            ("dos mil veintisiete", "2027"),
            ("un millon", "1000000"),
            ("dos millones y medio", "2500000"),
            ("un millon doscientos mil", "1200000"),
            ("dos mil millones", "2000000000"),
            ("medio", "0.5"),
            ("un cuarto", "0.25"),
            ("tres cuartos", "0.75"),
            ("una docena", "12"),
            ("media docena", "6"),
            ("menos tres", "-3"),
            ("3 mil", "3000"),
            ("2 millones", "2000000"),
            ("2,5 millones", "2500000"),
            ("un billon", "1000000000000"),
            ("cero", "0"),
        ];
        for (text, expected) in cases {
            assert_eq!(num(text, Lang::Es).as_deref(), Some(expected), "{text}");
        }
        for bad in ["", "tres patatas", "y", "mil y"] {
            assert_eq!(number(bad, Lang::Es), Err(Problem::Unreadable), "{bad}");
        }
    }

    #[test]
    fn english_number_words() {
        let cases = [
            ("three", "3"),
            ("twenty-one", "21"),
            ("twenty one", "21"),
            ("a hundred", "100"),
            ("one hundred and ten", "110"),
            ("fifteen hundred", "1500"),
            ("two thousand twenty-seven", "2027"),
            ("two thousand three hundred", "2300"),
            ("a million", "1000000"),
            ("two and a half million", "2500000"),
            ("a million and a half", "1500000"),
            ("half", "0.5"),
            ("a quarter", "0.25"),
            ("three quarters", "0.75"),
            ("a dozen", "12"),
            ("minus three", "-3"),
            ("negative three", "-3"),
            ("3 thousand", "3000"),
            ("2 million", "2000000"),
            ("2.5 million", "2500000"),
            ("a billion", "1000000000"),
        ];
        for (text, expected) in cases {
            assert_eq!(num(text, Lang::En).as_deref(), Some(expected), "{text}");
        }
        assert_eq!(number("tres", Lang::En), Err(Problem::Unreadable));
    }

    /// 2027-03-14, a Sunday: the spec's usual `now`.
    fn sunday() -> Date {
        Date::new(2027, 3, 14).unwrap()
    }

    fn day(text: &str, lang: Lang) -> String {
        match date(text, lang, sunday()) {
            Ok(v) => v.to_string(),
            Err(Problem::Ambiguous(readings)) => format!(
                "ambiguous {}",
                readings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Err(problem) => format!("{problem:?}"),
        }
    }

    #[test]
    fn relative_days() {
        let es = [
            ("hoy", "2027-03-14"),
            ("manana", "2027-03-15"),
            ("pasado manana", "2027-03-16"),
            ("ayer", "2027-03-13"),
            ("anteayer", "2027-03-12"),
            ("dentro de tres dias", "2027-03-17"),
            ("dentro de 100 dias", "2027-06-22"),
            ("hace dos semanas", "2027-02-28"),
            ("dentro de un mes", "2027-04-14"),
            ("a finales de mes", "2027-03-31"),
        ];
        for (text, expected) in es {
            assert_eq!(day(text, Lang::Es), expected, "{text}");
        }
        let en = [
            ("today", "2027-03-14"),
            ("tomorrow", "2027-03-15"),
            ("the day after tomorrow", "2027-03-16"),
            ("yesterday", "2027-03-13"),
            ("in three days", "2027-03-17"),
            ("two weeks ago", "2027-02-28"),
            ("end of the month", "2027-03-31"),
        ];
        for (text, expected) in en {
            assert_eq!(day(text, Lang::En), expected, "{text}");
        }
    }

    #[test]
    fn weekdays_relative_to_a_sunday() {
        // basics.md, clock conventions: a bare weekday is the next one after
        // today; on that same weekday, seven days ahead.
        let es = [
            ("el lunes", "2027-03-15"),
            ("el viernes", "2027-03-19"),
            ("el domingo", "2027-03-21"),
            ("este domingo", "2027-03-14"),
            ("este viernes", "2027-03-19"),
            ("el proximo viernes", "2027-03-19"),
            ("el viernes que viene", "2027-03-19"),
            ("el viernes pasado", "2027-03-12"),
            ("el domingo pasado", "2027-03-07"),
            ("el viernes de la semana que viene", "2027-03-19"),
            ("el miercoles", "2027-03-17"),
        ];
        for (text, expected) in es {
            assert_eq!(day(text, Lang::Es), expected, "{text}");
        }
        let en = [
            ("on monday", "2027-03-15"),
            ("next friday", "2027-03-19"),
            ("this sunday", "2027-03-14"),
            ("sunday", "2027-03-21"),
            ("last friday", "2027-03-12"),
            ("friday next week", "2027-03-19"),
        ];
        for (text, expected) in en {
            assert_eq!(day(text, Lang::En), expected, "{text}");
        }
        // From a Tuesday, "Friday next week" is not the coming Friday.
        let tuesday = Date::new(2027, 3, 16).unwrap();
        assert_eq!(
            date("friday next week", Lang::En, tuesday)
                .unwrap()
                .to_string(),
            "2027-03-26"
        );
        assert_eq!(
            date("el viernes", Lang::Es, tuesday).unwrap().to_string(),
            "2027-03-19"
        );
        assert_eq!(
            date("el lunes pasado", Lang::Es, tuesday)
                .unwrap()
                .to_string(),
            "2027-03-15"
        );
    }

    #[test]
    fn calendar_dates() {
        let es = [
            ("el 3 de mayo", "2027-05-03"),
            ("3 de mayo de 2028", "2028-05-03"),
            ("el uno de enero", "2028-01-01"),
            ("el primero de enero", "2028-01-01"),
            ("14 de marzo", "2027-03-14"),
            ("el 3/4", "2027-04-03"),
            ("3-4-2027", "2027-04-03"),
            ("25/12/27", "2027-12-25"),
            ("el dia 3", "2027-04-03"),
            ("el dia 20", "2027-03-20"),
            ("29 de febrero", "2028-02-29"),
            ("31 de abril", "DoesNotExist"),
            ("29 de febrero de 2027", "DoesNotExist"),
            ("el finde", "Unreadable"),
            ("el fin de semana", "Unreadable"),
        ];
        for (text, expected) in es {
            assert_eq!(day(text, Lang::Es), expected, "{text}");
        }
        let en = [
            ("march 3rd", "2028-03-03"),
            ("3 march", "2028-03-03"),
            ("the 3rd of march", "2028-03-03"),
            ("march 3, 2027", "2027-03-03"),
            ("dec 25", "2027-12-25"),
            ("3/4", "ambiguous 2027-04-03 or 2028-03-04"),
            ("25/12", "2027-12-25"),
            ("12/25", "2027-12-25"),
            ("5/5", "2027-05-05"),
            ("the 3rd", "2027-04-03"),
            ("february 29, 2027", "DoesNotExist"),
            ("the weekend", "Unreadable"),
        ];
        for (text, expected) in en {
            assert_eq!(day(text, Lang::En), expected, "{text}");
        }
    }

    fn items(text: &str) -> (Vec<String>, bool) {
        match list(text) {
            Value::List { items, uncertain } => (items, uncertain),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn option_lists_keep_the_users_spelling() {
        let cases: [(&str, &[&str], bool); 9] = [
            ("té, café o agua", &["té", "café", "agua"], false),
            ("tea, coffee or water", &["tea", "coffee", "water"], false),
            ("Pizza, Sushi y Tacos.", &["Pizza", "Sushi", "Tacos"], false),
            (
                "ir al cine o quedarnos en casa",
                &["ir al cine", "quedarnos en casa"],
                false,
            ),
            ("rojo/verde/azul", &["rojo", "verde", "azul"], false),
            ("stay in or go out", &["stay in", "go out"], false),
            ("pizza o pizza o PIZZA o sushi", &["pizza", "sushi"], false),
            ("arroz y pollo o pasta", &["arroz", "pollo", "pasta"], true),
            ("Ana, Luis and Marta!", &["Ana", "Luis", "Marta"], false),
        ];
        for (text, expected, uncertain) in cases {
            assert_eq!(
                items(text),
                (
                    expected.iter().map(|s| (*s).to_owned()).collect(),
                    uncertain
                ),
                "{text}"
            );
        }
        assert_eq!(items("  sushi?  ").0, ["sushi"]);
        assert_eq!(items("").0, Vec::<String>::new());
        // A conjunction inside a word is not one.
        assert_eq!(items("yogur o huevos").0, ["yogur", "huevos"]);
        assert_eq!(term("  «hola»? ").to_string(), "«hola»");
    }
}
