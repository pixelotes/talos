//! Everyday words looked up in the vocabulary pack through the query layer,
//! printed for a person to read (docs/DEVELOPMENT.md, "The vocabulary
//! pack"): each word's senses in rank order with their gloss, the words of
//! its first sense in the other language, and what the sense choice does
//! with no context. A reading aid, not a measurement.
//!
//! ```text
//! cargo run --release -p talos-pack --example vocabulary_sample [PACK] [WORD…]
//! ```
//!
//! With no words it reads the built-in lists of about forty everyday words
//! per language.

#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a reading aid"
)]

use std::path::PathBuf;

use talos_core::Lang;
use talos_core::knowledge::{Cues, Knowledge};

const ENGLISH: &[&str] = &[
    "house", "dog", "cat", "water", "bank", "book", "run", "eat", "good", "bad", "big", "small",
    "car", "tree", "head", "hand", "time", "day", "night", "man", "woman", "child", "friend",
    "city", "money", "work", "play", "go", "see", "love", "happy", "red", "fast", "slowly",
    "computer", "phone", "bread", "school", "river", "light", "geese", "went", "better",
    "children", "mice",
];

const SPANISH: &[&str] = &[
    "casa",
    "perro",
    "gato",
    "agua",
    "banco",
    "libro",
    "correr",
    "comer",
    "bueno",
    "malo",
    "grande",
    "pequeño",
    "coche",
    "árbol",
    "cabeza",
    "mano",
    "tiempo",
    "día",
    "noche",
    "hombre",
    "mujer",
    "niño",
    "amigo",
    "ciudad",
    "dinero",
    "trabajo",
    "jugar",
    "ir",
    "ver",
    "amor",
    "feliz",
    "rojo",
    "rápido",
    "lentamente",
    "ordenador",
    "teléfono",
    "pan",
    "escuela",
    "río",
    "luz",
    "guitarras",
    "perros",
    "comió",
];

fn short(text: &str, n: usize) -> String {
    if text.chars().count() <= n {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(n).collect::<String>())
    }
}

fn show(k: &Knowledge, word: &str, lang: Lang) {
    let Some(lookup) = k.lookup(word, lang).unwrap() else {
        println!("{word} [{}]: NOT FOUND", lang.code());
        return;
    };
    let choice = k
        .choose_sense(&lookup.senses, &Cues::none(lookup.lang))
        .unwrap();
    let chosen = choice
        .chosen
        .map_or_else(|| "asks".to_owned(), |(i, _)| format!("sense {}", i + 1));
    println!(
        "{word} [{}{}]: {} senses; no context → {chosen}",
        lookup.lang.code(),
        if lookup.lang == lang {
            ""
        } else {
            ", other language"
        },
        lookup.senses.len(),
    );
    for (i, sense) in lookup.senses.iter().take(4).enumerate() {
        let gloss = k.gloss(&sense.concept, lookup.lang).unwrap();
        let other: Vec<String> = k
            .words(&sense.concept, lookup.lang.other())
            .unwrap()
            .into_iter()
            .take(3)
            .map(|w| w.lemma)
            .collect();
        println!(
            "  {}. {} {} rank {}{} | {} | {}: {}",
            i + 1,
            sense.concept,
            sense.word.lemma,
            sense.rank,
            sense
                .features
                .as_deref()
                .map_or_else(String::new, |f| format!(" ({f})")),
            gloss.map_or_else(
                || "(no gloss)".to_owned(),
                |g| format!("{} [{}]", short(&g.text, 70), g.source.name)
            ),
            lookup.lang.other().code(),
            other.join(", ")
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let pack = args.next().map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/packs/vocabulary.talospack")
        },
        PathBuf::from,
    );
    let mut k = Knowledge::empty().unwrap();
    assert!(k.attach(&pack), "{:?}", k.refused());
    let words: Vec<String> = args.collect();
    if words.is_empty() {
        for w in ENGLISH {
            show(&k, w, Lang::En);
        }
        for w in SPANISH {
            show(&k, w, Lang::Es);
        }
    } else {
        for w in &words {
            show(&k, w, Lang::Es);
        }
    }
}
