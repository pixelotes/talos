//! `preferences`: the user's name, place and units, remembered across turns
//! and restarts (docs/spec/state.md). The handler reads what is stored from
//! `cx.preferences()` and asks the engine for every change with
//! `cx.remember(…)`; the engine stores it. A request the classifier guessed
//! changes nothing until the user says yes (DESIGN.md section 11).
//!
//! A place is read with the place table `timezones` uses (`crate::places`):
//! an ambiguous name is asked about as a choice, a country with several
//! zones asks for a city, a place the table does not know is not stored,
//! and what is stored is the name the table reads back as that one place,
//! so that `timezones` takes it as the reference place.

use talos_core::normalise::{fold, normalise};

use crate::places::{self, Found, Option_};
use talos_core::{
    Call, Choice, Context, Expectation, Lang, Message, PlaceRef, PreferenceChange, Preferences,
    Skill, SkillResult, Units, Value, extract,
};

/// The `preferences` handler.
#[derive(Debug, Default)]
pub(crate) struct Memory;

/// Every reply id the handler can return.
const REPLIES: &[&str] = &[
    "set_name",
    "set_name_call",
    "set_place_live",
    "set_place_are",
    "set_units",
    "set_ask_name",
    "set_ask_place",
    "set_ask_city",
    "set_ask_zones",
    "set_ask_units",
    "set_confirm_name",
    "set_confirm_place",
    "set_confirm_units",
    "set_kept",
    "set_other",
    "set_unknown_place",
    "set_which",
    "set_which3",
    "get_name",
    "get_name_yes",
    "get_name_know",
    "get_name_bare",
    "get_place",
    "get_place_where",
    "get_units",
    "get_all_n",
    "get_all_p",
    "get_all_u",
    "get_all_np",
    "get_all_nu",
    "get_all_pu",
    "get_all_npu",
    "get_none_name",
    "get_none_who",
    "get_none_place",
    "get_none_units",
    "get_none_all",
    "get_other",
    "forget_name",
    "forget_place",
    "forget_units",
    "forget_all",
    "forget_none_name",
    "forget_none_place",
    "forget_none_units",
    "forget_none_all",
    "forget_confirm_n",
    "forget_confirm_p",
    "forget_confirm_u",
    "forget_confirm_np",
    "forget_confirm_nu",
    "forget_confirm_pu",
    "forget_confirm_npu",
    "forget_confirm_one_name",
    "forget_confirm_one_place",
    "forget_confirm_one_units",
    "forget_kept",
    "forget_which",
    "forget_other",
    "forget_conversation",
    "word_metric",
    "word_imperial",
];

/// Which preference a request is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Name,
    Place,
    Units,
    /// Everything stored.
    All,
    /// Something Talos does not keep ("mi color favorito").
    Other,
    /// The conversation itself ("forget our conversation").
    Conversation,
}

impl Key {
    const fn id(self) -> &'static str {
        match self {
            Key::Name => "name",
            Key::Place => "place",
            Key::Units => "units",
            Key::All => "all",
            Key::Other => "other",
            Key::Conversation => "conversation",
        }
    }
}

/// The match form of a text.
fn folded(text: &str) -> String {
    fold(&text.to_lowercase())
}

/// The `key` slot: an id a pattern fixed or an answer chose, or a phrase a
/// pattern captured ("mi ciudad", "where I live"), mapped by its words.
fn key(call: &Call<'_>) -> Option<Key> {
    let text = folded(call.slots.get("key")?.as_text()?);
    let exact = [
        Key::Name,
        Key::Place,
        Key::Units,
        Key::All,
        Key::Other,
        Key::Conversation,
    ]
    .into_iter()
    .find(|k| k.id() == text);
    if exact.is_some() {
        return exact;
    }
    let has = |words: &[&str]| text.split_whitespace().any(|w| words.contains(&w));
    Some(if has(&["nombre", "llamo", "name"]) {
        Key::Name
    } else if has(&[
        "ciudad",
        "vivo",
        "lugar",
        "ubicacion",
        "location",
        "city",
        "live",
        "place",
    ]) {
        Key::Place
    } else if has(&["unidades", "units"]) {
        Key::Units
    } else {
        Key::Other
    })
}

fn text<'a>(call: &'a Call<'_>, name: &str) -> Option<&'a str> {
    call.slots
        .get(name)
        .and_then(Value::as_text)
        .filter(|t| !t.trim().is_empty())
}

