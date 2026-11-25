//! Properties the vocabulary pack must have (docs/DEVELOPMENT.md, "The
//! vocabulary pack"). Its sources and the built pack are not committed, so
//! these tests read the pack at `$TALOS_VOCABULARY_PACK`, or at
//! `target/packs/vocabulary.talospack` where `talos-pack wordnet` writes it,
//! and pass with a message saying they were skipped when it is not there.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    reason = "tests that say when they are skipped"
)]

use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use talos_core::Lang;
use talos_core::knowledge::{ConceptId, Knowledge, pack};

fn pack_path() -> Option<PathBuf> {
    let path = std::env::var_os("TALOS_VOCABULARY_PACK").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/packs/vocabulary.talospack")
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        eprintln!(
            "SKIPPED: no vocabulary pack at {}; build it with \
             `cargo run --release -p talos-pack -- wordnet DIR` (docs/DEVELOPMENT.md, \
             \"The vocabulary pack\") or set TALOS_VOCABULARY_PACK",
            path.display()
        );
        None
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn open(path: &PathBuf) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

#[test]
fn the_pack_passes_the_full_check_and_names_its_sources() {
    let Some(path) = pack_path() else { return };
    let checked = pack::check(&path).unwrap();
    assert!(checked.problems.is_empty(), "{:?}", checked.problems);
    assert_eq!(checked.outside_concepts, 0);
    let info = checked.info;
    assert_eq!(info.id, "vocabulary");
    assert_eq!(info.languages, [Lang::Es, Lang::En]);
    let names: Vec<&str> = info.sources.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["oewn", "mcr-spa"]);
    for needed in [
        "Open English WordNet",
        "CC BY 4.0",
        "Princeton WordNet 3.0",
        "Multilingual Central Repository",
        "CC BY 3.0",
        "no endorsement",
    ] {
        assert!(info.notice.contains(needed), "the notice lacks {needed}");
    }
    assert_eq!(info.license, "CC-BY-4.0 AND CC-BY-3.0 AND WordNet");
    let notice = path.with_extension("NOTICE");
    assert!(notice.is_file(), "no notice beside the pack");
}

#[test]
fn the_counts_are_in_the_expected_ranges() {
    let Some(path) = pack_path() else { return };
    let c = pack::info(&path).unwrap().1;
    // The sources hold 120,564 synsets, 161,875 English entries and 78,417
    // Spanish synsets with a lemma (wordnet-sources.md §2.3).
    assert!((115_000..=125_000).contains(&c.concepts), "{c:?}");
    assert!((150_000..=170_000).contains(&c.lexemes_en), "{c:?}");
    assert!((80_000..=100_000).contains(&c.lexemes_es), "{c:?}");
    assert!((330_000..=380_000).contains(&c.senses), "{c:?}");
    assert!((140_000..=170_000).contains(&c.claims), "{c:?}");
    assert!(c.forms >= c.lexemes_en + c.lexemes_es + 4_000, "{c:?}");
    let conn = open(&path);
    // The join keeps 99.7 % of the Spanish synsets.
    let with_spanish = count(
        &conn,
        "SELECT count(DISTINCT s.concept_id) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
         WHERE l.lang = 'es'",
    );
    assert!((77_000..=79_000).contains(&with_spanish), "{with_spanish}");
    let spanish_glosses = count(
        &conn,
        "SELECT count(DISTINCT s.concept_id) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
         WHERE l.lang = 'es' AND s.gloss IS NOT NULL",
    );
    assert!(
        (18_000..=19_500).contains(&spanish_glosses),
        "{spanish_glosses}"
    );
}

