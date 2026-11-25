//! Determinism (DESIGN.md section 1): identical input and state give
//! byte-identical output. The golden suite is run twice with fresh engines,
//! then again in the opposite file order, and the transcripts it produces
//! (not just pass or fail) are compared; two engines built from the same
//! `Config` answer a fixed list of messages identically.

#![allow(
    clippy::unwrap_used,
    reason = "test harness; a failure is a test failure"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use talos_core::golden::Golden;
use talos_core::{Config, ConvId, Engine, Lang, Outcome, Timestamp, Turn};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden")
}

fn golden_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(golden_files(&path));
        } else if path.extension().is_some_and(|e| e == "txt") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Runs one golden file the way the harness does, but returns what the
/// engines produced as text instead of comparing it. Engines are shared
/// across the files of one run, one per configuration, as in the harness.
fn transcript(file: &Path, engines: &mut Vec<(Config, Engine)>) -> String {
    let golden = Golden::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
    let conversation = ConvId::new("golden");
    let mut out = String::new();
    for case in &golden.cases {
        let s = &case.settings;
        let config = Config {
            default_lang: s.default_lang,
            seed: s.seed,
            ..Config::default()
        };
        let index = engines
            .iter()
            .position(|(c, _)| *c == config)
            .unwrap_or_else(|| {
                engines.push((config, talos_skills::engine(config).unwrap()));
                engines.len() - 1
            });
        let engine = &engines[index].1;
        if case.starts_conversation {
            engine.end(&conversation);
            engine.set_user_data(s.user_data()).unwrap();
        }
        let turn = Turn::new(case.text.clone(), conversation.clone(), s.now)
            .with_lang(s.hint)
            .with_place(s.place.clone())
            .with_trace();
        let outcome = match &case.draws {
            Some(draws) => engine.respond_scripted(turn, draws.clone()).0,
            None => engine.respond(turn),
        };
        let internal = outcome.trace.as_ref().map(|t| &t.internal_slots);
        let _ = writeln!(
            out,
            "[{}] {}\n{}\n{}",
            outcome.lang,
            case.text,
            outcome.reply,
            outcome.line(internal)
        );
    }
    out
}

/// One run of the whole suite: (file, transcript) in the order given.
fn run(files: &[PathBuf]) -> Vec<(PathBuf, String)> {
    let mut engines = Vec::new();
    files
        .iter()
        .map(|f| (f.clone(), transcript(f, &mut engines)))
        .collect()
}

#[test]
fn golden_transcripts_are_byte_identical_between_runs() {
    let files = golden_files(&golden_dir());
    assert!(files.len() > 1, "no golden files found");
    let first = run(&files);
    let second = run(&files);
    assert!(first.iter().any(|(_, t)| !t.is_empty()));
    assert_eq!(first, second, "same order, fresh engines");

    let mut reversed = files.clone();
    reversed.reverse();
    let mut third = run(&reversed);
    third.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(first, third, "opposite file order");
}

/// What an outcome must reproduce: not the elapsed time or the trace.
fn key(outcome: &Outcome) -> String {
    format!(
        "{:?}|{}|{:?}|{:?}|{:?}",
        outcome.reply, outcome.status, outcome.intent, outcome.slots, outcome.lang
    )
}

const INPUTS: &[(&str, Option<Lang>)] = &[
    ("hola", None),
    ("hello", None),
    ("good morning talos", None),
    ("¿qué tal?", Some(Lang::Es)),
    ("how are you", Some(Lang::En)),
    ("qué hora es", Some(Lang::Es)),
    ("what time is it", Some(Lang::En)),
    ("tira un dado", Some(Lang::Es)),
    ("roll a die", Some(Lang::En)),
    ("roll 3d6", Some(Lang::En)),
    ("tira 2d10", Some(Lang::Es)),
    ("cara o cruz", Some(Lang::Es)),
    ("flip a coin", Some(Lang::En)),
    ("elige entre té y café", Some(Lang::Es)),
    ("pick between tea and coffee", Some(Lang::En)),
    ("dime una frase célebre", Some(Lang::Es)),
    ("tell me a saying", Some(Lang::En)),
    ("otro", Some(Lang::Es)),
    ("another one", Some(Lang::En)),
    ("cuánto es 2+2", Some(Lang::Es)),
    ("what is 17 * 3", Some(Lang::En)),
    ("calcula 10 / 4", Some(Lang::Es)),
    ("qué puedes hacer", Some(Lang::Es)),
    ("what can you do", Some(Lang::En)),
    ("me voy a Sevilla el viernes", Some(Lang::Es)),
    ("I'm off to Paris", Some(Lang::En)),
    ("estoy muy triste", Some(Lang::Es)),
    ("I feel so tired of everything", Some(Lang::En)),
    ("escríbeme un poema", Some(Lang::Es)),
    ("write me a poem", Some(Lang::En)),
    ("asdf qwer", None),
    ("", None),
];

#[test]
fn same_config_gives_the_same_outcomes() {
    let config = Config {
        seed: 42,
        ..Config::default()
    };
    let a = talos_skills::engine(config).unwrap();
    let b = talos_skills::engine(config).unwrap();
    let now = Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600);
    let conversation = ConvId::new("determinism");
    for (text, lang) in INPUTS {
        let turn = || Turn::new(*text, conversation.clone(), now).with_lang(*lang);
        let (x, y) = (a.respond(turn()), b.respond(turn()));
        assert_eq!(key(&x), key(&y), "input {text:?}");
        assert_eq!(x.intent, y.intent);
        assert_eq!(x.slots, y.slots);
    }
}
