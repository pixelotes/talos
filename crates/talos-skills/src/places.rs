//! Places: the gazetteer of cities and regions (`table`), their time zones
//! (`zones`, `tz`), and the reading of a place phrase against them
//! ([`resolve`]). Shared by `timezones`, which tells the time there, and
//! `preferences`, which remembers where the user lives in a form
//! `timezones` resolves to one place (docs/spec/state.md).
//!
//! Each zone is a POSIX TZ rule from the IANA database (`zones.rs`),
//! evaluated at the turn's injected `now` by `tz.rs`: no host zoneinfo is
//! read, so the same turn gives the same answer on every machine.

mod table;
mod tz;
mod zones;

use talos_core::extract;
use talos_core::normalise::normalise;
use talos_core::{Date, Lang, PlaceRef, Value};

pub(crate) use table::CITIES;
use table::{City, REGIONS, Region, Target};
pub(crate) use tz::Rule;

/// At most this many places in one question or one choice.
pub(crate) const MAX_PLACES: usize = 3;
/// How far ahead a coming change of summer time is mentioned, in seconds.
pub(crate) const NOTICE: i64 = 30 * 86_400;

/// A place with a clock: what to call it in the reply, what to put on the
/// outcome, and its rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spot {
    pub(crate) name: String,
    pub(crate) report: String,
    pub(crate) rule: Rule,
}

impl Spot {
    pub(crate) fn offset(&self, now: i64) -> i32 {
        self.rule.offset_at(now)
    }

    /// The local wall clock: the date and the minute of the day.
    pub(crate) fn local(&self, now: i64) -> (Date, u32) {
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
pub(crate) struct Option_ {
    /// The region that tells it apart ("Argentina"), in the turn's language.
    pub(crate) region: String,
    /// Text that names this option alone when read back as a place.
    pub(crate) value: String,
    /// How the user may pick it.
    pub(crate) names: Vec<String>,
}

/// What a place phrase turned out to be.
#[derive(Debug, Clone)]
pub(crate) enum Found {
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
        if [city.names_es, city.names_en, city.names]
            .iter()
            .any(|l| close(l))
        {
            hits.push(Hit {
                entry: Entry::City(city),
                lang: None,
                qualified: false,
            });
        }
    }
    for region in REGIONS {
        if [region.names_es, region.names_en, region.names]
            .iter()
            .any(|l| close(l))
        {
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

pub(crate) fn pick(lang: Lang, es: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Es => es,
        Lang::En => en,
    }
}

pub(crate) fn region_by_code(code: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.code == code)
}

pub(crate) fn zone_rule(zone: &str) -> Option<Rule> {
    zones::ZONES
        .binary_search_by(|(id, _)| (*id).cmp(zone))
        .ok()
        .and_then(|i| Rule::parse(zones::ZONES[i].1))
}

/// A city as a spot, its name in `lang` (and with its region when asked).
pub(crate) fn city_spot(city: &City, lang: Lang, with_region: bool) -> Option<Spot> {
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
            let mut choice: Vec<String> = [
                region.names_es,
                region.names_en,
                region.names,
                region.adjectives,
            ]
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
pub(crate) fn trimmed(text: &str) -> (String, String) {
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

/// What to remember of a place the user named (`preferences`): its name
/// as Talos writes it, in a form that [`resolve`] reads back as this one
/// place. A city keeps its region when it needed one ("Valencia
/// (España)"); a country answered through its capital stays the country
/// ("Francia", not "París").
pub(crate) fn stored_name(spot: &Spot) -> String {
    if spot.report == spot.name || spot.report.ends_with(')') {
        spot.name.clone()
    } else {
        spot.report.clone()
    }
}

/// The display name of a named time zone.
pub(crate) fn zone_name(zone: &str, lang: Lang) -> String {
    match zone {
        "Europe/Madrid" => pick(lang, "la España peninsular", "mainland Spain").to_owned(),
        "Atlantic/Canary" => pick(lang, "Canarias", "the Canary Islands").to_owned(),
        other => other.to_owned(),
    }
}

/// Reads one place phrase.
pub(crate) fn resolve(text: &str, turn: Lang, now: i64) -> Found {
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
pub(crate) fn pieces(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut parts: Vec<Vec<&str>> = vec![Vec::new()];
    for word in &words {
        let folded = normalise(word).folded;
        if matches!(
            folded.as_str(),
            "y" | "e" | "and" | "&" | "," | "vs" | "vs."
        ) {
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
    fn within_one_letter() {
        assert!(within_one("sidney", "sydney"));
        assert!(!within_one("paris", "parma"));
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
            .chain(
                REGIONS
                    .iter()
                    .flat_map(|r| [r.names_es, r.names_en, r.names, r.adjectives]),
            )
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
        assert!(matches!(
            resolve("Narnia", Lang::Es, NOW),
            Found::Unknown(_)
        ));
        assert!(matches!(
            resolve("Estados Unidos", Lang::Es, NOW),
            Found::Several(_)
        ));
        assert!(matches!(
            resolve("somewhere warm", Lang::En, NOW),
            Found::Unreadable
        ));
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

    /// From the instant the table is exact (Cuba's return to its rules on
    /// 14 March 2027, the latest of the IANA database's one-off changes), to
    /// the end of 2036, every change of offset that Python's `zoneinfo`
    /// computes from the full IANA database (`places/transitions.txt`,
    /// see docs/DEVELOPMENT.md, "Zone data") is a change the rule table
    /// gives, and there is no other.
    #[test]
    fn the_rules_agree_with_the_iana_database() {
        let from = Date::new(2027, 3, 14).unwrap().days_since_epoch() * 86_400 + 5 * 3_600;
        let to = Date::new(2037, 1, 1).unwrap().days_since_epoch() * 86_400;
        let text = include_str!("places/transitions.txt");
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
            let at_from = listed
                .iter()
                .take_while(|(t, _)| *t <= from)
                .last()
                .unwrap()
                .1;
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
