//! `cargo run -p talos-pack`: builds Talos knowledge packs (docs/KNOWLEDGE.md,
//! "Pack sources"). Dev-time only; never linked into the `talos` binary.
//!
//! ```text
//! talos-pack                          build knowledge/seed.toml into target/packs/core.talospack
//! talos-pack build SOURCE OUT         build a pack source into OUT
//! talos-pack synthetic N OUT          a synthetic pack of N concepts, for measuring (DEVELOPMENT.md)
//! talos-pack wordnet DIR [OUT] [--unverified] [--wikidata FILE | --no-wikidata]
//!                                     the vocabulary pack from the wordnets in DIR (wordnet.rs)
//!                                     and the Wikidata snapshot (default
//!                                     knowledge/sources/wikidata-p8814-es.tsv), by default into
//!                                     target/packs/vocabulary.talospack
//! ```
//!
//! The same source always gives the same bytes (`build_file`, `wordnet::build`).

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool"
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rusqlite::{Connection, params};
use talos_core::knowledge::build::build_file;
use talos_core::knowledge::schema::{self, DbKind};

mod wikidata;
mod wordnet;

const USAGE: &str = "usage: talos-pack [build SOURCE OUT | synthetic N OUT | \
                     wordnet DIR [OUT] [--unverified] [--wikidata FILE | --no-wikidata]]";

