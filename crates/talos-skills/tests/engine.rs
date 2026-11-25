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
