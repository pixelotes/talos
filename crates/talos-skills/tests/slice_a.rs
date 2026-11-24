//! Slice A end to end: the built-in engine driven through `Engine::respond`,
//! with the "Done when" cases of docs/plans/v0.1.md as golden tests and
//! further cases from docs/spec/basics.md and conversation.md.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a failure is a test failure"
)]

use talos_core::{
    Config, ConvId, Engine, IntentId, Lang, Manifest, Outcome, Reason, Status, Timestamp, Turn,
};

/// 2027-03-14T17:42:00Z: 18:42 in Madrid, the spec's usual `now`.
const NOW: i64 = 1_805_046_120;

fn engine() -> Engine {
    talos_skills::engine(Config::default()).unwrap()
}

/// A first turn with no local clock offset known.
fn turn(text: &str, lang: Option<Lang>) -> Turn {
    Turn::new(text, ConvId::new("test"), 1, Timestamp::from_unix(NOW)).with_lang(lang)
}

/// A first turn at 18:42 Madrid time (UTC+1).
fn turn_madrid(text: &str, lang: Option<Lang>, hours_earlier: i64) -> Turn {
    let now = Timestamp::from_unix(NOW - hours_earlier * 3_600).with_utc_offset(3_600);
    Turn::new(text, ConvId::new("test"), 1, now).with_lang(lang)
}

#[track_caller]
fn assert_greet(outcome: &Outcome, reply: &str, lang: Lang) {
    assert_eq!(outcome.reply, reply, "{outcome:?}");
    assert_eq!(outcome.status, Status::Answered, "{outcome:?}");
    assert_eq!(
        outcome.intent.as_ref().map(IntentId::as_str),
        Some("smalltalk.greet")
    );
    assert_eq!(outcome.lang, lang, "{outcome:?}");
    assert!((outcome.confidence - 1.0).abs() < f32::EPSILON);
    assert_eq!(outcome.reason, None);
    assert!(!outcome.pending && !outcome.generated);
}

#[track_caller]
fn assert_unhandled(outcome: &Outcome, reply: &str, lang: Lang, reason: Reason) {
    assert_eq!(outcome.reply, reply, "{outcome:?}");
    assert_eq!(outcome.status, Status::Unhandled, "{outcome:?}");
    assert_eq!(outcome.intent, None, "{outcome:?}");
    assert_eq!(outcome.lang, lang, "{outcome:?}");
    assert_eq!(outcome.reason, Some(reason), "{outcome:?}");
    assert!(outcome.slots.is_empty());
    assert!(outcome.confidence.abs() < f32::EPSILON);
}

// --- Done when (docs/plans/v0.1.md, Slice A) --------------------------------

#[test]
fn golden_hola() {
    assert_greet(&engine().respond(turn("hola", None)), "¡Hola!", Lang::Es);
}

#[test]
fn golden_hello() {
    assert_greet(&engine().respond(turn("hello", None)), "Hello!", Lang::En);
}

#[test]
fn golden_generative_request_is_unhandled() {
    let outcome = engine().respond(turn("escríbeme un poema", None));
    assert_unhandled(
        &outcome,
        "No sé hacer eso.",
        Lang::Es,
        Reason::NotRecognised,
    );
}

// --- Greetings, Spanish -------------------------------------------------------

#[test]
fn spanish_greetings_in_any_case_and_punctuation() {
    let e = engine();
    for text in [
        "Hola",
        "HOLA",
        "¡Hola!",
        "hola!!!",
        "¿hola?",
        "holaaaa",
        "Hola, Talos.",
        "  hola  ",
        "ey",
    ] {
        assert_greet(&e.respond(turn(text, None)), "¡Hola!", Lang::Es);
    }
}

#[test]
fn buenas_is_answered_in_the_same_register() {
    let e = engine();
    for text in ["buenas", "Buenas!", "ey, muy buenas", "muy buenas, talos"] {
        let outcome = e.respond(turn(text, None));
        assert_greet(&outcome, "¡Buenas!", Lang::Es);
        assert_eq!(outcome.slots["register"], "casual");
    }
}

