//! The stand-alone conversation batteries (`evals/conversations.{es,en}.jsonl`,
//! format in `evals/README.md`) run end to end through the built-in engine.
//!
//! A baseline, not a gate (docs/plans/v0.1.md, Slice D): the test fails
//! only if a battery cannot be read or run. It reports, per language, the
//! catch-all rate (the share of turns ending `unhandled · catch-all`,
//! docs/spec/reflector.md 7), the turns by status, and how many turns got
//! the status the battery expects. To see the report:
//!
//! ```text
//! cargo test -p talos --test conversations -- --nocapture
//! ```
//!
//! Conversations with a `given` (knowledge in the store) are skipped. The
//! phrases are a test set: never copy them into a script or a manifest
//! (evals/README.md, independence rule).

#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "test harness; a failure is a test failure"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value as Json;
use talos_core::{Config, ConvId, Lang, Status, Timestamp, Turn};

/// What one language's battery gave.
#[derive(Debug, Default)]
struct Report {
    conversations: usize,
    skipped: usize,
    turns: usize,
    catch_all: usize,
    by_status: BTreeMap<&'static str, usize>,
    by_reason: BTreeMap<String, usize>,
    status_matches: usize,
    /// Turns the battery means for the reflector (no intent, not an
    /// acknowledgement, cancellation or follow-up), and how many of those
    /// ended in the catch-all: the rate without the skills still missing.
    reflector_turns: usize,
    reflector_catch_all: usize,
    /// Turns the battery expects `unhandled` that came out `deflected`:
    /// the false deflections of reflector.md 7.
    false_deflections: Vec<String>,
}

fn run(lang: Lang) -> Report {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../evals")
        .join(format!("conversations.{lang}.jsonl"));
    let text = std::fs::read_to_string(&path).unwrap();
    let engine = talos_skills::engine(Config::default()).unwrap();
    let now = Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600);
    let mut report = Report::default();
    for (n, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let conversation: Json = serde_json::from_str(line).unwrap();
        if conversation.get("given").is_some() {
            report.skipped += 1;
            continue;
        }
        report.conversations += 1;
        let id = ConvId::new(format!("battery-{lang}-{n}"));
        for t in conversation["turns"].as_array().unwrap() {
            let user = t["user"].as_str().unwrap();
            let expected = t["expect"]["status"].as_str().unwrap();
            let outcome = engine.respond(Turn::new(user, id.clone(), now).with_lang(Some(lang)));
            report.turns += 1;
            *report.by_status.entry(outcome.status.as_str()).or_default() += 1;
            let reason = outcome
                .reason
                .as_ref()
                .map_or("-", |r| r.as_str())
                .to_owned();
            if outcome.intent.is_none() {
                *report.by_reason.entry(reason.clone()).or_default() += 1;
            }
            if reason == "catch-all" {
                report.catch_all += 1;
            }
            let for_reflector = t["expect"]["intent"].is_null()
                && ["ack", "cancel", "follow_up"]
                    .iter()
                    .all(|k| t["expect"].get(*k).is_none());
            if for_reflector {
                report.reflector_turns += 1;
                if reason == "catch-all" {
                    report.reflector_catch_all += 1;
                }
            }
            if outcome.status.as_str() == expected {
                report.status_matches += 1;
            }
            if expected == "unhandled" && outcome.status == Status::Deflected {
                report
                    .false_deflections
                    .push(format!("{user:?} → {reason}"));
            }
        }
    }
    report
}

fn describe(lang: Lang, r: &Report) -> String {
    let pct = |n: usize| 100.0 * n as f64 / r.turns.max(1) as f64;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "[{lang}] {} conversations ({} skipped for `given`), {} turns",
        r.conversations, r.skipped, r.turns
    );
    let _ = writeln!(
        out,
        "  catch-all rate: {:.1} % ({} of {})",
        pct(r.catch_all),
        r.catch_all,
        r.turns
    );
    let _ = writeln!(
        out,
        "  catch-all on turns meant for the reflector: {:.1} % ({} of {})",
        100.0 * r.reflector_catch_all as f64 / r.reflector_turns.max(1) as f64,
        r.reflector_catch_all,
        r.reflector_turns
    );
    let statuses: Vec<String> = r
        .by_status
        .iter()
        .map(|(s, n)| format!("{s} {n}"))
        .collect();
    let _ = writeln!(out, "  by status: {}", statuses.join(", "));
    let reasons: Vec<String> = r
        .by_reason
        .iter()
        .map(|(s, n)| format!("{s} {n}"))
        .collect();
    let _ = writeln!(
        out,
        "  reflector turns by shape or reason: {}",
        reasons.join(", ")
    );
    let _ = writeln!(
        out,
        "  status as expected: {} of {} ({:.1} %)",
        r.status_matches,
        r.turns,
        pct(r.status_matches)
    );
    let _ = writeln!(out, "  false deflections: {}", r.false_deflections.len());
    for f in &r.false_deflections {
        let _ = writeln!(out, "    {f}");
    }
    out
}

#[test]
fn conversation_batteries_run_end_to_end() {
    for lang in Lang::ALL {
        let report = run(lang);
        assert!(report.turns > 0, "no turns in the {lang} battery");
        println!("{}", describe(lang, &report));
    }
}