#[test]
fn every_row_holds_together() {
    let Some(path) = pack_path() else { return };
    let conn = open(&path);
    for (what, sql) in [
        (
            "senses of no concept",
            "SELECT count(*) FROM sense WHERE concept_id NOT IN (SELECT id FROM concept)",
        ),
        (
            "concepts with no English word",
            "SELECT count(*) FROM concept c WHERE NOT EXISTS (SELECT 1 FROM sense s \
             JOIN lexeme l ON l.id = s.lexeme_id WHERE s.concept_id = c.id AND l.lang = 'en')",
        ),
        (
            "lexemes with no sense",
            "SELECT count(*) FROM lexeme l WHERE NOT EXISTS \
             (SELECT 1 FROM sense s WHERE s.lexeme_id = l.id)",
        ),
        (
            "empty lemmas or forms",
            "SELECT (SELECT count(*) FROM lexeme WHERE trim(lemma) = '') \
             + (SELECT count(*) FROM form WHERE trim(form) = '')",
        ),
        (
            "lexemes without their lemma's form row",
            "SELECT count(*) FROM lexeme l WHERE NOT EXISTS \
             (SELECT 1 FROM form f WHERE f.lexeme_id = l.id AND f.features IS NULL)",
        ),
        (
            "exception forms with no features",
            "SELECT count(*) FROM (SELECT lexeme_id FROM form WHERE features IS NULL \
             GROUP BY lexeme_id HAVING count(*) > 1)",
        ),
        (
            "parts of speech outside n v adj adv",
            "SELECT count(*) FROM lexeme WHERE pos NOT IN ('n', 'v', 'adj', 'adv')",
        ),
        (
            "English senses not from oewn",
            "SELECT count(*) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
             JOIN source x ON x.id = s.source_id WHERE l.lang = 'en' AND x.name <> 'oewn'",
        ),
        (
            "Spanish senses not from mcr-spa",
            "SELECT count(*) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
             JOIN source x ON x.id = s.source_id WHERE l.lang = 'es' AND x.name <> 'mcr-spa'",
        ),
        (
            "English senses without a gloss",
            "SELECT count(*) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
             WHERE l.lang = 'en' AND (s.gloss IS NULL OR trim(s.gloss) = '')",
        ),
        (
            "negative ranks",
            "SELECT count(*) FROM sense WHERE rank < 0",
        ),
        (
            "claims to unknown concepts",
            "SELECT count(*) FROM claim WHERE subject NOT IN (SELECT id FROM concept) \
             OR object NOT IN (SELECT id FROM concept)",
        ),
        (
            "mojibake (UTF-8 read as Latin-1)",
            "SELECT count(*) FROM lexeme WHERE lemma LIKE '%Ã%' OR lemma LIKE '%Â%'",
        ),
        (
            "glosses with mojibake",
            "SELECT count(*) FROM sense WHERE gloss LIKE '%Ã%' OR gloss LIKE '%Â%'",
        ),
    ] {
        assert_eq!(count(&conn, sql), 0, "{what}");
    }
}

#[test]
fn everyday_words_resolve_through_the_query_layer() {
    let Some(path) = pack_path() else { return };
    let mut k = Knowledge::empty().unwrap();
    assert!(k.attach(&path), "{:?}", k.refused());
    let first = |word: &str, lang: Lang| {
        let senses = k.senses_in(word, lang).unwrap();
        let s = senses.first().unwrap_or_else(|| panic!("{word} not found"));
        (
            s.concept.0.clone(),
            s.word.lemma.clone(),
            s.features.clone(),
        )
    };
    // The domestic dog, CILI i46360, from either language.
    assert_eq!(first("dog", Lang::En).0, "i46360");
    assert_eq!(first("perro", Lang::Es).0, "i46360");
    assert_eq!(first("perros", Lang::Es).0, "i46360");
    // Exceptions of the English wordnet, and the rules.
    assert_eq!(
        first("geese", Lang::En),
        (
            "i45102".to_owned(),
            "goose".to_owned(),
            Some("pl".to_owned())
        )
    );
    assert_eq!(first("went", Lang::En).1, "go");
    assert_eq!(first("guitarras", Lang::Es).1, "guitarra");
    // A Spanish gloss names its source; a concept with none has none.
    let cat = ConceptId::new("i46593");
    let gloss = k.gloss(&cat, Lang::Es).unwrap().unwrap();
    assert_eq!(gloss.source.name, "mcr-spa");
    assert_eq!(
        k.gloss(&cat, Lang::En).unwrap().unwrap().source.name,
        "oewn"
    );
    assert!(
        k.gloss(&ConceptId::new("i46360"), Lang::Es)
            .unwrap()
            .is_none()
    );
    // Translation both ways, and the plural kept.
    let perro = &k.words(&ConceptId::new("i46360"), Lang::Es).unwrap()[0];
    assert_eq!(perro.lemma, "perro");
    assert_eq!(k.form_of(perro, "pl").unwrap().as_deref(), Some("perros"));
    let goose = &k.words(&ConceptId::new("i45102"), Lang::En).unwrap()[0];
    assert_eq!(k.form_of(goose, "pl").unwrap().as_deref(), Some("geese"));
    // Relations are there for v0.6: a dog is a kind of canine and of
    // domestic animal.
    assert_eq!(
        k.objects(&ConceptId::new("i46360"), "is_a").unwrap().len(),
        2
    );
}
