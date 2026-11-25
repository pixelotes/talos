//! The reflector against the real scripts: the forbidden-reply check (D6),
//! the variant rules and the boundary.

use regex::Regex;

use super::*;
use crate::normalise::{fold, normalise};
use crate::{Config, Engine, Timestamp, Turn};

fn reflector() -> Reflector {
    Reflector::new().unwrap()
}

/// One turn in a fresh conversation.
fn once(r: &Reflector, lang: Lang, text: &str) -> Reply {
    let mut state = ConversationState::NEW;
    r.respond(lang, &normalise(text), &ConvId::new("t"), 1, &mut state)
}

/// An engine with no skills: everything goes to the safety check and the
/// reflector.
fn engine() -> Engine {
    Engine::new(Config::default(), Vec::new()).unwrap()
}

fn turn(text: &str, lang: Lang, conversation: &str) -> Turn {
    Turn::new(text, ConvId::new(conversation), Timestamp::from_unix(0)).with_lang(Some(lang))
}

#[test]
fn the_embedded_scripts_load() {
    let r = reflector();
    for lang in Lang::ALL {
        assert!(r.script(lang).rules.len() > 40, "{lang}");
    }
}

#[test]
fn sentences_split_and_trim() {
    let folded = normalise("Hola. ¿Qué tal? jajaja!!").folded;
    let parts: Vec<(&str, bool)> = sentences(&folded)
        .iter()
        .map(|s| (&folded[s.range.clone()], s.question))
        .collect();
    assert_eq!(
        parts,
        [("hola", false), ("que tal", true), ("jajaja", false)]
    );
}

#[test]
fn fillers_and_the_vocative_are_dropped() {
    let fillers = vec!["pues".to_owned(), "bueno".to_owned()];
    let text = "bueno, pues callate, talos";
    let range = strip_fillers(text, 0..text.len(), &fillers);
    assert_eq!(&text[range], "callate");
    // A filler alone is kept: it may be the whole message.
    let text = "bueno";
    assert_eq!(strip_fillers(text, 0..5, &fillers), 0..5);
}

#[test]
fn tidy_capitalises_and_closes_up() {
    assert_eq!(tidy("¿profe de instituto ?"), "¿Profe de instituto?");
    assert_eq!(tidy("  ayer  estuviste , vale"), "Ayer estuviste, vale");
}

// ---------------------------------------------------------------------------
// D6. Forbidden replies (docs/spec/reflector.md 5 and 7).

/// Phrases no template may contain, per language: claimed comprehension or
/// feelings, questions about feelings, advice, reassurance, judgement,
/// promises of memory, tastes, terms of endearment, gendered words about
/// the user or Talos. Matched as whole words on the match form.
const FORBIDDEN_ES: &[&str] = &[
    "te entiendo",
    "te comprendo",
    "como te hace sentir",
    "como te sientes",
    "como te sentiste",
    "deberias",
    "tendrias que",
    "te recomiendo",
    "te aconsejo",
    "lo tendre en cuenta",
    "lo apunto",
    "apuntado",
    "anotado",
    "me acordare",
    "lo recordare",
    "me alegra",
    "me alegro",
    "tranquilo",
    "tranquila",
    "todo ira bien",
    "animo",
    "a mi tambien",
    "a mi me gusta",
    "me encanta",
    "mi favorito",
    "mi favorita",
    "estoy encantado",
    "estoy encantada",
    "me haces feliz",
    "carino",
    "corazon",
    "guapo",
    "guapa",
    "que buena idea",
    "buena idea",
    "mala idea",
    "tienes toda la razon",
    "estas seguro",
    "estas segura",
    "bienvenido",
    "bienvenida",
    "cansado estaras",
    "impresentable",
    "por que te interesa",
    "cuentame mas sobre",
    "valiente",
    "pobrecito",
    "pobrecita",
    "no estoy seguro",
    "no estoy segura",
    "soy muy listo",
    "soy muy lista",
    "lo siento mucho",
    "que pena",
];

