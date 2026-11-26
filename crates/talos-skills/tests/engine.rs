//! The built-in engine through `Engine::respond`, for what golden files do
//! not express: traces, replay, and manifests that must be rejected.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a failure is a test failure"
)]

use talos_core::{Config, ConvId, Engine, Lang, Manifest, Timestamp, Turn};

/// 2027-03-14T17:42:00Z: 18:42 in Madrid, the spec's usual `now`.
const NOW: i64 = 1_805_046_120;

fn engine() -> Engine {
    talos_skills::engine(Config::default()).unwrap()
}

fn turn(text: &str) -> Turn {
    Turn::new(
        text,
        ConvId::new("test"),
        Timestamp::from_unix(NOW).with_utc_offset(3_600),
    )
}

#[test]
fn the_same_turn_in_the_same_state_gives_byte_identical_outcomes() {
    for text in ["hola", "hello", "escríbeme un poema", "Buenas tardes", ""] {
        let t = turn(text).with_trace();
        // Two fresh engines: no hidden state, no clock, no entropy.
        let first = engine().respond(t.clone());
        let second = engine().respond(t);
        assert_eq!(first, second);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }
}

#[test]
fn the_trace_lists_every_stage_on_request() {
    let e = engine();
    assert!(e.respond(turn("hola")).trace.is_none());
    let stages = |text: &str| -> Vec<&'static str> {
        e.end(&ConvId::new("test"));
        let trace = e.respond(turn(text).with_trace()).trace.unwrap();
        trace.steps.iter().map(|s| s.stage).collect()
    };
    assert_eq!(
        stages("hola"),
        [
            "normalise",
            "language",
            "safety",
            "rules",
            "slots",
            "dispatch",
            "render",
            "random",
            "state"
        ]
    );
    assert_eq!(
        stages("blorp"),
        [
            "normalise",
            "language",
            "safety",
            "rules",
            "classifier",
            "reflector",
            "random",
            "state"
        ]
    );
    // Distress is answered by the safety check: no rules, no reflector.
    assert_eq!(
        stages("quiero morirme"),
        ["normalise", "language", "safety", "random", "state"]
    );
    let trace = e.respond(turn("buenos días").with_trace()).trace.unwrap();
    let text: Vec<String> = trace
        .steps
        .iter()
        .map(|s| format!("{}: {}", s.stage, s.note))
        .collect();
    let text = text.join("\n");
    for expected in [
        "match form \"buenos dias\"",
        "smalltalk.greet (priority 100",
        "daypart=morning",
        "answer `hello`",
        "of 2 (turn 2)",
        "random: 0 draws",
        "recent topic smalltalk.greet",
    ] {
        assert!(text.contains(expected), "{expected:?} not in\n{text}");
    }
}

#[test]
fn the_classifier_answers_what_no_rule_matches_and_says_why() {
    // Training phrases (manifest examples), so the assertions hold for any
    // reasonable model; held-out quality is `talos eval`'s business.
    let e = engine();
    let note = |outcome: &talos_core::Outcome| -> String {
        let trace = outcome.trace.as_ref().unwrap();
        let step = trace.steps.iter().find(|s| s.stage == "classifier");
        step.map_or_else(String::new, |s| s.note.clone())
    };
    for (text, intent) in [
        ("me gustaría saber qué hora es", "clock.now"),
        ("i'd like to know what time it is", "clock.now"),
        ("tengo curiosidad por saber tu nombre", "identity.who"),
    ] {
        e.end(&ConvId::new("test"));
        let outcome = e.respond(turn(text).with_trace());
        assert_eq!(
            outcome.intent.as_ref().map(talos_core::IntentId::as_str),
            Some(intent),
            "{text}"
        );
        assert!(
            outcome.confidence > 0.0 && outcome.confidence < 1.0,
            "{text}"
        );
        assert!(
            note(&outcome).contains(&format!("accepted {intent}")),
            "{text}"
        );
    }
    // Out of scope: the classifier declines and the reflector answers.
    e.end(&ConvId::new("test"));
    let outcome = e.respond(turn("write me a poem").with_trace());
    assert_eq!(outcome.intent, None);
    assert_eq!(outcome.confidence, 0.0);
    assert!(note(&outcome).contains("out-of-scope is the top class"));
    // A rule match never reaches the classifier and is certain.
    let outcome = e.respond(turn("qué hora es").with_trace());
    assert_eq!(outcome.confidence, 1.0);
    assert_eq!(note(&outcome), "");
}

#[test]
fn the_language_model_decides_only_what_markers_leave_open() {
    let e = engine();
    let first = |text: &str| {
        e.end(&ConvId::new("test"));
        e.respond(turn(text)).lang
    };
    // No marker word in either: the model decides (ADR 0015, step 3).
    assert_eq!(first("bye"), Lang::En);
    assert_eq!(first("thx"), Lang::En);
    // Shared by both languages, or no word at all: the default (es).
    assert_eq!(first("hey"), Lang::Es);
    assert_eq!(first("ok"), Lang::Es);
    assert_eq!(first("2+2"), Lang::Es);
    assert_eq!(first("d20"), Lang::Es);
}

