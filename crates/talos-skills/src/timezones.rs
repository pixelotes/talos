//! `timezones`: the current time in a place, and the difference in hours
//! between two places (docs/spec/state.md, `timezones`).
//!
//! Places are read from `term` slots with the gazetteer in `places.rs`
//! (cities and regions in both languages) and `talos_core::extract::place`
//! (references such as "mi ciudad", and offsets such as "UTC+2"). Each zone
//! is a POSIX TZ rule from the IANA database (`zones.rs`), evaluated at the
//! turn's injected `now` by `tz.rs`: no host zoneinfo is read, so the same
//! turn gives the same answer on every machine.
//!
//! The little phrases that go inside a reply's variables are written here
//! per language, as the clock skill writes weekday names: a time with its
//! Spanish article ("son las 13:42"), the day note ("(ya es lunes)"), a
//! difference ("1 hora y media"). Everything around them is in the manifest.

mod places;
mod tz;
mod zones;

use talos_core::extract;
use talos_core::normalise::normalise;
use talos_core::{
    Call, Choice, Context, Date, Expectation, Lang, Message, PlaceRef, Skill, SkillResult, Value,
};

use places::{CITIES, City, REGIONS, Region, Target};
use tz::Rule;

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

/// At most this many places in one question (state.md).
const MAX_PLACES: usize = 3;
/// How far ahead a coming change of summer time is mentioned, in seconds.
const NOTICE: i64 = 30 * 86_400;

// --- Places ----------------------------------------------------------------------

/// A place with a clock: what to call it in the reply, what to put on the
/// outcome, and its rule.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spot {
    name: String,
    report: String,
    rule: Rule,
}

impl Spot {
    fn offset(&self, now: i64) -> i32 {
        self.rule.offset_at(now)
    }

    /// The local wall clock: the date and the minute of the day.
    fn local(&self, now: i64) -> (Date, u32) {
        let local = now + i64::from(self.offset(now));
        let date = Date::from_days_since_epoch(local.div_euclid(86_400));
        let minute = u32::try_from(local.rem_euclid(86_400) / 60).unwrap_or(0);
        (date, minute)
    }

    /// What decides whether two same-named places give the same answer: the
    /// offset now and its coming changes.
    fn behaviour(&self, now: i64) -> (i32, Vec<(i64, i32)>) {
        (self.offset(now), self.rule.changes(now, now + NOTICE))
    }
}

/// One option of an ambiguous name.
#[derive(Debug, Clone)]
struct Option_ {
    /// The region that tells it apart ("Argentina"), in the turn's language.
    region: String,
    /// Text that names this option alone when read back as a place.
    value: String,
    /// How the user may pick it.
    names: Vec<String>,
}

/// What a place phrase turned out to be.
#[derive(Debug, Clone)]
enum Found {
    One(Spot),
    /// Spain: the peninsula and the Canary Islands.
    Spain(String),
    /// A country with several zones and no city standing for them.
    Several(String),
    /// A name of several places that differ: which one?
    Choose(String, Vec<Option_>),
    Home,
    Here,
    There,
    /// A name, but of no place Talos knows.
    Unknown(String),
    /// Not a place at all ("somewhere warm", digits).
    Unreadable,
}