const FORBIDDEN_EN: &[&str] = &[
    "i understand",
    "i know how you feel",
    "i know exactly",
    "how does that make you feel",
    "how did that make you feel",
    "how do you feel",
    "how are you feeling",
    "you should",
    "you ought",
    "i recommend",
    "i suggest",
    "try an app",
    "i'll remember",
    "i will remember",
    "noted",
    "i'm glad",
    "i am glad",
    "glad to hear",
    "cheer up",
    "it can't be that bad",
    "don't worry",
    "it'll be fine",
    "me too",
    "i love",
    "i like",
    "i enjoy",
    "my favourite",
    "my favorite",
    "sweetie",
    "mate",
    "love",
    "darling",
    "sir",
    "great point",
    "great idea",
    "good idea",
    "bad idea",
    "brave",
    "toxic",
    "why do you ask",
    "tell me more about",
    "any particular",
    "apology accepted",
    "i'm flattered",
    "i'm happy",
    "that hurts",
    "my feelings",
    "poor you",
    "bless you",
];

fn forbidden(lang: Lang) -> Vec<Regex> {
    let list = match lang {
        Lang::Es => FORBIDDEN_ES,
        Lang::En => FORBIDDEN_EN,
    };
    list.iter()
        .map(|p| Regex::new(&format!(r"\b{}\b", regex::escape(p))).unwrap())
        .collect()
}

fn offends(lang: Lang, text: &str) -> Option<String> {
    offends_beyond(lang, text, "")
}

/// A forbidden phrase in `text` that the user's own `message` does not
/// already contain: an echo may repeat the user's words ("You love jazz?"),
/// never add the forbidden ones.
fn offends_beyond(lang: Lang, text: &str, message: &str) -> Option<String> {
    let text = fold(&text.to_lowercase());
    let message = fold(&message.to_lowercase());
    forbidden(lang)
        .iter()
        .find(|p| p.is_match(&text) && !p.is_match(&message))
        .map(|p| p.as_str().to_owned())
}

/// Every reply text of a script, as written.
fn templates(script: &Script) -> Vec<&str> {
    let mut all: Vec<&str> = Vec::new();
    for rule in &script.rules {
        all.extend(rule.replies.iter().map(|t| t.text.as_str()));
    }
    for list in [
        &script.catch_all,
        &script.noise,
        &script.other_language,
        &script.long_text,
    ] {
        all.extend(list.iter().map(|t| t.text.as_str()));
    }
    let d = &script.distress;
    all.extend([&d.two_numbers, &d.one_number, &d.no_number].map(|t| t.text.as_str()));
    all
}

#[test]
fn no_template_says_a_forbidden_phrase() {
    let r = reflector();
    for lang in Lang::ALL {
        for template in templates(r.script(lang)) {
            assert_eq!(offends(lang, template), None, "[{lang}] {template}");
        }
    }
}

/// The anti-examples of reflector.md 5: the user's message, and the reply
/// that must never come back.
const ANTI_ES: &[(&str, &str)] = &[
    ("estoy agotada", "Te entiendo perfectamente."),
    ("he discutido con mi hermana", "¿Y cómo te hace sentir eso?"),
    ("estoy fatal", "Tranquila, todo irá bien."),
    ("estoy fatal", "Deberías hablarlo con alguien."),
    ("creo que el teletrabajo es mejor", "Tienes toda la razón."),
    ("voy a dejar el curro", "¡Qué buena idea!"),
    ("mi jefe me grita", "Tu jefe parece un impresentable."),
    ("no estoy segura", "¿Estás seguro?"),
    (
        "me siento en el sofá a leer",
        "¿Por qué te sientes en el sofá?",
    ),
    ("voy a comprármelo", "¿Vas a comprármelo?"),
    ("eres tonto", "¿Soy tonto?"),
    ("eres tonto", "Tú más."),
    (
        "¿cuál es la capital de Perú?",
        "¿Por qué te interesa la capital de Perú?",
    ),
    ("escríbeme un poema", "Cuéntame más sobre ese poema."),
    ("me encanta el flamenco", "¡A mí también me encanta!"),
    ("eres un crack", "¡Me haces muy feliz! Estoy encantado."),
    ("no sé", "No pasa nada, cariño."),
    ("jajaja", "Me alegra que te rías."),
    (
        "me gustaría aprender a tocar la guitarra",
        "Lo tendré en cuenta.",
    ),
    ("quiero morirme", "¿Por qué quieres morirte?"),
    (
        "prefiero la sierra a la playa",
        "La sierra es más tranquila, sí.",
    ),
    (
        "si pudiera me iba a vivir al pueblo",
        "Si pudieras, te ibas al pueblo. ¿Y qué te lo impide?",
    ),
];

