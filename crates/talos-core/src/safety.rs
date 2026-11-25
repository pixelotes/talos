//! Stage 1b: the safety check for distress (docs/spec/reflector.md S29,
//! docs/spec/conversation.md 3.9, ADR 0004 as amended).
//!
//! A message that says the user wants to die, to hurt themselves, or that
//! someone is hurting them gets one fixed reply that points to emergency
//! help, before anything else can claim the message: ahead of the pending
//! dialog, the rules and the reflector. The check reads both languages'
//! patterns whatever the turn's language. Fixed idioms ("me muero de
//! risa", "I'm dying to") are blanked out first; anything else that looks
//! like distress is distress: a false alarm costs one odd reply, a miss
//! much more.
//!
//! The numbers are configuration ([`Emergency`]), chosen by the user's
//! place: one named in the message, else the caller's. (A remembered place
//! joins when preferences do; the machine's zone is never used.) With no
//! place, or a place with no entry, the reply names no number.

use std::fmt::Write as _;

use crate::normalise::{Normalised, fold};
use crate::reflector::Reflector;
use crate::{Lang, Place, Reason, Status, render};

/// Emergency numbers for one region (DESIGN.md section 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emergency {
    /// The region's name, for the trace.
    pub region: &'static str,
    /// Names that resolve to it: the country, its regions and main cities,
    /// its time zones. Matched without case or accents, as whole words,
    /// against the message and the caller's place.
    pub places: &'static [&'static str],
    /// The general emergency number.
    pub general: &'static str,
    /// A crisis line for suicidal behaviour, if the region has one.
    pub crisis: Option<&'static str>,
}

/// The default configuration's one entry (ruling 1, 2026-11-24): Spain,
/// 112 and 024, in either language.
pub const SPAIN: Emergency = Emergency {
    region: "Spain",
    places: &[
        "España",
        "Spain",
        "Europe/Madrid",
        "Atlantic/Canary",
        "Africa/Ceuta",
        // Autonomous communities.
        "Andalucía",
        "Andalusia",
        "Aragón",
        "Asturias",
        "Baleares",
        "Illes Balears",
        "Balearic Islands",
        "Canarias",
        "Canary Islands",
        "Cantabria",
        "Castilla y León",
        "Castilla-La Mancha",
        "Cataluña",
        "Catalunya",
        "Catalonia",
        "Comunidad Valenciana",
        "Extremadura",
        "Galicia",
        "Región de Murcia",
        "Navarra",
        "País Vasco",
        "Euskadi",
        "Ceuta",
        "Melilla",
        // Provincial capitals and other large cities. Names shared with a
        // large place elsewhere (Valencia, Córdoba, Guadalajara, León,
        // Mérida, Cartagena…) are left out: naming the wrong country's
        // number is worse than naming none.
        "Madrid",
        "Barcelona",
        "Sevilla",
        "Seville",
        "Zaragoza",
        "Málaga",
        "Bilbao",
        "Alicante",
        "Valladolid",
        "Vigo",
        "Gijón",
        "Oviedo",
        "Vitoria",
        "Gasteiz",
        "A Coruña",
        "La Coruña",
        "Elche",
        "Palma de Mallorca",
        "Las Palmas de Gran Canaria",
        "Santa Cruz de Tenerife",
        "Tenerife",
        "Gran Canaria",
        "Pamplona",
        "Iruña",
        "Almería",
        "San Sebastián",
        "Donostia",
        "Burgos",
        "Albacete",
        "Castellón",
        "Logroño",
        "Badajoz",
        "Huelva",
        "Lleida",
        "Lérida",
        "Tarragona",
        "Cádiz",
        "Jaén",
        "Ourense",
        "Girona",
        "Gerona",
        "Lugo",
        "Cáceres",
        "Pontevedra",
        "Palencia",
        "Ciudad Real",
        "Zamora",
        "Ávila",
        "Cuenca",
        "Huesca",
        "Segovia",
        "Soria",
        "Teruel",
        "Santiago de Compostela",
        "Jerez de la Frontera",
        "Marbella",
        "Ibiza",
        "Menorca",
        "Mallorca",
        "Lanzarote",
        "Fuerteventura",
    ],
    general: "112",
    crisis: Some("024"),
};

