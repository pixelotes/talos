//! Places, languages, choices and yes/no (docs/spec/conversation.md 2.4,
//! 2.5, 2.7 and 3.3).

use super::{trim_edges, words};
use crate::value::{PlaceRef, Problem, Value};

/// The words left after dropping leading ones while `skip` says so.
fn after<'a>(tokens: &'a [&'a str], skip: impl Fn(&str) -> bool) -> &'a [&'a str] {
    let n = tokens.iter().take_while(|w| skip(w)).count();
    &tokens[n..]
}

// --- Places --------------------------------------------------------------------

/// Words that refer to a place without naming one, and which.
fn reference(tokens: &[&str]) -> Option<PlaceRef> {
    Some(match tokens {
        [
            "mi" | "my",
            "ciudad" | "casa" | "sitio" | "pueblo" | "city" | "town" | "place",
        ]
        | ["casa" | "home"]
        | ["donde", "vivo"]
        | ["where", "i", "live"] => PlaceRef::Home,
        ["aqui" | "aca" | "here"] | ["donde", "estoy"] | ["where", "i", "am"] => PlaceRef::Here,
        ["alli" | "alla" | "ahi" | "there"] => PlaceRef::There,
        _ => return None,
    })
}

/// Time-zone abbreviations with a fixed offset, in hours.
const ZONES: &[(&str, i32)] = &[
    ("utc", 0),
    ("gmt", 0),
    ("wet", 0),
    ("west", 1),
    ("cet", 1),
    ("cest", 2),
    ("bst", 1),
    ("eet", 2),
    ("eest", 3),
    ("est", -5),
    ("edt", -4),
    ("cst", -6),
    ("cdt", -5),
    ("mst", -7),
    ("mdt", -6),
    ("pst", -8),
    ("pdt", -7),
    ("jst", 9),
];

/// `+2`, `-5`, `+05:30`, `+0530`, as seconds.
fn offset(text: &str) -> Option<i32> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = text[1..].replace(':', "");
    if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m) = if digits.len() <= 2 {
        (digits.parse::<i32>().ok()?, 0)
    } else {
        let (h, m) = digits.split_at(digits.len() - 2);
        (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?)
    };
    (h <= 14 && m < 60).then_some(sign * (h * 3_600 + m * 60))
}

/// A time zone: "UTC", "GMT-5", "utc+05:30", "CET", "hora peninsular".
fn zone(tokens: &[&str]) -> Option<PlaceRef> {
    let named = |label: &str| PlaceRef::Zone {
        label: label.to_owned(),
        offset: None,
    };
    match tokens {
        ["hora", "peninsular"] | ["hora", "de", "la", "peninsula"] => {
            return Some(named("Europe/Madrid"));
        }
        ["hora", "canaria"] | ["hora", "de", "canarias"] => return Some(named("Atlantic/Canary")),
        _ => {}
    }
    let joined: String = tokens.concat();
    let (name, rest) = ZONES
        .iter()
        .filter(|(name, _)| joined.starts_with(name))
        .max_by_key(|(name, _)| name.len())
        .map(|&(name, hours)| (name, (hours, &joined[name.len()..])))?;
    let (hours, rest) = rest;
    let label = |seconds: i32| {
        if seconds == 0 {
            "UTC".to_owned()
        } else {
            let sign = if seconds < 0 { '-' } else { '+' };
            let a = seconds.abs();
            format!("UTC{sign}{:02}:{:02}", a / 3_600, a % 3_600 / 60)
        }
    };
    if rest.is_empty() {
        let seconds = hours * 3_600;
        let label = if matches!(name, "utc" | "gmt") {
            label(0)
        } else {
            name.to_uppercase()
        };
        return Some(PlaceRef::Zone {
            label,
            offset: Some(seconds),
        });
    }
    if !matches!(name, "utc" | "gmt") {
        return None;
    }
    let seconds = offset(rest)?;
    Some(PlaceRef::Zone {
        label: label(seconds),
        offset: Some(seconds),
    })
}

/// Words that say "anywhere" instead of naming a place.
const VAGUE_PLACES: &[&str] = &[
    "algun",
    "alguna",
    "cualquier",
    "cualquiera",
    "ningun",
    "ninguna",
    "sea",
    "somewhere",
    "anywhere",
    "wherever",
    "nowhere",
    "everywhere",
    "someplace",
];