const ANTI_EN: &[(&str, &str)] = &[
    ("I'm exhausted", "I know exactly how you feel."),
    ("I had a row with my sister", "How did that make you feel?"),
    ("I'm sad", "Cheer up, it can't be that bad!"),
    ("I think remote work is better", "Great point!"),
    ("I'm going to quit my job", "That's a brave decision."),
    ("my boss shouts at me", "Your boss sounds toxic."),
    ("I want to learn German", "You should try an app for that."),
    ("you're stupid", "Am I stupid?"),
    ("you're stupid", "That hurts my feelings."),
    (
        "what's the capital of Peru?",
        "Why do you ask about the capital of Peru?",
    ),
    ("write me a poem", "Tell me more about the poem you'd like."),
    ("I don't know", "That's okay, sweetie."),
    ("I love jazz", "Me too!"),
    ("I'm Sam, by the way", "I'll remember that."),
    ("I was wrong", "You was wrong."),
    ("I was wrong", "I were wrong."),
    ("let me think", "Let you think?"),
    ("I want to die", "Why do you want to die?"),
    ("I'd love a dog", "Any particular breed?"),
    ("sorry", "Apology accepted, mate."),
];

/// Every reply a message can get over a run of turns: enough turns, in
/// conversations with different ids, to reach every variant.
fn every_reply(e: &Engine, lang: Lang, text: &str) -> Vec<String> {
    let mut replies = Vec::new();
    for conversation in ["a", "b", "c"] {
        for _ in 0..8 {
            replies.push(e.respond(turn(text, lang, conversation)).reply);
        }
        e.end(&ConvId::new(conversation));
    }
    replies
}

#[test]
fn no_anti_example_is_ever_produced() {
    let e = engine();
    for (lang, table) in [(Lang::Es, ANTI_ES), (Lang::En, ANTI_EN)] {
        for (user, never) in table {
            for reply in every_reply(&e, lang, user) {
                assert_ne!(&reply, never, "[{lang}] {user}");
                assert_eq!(
                    offends_beyond(lang, &reply, user),
                    None,
                    "[{lang}] {user} → {reply}"
                );
            }
        }
    }
}

/// First-person words: an echo never contains one, since reflection only
/// goes from the user's first person to the second.
fn first_person(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Es => &[
            "yo", "me", "mi", "mis", "conmigo", "mio", "mia", "mios", "mias",
        ],
        Lang::En => &[
            "i", "me", "my", "mine", "myself", "i'm", "i've", "i'll", "i'd", "im",
        ],
    }
}

#[test]
fn an_echo_never_speaks_in_the_first_person() {
    let r = reflector();
    let mut fragments: Vec<(Lang, &str)> = Vec::new();
    fragments.extend(ANTI_ES.iter().map(|(u, _)| (Lang::Es, *u)));
    fragments.extend(ANTI_EN.iter().map(|(u, _)| (Lang::En, *u)));
    fragments.extend(
        [
            "a mí me pasa lo mismo con mi madre",
            "mi novio dice que yo exagero conmigo misma",
            "para mí que me tienen manía",
            "me cojo el día libre con mis amigos",
            "yo fuera rico",
            "la mía es mejor que la tuya",
        ]
        .map(|t| (Lang::Es, t)),
    );
    fragments.extend(
        [
            "I was sure I'd locked my door myself",
            "me and my brother think I'm right",
            "a friend of mine told me I've changed",
            "I'm off to see my mum",
        ]
        .map(|t| (Lang::En, t)),
    );
    for (lang, fragment) in fragments {
        let tables = r.tables(lang);
        let Ok(echo) = tables.reflect(fragment, None).echo else {
            continue;
        };
        for word in echo.split(|c: char| !(c.is_alphanumeric() || c == '\'')) {
            let word = fold(&word.to_lowercase());
            assert!(
                !first_person(lang).contains(&word.as_str()),
                "[{lang}] {fragment:?} → {echo:?}"
            );
        }
    }
}

