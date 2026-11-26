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