/// Words that do not occur in a place name but do in a sentence about not
/// knowing one.
const NOT_IN_NAMES: &[&str] = &[
    "i", "know", "dont", "don't", "idea", "se", "sé", "no", "yes", "si", "what", "que", "quiero",
    "want", "think", "creo",
];

/// Reads a place (docs/spec/conversation.md 2.4) without a gazetteer: a
/// reference ("mi ciudad", "home": the remembered place; "aquí", "here":
/// where the user is; "allí", "there": the place named last), a time zone
/// or offset ("UTC+2", "GMT-5", "CET", "hora peninsular"), or else a name,
/// kept in the user's spelling with a leading preposition dropped ("en
/// Tokio" is "Tokio"). Whether a name is a real place, and which one, is for
/// the skill to decide; both languages' names reach it as typed.
///
/// `original` is the capture in the user's spelling, `folded` its match
/// form.
///
/// # Errors
///
/// [`Problem::Unreadable`] for an empty text, digits, a long phrase, or
/// words that say "anywhere" ("somewhere warm", "donde sea").
pub fn place(original: &str, folded: &str) -> Result<Value, Problem> {
    let all = words(folded);
    let tokens = after(&all, |w| {
        matches!(
            w,
            "en" | "in"
                | "at"
                | "para"
                | "for"
                | "de"
                | "from"
                | "desde"
                | "to"
                | "hacia"
                | "con"
                | "with"
        )
    });
    if tokens.is_empty() {
        return Err(Problem::Unreadable);
    }
    if let Some(found) = reference(tokens).or_else(|| zone(tokens)) {
        return Ok(Value::Place(found));
    }
    let bad = |w: &&str| {
        VAGUE_PLACES.contains(w)
            || NOT_IN_NAMES.contains(w)
            || w.chars().any(|c| c.is_ascii_digit())
    };
    if tokens.len() > 6 || tokens.iter().any(bad) || !tokens[0].starts_with(char::is_alphabetic) {
        return Err(Problem::Unreadable);
    }
    // The same words of the user's spelling.
    let dropped = all.len() - tokens.len();
    let original = trim_edges(original);
    let name = original
        .split_whitespace()
        .skip(dropped)
        .collect::<Vec<_>>()
        .join(" ");
    let name = trim_edges(&name);
    if name.is_empty() {
        return Err(Problem::Unreadable);
    }
    Ok(Value::Place(PlaceRef::Name(name.to_owned())))
}

// --- Languages -----------------------------------------------------------------

/// Language names in the match form: Spanish names, then English ones.
const LANGUAGES_ES: &[(&str, &str)] = &[
    ("espanol", "es"),
    ("castellano", "es"),
    ("ingles", "en"),
    ("frances", "fr"),
    ("aleman", "de"),
    ("italiano", "it"),
    ("portugues", "pt"),
    ("catalan", "ca"),
    ("gallego", "gl"),
    ("euskera", "eu"),
    ("vasco", "eu"),
    ("chino", "zh"),
    ("japones", "ja"),
    ("ruso", "ru"),
    ("arabe", "ar"),
    ("neerlandes", "nl"),
    ("holandes", "nl"),
];

const LANGUAGES_EN: &[(&str, &str)] = &[
    ("spanish", "es"),
    ("castilian", "es"),
    ("english", "en"),
    ("french", "fr"),
    ("german", "de"),
    ("italian", "it"),
    ("portuguese", "pt"),
    ("catalan", "ca"),
    ("galician", "gl"),
    ("basque", "eu"),
    ("chinese", "zh"),
    ("japanese", "ja"),
    ("russian", "ru"),
    ("arabic", "ar"),
    ("dutch", "nl"),
];

