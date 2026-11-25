//! `help`: what Talos can do (`help.capabilities`) and how to use one
//! capability (`help.skill`) (docs/spec/basics.md). Both answers are built
//! from the installed skills' manifests, so they cannot claim more or less
//! than is installed.

use talos_core::normalise::fold;
use talos_core::{Call, Context, Lang, Manifest, Message, Skill, SkillResult, Value};

/// The `help` handler.
#[derive(Debug, Default)]
pub(crate) struct Help;

/// Example phrases shown for one skill ("two or three", basics.md).
const EXAMPLES_SHOWN: usize = 3;

/// Skills named in the usage hint when no topic was given.
const NAMES_SHOWN: usize = 5;

/// Words that may precede a skill's name in a question ("lo de los dados",
/// "the calculator"), dropped before comparing.
const FILLER: [&str; 11] = [
    "lo", "de", "el", "la", "los", "las", "un", "una", "the", "a", "an",
];

/// The match form of a user's wording: lower case, no accents, filler words
/// dropped from the front.
fn topic_form(text: &str) -> String {
    let folded = fold(&text.to_lowercase());
    let mut words: &[&str] = &folded.split_whitespace().collect::<Vec<_>>();
    while let [first, rest @ ..] = words {
        if FILLER.contains(first) && !rest.is_empty() {
            words = rest;
        } else {
            break;
        }
    }
    words.join(" ")
}

/// Every wording a skill answers to: its id and its names in both languages.
fn wordings(manifest: &Manifest) -> Vec<String> {
    let mut all = vec![fold(&manifest.id.to_lowercase())];
    for list in [&manifest.names.es, &manifest.names.en] {
        all.extend(list.iter().map(|name| fold(&name.to_lowercase())));
    }
    all
}

/// The installed skill the user's words name: an exact id or name first,
/// else the longest name that appears in the words as whole words.
fn find_skill<'a>(skills: &'a [Manifest], topic: &str) -> Option<&'a Manifest> {
    let topic = topic_form(topic);
    if topic.is_empty() {
        return None;
    }
    if let Some(exact) = skills.iter().find(|m| wordings(m).contains(&topic)) {
        return Some(exact);
    }
    let padded = format!(" {topic} ");
    skills
        .iter()
        .flat_map(|m| wordings(m).into_iter().map(move |w| (m, w)))
        .filter(|(_, w)| !w.is_empty() && padded.contains(&format!(" {w} ")))
        .max_by_key(|(_, w)| w.len())
        .map(|(m, _)| m)
}

/// The name a user would call the skill by, in the language of the turn.
fn display_name(manifest: &Manifest, lang: Lang) -> &str {
    manifest
        .names
        .get(lang)
        .first()
        .map_or(manifest.id.as_str(), String::as_str)
}