#[test]
fn what_is_said_about_talos_is_never_echoed() {
    let r = reflector();
    for (lang, text) in [
        (Lang::Es, "eres lento"),
        (Lang::Es, "tú sabrás lo que haces"),
        (Lang::Es, "hablas como un contestador"),
        (Lang::En, "you're slow"),
        (Lang::En, "you sound like a robot"),
    ] {
        assert!(r.tables(lang).reflect(text, None).echo.is_err(), "{text}");
        let reply = once(&r, lang, text).text;
        for word in ["soy", "hablo", "estoy", "am", "i'm"] {
            assert!(
                !fold(&reply.to_lowercase()).starts_with(word),
                "[{lang}] {text} → {reply}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The boundary (reflector.md 4): unhandled, never deflected.

#[test]
fn guards_are_unhandled_with_their_reason() {
    let r = reflector();
    for (lang, text, reason) in [
        (Lang::Es, "¿cuál es la capital de Perú?", "unknown-fact"),
        (Lang::Es, "qué hora es", "unknown-fact"),
        (Lang::Es, "escríbeme un poema sobre mi gato", "generative"),
        (Lang::Es, "dime algo bonito", "generative"),
        (Lang::Es, "jjjj", "noise"),
        (Lang::Es, "😐", "noise"),
        (Lang::Es, "bon dia", "other-language"),
        (Lang::Es, "¿debería cambiar de trabajo?", "advice"),
        (Lang::Es, "apaga la luz del salón", "cannot"),
        (Lang::Es, "¿qué tiempo hace hoy?", "not-yet"),
        (Lang::En, "who invented the telephone?", "unknown-fact"),
        (Lang::En, "what time is it", "unknown-fact"),
        (
            Lang::En,
            "please write a short story about dragons",
            "generative",
        ),
        (Lang::En, "guten Tag, wie geht es dir?", "other-language"),
        (Lang::En, "should I buy a new laptop?", "advice"),
        (Lang::En, "play some jazz", "cannot"),
        (Lang::En, "set a timer for ten minutes", "cannot"),
    ] {
        let reply = once(&r, lang, text);
        assert_eq!(reply.status, Status::Unhandled, "{text}: {reply:?}");
        assert_eq!(reply.reason.as_str(), reason, "{text}: {reply:?}");
    }
    let long = "palabra ".repeat(61);
    assert_eq!(once(&r, Lang::Es, &long).reason.as_str(), "long-text");
    assert_eq!(
        once(&r, Lang::En, "line one\nline two").reason.as_str(),
        "long-text"
    );
}

#[test]
fn a_request_for_text_beats_any_shape() {
    let r = reflector();
    // A wish in form, and a first person in it (reflector.md 4.2).
    let reply = once(&r, Lang::Es, "dime algo que me anime");
    assert_eq!(reply.reason.as_str(), "generative");
    let reply = once(&r, Lang::Es, "jajaja, escríbeme un poema");
    assert_eq!(reply.reason.as_str(), "generative");
}

#[test]
fn a_shape_of_the_other_language_is_answered_in_this_one() {
    let r = reflector();
    let reply = once(&r, Lang::En, "jajajaja");
    assert_eq!(reply.reason.as_str(), "exclaim");
    assert_eq!(reply.text, "Ha.");
}

// ---------------------------------------------------------------------------
// Variants (reflector.md 6).

#[test]
fn no_reply_is_repeated_in_consecutive_turns() {
    let e = engine();
    for (lang, text) in [
        (Lang::Es, "mi jefe es un pesado"),
        (Lang::Es, "ñlkjh qwe"),
        (Lang::Es, "lo de ayer con lo otro"),
        (Lang::En, "hmm"),
        (Lang::En, "blah blah"),
    ] {
        let replies: Vec<String> = (0..6)
            .map(|_| e.respond(turn(text, lang, "repeat")).reply)
            .collect();
        e.end(&ConvId::new("repeat"));
        let single = text.starts_with('ñ');
        for pair in replies.windows(2) {
            // Guard replies with a single wording repeat unchanged.
            assert!(single || pair[0] != pair[1], "{text}: {replies:?}");
        }
    }
}

#[test]
fn hand_back_questions_thin_out() {
    let e = engine();
    let texts = [
        "mi hermana se ha comprado una moto",
        "mi jefe dice que tengo que venir el sábado",
        "mi madre también me agobia",
        "mi novio dice que exagero",
    ];
    let replies: Vec<String> = texts
        .iter()
        .map(|t| e.respond(turn(t, Lang::Es, "thin")).reply)
        .collect();
    for window in replies.windows(3) {
        assert!(
            !window.iter().all(|r| r.ends_with('?')),
            "three questions in a row: {replies:?}"
        );
    }
}

#[test]
fn replies_are_a_pure_function_of_conversation_and_turn() {
    let texts = [
        "creo que va a llover",
        "mi madre me llama",
        "jajaja",
        "no sé",
    ];
    let run = || -> Vec<String> {
        let e = engine();
        texts
            .iter()
            .map(|t| e.respond(turn(t, Lang::Es, "pure")).reply)
            .collect()
    };
    assert_eq!(run(), run());
}

// ---------------------------------------------------------------------------
// The safety check (S29).

#[test]
fn distress_is_caught_in_both_languages_and_idioms_are_not() {
    let e = engine();
    for (lang, text) in [
        (Lang::Es, "no quiero seguir viviendo"),
        (Lang::Es, "me quiero matar"),
        (Lang::Es, "mi pareja me pega"),
        (Lang::Es, "estoy pensando en suicidarme"),
        (Lang::En, "I want to die"),
        (Lang::En, "i'm going to kill myself"),
        (Lang::En, "someone is hurting me"),
        // Either language's patterns, whatever the turn's language.
        (Lang::En, "quiero morirme"),
    ] {
        let outcome = e.respond(turn(text, lang, "d"));
        assert_eq!(
            outcome.reason.as_ref().map(Reason::as_str),
            Some("distress"),
            "{text}"
        );
        assert_eq!(outcome.status, Status::Unhandled, "{text}");
        assert_eq!(outcome.lang, lang);
    }
    for (lang, text) in [
        (Lang::Es, "me muero de risa"),
        (Lang::Es, "me muero de ganas de verte"),
        (Lang::Es, "esto me mata"),
        (Lang::En, "I'm dying to see that film"),
        (Lang::En, "this traffic is killing me"),
        (Lang::En, "you killed it"),
    ] {
        let outcome = e.respond(turn(text, lang, "d"));
        assert_ne!(
            outcome.reason.as_ref().map(Reason::as_str),
            Some("distress"),
            "{text}"
        );
    }
}

/// A configuration with another region and no crisis line.
const ELSEWHERE: &[crate::Emergency] = &[crate::Emergency {
    region: "Elsewhere",
    places: &["Lisboa"],
    general: "999",
    crisis: None,
}];

#[test]
fn distress_numbers_come_from_the_configuration() {
    let e = engine();
    let at = |place: Option<&str>, text: &str, lang| {
        let mut t = turn(text, lang, "place");
        t.place = place.map(|p| crate::Place(p.to_owned()));
        e.respond(t).reply
    };
    let spain = at(Some("Sevilla"), "quiero morirme", Lang::Es);
    assert!(spain.contains("112") && spain.contains("024"), "{spain}");
    let zone = at(Some("Europe/Madrid"), "I want to die", Lang::En);
    assert!(zone.contains("112"), "{zone}");
    let named = at(None, "estoy en Bilbao y quiero morirme", Lang::Es);
    assert!(named.contains("112"), "{named}");
    let elsewhere = at(Some("Lisboa"), "quiero morirme", Lang::Es);
    assert!(
        !elsewhere.chars().any(|c| c.is_ascii_digit()),
        "{elsewhere}"
    );
    let nowhere = at(None, "I want to die", Lang::En);
    assert!(nowhere.contains("local emergency number"), "{nowhere}");

    // Another configuration: nothing is hard-coded in the scripts.
    let config = Config {
        emergency: ELSEWHERE,
        ..Config::default()
    };
    let e = Engine::new(config, Vec::new()).unwrap();
    let mut t = turn("quiero morirme", Lang::Es, "x");
    t.place = Some(crate::Place("Lisboa".to_owned()));
    let reply = e.respond(t).reply;
    assert!(reply.contains("999") && !reply.contains("112"), "{reply}");
}
