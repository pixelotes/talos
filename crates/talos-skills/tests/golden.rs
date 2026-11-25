//! Runs every golden transcript under `golden/` at the workspace root
//! against the built-in engine (format: docs/DEVELOPMENT.md).
//!
//! To accept new output deliberately: `TALOS_BLESS=1 cargo test -p
//! talos-skills --test golden`, then review the diff.

#![allow(
    clippy::unwrap_used,
    reason = "test harness; a failure is a test failure"
)]

use std::path::{Path, PathBuf};

use talos_core::golden;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden")
}

#[test]
fn golden_transcripts() {
    let bless = std::env::var_os("TALOS_BLESS").is_some();
    let summary = golden::run_dir(&dir(), &talos_skills::engine, bless).unwrap();
    for file in &summary.rewritten {
        eprintln!("rewrote {}", file.display());
    }
    assert!(
        summary.turns > 0,
        "no golden turns found in {}",
        dir().display()
    );
    assert!(summary.failures.is_empty(), "{}", summary.report());
}

#[test]
fn golden_runs_are_byte_identical() {
    // Determinism (DESIGN.md section 1): the whole suite twice, with fresh
    // engines, gives the same result. Not while regenerating, which
    // rewrites the files under it.
    if std::env::var_os("TALOS_BLESS").is_some() {
        return;
    }
    let first = golden::run_dir(&dir(), &talos_skills::engine, false).unwrap();
    let second = golden::run_dir(&dir(), &talos_skills::engine, false).unwrap();
    assert_eq!(first.turns, second.turns);
    assert_eq!(first.report(), second.report());
}