/// Reads a language name (docs/spec/conversation.md 2.5): "inglés",
/// "español", "castellano", "en inglés", "al español", "English",
/// "Spanish", "in Spanish", "into English". Both languages' names are
/// accepted in either conversation language ("in español" is), but a
/// Spanish article does not take an English name ("el English" is not).
/// Languages Talos does not speak are recognised by their code (`fr`,
/// `de`…) for the skill to report as unsupported. The text is the match
/// form of a capture.
///
/// # Errors
///
/// [`Problem::Unreadable`] for anything else.
pub fn language(text: &str) -> Result<Value, Problem> {
    let all = words(text);
    let (lead, name) = match all.as_slice() {
        [name] => (None, *name),
        [lead, name] => (Some(*lead), *name),
        _ => return Err(Problem::Unreadable),
    };
    let find = |table: &[(&str, &'static str)]| {
        table
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, code)| code)
    };
    let (code, spanish_name) = match (find(LANGUAGES_ES), find(LANGUAGES_EN)) {
        (Some(code), _) => (code, true),
        (None, Some(code)) => (code, false),
        (None, None) => return Err(Problem::Unreadable),
    };
    match lead {
        None | Some("en" | "a" | "in" | "into" | "to") => {}
        Some("el" | "al" | "del") if spanish_name => {}
        Some(_) => return Err(Problem::Unreadable),
    }
    Ok(Value::Language(code))
}

// --- Yes and no ----------------------------------------------------------------

/// Courtesy that may follow a yes or a no: "sí, por favor", "no, gracias",
/// "yes please", "no thanks".
fn without_courtesy(text: &str) -> &str {
    let mut text = trim_edges(text);
    for tail in [
        "por favor",
        "gracias",
        "please",
        "thanks",
        "thank you",
        "anda",
    ] {
        if let Some(rest) = text.strip_suffix(tail) {
            let rest = trim_edges(rest);
            if !rest.is_empty() {
                text = rest;
            }
        }
    }
    text
}

/// The specification's lists (3.3), with a few spellings of the same words.
const YES: &[&str] = &[
    "si",
    "sip",
    "vale",
    "claro",
    "por supuesto",
    "venga",
    "adelante",
    "ok",
    "okay",
    "okey",
    "dale",
    "yes",
    "yeah",
    "yep",
    "yup",
    "sure",
    "go ahead",
];

const NO: &[&str] = &[
    "no",
    "nop",
    "nope",
    "mejor no",
    "ni hablar",
    "nah",
    "don't",
    "dont",
];

/// Reads a yes or a no (docs/spec/conversation.md 3.3): "sí", "vale",
/// "claro", "por supuesto", "venga", "adelante", "ok", "dale" / "yes",
/// "yeah", "yep", "sure", "go ahead" are yes; "no", "nope", "mejor no", "ni
/// hablar" / "nah", "don't", "no thanks" are no. A trailing courtesy ("sí,
/// por favor", "no, gracias") is allowed. "Vale" and "ok" count as yes here;
/// that they are only a yes when a yes/no is open is the caller's business.
/// The text is the match form.
///
/// # Errors
///
/// [`Problem::Unreadable`] when the text is neither.
pub fn confirm(text: &str) -> Result<bool, Problem> {
    let text = without_courtesy(text);
    let joined = words(text).join(" ");
    if YES.contains(&joined.as_str()) {
        Ok(true)
    } else if NO.contains(&joined.as_str()) {
        Ok(false)
    } else {
        Err(Problem::Unreadable)
    }
}

// --- Choices -------------------------------------------------------------------

/// Words around a choice that carry nothing: articles, "option", "one".
fn filler(word: &str) -> bool {
    matches!(
        word,
        "el" | "la"
            | "lo"
            | "los"
            | "las"
            | "the"
            | "option"
            | "opcion"
            | "number"
            | "numero"
            | "de"
            | "del"
            | "of"
            | "in"
            | "en"
            | "from"
            | "pues"
            | "i"
            | "mean"
            | "me"
            | "refiero"
            | "a"
            | "that"
            | "esa"
            | "ese"
            | "one"
            | "uno"
            | "una"
            | "please"
            | "por"
            | "favor"
    )
}

/// The position a word names, 1-based, if it names one; 0 for "the last".
fn position(word: &str) -> Option<usize> {
    Some(match word {
        "primero" | "primera" | "primer" | "first" | "1st" | "1" | "uno" | "una" | "one" => 1,
        "segundo" | "segunda" | "second" | "2nd" | "2" | "dos" | "two" => 2,
        "tercero" | "tercera" | "tercer" | "third" | "3rd" | "3" | "tres" | "three" => 3,
        "cuarto" | "cuarta" | "fourth" | "4th" | "4" | "cuatro" | "four" => 4,
        "quinto" | "quinta" | "fifth" | "5th" | "5" | "cinco" | "five" => 5,
        "ultimo" | "ultima" | "last" => 0,
        _ => return None,
    })
}

