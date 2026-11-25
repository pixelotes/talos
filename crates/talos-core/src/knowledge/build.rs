//! Building a pack from its hand-written source (docs/KNOWLEDGE.md, "Pack
//! sources"; the seed is `knowledge/seed.toml`).
//!
//! The pack builder (`tools/pack`) writes the result to a file; tests build
//! it into an in-memory database attached to a store
//! ([`super::Knowledge::with_source`]). Both run the same code, so a test
//! answers from exactly the pack the builder ships. The build checks what
//! SQLite cannot across packs: every reference resolves, every gloss has a
//! word to sit on, every claim names a known predicate.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use rusqlite::{Connection, params};
use serde::Deserialize;

use super::inflect::key;
use super::schema::{self, DbKind, sql_error};
use crate::civil::Date;
use crate::{Error, Lang};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    pack: RawPack,
    #[serde(default)]
    source: Vec<RawSourceRow>,
    #[serde(default)]
    predicate: Vec<RawPredicate>,
    #[serde(default)]
    concept: Vec<RawConcept>,
    relations: Option<RawRelations>,
    #[serde(default)]
    phrase: Vec<RawPhrase>,
    lexicon: Option<RawLexicon>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPack {
    id: String,
    version: String,
    title: String,
    languages: Vec<String>,
    license: String,
    notice: String,
    date: String,
    source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRow {
    name: String,
    kind: String,
    license: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPredicate {
    name: String,
    inverse: Option<String>,
    #[serde(default)]
    transitive: bool,
    #[serde(default)]
    symmetric: bool,
    #[serde(default)]
    functional: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConcept {
    id: String,
    kind: Option<String>,
    #[serde(default)]
    es: Vec<RawWord>,
    #[serde(default)]
    en: Vec<RawWord>,
    #[serde(default)]
    gloss: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawWord {
    Lemma(String),
    Full(RawFullWord),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFullWord {
    lemma: String,
    #[serde(default)]
    rank: i64,
    pos: Option<String>,
    #[serde(default)]
    forms: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRelations {
    claims: Vec<RawClaim>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClaim {
    id: i64,
    s: String,
    p: String,
    o: Option<String>,
    literal: Option<String>,
    #[serde(default)]
    negated: bool,
    confidence: Option<f64>,
    valid_from: Option<String>,
    valid_to: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPhrase {
    id: String,
    #[serde(default)]
    approximate: bool,
    es: Vec<RawPhraseText>,
    en: Vec<RawPhraseText>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPhraseText {
    Text(String),
    Noted(RawNotedText),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNotedText {
    text: String,
    #[serde(default)]
    note: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLexicon {
    source: String,
    pairs: Vec<RawPair>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    from: String,
    word: String,
    to: Vec<String>,
}

/// The parts of speech a source may name.
const POS: &[&str] = &["n", "v", "adj", "adv"];

fn invalid(message: impl Into<String>) -> Error {
    Error::Knowledge(message.into())
}

fn lang(code: &str) -> Result<Lang, Error> {
    code.parse()
        .map_err(|_| invalid(format!("unknown language `{code}`")))
}

/// `YYYY-MM-DD` as Unix seconds at midnight UTC.
fn date_seconds(text: &str) -> Result<i64, Error> {
    let bad = || invalid(format!("`{text}` is not a date (YYYY-MM-DD)"));
    let mut parts = text.splitn(3, '-');
    let mut next = || parts.next().ok_or_else(bad);
    let (y, m, d) = (next()?, next()?, next()?);
    let date = Date::new(
        y.parse().map_err(|_| bad())?,
        m.parse().map_err(|_| bad())?,
        d.parse().map_err(|_| bad())?,
    )
    .ok_or_else(bad)?;
    Ok(date.days_since_epoch() * 86_400)
}

/// The words of one language of a concept, with their defaults applied.
fn words(raw: &[RawWord]) -> Vec<(String, i64, String, &BTreeMap<String, String>)> {
    static NONE: BTreeMap<String, String> = BTreeMap::new();
    raw.iter()
        .map(|w| match w {
            RawWord::Lemma(lemma) => (lemma.clone(), 0, "n".to_owned(), &NONE),
            RawWord::Full(full) => (
                full.lemma.clone(),
                full.rank,
                full.pos.clone().unwrap_or_else(|| "n".to_owned()),
                &full.forms,
            ),
        })
        .collect()
}

/// Writes a pack source into the empty database `schema` of `conn`: the
/// tables, the metadata and every row, in the source's order, in one
/// transaction.
///
/// # Errors
///
/// [`Error::Knowledge`] when the source is not valid TOML of this format or
/// does not hold together (an unknown concept, predicate, source or
/// language; a gloss with no word; a duplicate), or a statement fails.
#[allow(clippy::too_many_lines)]
pub fn build_into(conn: &mut Connection, schema: &str, source: &str) -> Result<(), Error> {
    let raw: RawSource = toml::from_str(source).map_err(|e| invalid(e.to_string()))?;
    let tx = conn.transaction().map_err(|e| sql_error(&e))?;
    schema::migrate(&tx, schema, DbKind::Pack)?;
    let s = schema;
    let pack = &raw.pack;
    let date = date_seconds(&pack.date)?;
    let mut languages = Vec::new();
    for code in &pack.languages {
        languages.push(lang(code)?.code());
    }
    let meta = [
        ("id", pack.id.as_str()),
        ("version", pack.version.as_str()),
        ("title", pack.title.as_str()),
        ("languages", &languages.join(" ")),
        ("license", pack.license.as_str()),
        ("notice", pack.notice.as_str()),
    ];
    for (k, v) in meta {
        if v.trim().is_empty() {
            return Err(invalid(format!("[pack] `{k}` is empty")));
        }
        tx.execute(
            &format!("INSERT INTO {s}.meta (key, value) VALUES (?1, ?2)"),
            [k, v],
        )
        .map_err(|e| sql_error(&e))?;
    }

    let mut sources: HashMap<&str, i64> = HashMap::new();
    for (i, row) in raw.source.iter().enumerate() {
        let id = i64::try_from(i + 1).map_err(|_| invalid("too many sources"))?;
        if sources.insert(&row.name, id).is_some() {
            return Err(invalid(format!("source `{}` twice", row.name)));
        }
        tx.execute(
            &format!(
                "INSERT INTO {s}.source (id, kind, name, license, url, added_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ),
            params![id, row.kind, row.name, row.license, row.url, date],
        )
        .map_err(|e| sql_error(&e))?;
    }
    let source_id = |name: &str| {
        sources
            .get(name)
            .copied()
            .ok_or_else(|| invalid(format!("unknown source `{name}`")))
    };
    let main_source = source_id(&pack.source)?;

    let mut predicates: HashMap<&str, i64> = HashMap::new();
    for (i, p) in raw.predicate.iter().enumerate() {
        let id = i64::try_from(i + 1).map_err(|_| invalid("too many predicates"))?;
        if predicates.insert(&p.name, id).is_some() {
            return Err(invalid(format!("predicate `{}` twice", p.name)));
        }
        tx.execute(
            &format!(
                "INSERT INTO {s}.predicate (id, name, inverse, transitive, symmetric, functional) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ),
            params![
                id,
                p.name,
                p.inverse,
                p.transitive,
                p.symmetric,
                p.functional
            ],
        )
        .map_err(|e| sql_error(&e))?;
    }

    let mut concepts: BTreeSet<&str> = BTreeSet::new();
    let mut lexemes: HashMap<(Lang, String, String), i64> = HashMap::new();
    let mut senses: BTreeSet<(i64, &str)> = BTreeSet::new();
    for c in &raw.concept {
        if c.id.trim().is_empty() || !concepts.insert(&c.id) {
            return Err(invalid(format!("concept `{}` empty or twice", c.id)));
        }
        tx.execute(
            &format!("INSERT INTO {s}.concept (id, kind) VALUES (?1, ?2)"),
            params![c.id, c.kind.as_deref().unwrap_or("class")],
        )
        .map_err(|e| sql_error(&e))?;
        for code in c.gloss.keys() {
            let l = lang(code)?;
            let has_words = match l {
                Lang::Es => !c.es.is_empty(),
                Lang::En => !c.en.is_empty(),
            };
            if !has_words {
                return Err(invalid(format!(
                    "concept `{}` has a {code} gloss but no {code} word",
                    c.id
                )));
            }
        }
        if c.es.is_empty() && c.en.is_empty() {
            return Err(invalid(format!("concept `{}` has no word", c.id)));
        }
        for (l, raw_words) in [(Lang::Es, &c.es), (Lang::En, &c.en)] {
            let gloss = c.gloss.get(l.code());
            for (lemma, rank, pos, forms) in words(raw_words) {
                if lemma.trim().is_empty() || key(&lemma).is_empty() {
                    return Err(invalid(format!("an empty word in `{}`", c.id)));
                }
                if !POS.contains(&pos.as_str()) {
                    return Err(invalid(format!("unknown part of speech `{pos}`")));
                }
                let next =
                    i64::try_from(lexemes.len() + 1).map_err(|_| invalid("too many words"))?;
                let known = lexemes.get(&(l, lemma.clone(), pos.clone())).copied();
                let lexeme = if let Some(id) = known {
                    if !forms.is_empty() {
                        return Err(invalid(format!(
                            "the forms of `{lemma}` go on its first use only"
                        )));
                    }
                    id
                } else {
                    {
                        lexemes.insert((l, lemma.clone(), pos.clone()), next);
                        tx.execute(
                            &format!(
                                "INSERT INTO {s}.lexeme (id, lang, lemma, pos) VALUES (?1, ?2, ?3, ?4)"
                            ),
                            params![next, l.code(), lemma, pos],
                        )
                        .map_err(|e| sql_error(&e))?;
                        tx.execute(
                            &format!(
                                "INSERT INTO {s}.form (lexeme_id, form, features) VALUES (?1, ?2, NULL)"
                            ),
                            params![next, key(&lemma)],
                        )
                        .map_err(|e| sql_error(&e))?;
                        for (form, features) in forms {
                            if features.trim().is_empty() || key(form).is_empty() {
                                return Err(invalid(format!("a bad form of `{lemma}`")));
                            }
                            tx.execute(
                                &format!(
                                    "INSERT INTO {s}.form (lexeme_id, form, features) VALUES (?1, ?2, ?3)"
                                ),
                                params![next, key(form), features],
                            )
                            .map_err(|e| sql_error(&e))?;
                        }
                        next
                    }
                };
                if !senses.insert((lexeme, &c.id)) {
                    return Err(invalid(format!("`{lemma}` twice in `{}`", c.id)));
                }
                tx.execute(
                    &format!(
                        "INSERT INTO {s}.sense (lexeme_id, concept_id, gloss, rank, source_id) \
                         VALUES (?1, ?2, ?3, ?4, ?5)"
                    ),
                    params![lexeme, c.id, gloss, rank, main_source],
                )
                .map_err(|e| sql_error(&e))?;
            }
        }
    }

    let mut claim_ids = BTreeSet::new();
    for c in raw.relations.iter().flat_map(|r| &r.claims) {
        if !claim_ids.insert(c.id) {
            return Err(invalid(format!("claim {} twice", c.id)));
        }
        let predicate = predicates
            .get(c.p.as_str())
            .ok_or_else(|| invalid(format!("claim {}: unknown predicate `{}`", c.id, c.p)))?;
        for concept in std::iter::once(&c.s).chain(&c.o) {
            if !concepts.contains(concept.as_str()) {
                return Err(invalid(format!(
                    "claim {}: unknown concept `{concept}`",
                    c.id
                )));
            }
        }
        if c.o.is_some() == c.literal.is_some() {
            return Err(invalid(format!(
                "claim {}: needs an object or a literal, not both",
                c.id
            )));
        }
        let confidence = c.confidence.unwrap_or(1.0);
        if !(0.0..=1.0).contains(&confidence) {
            return Err(invalid(format!("claim {}: confidence out of 0–1", c.id)));
        }
        let valid_from = c.valid_from.as_deref().map(date_seconds).transpose()?;
        let valid_to = c.valid_to.as_deref().map(date_seconds).transpose()?;
        tx.execute(
            &format!(
                "INSERT INTO {s}.claim (id, subject, predicate, object, literal, negated, \
                 confidence, source_id, valid_from, valid_to, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                c.id,
                c.s,
                predicate,
                c.o,
                c.literal,
                c.negated,
                confidence,
                main_source,
                valid_from,
                valid_to,
                date
            ],
        )
        .map_err(|e| sql_error(&e))?;
    }

    let mut entries = BTreeSet::new();
    for p in &raw.phrase {
        if !entries.insert(&p.id) || p.es.is_empty() || p.en.is_empty() {
            return Err(invalid(format!(
                "phrase `{}` twice or missing a side",
                p.id
            )));
        }
        for (l, texts) in [(Lang::Es, &p.es), (Lang::En, &p.en)] {
            for (rank, text) in texts.iter().enumerate() {
                let (text, notes) = match text {
                    RawPhraseText::Text(t) => (t, None),
                    RawPhraseText::Noted(n) => (&n.text, Some(&n.note)),
                };
                if key(text).is_empty() {
                    return Err(invalid(format!("phrase `{}`: an empty text", p.id)));
                }
                tx.execute(
                    &format!(
                        "INSERT INTO {s}.phrase (entry, lang, text, form, rank, approximate, source_id) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                    ),
                    params![
                        p.id,
                        l.code(),
                        text,
                        key(text),
                        i64::try_from(rank).unwrap_or(i64::MAX),
                        p.approximate,
                        main_source
                    ],
                )
                .map_err(|e| sql_error(&e))?;
                let row = tx.last_insert_rowid();
                for (code, note) in notes.into_iter().flatten() {
                    tx.execute(
                        &format!(
                            "INSERT INTO {s}.phrase_note (phrase_id, lang, note) VALUES (?1, ?2, ?3)"
                        ),
                        params![row, lang(code)?.code(), note],
                    )
                    .map_err(|e| sql_error(&e))?;
                }
            }
        }
    }

    if let Some(lexicon) = &raw.lexicon {
        let source = source_id(&lexicon.source)?;
        let mut seen = BTreeSet::new();
        for pair in &lexicon.pairs {
            let from = lang(&pair.from)?;
            if !seen.insert((from, key(&pair.word))) || pair.to.is_empty() {
                return Err(invalid(format!(
                    "lexicon: `{}` twice or with no translation",
                    pair.word
                )));
            }
            for (rank, target) in pair.to.iter().enumerate() {
                tx.execute(
                    &format!(
                        "INSERT INTO {s}.word_pair (lang, word, form, target, rank, source_id) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                    ),
                    params![
                        from.code(),
                        pair.word,
                        key(&pair.word),
                        target,
                        i64::try_from(rank).unwrap_or(i64::MAX),
                        source
                    ],
                )
                .map_err(|e| sql_error(&e))?;
            }
        }
    }
    tx.commit().map_err(|e| sql_error(&e))
}

/// Builds a pack file from its source: in memory first, then written whole
/// with `VACUUM INTO`, so the same source always gives the same bytes (with
/// the same SQLite, which `Cargo.lock` fixes). An existing file at `out` is
/// replaced.
///
/// # Errors
///
/// [`Error::Knowledge`] as for [`build_into`], or when the file cannot be
/// written.
pub fn build_file(source: &str, out: &Path) -> Result<(), Error> {
    let mut conn = Connection::open_in_memory().map_err(|e| sql_error(&e))?;
    build_into(&mut conn, "main", source)?;
    conn.execute_batch("ANALYZE").map_err(|e| sql_error(&e))?;
    match std::fs::remove_file(out) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(invalid(format!("{}: {e}", out.display()))),
    }
    let path = out
        .to_str()
        .ok_or_else(|| invalid(format!("{}: not a UTF-8 path", out.display())))?;
    conn.execute("VACUUM INTO ?1", [path])
        .map_err(|e| invalid(format!("{}: {e}", out.display())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const SEED: &str = include_str!("../../../../knowledge/seed.toml");

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn the_seed_builds_with_everything_the_fixture_has() {
        let mut conn = Connection::open_in_memory().unwrap();
        build_into(&mut conn, "main", SEED).unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM concept"), 63);
        assert_eq!(count(&conn, "SELECT count(*) FROM claim"), 55);
        assert_eq!(count(&conn, "SELECT count(*) FROM predicate"), 3);
        assert_eq!(count(&conn, "SELECT count(*) FROM source"), 2);
        assert_eq!(count(&conn, "SELECT count(DISTINCT entry) FROM phrase"), 10);
        assert_eq!(
            count(&conn, "SELECT count(DISTINCT form || lang) FROM word_pair"),
            8
        );
        // "banco" is one lexeme with three senses.
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 WHERE l.lemma = 'banco'"
            ),
            3
        );
        // The two glosses the fixture leaves out.
        assert_eq!(
            count(&conn, "SELECT count(*) FROM sense WHERE gloss IS NULL"),
            2
        );
        assert_eq!(
            count(&conn, "SELECT count(*) FROM claim WHERE negated = 1"),
            3
        );
    }

    #[test]
    fn a_source_that_does_not_hold_together_is_refused() {
        let base = "[pack]\nid='x'\nversion='1'\ntitle='t'\nlanguages=['es']\n\
                    license='MIT'\nnotice='n'\ndate='2026-01-01'\nsource='s'\n\
                    [[source]]\nname='s'\nkind='pack'\n";
        let cases = [
            (
                "[[concept]]\nid='a'\nes=['x']\ngloss.en='no english word'\n",
                "gloss",
            ),
            ("[[concept]]\nid='a'\n", "no word"),
            (
                "[relations]\nclaims=[{id=1,s='a',p='is_a',o='b'}]\n",
                "predicate",
            ),
            (
                "[[predicate]]\nname='is_a'\n[[concept]]\nid='a'\nes=['x']\n\
                 [relations]\nclaims=[{id=1,s='a',p='is_a',o='b'}]\n",
                "unknown concept",
            ),
            (
                "[[concept]]\nid='a'\nes=[{lemma='x',pos='noun'}]\n",
                "part of speech",
            ),
            ("colour='red'\n", "unknown field"),
        ];
        for (extra, expected) in cases {
            let mut conn = Connection::open_in_memory().unwrap();
            let error = build_into(&mut conn, "main", &format!("{base}{extra}")).unwrap_err();
            assert!(error.to_string().contains(expected), "{extra}: {error}");
        }
    }

    #[test]
    fn the_same_source_builds_the_same_bytes() {
        let dir = std::env::temp_dir().join(format!("talos-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.talospack"), dir.join("b.talospack"));
        build_file(SEED, &a).unwrap();
        build_file(SEED, &b).unwrap();
        build_file(SEED, &b).unwrap();
        let (a_bytes, b_bytes) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        assert!(a_bytes.len() > 4096);
        assert!(a_bytes == b_bytes, "two builds differ");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
