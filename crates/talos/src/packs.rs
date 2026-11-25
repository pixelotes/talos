//! `talos packs list | check <file> | info <file>` (docs/KNOWLEDGE.md
//! section 7): what packs the data directory holds, and what a pack file
//! is. Behind the `knowledge` feature.

use std::fmt::Write as _;
use std::path::Path;

use talos_core::knowledge::{self, Counts, Knowledge, PackInfo};

/// One line per attached pack, then one per refused file.
#[must_use]
pub fn list() -> (String, bool) {
    let Some(dir) = crate::data::data_dir() else {
        return ("no data directory, so no packs\n".to_owned(), true);
    };
    let packs = dir.join("packs");
    if knowledge::pack_files(&packs).is_empty() {
        return (format!("no packs in {}\n", packs.display()), true);
    }
    let store = match Knowledge::open_dir(&dir) {
        Ok(store) => store,
        Err(error) => return (format!("{error}\n"), false),
    };
    let mut out = String::new();
    for p in store.packs() {
        let path = p.path.as_deref().map(Path::display);
        let _ = writeln!(
            out,
            "{:<12} {:<8} {:<6} {:<10} {}",
            p.id,
            p.version,
            languages(p),
            p.license,
            path.map_or_default(|p| p.to_string())
        );
    }
    for r in store.refused() {
        let _ = writeln!(out, "refused      {}: {}", r.path.display(), r.reason);
    }
    (out, store.refused().is_empty())
}

fn languages(p: &PackInfo) -> String {
    p.languages
        .iter()
        .map(|l| l.code())
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe(out: &mut String, p: &PackInfo, c: &Counts) {
    let _ = writeln!(out, "{} {}: {}", p.id, p.version, p.title);
    let _ = writeln!(out, "schema      {}", p.schema_version);
    let _ = writeln!(out, "languages   {}", languages(p));
    let _ = writeln!(out, "licence     {}", p.license);
    for s in &p.sources {
        let _ = writeln!(
            out,
            "source      {} ({}{}{})",
            s.name,
            s.kind,
            s.license.as_deref().map_or_default(|l| format!(", {l}")),
            s.url.as_deref().map_or_default(|u| format!(", {u}"))
        );
    }
    let _ = writeln!(out, "concepts    {}", c.concepts);
    let _ = writeln!(out, "lexemes     {} es, {} en", c.lexemes_es, c.lexemes_en);
    let _ = writeln!(out, "forms       {}", c.forms);
    let _ = writeln!(out, "senses      {} ({} with a gloss)", c.senses, c.glosses);
    let _ = writeln!(out, "claims      {}", c.claims);
    let _ = writeln!(out, "phrases     {}", c.phrases);
    let _ = writeln!(out, "word pairs  {}", c.word_pairs);
    let _ = writeln!(
        out,
        "notice      {}",
        p.notice.replace('\n', "\n            ")
    );
}

/// `talos packs info <file>`.
#[must_use]
pub fn info(path: &Path) -> (String, bool) {
    match knowledge::pack::info(path) {
        Ok((p, c)) => {
            let mut out = String::new();
            describe(&mut out, &p, &c);
            (out, true)
        }
        Err(error) => (format!("{}: refused: {error}\n", path.display()), false),
    }
}

/// `talos packs check <file>`: the structural check, SQLite's integrity
/// check and the references the builder guarantees.
#[must_use]
pub fn check(path: &Path) -> (String, bool) {
    match knowledge::pack::check(path) {
        Ok(checked) => {
            let mut out = String::new();
            for problem in &checked.problems {
                let _ = writeln!(out, "error: {problem}");
            }
            if checked.outside_concepts > 0 {
                let _ = writeln!(
                    out,
                    "note: refers to {} concepts defined in other packs",
                    checked.outside_concepts
                );
            }
            let ok = checked.problems.is_empty();
            let _ = writeln!(
                out,
                "{}: {} {} {}",
                path.display(),
                checked.info.id,
                checked.info.version,
                if ok { "ok" } else { "has errors" }
            );
            (out, ok)
        }
        Err(error) => (format!("{}: refused: {error}\n", path.display()), false),
    }
}