/// A gazetteer entry.
#[derive(Debug, Clone, Copy)]
enum Entry {
    City(&'static City),
    Region(&'static Region),
}

/// A name that matched an entry, and the language of that name (`None`:
/// the same in both).
#[derive(Debug, Clone, Copy)]
struct Hit {
    entry: Entry,
    lang: Option<Lang>,
    /// Matched with its region ("Córdoba, Argentina"): name the region.
    qualified: bool,
}

fn names(list: &str) -> impl Iterator<Item = &str> {
    list.split('|').filter(|n| !n.is_empty())
}

/// Which language a matched name belongs to.
#[derive(Debug, Clone, Copy)]
enum Named {
    Only(Lang),
    Both,
}

impl Named {
    const fn lang(self) -> Option<Lang> {
        match self {
            Named::Only(lang) => Some(lang),
            Named::Both => None,
        }
    }
}

/// Which of the three lists of an entry hold `key`.
fn list_lang(spanish: &str, english: &str, both: &str, key: &str) -> Option<Named> {
    let in_spanish = names(spanish).any(|n| n == key);
    let in_english = names(english).any(|n| n == key);
    if names(both).any(|n| n == key) || (in_spanish && in_english) {
        Some(Named::Both)
    } else if in_spanish {
        Some(Named::Only(Lang::Es))
    } else if in_english {
        Some(Named::Only(Lang::En))
    } else {
        None
    }
}

fn city_lang(city: &City, key: &str) -> Option<Named> {
    list_lang(city.names_es, city.names_en, city.names, key)
}

fn region_lang(region: &Region, key: &str) -> Option<Named> {
    list_lang(region.names_es, region.names_en, region.names, key)
}

/// Every entry with exactly this name.
fn exact(key: &str) -> Vec<Hit> {
    let cities = CITIES.iter().filter_map(|city| {
        city_lang(city, key).map(|named| Hit {
            entry: Entry::City(city),
            lang: named.lang(),
            qualified: false,
        })
    });
    let regions = REGIONS.iter().filter_map(|region| {
        region_lang(region, key).map(|named| Hit {
            entry: Entry::Region(region),
            lang: named.lang(),
            qualified: false,
        })
    });
    cities.chain(regions).collect()
}

/// A city named with its region: "cordoba argentina", "cordoba de
/// espana", "portland, oregon".
fn qualified(key: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for region in REGIONS {
        let all = [
            region.names_es,
            region.names_en,
            region.names,
            region.adjectives,
        ];
        for name in all.iter().flat_map(|list| names(list)) {
            let Some(rest) = key.strip_suffix(name).and_then(|r| r.strip_suffix(' ')) else {
                continue;
            };
            let rest = ["de", "en", "in", "del"]
                .iter()
                .find_map(|p| rest.strip_suffix(p).and_then(|r| r.strip_suffix(' ')))
                .unwrap_or(rest)
                .trim_end();
            for city in CITIES {
                let inside = city.region == region.code
                    || city
                        .region
                        .strip_prefix(region.code)
                        .is_some_and(|r| r.starts_with('-'));
                if let Some(named) = city_lang(city, rest).filter(|_| inside) {
                    hits.push(Hit {
                        entry: Entry::City(city),
                        lang: named.lang(),
                        qualified: true,
                    });
                }
            }
        }
    }
    hits
}

/// Levenshtein distance, stopping early above 1.
fn within_one(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + (a.len() - i) + (b.len() - j) <= 1
}

/// Names one letter away from `key`, for long names only ("sidney").
fn near(key: &str) -> Vec<Hit> {
    if key.chars().count() < 6 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let close = |list: &str| names(list).any(|n| n.chars().count() >= 6 && within_one(n, key));
    for city in CITIES {
        if [city.names_es, city.names_en, city.names].iter().any(|l| close(l)) {
            hits.push(Hit {
                entry: Entry::City(city),
                lang: None,
                qualified: false,
            });
        }
    }
    for region in REGIONS {
        if [region.names_es, region.names_en, region.names].iter().any(|l| close(l)) {
            hits.push(Hit {
                entry: Entry::Region(region),
                lang: None,
                qualified: false,
            });
        }
    }
    hits
}

const ARTICLES: &[&str] = &["el ", "la ", "los ", "las ", "the "];

/// The entries a name may be, most direct reading first.
fn lookup(key: &str) -> Vec<Hit> {
    let attempts = std::iter::once(key).chain(ARTICLES.iter().filter_map(|a| key.strip_prefix(a)));
    for attempt in attempts.clone() {
        let hits = exact(attempt);
        if !hits.is_empty() {
            return hits;
        }
    }
    for attempt in attempts.clone() {
        let hits = qualified(attempt);
        if !hits.is_empty() {
            return hits;
        }
    }
    for attempt in attempts {
        let hits = near(attempt);
        if !hits.is_empty() {
            return hits;
        }
    }
    Vec::new()
}

fn pick(lang: Lang, es: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Es => es,
        Lang::En => en,
    }
}

fn region_by_code(code: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.code == code)
}

fn zone_rule(zone: &str) -> Option<Rule> {
    zones::ZONES
        .binary_search_by(|(id, _)| (*id).cmp(zone))
        .ok()
        .and_then(|i| Rule::parse(zones::ZONES[i].1))
}

/// A city as a spot, its name in `lang` (and with its region when asked).
fn city_spot(city: &City, lang: Lang, with_region: bool) -> Option<Spot> {
    let rule = zone_rule(city.zone)?;
    let name = pick(lang, city.es, city.en);
    let Some(region) = region_by_code(city.region).filter(|_| with_region) else {
        return Some(Spot {
            name: name.to_owned(),
            report: name.to_owned(),
            rule,
        });
    };
    Some(Spot {
        name: format!("{name} ({})", pick(lang, region.es, region.en)),
        report: format!("{name} ({})", city.region),
        rule,
    })
}