/// `talos-pack wordnet`: builds the vocabulary pack, prints what it counted
/// and dropped, and writes that report beside the pack (`*.report.txt`),
/// with the notice (`*.NOTICE`).
fn vocabulary(
    dir: &Path,
    out: &Path,
    unverified: bool,
    wikidata: &wordnet::WikidataSource,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let report = wordnet::build(dir, out, unverified, wikidata)?;
    let text = report.text();
    print!("{text}");
    let path = out.with_extension("report.txt");
    std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    let size = std::fs::metadata(out).map_or(0, |m| m.len());
    println!(
        "{} ({size} bytes) in {:.1} s; notice in {}, report in {}",
        out.display(),
        started.elapsed().as_secs_f64(),
        out.with_extension("NOTICE").display(),
        path.display()
    );
    Ok(())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build(source: &Path, out: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    build_file(&text, out).map_err(|e| format!("{}: {e}", source.display()))?;
    let size = std::fs::metadata(out).map_or(0, |m| m.len());
    println!("{} ({size} bytes)", out.display());
    Ok(())
}

/// A deterministic word for `n` from a syllable table: distinct for every
/// `n`, pronounceable, no digits (so the inflection rules apply to it).
fn word(mut n: usize, syllables: &[&str]) -> String {
    let mut out = String::new();
    loop {
        out.push_str(syllables[n % syllables.len()]);
        n /= syllables.len();
        if n == 0 {
            break;
        }
    }
    out
}

/// A xorshift source, seeded, so the synthetic pack is always the same.
struct Draw(u64);

impl Draw {
    fn below(&mut self, m: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        let m = u64::try_from(m.max(1)).unwrap_or(u64::MAX);
        usize::try_from(self.0 % m).unwrap_or(0)
    }
}

const EN: &[&str] = &[
    "ba", "ke", "lo", "mi", "nu", "ra", "so", "ti", "vo", "zu", "dar", "pel",
];
const ES: &[&str] = &[
    "ca", "de", "fi", "go", "ja", "le", "mo", "ni", "pa", "re", "sol", "tur",
];
const GLOSS: &[&str] = &[
    "a", "the", "of", "small", "large", "thing", "made", "used", "for", "part", "kind", "place",
    "water", "animal", "tool", "person", "that", "which", "is", "with", "without", "long", "round",
    "body", "plant", "food", "sound", "light", "metal", "wood", "cloth", "house",
];

/// A synthetic pack shaped like the vocabulary pack of Slice C: `n`
/// concepts in an is-a tree, about 1.7 lexemes per concept in English and
/// 1.2 in Spanish with some polysemy, a gloss of 8–14 words per sense, and
/// an is-a and a part-of claim per concept. Only for measuring memory and
/// latency; its words mean nothing.
fn synthetic(n: usize, out: &Path) -> Result<(), String> {
    let sql = |e: rusqlite::Error| e.to_string();
    let mut conn = Connection::open_in_memory().map_err(sql)?;
    let tx = conn.transaction().map_err(sql)?;
    schema::migrate(&tx, "main", DbKind::Pack).map_err(|e| e.to_string())?;
    for (k, v) in [
        ("id", "synthetic"),
        ("version", "0"),
        ("title", "synthetic pack for measurements"),
        ("languages", "es en"),
        ("license", "MIT"),
        ("notice", "Generated by talos-pack; its words mean nothing."),
    ] {
        tx.execute("INSERT INTO meta (key, value) VALUES (?1, ?2)", [k, v])
            .map_err(sql)?;
    }
    tx.execute_batch(
        "INSERT INTO source (id, kind, name, license, added_at)
           VALUES (1, 'pack', 'synthetic', 'MIT', 0);
         INSERT INTO predicate (id, name, inverse, transitive)
           VALUES (1, 'is_a', 'has_kind', 1), (2, 'part_of', 'has_part', 1);",
    )
    .map_err(sql)?;
    {
        let mut concept = tx
            .prepare("INSERT INTO concept (id, kind) VALUES (?1, 'class')")
            .map_err(sql)?;
        let mut find = tx
            .prepare("SELECT id FROM lexeme WHERE lang = ?1 AND lemma = ?2")
            .map_err(sql)?;
        let mut lexeme = tx
            .prepare("INSERT INTO lexeme (id, lang, lemma, pos) VALUES (?1, ?2, ?3, 'n')")
            .map_err(sql)?;
        let mut form = tx
            .prepare("INSERT INTO form (lexeme_id, form, features) VALUES (?1, ?2, NULL)")
            .map_err(sql)?;
        let mut sense = tx
            .prepare(
                "INSERT INTO sense (lexeme_id, concept_id, gloss, rank, source_id) \
                 VALUES (?1, ?2, ?3, ?4, 1)",
            )
            .map_err(sql)?;
        let mut claim = tx
            .prepare(
                "INSERT INTO claim (subject, predicate, object, confidence, source_id, created_at) \
                 VALUES (?1, ?2, ?3, 1.0, 1, 0)",
            )
            .map_err(sql)?;
        let mut next_lexeme = 0_i64;
        let mut draw = Draw(0x2545_F491_4F6C_DD1D);
        for i in 0..n {
            let id = format!("syn:{i:08}");
            concept.execute([&id]).map_err(sql)?;
            let english = 1 + usize::from(draw.below(10) < 7);
            let spanish = 1 + usize::from(draw.below(10) < 2);
            for (lang, syllables, words) in [("en", EN, english), ("es", ES, spanish)] {
                for w in 0..words {
                    // About one word in five is an earlier concept's: polysemy.
                    let base = if i > 10 && draw.below(5) == 0 {
                        draw.below(i)
                    } else {
                        i
                    };
                    let lemma = word(base * 3 + w, syllables);
                    let found: Option<i64> =
                        find.query_row(params![lang, lemma], |r| r.get(0)).ok();
                    let lexeme_id = if let Some(id) = found {
                        id
                    } else {
                        next_lexeme += 1;
                        lexeme
                            .execute(params![next_lexeme, lang, lemma])
                            .map_err(sql)?;
                        form.execute(params![next_lexeme, lemma]).map_err(sql)?;
                        next_lexeme
                    };
                    let length = 8 + draw.below(7);
                    let gloss: Vec<&str> = (0..length)
                        .map(|_| GLOSS[draw.below(GLOSS.len())])
                        .collect();
                    let rank = i64::try_from(w).unwrap_or(0);
                    sense
                        .execute(params![lexeme_id, id, gloss.join(" "), rank])
                        .map_err(sql)?;
                }
            }
            if i > 0 {
                let parent = format!("syn:{:08}", (i - 1) / 7);
                claim.execute(params![id, 1, parent]).map_err(sql)?;
                let whole = format!("syn:{:08}", draw.below(i));
                claim.execute(params![id, 2, whole]).map_err(sql)?;
            }
        }
    }
    tx.commit().map_err(sql)?;
    conn.execute_batch("ANALYZE").map_err(sql)?;
    let _ = std::fs::remove_file(out);
    let path = out.to_str().ok_or("not a UTF-8 path")?;
    conn.execute("VACUUM INTO ?1", [path]).map_err(sql)?;
    let size = std::fs::metadata(out).map_or(0, |m| m.len());
    println!("{} ({n} concepts, {size} bytes)", out.display());
    Ok(())
}

/// `wordnet DIR [OUT] [--unverified] [--wikidata FILE | --no-wikidata]`.
fn wordnet_command(dir: &str, rest: &[&str]) -> Result<(), String> {
    let (mut unverified, mut off, mut snapshot) = (false, false, None);
    let mut outs = Vec::new();
    let mut args = rest.iter();
    while let Some(&arg) = args.next() {
        match arg {
            "--unverified" => unverified = true,
            "--no-wikidata" => off = true,
            "--wikidata" => {
                let file = args
                    .next()
                    .ok_or_else(|| format!("--wikidata needs a file\n{USAGE}"))?;
                snapshot = Some(PathBuf::from(file));
            }
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag}\n{USAGE}"));
            }
            out => outs.push(out),
        }
    }
    if off && snapshot.is_some() {
        return Err(format!(
            "--wikidata and --no-wikidata exclude each other\n{USAGE}"
        ));
    }
    let wikidata = if off {
        wordnet::WikidataSource::Off
    } else {
        wordnet::WikidataSource::Snapshot(
            snapshot.unwrap_or_else(|| root().join("knowledge/sources/wikidata-p8814-es.tsv")),
        )
    };
    let default = root().join("target/packs/vocabulary.talospack");
    match outs.as_slice() {
        [] => vocabulary(Path::new(dir), &default, unverified, &wikidata),
        [out] => vocabulary(Path::new(dir), Path::new(out), unverified, &wikidata),
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => build(
            &root().join("knowledge/seed.toml"),
            &root().join("target/packs/core.talospack"),
        ),
        ["build", source, out] => build(Path::new(source), Path::new(out)),
        ["synthetic", n, out] => match n.parse() {
            Ok(n) => synthetic(n, Path::new(out)),
            Err(_) => Err(format!("`{n}` is not a number\n{USAGE}")),
        },
        ["wordnet", dir, rest @ ..] => wordnet_command(dir, rest),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("talos-pack: {message}");
            ExitCode::FAILURE
        }
    }
}