fn confirm(call: &Call<'_>) -> Option<bool> {
    call.slots.get("confirm").and_then(Value::as_confirm)
}

/// The manifest's word for units in a language (`word_*` replies), so that
/// no wording is written in code.
fn units_word(cx: &Context<'_>, lang: Lang, units: Units) -> String {
    let id = match units {
        Units::Metric => "word_metric",
        Units::Imperial => "word_imperial",
    };
    cx.skills()
        .iter()
        .find(|m| m.id == "preferences")
        .and_then(|m| m.replies.get(id))
        .and_then(|r| r.get(lang).first())
        .cloned()
        .unwrap_or_else(|| units.as_str().to_owned())
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Words that do not start a capitalised place-name word unless first.
const PARTICLES: &[&str] = &[
    "de", "del", "la", "las", "los", "el", "y", "e", "of", "the", "upon", "on", "and",
];

/// A name or a place as stored: as typed, but with capitals added when it
/// was typed all in lower case ("ana maría" is "Ana María"; "ciudad de
/// méxico" is "Ciudad de México").
fn with_capitals(text: &str) -> String {
    if text.chars().any(char::is_uppercase) {
        return text.to_owned();
    }
    text.split(' ')
        .enumerate()
        .map(|(i, word)| {
            if i > 0 && PARTICLES.contains(&folded(word).as_str()) {
                word.to_owned()
            } else {
                capitalise(word)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What may open an answer that gives a name: "it's Sam", "soy Raúl".
const NAME_LEADS: &[&[&str]] = &[
    &["it's"],
    &["its"],
    &["it", "is"],
    &["es"],
    &["soy"],
    &["me", "llamo"],
    &["mi", "nombre", "es"],
    &["my", "name", "is"],
    &["my", "name's"],
    &["i'm"],
    &["im"],
    &["i", "am"],
    &["call", "me"],
    &["llamame"],
    &["pues"],
];

/// What may close it: "Raúl, por favor".
const NAME_TAILS: &[&[&str]] = &[
    &["a", "partir", "de", "ahora"],
    &["de", "ahora", "en", "adelante"],
    &["from", "now", "on"],
    &["por", "favor"],
    &["porfa"],
    &["please"],
    &["gracias"],
    &["thanks"],
];

/// The name in a `value`, its framing removed, quotes and surrounding
/// punctuation dropped.
fn name_of(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let keys: Vec<String> = words
        .iter()
        .map(|w| folded(w.trim_matches(|c: char| ",.;:!?¡¿".contains(c))))
        .collect();
    let (mut from, mut to) = (0, words.len());
    'lead: loop {
        for lead in NAME_LEADS {
            let n = lead.len();
            if to - from > n && keys[from..from + n].iter().zip(*lead).all(|(k, l)| k == l) {
                from += n;
                continue 'lead;
            }
        }
        break;
    }
    'tail: loop {
        for tail in NAME_TAILS {
            let n = tail.len();
            if to - from > n && keys[to - n..to].iter().zip(*tail).all(|(k, l)| k == l) {
                to -= n;
                continue 'tail;
            }
        }
        break;
    }
    let name = words[from..to].join(" ");
    let quotes: &[char] = &['"', '\'', '«', '»', '“', '”', '‘', '’', ',', '.', ';', ':', '!', '?'];
    // Quotes go wherever they are at the ends; a final full stop goes, an
    // abbreviation's inside stays ("Dr. Sam").
    name.trim_matches(|c: char| quotes.contains(&c) || c.is_whitespace())
        .to_owned()
}

/// Capitalised words after "soy"/"I'm" that are not names: nationalities,
/// beliefs, states.
const NOT_NAMES: &[&str] = &[
    "spanish", "english", "british", "american", "french", "german", "italian", "mexican",
    "canadian", "irish", "scottish", "welsh", "australian", "chinese", "japanese", "portuguese",
    "dutch", "catholic", "christian", "muslim", "jewish", "vegan", "vegetarian", "ok", "okay",
    "fine", "good", "here", "back", "done", "ready", "sorry", "espanol", "espanola", "ingles",
    "inglesa", "frances", "francesa", "aleman", "alemana", "italiano", "italiana", "mexicano",
    "mexicana", "argentino", "argentina", "yo", "i",
];

/// Whether what follows "soy"/"I'm" reads as a name: it starts with a
/// capital, as names do, and is not a nationality or a state.
fn looks_like_name(raw: &str) -> bool {
    raw.chars().next().is_some_and(char::is_uppercase)
        && !raw
            .split_whitespace()
            .any(|w| NOT_NAMES.contains(&folded(w).as_str()))
}

/// Words that, first after "llámame"/"call me", make it a request to be
/// called (on the phone, later) or an idiom, not a name.
const NOT_CALLED: &[&str] = &[
    "manana", "luego", "despues", "cuando", "si", "en", "a", "al", "por", "un", "una", "esta", "este", "ahora",
    "pronto", "tomorrow", "later", "when", "whenever", "if", "back", "at", "on", "in", "an", "tonight", "soon",
    "crazy", "now",
];

/// Whether what follows "llámame"/"call me" can be a name.
fn called(raw: &str) -> bool {
    let first = raw.split_whitespace().next().map_or_else(String::new, folded);
    let first = first.trim_matches(|c: char| !c.is_alphanumeric());
    !NOT_CALLED.contains(&first)
}

/// Words that start a "estoy en …"/"I'm in …" that is a situation, not a
/// place ("estoy en el trabajo", "I'm in bed").
const NOT_PLACES: &[&str] = &[
    "el", "la", "los", "las", "un", "una", "unos", "unas", "ello", "eso", "esto", "ese", "esa",
    "casa", "clase", "camino", "reunion", "mi", "tu", "su", "nuestro", "nuestra", "contra",
    "favor", "paro", "the", "a", "an", "it", "that", "this", "bed", "work", "class", "love",
    "trouble", "my", "your", "his", "her", "our", "their", "pain", "charge", "luck", "favour",
    "favor", "time", "shock", "hospital", "town",
];

/// Unit words of each system, by the start of their match form.
const METRIC_WORDS: &[&str] = &[
    "metric", "kilometr", "kilometer", "metro", "meter", "metre", "celsius", "centigrad", "kilo",
    "gram", "litro", "liter", "litre", "internacional",
];
const IMPERIAL_WORDS: &[&str] = &[
    "imperial", "milla", "mile", "fahrenheit", "libra", "pound", "pie", "feet", "foot", "pulgada",
    "inch", "onza", "ounce", "galon", "gallon", "yarda", "yard",
];

/// The unit system a phrase prefers: "millas y fahrenheit, no kilómetros"
/// is imperial. Clauses that start with "no"/"not" are what the user does
/// not want and do not count.
fn units_of(raw: &str) -> Option<Units> {
    let text = folded(raw);
    let (mut metric, mut imperial) = (0, 0);
    for clause in text.split([',', ';']) {
        let words: Vec<&str> = clause
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        if matches!(words.first(), Some(&("no" | "not" | "sin" | "without"))) {
            continue;
        }
        for word in words {
            if METRIC_WORDS.iter().any(|m| word.starts_with(m)) {
                metric += 1;
            } else if IMPERIAL_WORDS.iter().any(|m| word.starts_with(m)) {
                imperial += 1;
            }
        }
    }
    match metric.cmp(&imperial) {
        std::cmp::Ordering::Greater => Some(Units::Metric),
        std::cmp::Ordering::Less => Some(Units::Imperial),
        std::cmp::Ordering::Equal => None,
    }
}

/// The options of "¿métricas o imperiales?".
fn units_choice() -> Vec<Choice> {
    vec![
        Choice::new(
            "metric",
            [
                "metric",
                "métricas",
                "métricos",
                "métrica",
                "métrico",
                "sistema métrico",
                "unidades métricas",
                "the metric system",
                "metric units",
            ],
        ),
        Choice::new(
            "imperial",
            [
                "imperial",
                "imperiales",
                "sistema imperial",
                "unidades imperiales",
                "the imperial system",
                "imperial units",
            ],
        ),
    ]
}

/// The options of "¿qué olvido?".
fn key_choice() -> Vec<Choice> {
    vec![
        Choice::new("name", ["nombre", "tu nombre", "el nombre", "name", "my name", "the name"]),
        Choice::new(
            "place",
            [
                "lugar",
                "tu lugar",
                "el lugar",
                "ciudad",
                "la ciudad",
                "donde vivo",
                "location",
                "my location",
                "the location",
                "place",
                "city",
                "where i live",
            ],
        ),
        Choice::new(
            "units",
            ["unidades", "tus unidades", "las unidades", "units", "my units", "the units"],
        ),
    ]
}

/// "¿Valencia de España o de Venezuela?": a name of several places that
/// differ, as the place table reads it. The chosen option's value names
/// that one place alone, so the answer completes the request.
fn ask_which(name: &str, options: &[Option_]) -> SkillResult {
    let choices: Vec<Choice> = options
        .iter()
        .map(|o| Choice::new(o.value.clone(), o.names.iter()))
        .collect();
    let message = match options {
        [a, b] => Message::new("set_which")
            .var("name", name)
            .var("a", &a.region)
            .var("b", &b.region),
        [a, b, c, ..] => Message::new("set_which3")
            .var("name", name)
            .var("a", &a.region)
            .var("b", &b.region)
            .var("c", &c.region),
        _ => return SkillResult::Ask(Expectation::slot("value"), Message::new("set_ask_city")),
    };
    SkillResult::Ask(Expectation::choice("value", choices), message)
}

/// A confirmation question, or a "no" answered.
fn ask_confirm(message: Message) -> SkillResult {
    SkillResult::Ask(Expectation::confirm("confirm"), message)
}

impl Memory {
    fn set(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let guessed = cx.guessed();
        if confirm(call) == Some(false) {
            return SkillResult::Answer(Message::new("set_kept"));
        }
        let Some(key) = key(call) else {
            return SkillResult::Decline;
        };
        cx.report("key", Value::Text(key.id().to_owned()));
        match key {
            Key::Name => Self::set_name(cx, call, guessed),
            Key::Place => Self::set_place(cx, call, guessed),
            Key::Units => Self::set_units(cx, call, guessed),
            Key::Other if !guessed => SkillResult::Unable(Message::new("set_other")),
            Key::Other | Key::All | Key::Conversation => SkillResult::Decline,
        }
    }

    fn set_name(cx: &mut Context<'_>, call: &Call<'_>, guessed: bool) -> SkillResult {
        let form = text(call, "form");
        let Some(raw) = text(call, "value") else {
            return if guessed {
                SkillResult::Decline
            } else {
                SkillResult::Ask(Expectation::slot("value"), Message::new("set_ask_name"))
            };
        };
        if form == Some("call") && !called(raw) {
            // "llámame mañana", "call me a taxi": not a name.
            return SkillResult::Decline;
        }
        if form == Some("soy") && !looks_like_name(raw) {
            // "soy muy pesado", "I'm tired": not a name; the reflector's.
            return SkillResult::Decline;
        }
        let name = with_capitals(&name_of(raw));
        if name.is_empty() {
            return SkillResult::Ask(Expectation::slot("value"), Message::new("set_ask_name"));
        }
        cx.report("value", Value::Text(name.clone()));
        if guessed && confirm(call) != Some(true) {
            return ask_confirm(Message::new("set_confirm_name").var("name", name));
        }
        cx.remember(PreferenceChange::Name(Some(name.clone())));
        let id = if form == Some("call") {
            "set_name_call"
        } else {
            "set_name"
        };
        SkillResult::Answer(Message::new(id).var("name", name))
    }

    fn set_place(cx: &mut Context<'_>, call: &Call<'_>, guessed: bool) -> SkillResult {
        let form = text(call, "form");
        let Some(raw) = text(call, "value") else {
            if guessed {
                return SkillResult::Decline;
            }
            let id = if form == Some("city") {
                "set_ask_city"
            } else {
                "set_ask_place"
            };
            return SkillResult::Ask(Expectation::slot("value"), Message::new(id));
        };
        let n = normalise(raw);
        let first = n.folded.split_whitespace().next().unwrap_or("");
        if form == Some("are") && NOT_PLACES.contains(&first) {
            // "estoy en el trabajo", "I'm in bed": where the user is, not a
            // place to remember.
            return SkillResult::Decline;
        }
        let unknown = || {
            if guessed {
                SkillResult::Decline
            } else {
                SkillResult::Unable(Message::new("set_unknown_place"))
            }
        };
        let (place, reported) = match extract::place(&n.original, &n.folded) {
            Ok(Value::Place(PlaceRef::Name(name))) => {
                match places::resolve(&name, call.lang, cx.now().unix_seconds) {
                    // Stored as Talos writes it, in a form the place table
                    // reads back as this one place: "Valencia (España)"
                    // after a choice, "Francia" (not its capital).
                    Found::One(spot) => (places::stored_name(&spot), spot.report),
                    Found::Spain(name) => (name.clone(), name),
                    Found::Several(_) => {
                        return SkillResult::Ask(
                            Expectation::slot("value"),
                            Message::new("set_ask_zones"),
                        );
                    }
                    Found::Choose(name, options) => return ask_which(&name, &options),
                    // "vivo en el quinto piso": an unknown name that starts
                    // with an article is a situation, not a place.
                    Found::Unknown(_) if NOT_PLACES.contains(&first) => return SkillResult::Decline,
                    Found::Unknown(_) | Found::Unreadable => return unknown(),
                    Found::Home | Found::Here | Found::There => return SkillResult::Decline,
                }
            }
            Ok(Value::Place(PlaceRef::Zone { label, .. })) => (label.clone(), label),
            Ok(_) => return SkillResult::Decline,
            Err(_) => return unknown(),
        };
        cx.report("value", Value::Text(reported));
        if guessed && confirm(call) != Some(true) {
            return ask_confirm(Message::new("set_confirm_place").var("place", place));
        }
        cx.remember(PreferenceChange::Place(Some(place.clone())));
        let id = if form == Some("are") {
            "set_place_are"
        } else {
            "set_place_live"
        };
        SkillResult::Answer(Message::new(id).var("place", place))
    }

    fn set_units(cx: &mut Context<'_>, call: &Call<'_>, guessed: bool) -> SkillResult {
        let Some(raw) = text(call, "value") else {
            return if guessed {
                SkillResult::Decline
            } else {
                SkillResult::Ask(
                    Expectation::choice("value", units_choice()),
                    Message::new("set_ask_units"),
                )
            };
        };
        let Some(units) = Units::parse(&folded(raw)).or_else(|| units_of(raw)) else {
            return SkillResult::Decline;
        };
        cx.report("value", Value::Text(units.as_str().to_owned()));
        let word = units_word(cx, call.lang, units);
        if guessed && confirm(call) != Some(true) {
            return ask_confirm(Message::new("set_confirm_units").var("units", word));
        }
        cx.remember(PreferenceChange::Units(Some(units)));
        SkillResult::Answer(Message::new("set_units").var("units", word))
    }

    fn get(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        // A guess that names nothing Talos keeps is not a question about
        // the user ("para mí que…").
        let Some(key) = key(call).or((!cx.guessed()).then_some(Key::All)) else {
            return SkillResult::Decline;
        };
        cx.report("key", Value::Text(key.id().to_owned()));
        let form = text(call, "form");
        let stored = cx.preferences().clone();
        let lang = call.lang;
        match key {
            Key::Name => match stored.name() {
                Some(name) => {
                    let id = match form {
                        Some("yes") => "get_name_yes",
                        Some("know") => "get_name_know",
                        Some("bare") => "get_name_bare",
                        _ => "get_name",
                    };
                    SkillResult::Answer(Message::new(id).var("name", name))
                }
                None if form == Some("who") => SkillResult::Answer(Message::new("get_none_who")),
                None => SkillResult::Answer(Message::new("get_none_name")),
            },
            Key::Place => match stored.place() {
                Some(place) => {
                    let id = if form == Some("where") {
                        "get_place_where"
                    } else {
                        "get_place"
                    };
                    SkillResult::Answer(Message::new(id).var("place", place))
                }
                None => SkillResult::Answer(Message::new("get_none_place")),
            },
            Key::Units => match stored.units() {
                Some(units) => {
                    let word = units_word(cx, lang, units);
                    let word = if lang == Lang::En {
                        capitalise(&word)
                    } else {
                        word
                    };
                    SkillResult::Answer(Message::new("get_units").var("units", word))
                }
                None => SkillResult::Answer(Message::new("get_none_units")),
            },
            Key::All => Self::get_all(cx, &stored, lang),
            Key::Other | Key::Conversation => SkillResult::Unable(Message::new("get_other")),
        }
    }

    fn get_all(cx: &Context<'_>, stored: &Preferences, lang: Lang) -> SkillResult {
        let id = match (
            stored.name().is_some(),
            stored.place().is_some(),
            stored.units().is_some(),
        ) {
            (false, false, false) => return SkillResult::Answer(Message::new("get_none_all")),
            (true, false, false) => "get_all_n",
            (false, true, false) => "get_all_p",
            (false, false, true) => "get_all_u",
            (true, true, false) => "get_all_np",
            (true, false, true) => "get_all_nu",
            (false, true, true) => "get_all_pu",
            (true, true, true) => "get_all_npu",
        };
        let mut message = Message::new(id);
        if let Some(name) = stored.name() {
            message = message.var("name", name);
        }
        if let Some(place) = stored.place() {
            message = message.var("place", place);
        }
        if let Some(units) = stored.units() {
            message = message.var("units", units_word(cx, lang, units));
        }
        SkillResult::Answer(message)
    }

    fn forget(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        let guessed = cx.guessed();
        let Some(key) = key(call) else {
            return if guessed {
                SkillResult::Decline
            } else {
                SkillResult::Ask(
                    Expectation::choice("key", key_choice()),
                    Message::new("forget_which"),
                )
            };
        };
        cx.report("key", Value::Text(key.id().to_owned()));
        if confirm(call) == Some(false) {
            return SkillResult::Answer(Message::new("forget_kept"));
        }
        let stored = cx.preferences().clone();
        let (change, stored_now, done, none, one) = match key {
            Key::Name => (
                PreferenceChange::Name(None),
                stored.name().is_some(),
                "forget_name",
                "forget_none_name",
                "forget_confirm_one_name",
            ),
            Key::Place => (
                PreferenceChange::Place(None),
                stored.place().is_some(),
                "forget_place",
                "forget_none_place",
                "forget_confirm_one_place",
            ),
            Key::Units => (
                PreferenceChange::Units(None),
                stored.units().is_some(),
                "forget_units",
                "forget_none_units",
                "forget_confirm_one_units",
            ),
            Key::All => return Self::forget_all(cx, call, &stored),
            Key::Other if guessed => return SkillResult::Decline,
            Key::Other => return SkillResult::Unable(Message::new("forget_other")),
            Key::Conversation => {
                return SkillResult::Unable(Message::new("forget_conversation"));
            }
        };
        if !stored_now {
            return SkillResult::Answer(Message::new(none));
        }
        if guessed && confirm(call) != Some(true) {
            return ask_confirm(Message::new(one));
        }
        cx.remember(change);
        SkillResult::Answer(Message::new(done))
    }

    /// Forgetting everything is permanent, so it is always confirmed first;
    /// with nothing stored there is nothing to confirm.
    fn forget_all(cx: &mut Context<'_>, call: &Call<'_>, stored: &Preferences) -> SkillResult {
        let question = match (
            stored.name().is_some(),
            stored.place().is_some(),
            stored.units().is_some(),
        ) {
            (false, false, false) => return SkillResult::Answer(Message::new("forget_none_all")),
            (true, false, false) => "forget_confirm_n",
            (false, true, false) => "forget_confirm_p",
            (false, false, true) => "forget_confirm_u",
            (true, true, false) => "forget_confirm_np",
            (true, false, true) => "forget_confirm_nu",
            (false, true, true) => "forget_confirm_pu",
            (true, true, true) => "forget_confirm_npu",
        };
        if confirm(call) != Some(true) {
            return ask_confirm(Message::new(question));
        }
        cx.remember(PreferenceChange::Name(None));
        cx.remember(PreferenceChange::Place(None));
        cx.remember(PreferenceChange::Units(None));
        SkillResult::Answer(Message::new("forget_all"))
    }
}

impl Skill for Memory {
    fn id(&self) -> &'static str {
        "preferences"
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "preferences.set" => Self::set(cx, call),
            "preferences.get" => Self::get(cx, call),
            "preferences.forget" => Self::forget(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{IntentId, Random, Slots, Timestamp};

    fn call(
        intent: &str,
        slots: &[(&str, Value)],
        prefs: &Preferences,
        guessed: bool,
    ) -> SkillResult {
        let id = IntentId(intent.to_owned());
        let slots: Slots = slots
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let call = Call {
            intent: &id,
            lang: Lang::Es,
            slots: &slots,
            text: "",
        };
        let mut cx = Context::standalone(Timestamp::from_unix(0), Random::seeded(0))
            .with_preferences(prefs);
        if guessed {
            cx = cx.as_guess();
        }
        Memory.handle(&mut cx, &call)
    }

    fn t(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn id(result: &SkillResult) -> &'static str {
        match result {
            SkillResult::Answer(m) | SkillResult::Unable(m) | SkillResult::Ask(_, m) => m.id,
            SkillResult::Decline => "decline",
            SkillResult::Fail(_) => "fail",
        }
    }

    fn var(result: &SkillResult, name: &str) -> String {
        match result {
            SkillResult::Answer(m) | SkillResult::Unable(m) | SkillResult::Ask(_, m) => {
                m.vars.get(name).cloned().unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    #[test]
    fn names_are_cleaned_and_capitalised() {
        assert_eq!(name_of("\"Dr. Sam O'Neil\""), "Dr. Sam O'Neil");
        assert_eq!(name_of("it's Sam"), "Sam");
        assert_eq!(name_of("Raúl, por favor"), "Raúl");
        assert_eq!(with_capitals("ana maría"), "Ana María");
        assert_eq!(with_capitals("ciudad de méxico"), "Ciudad de México");
        assert_eq!(with_capitals("McDonald"), "McDonald");
        assert!(looks_like_name("Raúl"));
        assert!(!looks_like_name("muy pesado"));
        assert!(!looks_like_name("Spanish"));
    }

    #[test]
    fn units_follow_the_words_the_user_wants() {
        assert_eq!(units_of("millas y fahrenheit, no kilómetros"), Some(Units::Imperial));
        assert_eq!(units_of("unidades métricas"), Some(Units::Metric));
        assert_eq!(units_of("kilometres and Celsius"), Some(Units::Metric));
        assert_eq!(units_of("té"), None);
    }

    #[test]
    fn a_guess_asks_before_it_stores() {
        let none = Preferences::NONE;
        let slots = [("key", t("name")), ("value", t("Rulo"))];
        let r = call("preferences.set", &slots, &none, true);
        assert_eq!(id(&r), "set_confirm_name");
        let mut yes = slots.to_vec();
        yes.push(("confirm", Value::Confirm(true)));
        assert_eq!(id(&call("preferences.set", &yes, &none, true)), "set_name");
        let mut no = slots.to_vec();
        no.push(("confirm", Value::Confirm(false)));
        assert_eq!(id(&call("preferences.set", &no, &none, true)), "set_kept");
        // Without a value, a guess is not the skill's.
        assert_eq!(id(&call("preferences.set", &[("key", t("name"))], &none, true)), "decline");
        assert_eq!(id(&call("preferences.set", &[("key", t("name"))], &none, false)), "set_ask_name");
    }

    #[test]
    fn places_are_read_and_checked() {
        let none = Preferences::NONE;
        let place = |v: &str, form: Option<&str>| {
            let mut slots = vec![("key", t("place")), ("value", t(v))];
            if let Some(f) = form {
                slots.push(("form", t(f)));
            }
            call("preferences.set", &slots, &none, false)
        };
        let r = place("new york", Some("are"));
        assert_eq!((id(&r), var(&r, "place")), ("set_place_are", "New York".to_owned()));
        assert_eq!(id(&place("el trabajo", Some("are"))), "decline");
        assert_eq!(id(&place("Estados Unidos", None)), "set_ask_zones");
        assert_eq!(id(&place("algún sitio", None)), "set_unknown_place");
    }

    #[test]
    fn reading_and_forgetting() {
        let all = Preferences::NONE
            .with_name("Raúl")
            .with_place("Sevilla")
            .with_units(Units::Metric);
        let r = call("preferences.get", &[("key", t("all"))], &all, false);
        assert_eq!(id(&r), "get_all_npu");
        let r = call("preferences.get", &[], &Preferences::NONE, false);
        assert_eq!(id(&r), "get_none_all");
        let r = call("preferences.forget", &[("key", t("mi ciudad"))], &all, false);
        assert_eq!(id(&r), "forget_place");
        let r = call("preferences.forget", &[("key", t("all"))], &all, false);
        assert_eq!(id(&r), "forget_confirm_npu");
        let r = call("preferences.forget", &[("key", t("color favorito"))], &all, false);
        assert_eq!(id(&r), "forget_other");
        let r = call("preferences.forget", &[], &all, false);
        assert_eq!(id(&r), "forget_which");
        let r = call("preferences.forget", &[("key", t("name"))], &Preferences::NONE, false);
        assert_eq!(id(&r), "forget_none_name");
    }
}