/// A word with an ordinal ending ("4th", "3rd", "1º") as its digits.
fn bare_ordinal(word: &str) -> &str {
    let digits = word.trim_end_matches(|c: char| !c.is_ascii_digit());
    let suffix = &word[digits.len()..];
    if !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(suffix, "st" | "nd" | "rd" | "th" | "o" | "a" | "º" | "ª")
    {
        digits
    } else {
        word
    }
}

/// Picks one of `options` (docs/spec/conversation.md 2.7) and returns its
/// index. Each option is the list of ways to name it, in the match form and
/// in either language ("cordoba argentina", "argentina"). An answer chooses
/// by position ("el primero", "la segunda", "the last one", "2", "number
/// two", "option b"), or by naming it, wholly or by a part only that option
/// has ("la argentina", "the 4th of March"). The text is the match form.
///
/// # Errors
///
/// [`Problem::Unreadable`] when no option, or more than one, fits.
pub fn choice(text: &str, options: &[Vec<String>]) -> Result<usize, Problem> {
    let all = words(text);
    let tokens: Vec<&str> = all.iter().map(|w| bare_ordinal(w)).collect();
    let count = options.len();
    // By position: one position word, with fillers around it.
    let content: Vec<&str> = tokens.iter().copied().filter(|w| !filler(w)).collect();
    let positional = match content.as_slice() {
        [] => tokens.iter().copied().rev().find_map(position),
        [word] => position(word),
        _ => None,
    };
    if let Some(n) = positional {
        let index = if n == 0 { count } else { n };
        return (1..=count)
            .contains(&index)
            .then(|| index - 1)
            .ok_or(Problem::Unreadable);
    }
    // A labelled option: "option b", "opción a".
    if let Some(at) = tokens
        .iter()
        .position(|w| matches!(*w, "option" | "opcion"))
        && let Some(label) = tokens.get(at + 1)
        && let [letter] = label.as_bytes()
        && letter.is_ascii_lowercase()
    {
        let index = usize::from(letter - b'a');
        return (index < count).then_some(index).ok_or(Problem::Unreadable);
    }
    // By name: every content word belongs to the option's names.
    if content.is_empty() {
        return Err(Problem::Unreadable);
    }
    let joined = content.join(" ");
    let named: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, names)| {
            names.iter().any(|name| {
                let name_words: Vec<&str> = name.split_whitespace().map(bare_ordinal).collect();
                *name == joined || content.iter().all(|w| name_words.contains(w))
            })
        })
        .map(|(i, _)| i)
        .collect();
    match named.as_slice() {
        [one] => Ok(*one),
        _ => Err(Problem::Unreadable),
    }
}