#[test]
fn time_of_day_greetings_with_and_without_accents() {
    let e = engine();
    // 18:42 in Madrid: "buenas tardes" agrees with the clock, "buenos días"
    // does not and gets a plain hello (basics.md, smalltalk.greet edge case).
    assert_greet(
        &e.respond(turn_madrid("Buenas tardes", None, 0)),
        "¡Buenas tardes!",
        Lang::Es,
    );
    assert_greet(
        &e.respond(turn_madrid("buenos días", None, 0)),
        "¡Hola!",
        Lang::Es,
    );
    assert_greet(
        &e.respond(turn_madrid("buenos dias talos", None, 0)),
        "¡Hola!",
        Lang::Es,
    );
    // 09:42 in Madrid.
    assert_greet(
        &e.respond(turn_madrid("Buenos días", None, 9)),
        "¡Buenos días!",
        Lang::Es,
    );
    assert_greet(
        &e.respond(turn_madrid("buenos dias", None, 9)),
        "¡Buenos días!",
        Lang::Es,
    );
    // No local clock: never risk contradicting it.
    let outcome = e.respond(turn("buenas tardes", None));
    assert_greet(&outcome, "¡Hola!", Lang::Es);
    assert_eq!(outcome.slots["daypart"], "afternoon");
}

// --- Greetings, English -------------------------------------------------------

#[test]
fn english_greetings_in_any_case_and_punctuation() {
    let e = engine();
    for text in ["hi", "Hello there", "hiya!", "HIII!!!", "Hello, Talos."] {
        assert_greet(&e.respond(turn(text, None)), "Hello!", Lang::En);
    }
}

#[test]
fn english_time_of_day_greetings() {
    let e = engine();
    assert_greet(
        &e.respond(turn_madrid("good evening", None, 0)),
        "Good evening!",
        Lang::En,
    );
    assert_greet(
        &e.respond(turn_madrid("Good morning, Talos", None, 0)),
        "Hello!",
        Lang::En,
    );
    assert_greet(
        &e.respond(turn_madrid("Good morning, Talos", None, 9)),
        "Good morning!",
        Lang::En,
    );
    assert_greet(&e.respond(turn("good evening", None)), "Hello!", Lang::En);
}

#[test]
fn hey_carries_no_language_evidence() {
    let e = engine();
    // One word valid in both languages: the configured default decides.
    assert_greet(&e.respond(turn("hey", None)), "¡Hola!", Lang::Es);
    assert_greet(&e.respond(turn("hey", Some(Lang::En))), "Hey!", Lang::En);
    let english_default = talos_skills::engine(Config {
        default_lang: Lang::En,
    })
    .unwrap();
    assert_greet(
        &english_default.respond(turn("hey", None)),
        "Hey!",
        Lang::En,
    );
}

// --- Language -----------------------------------------------------------------

#[test]
fn the_callers_hint_wins_and_the_other_languages_rules_still_apply() {
    let e = engine();
    assert_greet(&e.respond(turn("hola", Some(Lang::En))), "Hello!", Lang::En);
    assert_greet(
        &e.respond(turn("hello", Some(Lang::Es))),
        "¡Hola!",
        Lang::Es,
    );
    assert_greet(&e.respond(turn("buenas", Some(Lang::En))), "Hey!", Lang::En);
}

// --- Not greetings -----------------------------------------------------------

#[test]
fn a_greeting_with_more_after_it_is_not_a_bare_greeting() {
    let e = engine();
    for text in ["hola, qué hora es", "buenas noches", "hola, ¿qué tal?"] {
        assert_unhandled(
            &e.respond(turn(text, None)),
            "No sé hacer eso.",
            Lang::Es,
            Reason::NotRecognised,
        );
    }
    for text in ["good night", "hello, what's 3 times 4"] {
        assert_unhandled(
            &e.respond(turn(text, None)),
            "I can't do that.",
            Lang::En,
            Reason::NotRecognised,
        );
    }
}