/// What one gazetteer hit means.
fn meaning(hit: Hit, turn: Lang) -> Option<Found> {
    let lang = hit.lang.unwrap_or(turn);
    Some(match hit.entry {
        Entry::City(city) => Found::One(city_spot(city, lang, hit.qualified)?),
        Entry::Region(region) => {
            let name = pick(lang, region.es, region.en).to_owned();
            match region.target {
                Target::Capital(capital) => {
                    let city = CITIES.iter().find(|c| c.en == capital)?;
                    let mut spot = city_spot(city, lang, false)?;
                    spot.report = name;
                    Found::One(spot)
                }
                Target::Zone(zone) => Found::One(Spot {
                    report: name.clone(),
                    name,
                    rule: zone_rule(zone)?,
                }),
                Target::Several => Found::Several(name),
                Target::Spain => Found::Spain(name),
            }
        }
    })
}

/// A choice between same-named cities that do not give the same answer.
fn options(hits: &[Hit], key: &str, turn: Lang) -> Vec<Option_> {
    hits.iter()
        .filter_map(|hit| {
            let Entry::City(city) = hit.entry else {
                return None;
            };
            let region = region_by_code(city.region)?;
            let tag = names(region.names_en)
                .chain(names(region.names))
                .next()
                .unwrap_or(region.code);
            let mut choice: Vec<String> = [region.names_es, region.names_en, region.names, region.adjectives]
                .iter()
                .flat_map(|l| names(l))
                .map(str::to_owned)
                .collect();
            choice.push(normalise(region.es).folded);
            choice.push(normalise(region.en).folded);
            Some(Option_ {
                region: pick(turn, region.es, region.en).to_owned(),
                value: format!("{key} {tag}"),
                names: choice,
            })
        })
        .take(MAX_PLACES)
        .collect()
}

/// Trailing words that are not part of a place: "ahora mismo", "please".
const TAIL_WORDS: &[&str] = &[
    "ahora",
    "mismo",
    "ahorita",
    "now",
    "right",
    "currently",
    "please",
    "porfa",
    "porfi",
    "favor",
    "por",
    "gracias",
    "thanks",
    "anda",
    "hoy",
    "today",
    "exactamente",
    "exactly",
    "momento",
    "moment",
    "este",
    "the",
    "at",
    "en",
];

/// A place phrase without the words around it, in the user's spelling and
/// in the match form.
fn trimmed(text: &str) -> (String, String) {
    let words: Vec<(&str, String)> = text
        .split_whitespace()
        .map(|w| {
            let folded = normalise(w).folded;
            let bare = folded
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '+' && c != '-')
                .to_owned();
            (w, bare)
        })
        .collect();
    let mut end = words.len();
    while end > 0 && TAIL_WORDS.contains(&words[end - 1].1.as_str()) {
        end -= 1;
    }
    let mut start = 0;
    while start + 1 < end && ["en", "in", "at", "de", "of"].contains(&words[start].1.as_str()) {
        start += 1;
    }
    let original = words[start..end]
        .iter()
        .map(|(w, _)| *w)
        .collect::<Vec<_>>()
        .join(" ");
    let original = original
        .trim_matches(|c: char| ",.;:!?¡¿\"'()«»".contains(c))
        .to_owned();
    let key = words[start..end]
        .iter()
        .map(|(_, f)| f.as_str())
        .filter(|f| !f.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (original, key)
}

/// The display name of a named time zone.
fn zone_name(zone: &str, lang: Lang) -> String {
    match zone {
        "Europe/Madrid" => pick(lang, "la España peninsular", "mainland Spain").to_owned(),
        "Atlantic/Canary" => pick(lang, "Canarias", "the Canary Islands").to_owned(),
        other => other.to_owned(),
    }
}