#[test]
fn a_hint_settles_the_conversation_language() {
    let e = engine();
    assert_eq!(
        e.respond(turn("hola").with_lang(Some(Lang::En))).lang,
        Lang::En
    );
    assert_eq!(e.respond(turn("hola")).lang, Lang::En);
}

#[test]
fn a_manifest_missing_a_language_is_rejected() {
    let (dir, text) = talos_skills::manifest_sources()
        .iter()
        .find(|(dir, _)| *dir == "smalltalk")
        .unwrap();
    assert!(Manifest::parse(dir, text).is_ok());

    let no_english_reply = text.replace("en = [\"Hello!\", \"Hi!\"]\n", "");
    assert_ne!(&no_english_reply, text);
    let error = Manifest::parse(dir, &no_english_reply)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "manifest `smalltalk`: reply `hello` lacks `en`: both languages are required"
    );
}

/// What `define` answers from a dictionary pack: the entry as the
/// dictionary has it, not a concept chosen among senses.
#[cfg(feature = "knowledge")]
#[test]
fn a_dictionary_pack_answers_define_with_its_entry() {
    use talos_core::Knowledge;
    use talos_core::Status;

    let mut k = Knowledge::from_source(include_str!("../../../knowledge/seed.toml")).unwrap();
    k.attach_source(include_str!("../../../knowledge/test-dictionary.toml"))
        .unwrap();
    assert_eq!(k.stores(), ["core"], "a dictionary is not a concept store");
    let mut e = engine();
    e.use_knowledge(k);
    let ask = |text: &str| {
        e.end(&ConvId::new("test"));
        e.respond(turn(text))
    };

    // An inflected form: the headwords it belongs to, the shortest first,
    // the pronominal verb folded into its verb; three definitions, each
    // closed as a sentence, and how many are left.
    let fui = ask("qué significa fui");
    assert_eq!(fui.status, Status::Answered);
    assert_eq!(
        fui.reply,
        "«fui» es una forma de «ir», y también de «ser».\n\
         Ir, verbo: 1. Desplazarse de un lugar a otro. 2. Extenderse algo de un punto a otro. \
         3. Combinar una cosa con otra. (y 1 más)\n\
         Fuente: Diccionario de prueba."
    );
    // A headword with two parts of speech: a line each.
    assert_eq!(
        ask("define ser").reply,
        "Ser, verbo: Existir.\nSer, sustantivo: Cualquier cosa que existe.\n\
         Fuente: Diccionario de prueba."
    );
    assert_eq!(
        ask("qué significa lentamente").reply,
        "Lentamente, adverbio: De un modo lento.\nFuente: Diccionario de prueba."
    );
    // A word the dictionary lacks is answered from the concept packs, as
    // before; so is an English conversation, which this dictionary is not
    // for.
    let guitarra = ask("qué significa guitarra").reply;
    assert!(guitarra.starts_with("Guitarra: instrumento"), "{guitarra}");
    let guitar = ask("what does guitar mean").reply;
    assert!(guitar.starts_with("Guitar: "), "{guitar}");
}

/// A word of many senses in a concept pack is answered as an entry, by
/// part of speech, when nothing in the conversation chooses a sense.
#[cfg(feature = "knowledge")]
#[test]
fn a_word_of_many_senses_is_answered_as_an_entry() {
    use talos_core::Knowledge;

    let mut source = String::from(
        "[pack]\nid = \"many\"\nversion = \"1\"\ntitle = \"many senses\"\n\
         languages = [\"en\"]\nlicense = \"MIT\"\nnotice = \"for a test\"\n\
         date = \"2026-11-26\"\nsource = \"many\"\n\n\
         [[source]]\nname = \"many\"\nkind = \"pack\"\n",
    );
    let senses = [
        ("v", "move fast on foot (briefly)", ""),
        ("v", "flee", ""),
        ("n", "a score in baseball", ""),
        ("v", "extend between two points", ""),
        ("v", "operate", ""),
        ("n", "a race", ""),
        ("adj", "melted", ""),
        ("v", "function", ""),
    ];
    for (i, (pos, gloss, _)) in senses.iter().enumerate() {
        let forms = if i == 0 {
            ", forms = { ran = \"past\" }"
        } else {
            ""
        };
        source += &[
            format!("\n[[concept]]\nid = \"many:{i}\"\n"),
            format!("en = [{{ lemma = \"run\", pos = \"{pos}\", rank = {i}{forms} }}]\n"),
            format!("gloss.en = \"{gloss}\"\n"),
        ]
        .concat();
    }
    let mut e = engine();
    e.use_knowledge(Knowledge::from_source(&source).unwrap());
    let ask = |text: &str| {
        e.end(&ConvId::new("test"));
        e.respond(turn(text)).reply
    };
    let entry = "Run, verb: 1. Move fast on foot (briefly). 2. Flee. \
                 3. Extend between two points. (and 2 more)";
    assert_eq!(
        ask("what does run mean"),
        format!(
            "{entry}\nRun, noun: 1. A score in baseball. 2. A race.\nAlso listed as adjective."
        )
    );
    // A verb's form reaches only the verb.
    assert_eq!(
        ask("what does ran mean"),
        format!("\"ran\" is a form of \"run\".\n{entry}")
    );
}