/// The default emergency configuration.
pub const DEFAULT_EMERGENCY: &[Emergency] = &[SPAIN];

/// What the check decided, when it fired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alarm {
    pub(crate) reply: String,
    pub(crate) status: Status,
    pub(crate) reason: Reason,
    pub(crate) note: String,
}

/// The match form padded with spaces and with punctuation turned into
/// spaces, so whole words are found with `contains(" word ")`.
fn padded(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(' ');
    out.extend(
        text.chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' }),
    );
    out.push(' ');
    out
}

fn place_key(name: &str) -> String {
    padded(&fold(&name.to_lowercase()))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The region of a place named in the message, else of the caller's.
fn region<'a>(
    message: &Normalised,
    place: Option<&Place>,
    table: &'a [Emergency],
) -> Option<(&'a Emergency, String)> {
    let words = padded(&message.folded);
    let words = format!(
        " {} ",
        words.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    for entry in table {
        if let Some(name) = entry
            .places
            .iter()
            .find(|p| words.contains(&format!(" {} ", place_key(p))))
        {
            return Some((entry, format!("{name}, named in the message")));
        }
    }
    let place = place?;
    let caller = format!(" {} ", place_key(place.as_str()));
    let zone = fold(&place.as_str().trim().to_lowercase());
    table.iter().find_map(|entry| {
        entry
            .places
            .iter()
            .find(|p| {
                fold(&p.to_lowercase()) == zone || caller.contains(&format!(" {} ", place_key(p)))
            })
            .map(|_| (entry, format!("the caller's place {:?}", place.as_str())))
    })
}

/// Runs the check: `Some` with the fixed reply when the message reads as
/// distress in either language.
pub(crate) fn check(
    reflector: &Reflector,
    lang: Lang,
    message: &Normalised,
    place: Option<&Place>,
    table: &[Emergency],
) -> Option<Alarm> {
    let mut hit = None;
    for script_lang in [lang, lang.other()] {
        let distress = &reflector.script(script_lang).distress;
        let mut text = message.folded.clone();
        for idiom in 0..distress.idioms.len() {
            text = distress.idioms.blank(idiom, &text);
        }
        if let Some(&pattern) = distress.patterns.matches(&text).first() {
            hit = Some((script_lang, distress.patterns.source(pattern).to_owned()));
            break;
        }
    }
    let (matched, pattern) = hit?;
    let distress = &reflector.script(lang).distress;
    let region = region(message, place, table);
    let mut vars = render::Vars::new();
    let template = match region {
        Some((entry, _)) => {
            vars.insert("general".to_owned(), entry.general.to_owned());
            match entry.crisis {
                Some(crisis) => {
                    vars.insert("crisis".to_owned(), crisis.to_owned());
                    &distress.two_numbers
                }
                None => &distress.one_number,
            }
        }
        None => &distress.no_number,
    };
    // Validated at load: the template uses exactly these variables.
    let reply = render::fill(&template.text, &vars).unwrap_or_else(|_| template.text.clone());
    let mut note = format!("distress ({matched} /{pattern}/); ");
    match region {
        Some((entry, from)) => {
            let _ = write!(
                note,
                "region {} from {from}: {}",
                entry.region, entry.general
            );
            if let Some(crisis) = entry.crisis {
                let _ = write!(note, ", {crisis}");
            }
        }
        None => note.push_str("no place known, so no number is named"),
    }
    Some(Alarm {
        reply,
        status: Status::Unhandled,
        reason: Reason::new("distress"),
        note,
    })
}