/// Reads one place phrase.
fn resolve(text: &str, turn: Lang, now: i64) -> Found {
    let (original, key) = trimmed(text);
    if key.is_empty() {
        return Found::Unreadable;
    }
    match extract::place(&original, &key) {
        Ok(Value::Place(PlaceRef::Home)) => return Found::Home,
        Ok(Value::Place(PlaceRef::Here)) => return Found::Here,
        Ok(Value::Place(PlaceRef::There)) => return Found::There,
        Ok(Value::Place(PlaceRef::Zone { label, offset })) => {
            let (name, rule) = match offset {
                Some(seconds) => (original.to_uppercase(), Some(Rule::fixed(seconds))),
                None => (zone_name(&label, turn), zone_rule(&label)),
            };
            return rule.map_or(Found::Unreadable, |rule| {
                Found::One(Spot {
                    report: name.clone(),
                    name,
                    rule,
                })
            });
        }
        Ok(_) => {}
        Err(_) => return Found::Unreadable,
    }
    let hits = lookup(&key);
    let mut found: Vec<(Hit, Found)> = Vec::new();
    for hit in &hits {
        let Some(meaning) = meaning(*hit, turn) else {
            continue;
        };
        let duplicate = found.iter().any(|(_, f)| match (f, &meaning) {
            (Found::One(a), Found::One(b)) => a.name == b.name && a.rule == b.rule,
            _ => false,
        });
        if !duplicate {
            found.push((*hit, meaning));
        }
    }
    let spots: Vec<&(Hit, Found)> = found
        .iter()
        .filter(|(_, f)| matches!(f, Found::One(_)))
        .collect();
    match spots.as_slice() {
        [] => found
            .into_iter()
            .next()
            .map_or(Found::Unknown(original), |(_, f)| f),
        [(_, one)] => one.clone(),
        [(first, Found::One(spot)), rest @ ..] => {
            let same = rest.iter().all(|(_, f)| match f {
                Found::One(other) => other.behaviour(now) == spot.behaviour(now),
                _ => true,
            });
            if same {
                // The same answer whichever it is: name the one taken.
                let lang = first.lang.unwrap_or(turn);
                match first.entry {
                    Entry::City(city) => city_spot(city, lang, true)
                        .map_or_else(|| Found::One(spot.clone()), Found::One),
                    Entry::Region(_) => Found::One(spot.clone()),
                }
            } else {
                // Asked about as the user wrote it ("Santiago", not
                // "Santiago de Compostela").
                let hits: Vec<Hit> = spots.iter().map(|(h, _)| *h).collect();
                let mut chars = original.chars();
                let name: String = chars
                    .next()
                    .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
                Found::Choose(name, options(&hits, &key, turn))
            }
        }
        [..] => Found::Unknown(original),
    }
}