/// Pages recorded for the tests of the network skills: the first whose
/// marker the address contains answers; an address with none fails.
#[derive(Debug)]
struct Recorded(Vec<(&'static str, &'static str)>);

impl talos_core::net::Fetch for Recorded {
    fn get(&self, url: &str) -> Result<String, talos_core::net::FetchError> {
        self.0
            .iter()
            .find(|(marker, _)| url.contains(marker))
            .map(|(_, body)| (*body).to_owned())
            .ok_or_else(|| talos_core::net::FetchError::Failed(format!("not recorded: {url}")))
    }
}

const VALENCIA: &str = r#"{"batchcomplete":true,"query":{"pages":[
  {"pageid":1,"title":"Valencia","index":1,
   "extract":"Valencia es un municipio y una ciudad de España,\u200b capital de la Comunidad Valenciana.\nSegundo párrafo.",
   "fullurl":"https://es.wikipedia.org/wiki/Valencia"},
  {"pageid":2,"title":"Valencia Club de Fútbol","index":2,"extract":"El Valencia es un club.",
   "fullurl":"https://es.wikipedia.org/wiki/Valencia_Club_de_F%C3%BAtbol"}]}}"#;
const MERCURIO: &str = r#"{"query":{"pages":[
  {"title":"Mercurio","index":1,"extract":"Mercurio puede referirse a:","pageprops":{"disambiguation":""},
   "fullurl":"https://es.wikipedia.org/wiki/Mercurio"},
  {"title":"Mercurio (planeta)","index":3,"extract":"Mercurio es el planeta más próximo al Sol.",
   "fullurl":"https://es.wikipedia.org/wiki/Mercurio_(planeta)"},
  {"title":"Mercurio (elemento)","index":2,"extract":"El mercurio es un elemento químico.",
   "fullurl":"https://es.wikipedia.org/wiki/Mercurio_(elemento)"}]}}"#;
const PLANETA: &str = r#"{"query":{"pages":[
  {"title":"Mercurio (elemento)","index":1,"extract":"El mercurio es un elemento químico.",
   "fullurl":"https://es.wikipedia.org/wiki/Mercurio_(elemento)"},
  {"title":"Mercurio (planeta)","index":2,"extract":"Mercurio es el planeta más próximo al Sol.",
   "fullurl":"https://es.wikipedia.org/wiki/Mercurio_(planeta)"}]}}"#;
const NOTHING: &str = r#"{"batchcomplete":true}"#;
const LOVELACE: &str = r#"{"query":{"pages":[
  {"title":"Ada Lovelace","index":1,"extract":"Ada Lovelace was an English mathematician.",
   "fullurl":"https://en.wikipedia.org/wiki/Ada_Lovelace"}]}}"#;
const BRUTALISMO: &str = r#"{"query":{"pages":[
  {"title":"Arquitectura brutalista","index":1,"extract":"El brutalismo es un estilo arquitectónico.",
   "fullurl":"https://es.wikipedia.org/wiki/Arquitectura_brutalista"}]}}"#;

fn online() -> Engine {
    let mut e = engine();
    e.use_network(Box::new(Recorded(vec![
        ("es.wikipedia.org/w/api.php?action=query&format=json&formatversion=2&redirects=1&generator=search&gsrnamespace=0&gsrlimit=5&gsrsearch=Valencia&", VALENCIA),
        ("gsrsearch=Segunda%20Guerra&", VALENCIA),
        ("gsrsearch=Mercurio%20%28planeta%29&", PLANETA),
        ("gsrsearch=Mercurio&", MERCURIO),
        ("es.wikipedia.org/w/api.php?action=query&format=json&formatversion=2&redirects=1&generator=search&gsrnamespace=0&gsrlimit=5&gsrsearch=Ada%20Lovelace&", NOTHING),
        ("en.wikipedia.org/w/api.php?action=query&format=json&formatversion=2&redirects=1&generator=search&gsrnamespace=0&gsrlimit=5&gsrsearch=Ada%20Lovelace&", LOVELACE),
        ("gsrsearch=xyzzy&", NOTHING),
        ("gsrsearch=brutalismo&", BRUTALISMO),
    ])));
    e
}