/// Reads a choice among the options a manifest declares for a slot: the
/// option's id. See [`choice`].
///
/// # Errors
///
/// As [`choice`].
pub fn choice_value(text: &str, ids: &[&str], options: &[Vec<String>]) -> Result<Value, Problem> {
    let index = choice(text, options)?;
    ids.get(index)
        .map(|id| Value::Choice((*id).to_owned()))
        .ok_or(Problem::Unreadable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place_of(text: &str) -> Option<String> {
        let folded = crate::normalise::normalise(text).folded;
        place(text, &folded).ok().map(|v| v.to_string())
    }

    #[test]
    fn places() {
        let cases = [
            ("Tokio", "Tokio"),
            ("en Tokio", "Tokio"),
            ("in New York", "New York"),
            ("Nueva York", "Nueva York"),
            ("Córdoba", "Córdoba"),
            ("mi ciudad", "@home"),
            ("home", "@home"),
            ("where I live", "@home"),
            ("aquí", "@here"),
            ("here", "@here"),
            ("allí", "@there"),
            ("there", "@there"),
            ("UTC", "UTC"),
            ("GMT", "UTC"),
            ("UTC+2", "UTC+02:00"),
            ("GMT-5", "UTC-05:00"),
            ("utc+05:30", "UTC+05:30"),
            ("CET", "CET"),
            ("EST", "EST"),
            ("hora peninsular", "Europe/Madrid"),
        ];
        for (text, expected) in cases {
            assert_eq!(place_of(text).as_deref(), Some(expected), "{text}");
        }
        for bad in [
            "",
            "somewhere warm",
            "I don't know, wherever",
            "en cualquier sitio",
            "donde sea",
            "5 km",
            "en",
        ] {
            assert_eq!(place_of(bad), None, "{bad}");
        }
    }

    fn lang_of(text: &str) -> Option<String> {
        let folded = crate::normalise::normalise(text).folded;
        language(&folded).ok().map(|v| v.to_string())
    }

    #[test]
    fn languages() {
        let cases = [
            ("inglés", "en"),
            ("español", "es"),
            ("castellano", "es"),
            ("en inglés", "en"),
            ("al español", "es"),
            ("English", "en"),
            ("Spanish", "es"),
            ("in Spanish", "es"),
            ("into English", "en"),
            ("in español", "es"),
            ("francés", "fr"),
            ("French", "fr"),
            ("alemán", "de"),
            ("German", "de"),
            ("catalán", "ca"),
            ("Catalan", "ca"),
        ];
        for (text, expected) in cases {
            assert_eq!(lang_of(text).as_deref(), Some(expected), "{text}");
        }
        for bad in ["el English", "klingon", "en el inglés antiguo", ""] {
            assert_eq!(lang_of(bad), None, "{bad}");
        }
    }

    fn yes_no(text: &str) -> Option<bool> {
        confirm(&crate::normalise::normalise(text).folded).ok()
    }

    #[test]
    fn yes_and_no() {
        for yes in [
            "sí",
            "si",
            "Sí, por favor",
            "vale",
            "claro",
            "por supuesto",
            "venga",
            "adelante",
            "ok",
            "dale",
            "yes",
            "yeah",
            "yep",
            "sure",
            "go ahead",
            "yes please",
        ] {
            assert_eq!(yes_no(yes), Some(true), "{yes}");
        }
        for no in [
            "no",
            "nope",
            "mejor no",
            "no, gracias",
            "ni hablar",
            "nah",
            "don't",
            "no thanks",
        ] {
            assert_eq!(yes_no(no), Some(false), "{no}");
        }
        for neither in ["quizá", "maybe", "sí y no", "gracias", ""] {
            assert_eq!(yes_no(neither), None, "{neither}");
        }
    }

    fn pick(text: &str, options: &[&[&str]]) -> Option<usize> {
        let options: Vec<Vec<String>> = options
            .iter()
            .map(|names| names.iter().map(|n| (*n).to_owned()).collect())
            .collect();
        choice(&crate::normalise::normalise(text).folded, &options).ok()
    }

    #[test]
    fn choices() {
        let cordoba: &[&[&str]] = &[
            &["cordoba espana", "espana", "spain", "spanish"],
            &["cordoba argentina", "argentina", "argentinian"],
        ];
        let cases = [
            ("el primero", Some(0)),
            ("la segunda", Some(1)),
            ("the first one", Some(0)),
            ("the second", Some(1)),
            ("the last one", Some(1)),
            ("el último", Some(1)),
            ("1", Some(0)),
            ("2", Some(1)),
            ("la 2", Some(1)),
            ("number two", Some(1)),
            ("option b", Some(1)),
            ("la argentina", Some(1)),
            ("the Spanish one", Some(0)),
            ("Córdoba", None),
            ("la tercera", None),
            ("Madrid", None),
        ];
        for (text, expected) in cases {
            assert_eq!(pick(text, cordoba), expected, "{text}");
        }
        let dates: &[&[&str]] = &[&["3 april", "april 3"], &["4 march", "march 4"]];
        assert_eq!(pick("4 March", dates), Some(1));
        assert_eq!(pick("the 4th of March", dates), Some(1));
        assert_eq!(pick("April", dates), Some(0));
        assert_eq!(pick("3 March", dates), None);
        let units: &[&[&str]] = &[&["metric", "metrico"], &["imperial"]];
        assert_eq!(
            choice_value("metrico", &["metric", "imperial"], &units_owned(units)),
            Ok(Value::Choice("metric".to_owned()))
        );
    }

    fn units_owned(options: &[&[&str]]) -> Vec<Vec<String>> {
        options
            .iter()
            .map(|names| names.iter().map(|n| (*n).to_owned()).collect())
            .collect()
    }
}