/// Splits a phrase naming several places: "Lima y Bogotá", "Delhi and
/// Dubai", "Madrid, Tokio y Londres". A phrase that is one known name
/// ("Trinidad y Tobago") stays whole.
fn pieces(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut parts: Vec<Vec<&str>> = vec![Vec::new()];
    for word in &words {
        let folded = normalise(word).folded;
        if matches!(folded.as_str(), "y" | "e" | "and" | "&" | "," | "vs" | "vs.") {
            parts.push(Vec::new());
            continue;
        }
        let (word, comma) = match word.strip_suffix(',') {
            Some(w) => (w, true),
            None => (*word, false),
        };
        if let Some(last) = parts.last_mut()
            && !word.is_empty()
        {
            last.push(word);
        }
        if comma {
            parts.push(Vec::new());
        }
    }
    let parts: Vec<String> = parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| p.join(" "))
        .collect();
    if parts.len() > 1 && !lookup(&trimmed(text).1).is_empty() {
        return vec![text.trim().to_owned()];
    }
    parts
}

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
            Found::Unknown(name)
                if slot_text(call, "form").is_none() && slot_text(call, "follow").is_none() =>
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

    const NOW: i64 = 1_805_046_120; // 2027-03-14 17:42 UTC

    fn spot(text: &str, lang: Lang) -> Spot {
        match resolve(text, lang, NOW) {
            Found::One(spot) => spot,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn every_zone_rule_parses_and_every_place_has_one() {
        for (zone, rule) in zones::ZONES {
            assert!(Rule::parse(rule).is_some(), "{zone}: {rule}");
        }
        assert!(zones::ZONES.windows(2).all(|w| w[0].0 < w[1].0), "sorted");
        for city in CITIES {
            assert!(zone_rule(city.zone).is_some(), "{}", city.en);
            assert!(region_by_code(city.region).is_some(), "{}", city.en);
        }
        for region in REGIONS {
            match region.target {
                Target::Capital(name) => assert_eq!(
                    CITIES.iter().filter(|c| c.en == name).count(),
                    1,
                    "{}",
                    region.en
                ),
                Target::Zone(zone) => assert!(zone_rule(zone).is_some()),
                Target::Several | Target::Spain => {}
            }
        }
    }

    #[test]
    fn names_are_in_the_match_form() {
        for list in CITIES
            .iter()
            .flat_map(|c| [c.names_es, c.names_en, c.names])
            .chain(REGIONS.iter().flat_map(|r| [r.names_es, r.names_en, r.names, r.adjectives]))
        {
            for name in names(list) {
                assert_eq!(name, normalise(name).folded, "{name}");
                assert_eq!(name, name.trim());
            }
        }
    }

    #[test]
    fn names_in_both_languages_are_the_same_place() {
        assert_eq!(spot("Tokio", Lang::Es).name, "Tokio");
        assert_eq!(spot("Tokyo", Lang::Es).name, "Tokyo");
        assert_eq!(spot("pekin", Lang::Es).name, "Pekín");
        assert_eq!(spot("Sidney", Lang::Es).name, "Sídney");
        assert_eq!(spot("nyc", Lang::En).name, "New York");
        assert_eq!(spot("Istambul", Lang::Es).name, "Estambul");
        assert_eq!(spot("New York right now", Lang::En).name, "New York");
        assert_eq!(spot("México", Lang::Es).name, "Ciudad de México");
        assert_eq!(spot("México", Lang::Es).report, "México");
        assert_eq!(spot("UTC+2", Lang::Es).name, "UTC+2");
        assert_eq!(spot("cordoba argentina", Lang::Es).report, "Córdoba (AR)");
        assert_eq!(spot("Córdoba, España", Lang::Es).name, "Córdoba (España)");
        assert!(matches!(resolve("Córdoba", Lang::Es, NOW), Found::Choose(_, o) if o.len() == 2));
        assert!(matches!(resolve("Narnia", Lang::Es, NOW), Found::Unknown(_)));
        assert!(matches!(resolve("Estados Unidos", Lang::Es, NOW), Found::Several(_)));
        assert!(matches!(resolve("somewhere warm", Lang::En, NOW), Found::Unreadable));
        assert_eq!(pieces("Lima y en Bogotá"), ["Lima", "en Bogotá"]);
        assert_eq!(pieces("Bosnia y Herzegovina"), ["Bosnia y Herzegovina"]);
        assert_eq!(pieces("madrid and tokyo and london and sydney").len(), 4);
    }

    #[test]
    fn offsets_on_the_specification_date() {
        for (place, hours) in [
            ("Madrid", 60),
            ("Londres", 0),
            ("Canarias", 0),
            ("Nueva York", -240),
            ("Chicago", -300),
            ("Los Angeles", -420),
            ("Mexico City", -360),
            ("Bogotá", -300),
            ("Buenos Aires", -180),
            ("Tokio", 540),
            ("Pekín", 480),
            ("Sydney", 660),
            ("Delhi", 330),
            ("Dubai", 240),
        ] {
            assert_eq!(spot(place, Lang::Es).offset(NOW) / 60, hours, "{place}");
        }
    }

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
        assert!(within_one("sidney", "sydney"));
        assert!(!within_one("paris", "parma"));
    }

    /// From the instant the table is exact (Cuba's return to its rules on
    /// 14 March 2027, the latest of the IANA database's one-off changes), to
    /// the end of 2036, every change of offset that Python's `zoneinfo`
    /// computes from the full IANA database (`timezones/transitions.txt`,
    /// see docs/DEVELOPMENT.md, "Zone data") is a change the rule table
    /// gives, and there is no other.
    #[test]
    fn the_rules_agree_with_the_iana_database() {
        let from = Date::new(2027, 3, 14).unwrap().days_since_epoch() * 86_400 + 5 * 3_600;
        let to = Date::new(2037, 1, 1).unwrap().days_since_epoch() * 86_400;
        let text = include_str!("timezones/transitions.txt");
        let mut zones: std::collections::BTreeMap<&str, Vec<(i64, i32)>> =
            std::collections::BTreeMap::new();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let fields: Vec<&str> = line.split(' ').collect();
            let [zone, t, o] = fields[..] else {
                panic!("{line}");
            };
            zones
                .entry(zone)
                .or_default()
                .push((t.parse().unwrap(), o.parse().unwrap()));
        }
        assert_eq!(zones.len(), zones::ZONES.len());
        let mut wrong = Vec::new();
        for (zone, listed) in zones {
            let rule = zone_rule(zone).unwrap();
            let at_from = listed.iter().take_while(|(t, _)| *t <= from).last().unwrap().1;
            let expected: Vec<(i64, i32)> = listed.into_iter().filter(|(t, _)| *t > from).collect();
            let mut current = rule.offset_at(from);
            if current != at_from {
                wrong.push(zone);
            }
            let mut actual = Vec::new();
            for (t, o) in rule.changes(from, to) {
                if o != current {
                    actual.push((t, o));
                    current = o;
                }
            }
            if actual != expected {
                wrong.push(zone);
            }
        }
        assert!(wrong.is_empty(), "{wrong:?}");
    }
}