#[test]
fn wikipedia_says_it_has_no_network_until_it_is_given_one() {
    use talos_core::Status;

    let e = engine();
    let outcome = e.respond(turn("háblame de Valencia"));
    assert_eq!(outcome.status, Status::Unhandled);
    assert_eq!(
        outcome.intent.as_ref().map(talos_core::IntentId::as_str),
        Some("wikipedia.about")
    );
    assert_eq!(
        outcome.reply,
        "No tengo acceso a la red, así que no puedo consultar Wikipedia."
    );
}

#[test]
fn wikipedia_answers_with_the_opening_as_written_and_its_source() {
    use talos_core::Status;

    let e = online();
    let ask = |text: &str| {
        e.end(&ConvId::new("test"));
        e.respond(turn(text))
    };
    let valencia = "Valencia es un municipio y una ciudad de España, capital de la Comunidad \
                    Valenciana.\n\
                    Fuente: Wikipedia, «Valencia»: https://es.wikipedia.org/wiki/Valencia";
    for text in [
        "háblame de Valencia",
        "cuéntame lo que sepas de Valencia",
        "quiero saber más acerca de Valencia",
        "¿Qué sabes de Valencia?",
        "/wiki Valencia",
    ] {
        let outcome = ask(text);
        assert_eq!(outcome.status, Status::Answered, "{text}");
        assert_eq!(outcome.reply, valencia, "{text}");
        assert_eq!(outcome.fallback, None, "{text}");
    }
    // The article it was said with is not part of the search.
    assert!(
        ask("háblame de la Segunda Guerra")
            .reply
            .starts_with("Valencia es")
    );
    // Not in the conversation's Wikipedia: the other one, saying so.
    let ada = ask("quién fue Ada Lovelace");
    assert_eq!(
        ada.reply,
        "No lo encuentro en la Wikipedia en español; esto dice la inglesa.\n\
         Ada Lovelace was an English mathematician.\n\
         Fuente: Wikipedia, «Ada Lovelace»: https://en.wikipedia.org/wiki/Ada_Lovelace"
    );
    assert_eq!(
        ada.fallback.as_ref().map(ToString::to_string).as_deref(),
        Some("other-language")
    );
    // In neither; and a page that was not recorded stands for a failure.
    let none = ask("háblame de xyzzy");
    assert_eq!(none.status, Status::Unhandled);
    assert_eq!(none.reply, "No encuentro nada sobre «xyzzy» en Wikipedia.");
    assert_eq!(
        ask("háblame de Zaragoza").reply,
        "No he podido consultar Wikipedia ahora mismo. Prueba dentro de un rato."
    );
    // Who Talos is stays `identity`'s.
    assert!(ask("quién eres").reply.starts_with("Soy Talos"));
}

#[test]
fn wikipedia_asks_which_when_a_name_is_several_things() {
    let e = online();
    let question = e.respond(turn("háblame de Mercurio"));
    assert!(question.pending);
    assert_eq!(
        question.reply,
        "«Mercurio» puede ser varias cosas. ¿Cuál: Mercurio (elemento) o Mercurio (planeta)?"
    );
    assert_eq!(
        e.respond(turn("el planeta")).reply,
        "Mercurio es el planeta más próximo al Sol.\n\
         Fuente: Wikipedia, «Mercurio (planeta)»: https://es.wikipedia.org/wiki/Mercurio_(planeta)"
    );
}

/// A word no dictionary has is looked up in the encyclopedia when the
/// network is allowed, and the reply says where it comes from.
#[cfg(feature = "knowledge")]
#[test]
fn define_falls_back_to_wikipedia_for_a_word_no_dictionary_has() {
    use talos_core::{Knowledge, Status};

    let mut e = online();
    e.use_knowledge(Knowledge::from_source(include_str!("../../../knowledge/seed.toml")).unwrap());
    let outcome = e.respond(turn("qué significa brutalismo"));
    assert_eq!(outcome.status, Status::Answered);
    assert_eq!(
        outcome.reply,
        "No tengo «brutalismo» en el diccionario; esto dice Wikipedia.\n\
         El brutalismo es un estilo arquitectónico.\n\
         Fuente: Wikipedia, «Arquitectura brutalista»: \
         https://es.wikipedia.org/wiki/Arquitectura_brutalista"
    );
    assert_eq!(
        outcome
            .fallback
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("encyclopedia")
    );
    // With the network off, as before.
    let mut offline = engine();
    offline.use_knowledge(
        Knowledge::from_source(include_str!("../../../knowledge/seed.toml")).unwrap(),
    );
    assert_eq!(
        offline.respond(turn("qué significa brutalismo")).reply,
        "No conozco la palabra «brutalismo»."
    );
}
