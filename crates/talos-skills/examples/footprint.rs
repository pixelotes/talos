//! Start-up, memory and per-turn cost of the engine with every built-in
//! skill (docs/DEVELOPMENT.md, "Footprint").
//!
//! ```text
//! cargo build --release -p talos-skills --example footprint
//! /usr/bin/time -l target/release/examples/footprint      # one turn of each kind
//! target/release/examples/footprint 5000                  # and p50/p99 over 5000 each
//! ```
//!
//! It builds the engine, then serves one rule-matched, one classifier and
//! one reflector turn in each language, printing the time of each and the
//! resident set size after it (read with `ps`, so Unix only); with a
//! number, it then times that many turns of each kind in the same process,
//! and the classifier stage alone (stage 4, on the match form) over the
//! same messages.

use std::process::Command;
use std::time::{Duration, Instant};

use talos_core::normalise::normalise;
use talos_core::{Config, ConvId, Engine, Lang, Timestamp, Turn};

/// The turns, by kind: (label, message).
const TURNS: &[(&str, &str)] = &[
    ("rule es", "hola"),
    ("rule en", "hello"),
    (
        "classifier es",
        "oye, me podrías decir qué hora tienes por ahí",
    ),
    (
        "classifier en",
        "would you mind telling me what the date is",
    ),
    ("reflector es", "creo que el cine es aburrido"),
    ("reflector en", "i think movies are boring"),
];

fn turn(engine: &Engine, label: &str, text: &str) -> Duration {
    let now = Timestamp::from_unix(1_764_000_000).with_utc_offset(3_600);
    let start = Instant::now();
    let outcome = engine.respond(Turn::new(text, ConvId::new(label), now));
    let elapsed = start.elapsed();
    std::hint::black_box(outcome);
    elapsed
}

/// This process's resident set size, as `ps` reports it.
fn rss() -> String {
    Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map_or_else(
            |_| "?".to_owned(),
            |out| format!("{} KB", String::from_utf8_lossy(&out.stdout).trim()),
        )
}

fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    println!("start: rss {}", rss());
    let start = Instant::now();
    let Ok(engine) = talos_skills::engine(Config::default()) else {
        eprintln!("the engine did not build");
        std::process::exit(1);
    };
    println!("construction: {:?}, rss {}", start.elapsed(), rss());
    for (label, text) in TURNS {
        let time = turn(&engine, label, text);
        println!("first {label}: {time:?}, rss {}", rss());
    }
    if iterations == 0 {
        return;
    }
    for (label, text) in TURNS {
        let mut times: Vec<Duration> = (0..iterations)
            .map(|_| turn(&engine, label, text))
            .collect();
        times.sort_unstable();
        let at = |q: usize| times[(times.len() - 1) * q / 100];
        println!("{label}: p50 {:?}, p99 {:?}", at(50), at(99));
    }
    // Stage 4 alone: every message above, each a fresh measurement.
    let mut stage: Vec<Duration> = Vec::with_capacity(iterations * TURNS.len());
    for (label, text) in TURNS {
        let lang = if label.ends_with("es") {
            Lang::Es
        } else {
            Lang::En
        };
        let folded = normalise(text).folded;
        for _ in 0..iterations {
            let start = Instant::now();
            let verdict = engine.classifiers().recognise(lang, &folded);
            stage.push(start.elapsed());
            std::hint::black_box(verdict);
        }
    }
    stage.sort_unstable();
    let at = |q: usize| stage[(stage.len() - 1) * q / 100];
    println!("classifier stage: p50 {:?}, p99 {:?}", at(50), at(99));
    println!("end: rss {}", rss());
}
