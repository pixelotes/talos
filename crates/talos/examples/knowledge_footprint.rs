//! Footprint of the knowledge store (DEVELOPMENT.md, "The SQLite binding"):
//! builds the engine as `talos ask` does (user store, packs of the data
//! directory), answers "hola", then looks words up the way `define` and
//! `translate` will: senses, the gloss of the first, its words in the other
//! language, and the sense choice. Prints the latency of a whole lookup;
//! run it under `/usr/bin/time -l` for the resident set.
//!
//! ```text
//! TALOS_DATA_DIR=DIR cargo run --release -p talos --example knowledge_footprint -- WORDS [LANG]
//! ```
//!
//! `WORDS` is a file of words, one per line (the lemmas of a pack, say);
//! `LANG` is the language they are looked up in first (`en` by default).

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a measuring tool"
)]

use std::time::Instant;

use talos_core::knowledge::Cues;
use talos_core::{Config, ConvId, Lang, Timestamp, Turn};

fn main() {
    let mut args = std::env::args().skip(1);
    let words = std::fs::read_to_string(args.next().expect("a file of words")).unwrap();
    let lang: Lang = args.next().as_deref().unwrap_or("en").parse().unwrap();
    let mut engine = talos_skills::engine(Config::default()).unwrap();
    if let Some(warning) = talos::data::attach_user_store(&mut engine) {
        eprintln!("{warning}");
    }
    for warning in talos::data::attach_knowledge(&mut engine) {
        eprintln!("{warning}");
    }
    let reply = engine
        .respond(Turn::new("hola", ConvId::new("f"), Timestamp::from_unix(0)))
        .reply;
    println!("hola → {reply}");
    let Some(k) = engine.knowledge() else {
        println!("no knowledge store");
        return;
    };
    println!("stores: {:?}", k.stores());
    let mut times = Vec::new();
    let (mut found, mut glossed) = (0, 0);
    for word in words.lines().filter(|w| !w.trim().is_empty()) {
        let started = Instant::now();
        if let Some(lookup) = k.lookup(word, lang).unwrap() {
            found += 1;
            let first = &lookup.senses[0].concept;
            if k.gloss(first, lookup.lang).unwrap().is_some() {
                glossed += 1;
            }
            let _ = k.words(first, lookup.lang.other()).unwrap();
            let _ = k
                .choose_sense(&lookup.senses, &Cues::none(lookup.lang))
                .unwrap();
        }
        times.push(started.elapsed().as_micros());
    }
    times.sort_unstable();
    let at = |p: usize| times.get(times.len() * p / 100).copied().unwrap_or(0);
    println!(
        "{} lookups, {found} found, {glossed} glossed: p50 {} µs, p99 {} µs, max {} µs",
        times.len(),
        at(50),
        at(99),
        times.last().copied().unwrap_or(0)
    );
}