#[test]
fn generative_requests_and_gibberish_are_unhandled_in_the_turns_language() {
    let e = engine();
    let cases = [
        (
            "escríbeme un poema sobre el mar",
            None,
            Lang::Es,
            "No sé hacer eso.",
        ),
        (
            "write me a poem about the sea",
            None,
            Lang::En,
            "I can't do that.",
        ),
        ("asdkjh qwe", None, Lang::Es, "No sé hacer eso."),
        ("blorp zzz", Some(Lang::En), Lang::En, "I can't do that."),
        ("?", None, Lang::Es, "No sé hacer eso."),
    ];
    for (text, hint, lang, reply) in cases {
        assert_unhandled(
            &e.respond(turn(text, hint)),
            reply,
            lang,
            Reason::NotRecognised,
        );
    }
}

#[test]
fn empty_input_is_unhandled() {
    let e = engine();
    assert_unhandled(
        &e.respond(turn("", None)),
        "No he recibido ningún texto.",
        Lang::Es,
        Reason::Empty,
    );
    assert_unhandled(
        &e.respond(turn(" \t ", Some(Lang::En))),
        "I didn't receive any text.",
        Lang::En,
        Reason::Empty,
    );
}

// --- Determinism and variants ------------------------------------------------

#[test]
fn the_same_turn_twice_gives_byte_identical_outcomes() {
    let e = engine();
    for text in ["hola", "hello", "escríbeme un poema", "Buenas tardes", ""] {
        let t = turn_madrid(text, None, 0).with_trace();
        let first = e.respond(t.clone());
        let second = e.respond(t.clone());
        assert_eq!(first, second);
        assert_eq!(
            format!("{first:?}").into_bytes(),
            format!("{second:?}").into_bytes()
        );
        // A second engine built from scratch agrees too: no hidden state.
        assert_eq!(first, engine().respond(t));
    }
}

#[test]
fn variants_depend_only_on_conversation_and_turn_number() {
    let e = engine();
    let at = |conversation: &str, number: u32| {
        let t = Turn::new(
            "hola",
            ConvId::new(conversation),
            number,
            Timestamp::from_unix(NOW),
        );
        e.respond(t).reply
    };
    // A conversation opens with the canonical wording.
    assert_eq!(at("a", 1), "¡Hola!");
    assert_eq!(at("b", 1), "¡Hola!");
    // Later turns vary, reproducibly, and never change the content's kind.
    let replies: Vec<String> = (1..=6).map(|n| at("a", n)).collect();
    assert_eq!(replies, (1..=6).map(|n| at("a", n)).collect::<Vec<_>>());
    assert!(
        replies
            .iter()
            .all(|r| r == "¡Hola!" || r == "¡Hola, buenas!")
    );
    assert!(replies.iter().any(|r| r == "¡Hola, buenas!"));
}

#[test]
fn the_trace_lists_every_stage_on_request() {
    let e = engine();
    assert!(e.respond(turn("hola", None)).trace.is_none());
    let trace = e.respond(turn("hola", None).with_trace()).trace.unwrap();
    let stages: Vec<&str> = trace.steps.iter().map(|s| s.stage).collect();
    assert_eq!(
        stages,
        ["normalise", "language", "rules", "dispatch", "render"]
    );
    let trace = e.respond(turn("blorp", None).with_trace()).trace.unwrap();
    let stages: Vec<&str> = trace.steps.iter().map(|s| s.stage).collect();
    assert_eq!(stages, ["normalise", "language", "rules", "fallback"]);
}

// --- Manifests ---------------------------------------------------------------

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

    let no_spanish_examples = text.replacen("  es = [", "  # es = [", 1);
    let error = Manifest::parse(dir, &no_spanish_examples)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("intent `smalltalk.greet` examples lacks `es`"),
        "{error}"
    );
}