/// One "· summary" line per skill, in the order given.
fn capability_lines(skills: &[Manifest], lang: Lang) -> String {
    skills
        .iter()
        .map(|m| format!("· {}", m.summary.get(lang)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The skill's example phrases, quoted the way each language quotes.
fn quoted_examples(manifest: &Manifest, lang: Lang) -> String {
    let (open, close) = match lang {
        Lang::Es => ('«', '»'),
        Lang::En => ('"', '"'),
    };
    manifest
        .help
        .get(lang)
        .iter()
        .take(EXAMPLES_SHOWN)
        .map(|e| format!("{open}{e}{close}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The reply for the overall list.
fn capabilities(skills: &[Manifest], lang: Lang) -> Message {
    Message::new("capabilities").var("skills", capability_lines(skills, lang))
}

/// The reply for one skill: summary and examples; or none when no installed
/// skill answers to `topic`.
fn skill_help(skills: &[Manifest], lang: Lang, topic: &str) -> Option<(String, Message)> {
    let manifest = find_skill(skills, topic)?;
    let examples = quoted_examples(manifest, lang);
    let base = |id| {
        Message::new(id)
            .var("name", display_name(manifest, lang))
            .var("summary", manifest.summary.get(lang).as_str())
    };
    let message = if examples.is_empty() {
        base("skill_bare")
    } else {
        base("skill").var("examples", examples)
    };
    Some((manifest.id.clone(), message))
}

/// The usage hint when no topic was given: the skills' names.
fn which(skills: &[Manifest], lang: Lang) -> Message {
    let names = skills
        .iter()
        .take(NAMES_SHOWN)
        .map(|m| display_name(m, lang))
        .collect::<Vec<_>>()
        .join(", ");
    Message::new("which").var("names", names)
}

impl Skill for Help {
    fn id(&self) -> &'static str {
        "help"
    }

    fn replies(&self) -> &'static [&'static str] {
        &["capabilities", "skill", "skill_bare", "unknown", "which"]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "help.capabilities" => {
                SkillResult::Answer(capabilities(cx.skills(), call.lang))
            }
            "help.skill" => {
                let Some(topic) = call.slots.get("skill").and_then(Value::as_text) else {
                    return SkillResult::Unable(which(cx.skills(), call.lang));
                };
                match skill_help(cx.skills(), call.lang, topic) {
                    Some((id, message)) => {
                        cx.report("skill", Value::Text(id));
                        SkillResult::Answer(message)
                    }
                    None => SkillResult::Unable(Message::new("unknown")),
                }
            }
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{Config, ConvId, Engine, IntentId, Random, Slots, Status, Timestamp, Turn};

    fn skill(id: &str, es: (&str, &str, &str), en: (&str, &str, &str)) -> Manifest {
        let list = |s: &str| {
            s.split('|')
                .filter(|x| !x.is_empty())
                .map(|x| format!("\"{x}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let text = format!(
            "[skill]\nid = \"{id}\"\nsummary.es = \"{}\"\nsummary.en = \"{}\"\n\
             names.es = [{}]\nnames.en = [{}]\nhelp.es = [{}]\nhelp.en = [{}]\n",
            es.0,
            en.0,
            list(es.1),
            list(en.1),
            list(es.2),
            list(en.2),
        );
        Manifest::parse(id, &text).unwrap()
    }

    fn fakes() -> Vec<Manifest> {
        vec![
            skill(
                "clock",
                ("la hora y la fecha", "reloj|hora", "qué hora es|qué día es hoy"),
                ("the time and date", "clock|time", "what time is it|what's the date"),
            ),
            skill(
                "calc",
                (
                    "cálculos y conversión de unidades",
                    "calculadora|calc|conversiones",
                    "cuánto es 3 por 4|el 15% de 80|5 km a millas|una cuarta",
                ),
                (
                    "arithmetic and unit conversion",
                    "calculator|calc",
                    "what's 3 times 4",
                ),
            ),
            skill("homelab", ("estado de tus servidores", "homelab", ""), (
                "your servers' status",
                "homelab",
                "",
            )),
        ]
    }

    #[test]
    fn the_list_has_one_summary_line_per_skill_in_the_given_order() {
        let skills = fakes();
        let es = capabilities(&skills, Lang::Es);
        assert_eq!(es.id, "capabilities");
        assert_eq!(
            es.vars["skills"],
            "· la hora y la fecha\n· cálculos y conversión de unidades\n· estado de tus servidores"
        );
        let en = capabilities(&skills, Lang::En);
        assert!(en.vars["skills"].ends_with("· your servers' status"));
        assert_eq!(capability_lines(&[], Lang::En), "");
    }

    #[test]
    fn a_skill_is_found_by_id_or_name_in_either_language() {
        let skills = fakes();
        for (said, id) in [
            ("la calculadora", "calc"),
            ("calc", "calc"),
            ("lo de las conversiones", "calc"),
            ("Reloj", "clock"),
            ("the clock", "clock"),
            ("el RELOJ", "clock"),
            ("la hora", "clock"),
            ("homelab", "homelab"),
            ("la calculadora de bolsillo", "calc"),
        ] {
            let found = find_skill(&skills, said).map(|m| m.id.as_str());
            assert_eq!(found, Some(id), "{said}");
        }
        assert!(find_skill(&skills, "divisas").is_none());
        assert!(find_skill(&skills, "").is_none());
        assert!(find_skill(&skills, "la").is_none());
    }

    #[test]
    fn skill_help_gives_the_summary_and_at_most_three_examples() {
        let skills = fakes();
        let (id, message) = skill_help(&skills, Lang::Es, "calculadora").unwrap();
        assert_eq!(id, "calc");
        assert_eq!(message.id, "skill");
        assert_eq!(message.vars["name"], "calculadora");
        assert_eq!(message.vars["summary"], "cálculos y conversión de unidades");
        assert_eq!(
            message.vars["examples"],
            "«cuánto es 3 por 4», «el 15% de 80», «5 km a millas»"
        );
        let (_, en) = skill_help(&skills, Lang::En, "calc").unwrap();
        assert_eq!(en.vars["examples"], "\"what's 3 times 4\"");
        let (_, bare) = skill_help(&skills, Lang::En, "homelab").unwrap();
        assert_eq!(bare.id, "skill_bare");
        assert!(skill_help(&skills, Lang::En, "weather").is_none());
    }

    #[test]
    fn without_a_topic_the_hint_names_the_skills() {
        let hint = which(&fakes(), Lang::En);
        assert_eq!(hint.vars["names"], "clock, calculator, homelab");
    }

    fn call_with(intent: &str, topic: Option<&str>) -> SkillResult {
        let intent = IntentId(intent.to_owned());
        let slots: Slots = topic
            .map(|t| ("skill".to_owned(), Value::Text(t.to_owned())))
            .into_iter()
            .collect();
        let mut cx = Context::standalone(Timestamp::from_unix(0), Random::seeded(0));
        let call = Call {
            intent: &intent,
            lang: Lang::En,
            slots: &slots,
            text: "x",
        };
        Help.handle(&mut cx, &call)
    }

    #[test]
    fn declines_foreign_intents_and_hints_when_the_topic_is_missing_or_unknown() {
        assert_eq!(call_with("help.other", None), SkillResult::Decline);
        assert!(matches!(
            call_with("help.skill", None),
            SkillResult::Unable(m) if m.id == "which"
        ));
        assert!(matches!(
            call_with("help.skill", Some("weather")),
            SkillResult::Unable(m) if m.id == "unknown"
        ));
        assert!(matches!(
            call_with("help.capabilities", None),
            SkillResult::Answer(m) if m.id == "capabilities"
        ));
    }

    /// Phrasings of the built-in engine: the intent each reaches, whatever
    /// the installed set (so the golden files need not repeat every list).
    #[test]
    fn the_obvious_phrasings_reach_the_right_intent() {
        let engine: Engine = crate::engine(Config::default()).unwrap();
        let ask = |text: &str| {
            let out = engine.respond(Turn::new(
                text,
                ConvId::new("phrasings"),
                Timestamp::from_unix(0),
            ));
            engine.end(&ConvId::new("phrasings"));
            out
        };
        let capabilities = [
            "qué sabes hacer",
            "qué puedes hacer",
            "ayuda",
            "¡Ayuda!",
            "ayúdame",
            "para qué sirves",
            "que comandos tienes",
            "lista de funciones",
            "lista de cosas que sabes hacer",
            "oye, en qué me puedes ayudar",
            "help",
            "help me",
            "what can you do?",
            "what are your features",
            "list your skills",
            "what are you able to do?",
        ];
        for text in capabilities {
            let out = ask(text);
            assert_eq!(out.status, Status::Answered, "{text}");
            assert_eq!(
                out.intent.as_ref().map(IntentId::as_str),
                Some("help.capabilities"),
                "{text}"
            );
        }
        let skill = [
            "cómo funciona la calculadora",
            "qué puedo preguntarte sobre la hora",
            "sabes hacer porcentajes?",
            "como se usa lo de los dados",
            "ayuda con lo de enseñarte cosas",
            "how do I use the calculator?",
            "what can I ask about dates?",
            "do you do percentages",
            "how does the dice thing work",
            "cómo se usa",
            "how do I use this",
        ];
        for text in skill {
            let out = ask(text);
            assert_eq!(
                out.intent.as_ref().map(IntentId::as_str),
                Some("help.skill"),
                "{text}"
            );
        }
        // Not help requests.
        for text in ["help me write a cover letter", "ayúdame con mi redacción"] {
            let out = ask(text);
            assert_eq!(out.status, Status::Unhandled, "{text}");
            assert!(out.intent.is_none(), "{text}");
        }
    }
}
