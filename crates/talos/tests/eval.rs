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
