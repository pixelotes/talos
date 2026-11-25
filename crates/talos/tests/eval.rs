//! `talos eval` on the real batteries: only that the report is produced and
//! that its row counts match the files. The scores are a baseline, not a gate.

#![allow(
    clippy::unwrap_used,
    reason = "test harness; a failure is a test failure"
)]

use std::path::Path;
use std::process::Command;

use serde_json::Value as Json;

fn rows(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals")
}

#[test]
fn the_report_counts_every_row_of_the_batteries() {
    let dir = dir();
    let output = Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(["eval", "--json", "--dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Json = serde_json::from_slice(&output.stdout).unwrap();
    let languages = report["languages"].as_array().unwrap();
    assert_eq!(languages.len(), 2);
    for language in languages {
        let lang = language["lang"].as_str().unwrap();
        let intents = rows(&dir.join(format!("intents.{lang}.jsonl")));
        let reject = rows(&dir.join(format!("reject.{lang}.jsonl")));
        assert_eq!(language["intents"]["rows"], intents, "{lang}");
        assert_eq!(language["reject"]["rows"], reject, "{lang}");
        let per_intent: u64 = language["intents"]["per_intent"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["rows"].as_u64().unwrap())
            .sum();
        assert_eq!(per_intent, intents as u64, "{lang}");
    }
}

#[test]
fn the_text_report_is_printed() {
    let output = Command::new(env!("CARGO_BIN_EXE_talos"))
        .arg("eval")
        .arg("--dir")
        .arg(dir())
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("[es] intents, built skills") && text.contains("[en] reject ("));
    assert!(text.contains("+classifier"));
}

/// The floors: recognition may not fall below what was last measured and
/// recorded (evals/BASELINE.md), per language. They are set a little under
/// the measured figures, not at the milestone's targets, which are not met
/// yet; raise them when the figures rise.
#[test]
fn recognition_does_not_fall_below_its_floors() {
    // (language, built-skill accuracy at least, held-out accuracy at least,
    // false accepts at most)
    // Reset on 2026-11-25 when `define`, `translate` and `teach` were built:
    // their intents joined the "built" rows at about 60 % and pulled the
    // aggregate down, while the nine earlier skills held (222 and 216 of 250).
    const FLOORS: [(&str, f64, f64, u64); 2] = [("es", 0.84, 0.82, 4), ("en", 0.81, 0.78, 3)];
    let output = Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(["eval", "--json", "--dir"])
        .arg(dir())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["classifier"], true, "the floors need the model");
    for language in report["languages"].as_array().unwrap() {
        let lang = language["lang"].as_str().unwrap();
        let (_, built, held_out, accepts) = FLOORS.iter().find(|f| f.0 == lang).unwrap();
        let intents = &language["intents"];
        assert!(
            intents["built_accuracy"].as_f64().unwrap() >= *built,
            "{lang}: built-skill accuracy {} is under the floor {built}",
            intents["built_accuracy"]
        );
        assert!(
            intents["built_held_out_accuracy"].as_f64().unwrap() >= *held_out,
            "{lang}: held-out accuracy {} is under the floor {held_out}",
            intents["built_held_out_accuracy"]
        );
        assert!(
            language["reject"]["false_accepts"].as_u64().unwrap() <= *accepts,
            "{lang}: {} false accepts, over the ceiling {accepts}",
            language["reject"]["false_accepts"]
        );
    }
}
