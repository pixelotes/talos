//! The dictionary queries (docs/spec/knowledge.md 2.1): a word to its
//! concepts, a concept's gloss and words in a language, a word's inflected
//! form, stock phrases and the word-pair lexicon. Typed results; no SQL
//! leaves this module.
//!
//! Each query runs once per attached database (`user.db` first, then the
//! packs in order) with a statement generated for that database and kept
//! prepared, and the results are combined here (pack-attachment.md section
//! 9, rule 1). A word found in several databases is one word: lexemes are
//! matched by language, lemma and part of speech, not by row id.

use std::collections::BTreeSet;
use std::fmt;

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::inflect::{self, Class, analyses, key};
use super::pack::Source;
use super::schema::sql_error;
use super::store::{Db, Knowledge};
use crate::{Error, Lang};

/// A concept's identifier: a global text key (`core:guitar`,
/// `wn:03467517-n`, `user:01J…`), so packs compose without renumbering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConceptId(pub String);

impl ConceptId {
    /// A concept id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The text key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A word of one language (a lexeme): its lemma as written ("violín",
/// "Plutón") and its part of speech (`n`, `v`, `adj`, `adv`; none for a
/// word the user taught).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Word {
    /// Its language.
    pub lang: Lang,
    /// The lemma, in its own spelling.
    pub lemma: String,
    /// The part of speech.
    pub pos: Option<String>,
}

/// One meaning of a typed word: the concept, the lexeme it was reached
/// through, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sense {
    /// The meaning.
    pub concept: ConceptId,
    /// The lexeme the typed word resolved to ("guitarras" → *guitarra*).
    pub word: Word,
    /// The features of the typed form (`pl`, `pres,3sg`…); none when it was
    /// the lemma itself.
    pub features: Option<String>,
    /// The baseline order of the word's senses (KNOWLEDGE.md section 5): 0
    /// first; equal ranks at the top are a tie.
    pub rank: i64,
    /// Where the sense comes from.
    pub source: Source,
}

/// What a typed word means: its senses, best rank first, and the language
/// it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    /// The language the word was found in: the one asked first, or the
    /// other when only the other has it (2.1, step 3).
    pub lang: Lang,
    /// One per concept, ordered by rank, then by store and order of
    /// writing. Never empty.
    pub senses: Vec<Sense>,
}

/// A definition: a gloss in one language and where it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gloss {
    /// The text, as written.
    pub text: String,
    /// Its language.
    pub lang: Lang,
    /// Its source (hand-written, translated, taught…).
    pub source: Source,
}

/// One side's wording of a stock phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseText {
    /// As written ("¡salud!").
    pub text: String,
    /// Its language.
    pub lang: Lang,
    /// When this wording is the one to use, in each language it is written
    /// in ("al despedirse" / "when leaving").
    pub notes: Vec<(Lang, String)>,
}

impl PhraseText {
    /// The note in a language.
    #[must_use]
    pub fn note(&self, lang: Lang) -> Option<&str> {
        self.notes
            .iter()
            .find(|(l, _)| *l == lang)
            .map(|(_, n)| n.as_str())
    }
}

/// A stock phrase (docs/spec/knowledge.md 1.6) that a text matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockPhrase {
    /// The entry's id, the same for both sides.
    pub entry: String,
    /// The wording the text matched.
    pub matched: PhraseText,
    /// The Spanish wordings, best first.
    pub es: Vec<PhraseText>,
    /// The English wordings, best first.
    pub en: Vec<PhraseText>,
    /// Whether the two sides are only roughly equivalent.
    pub approximate: bool,
    /// Where it comes from.
    pub source: Source,
}

impl StockPhrase {
    /// The wordings in a language, best first.
    #[must_use]
    pub fn side(&self, lang: Lang) -> &[PhraseText] {
        match lang {
            Lang::Es => &self.es,
            Lang::En => &self.en,
        }
    }
}

/// An entry of the word-pair lexicon (KNOWLEDGE.md section 7): a word's
/// candidate translations, best first. No concept, no sense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordPairs {
    /// The word, as the lexicon writes it.
    pub word: String,
    /// Its language; the translations are in the other.
    pub lang: Lang,
    /// Candidates, best first.
    pub translations: Vec<String>,
    /// The lexicon's source.
    pub source: Source,
}

fn source_at(row: &Row<'_>, first: usize, store: &str) -> rusqlite::Result<Source> {
    Ok(Source {
        name: row.get(first)?,
        kind: row.get(first + 1)?,
        license: row.get(first + 2)?,
        url: row.get(first + 3)?,
        store: store.to_owned(),
    })
}

/// The columns [`source_at`] reads, for a `source` aliased `src`.
const SOURCE_COLUMNS: &str = "src.name, src.kind, src.license, src.url";

fn parse_lang(code: &str) -> Option<Lang> {
    code.parse().ok()
}

/// A lexeme found for a typed form, before its senses are read.
struct Found {
    db: usize,
    id: i64,
    word: Word,
    features: Option<String>,
}

impl Knowledge {
    /// The lexemes whose stored form is `form`, with the form's features
    /// (`None` for the lemma itself), in one database.
    fn stored_forms(
        conn: &Connection,
        db: &Db,
        form: &str,
        lang: Lang,
    ) -> rusqlite::Result<Vec<(i64, Word, Option<String>)>> {
        let mut statement = conn.prepare_cached(&format!(
            "SELECT l.id, l.lemma, l.pos, f.features FROM {s}.form f \
             JOIN {s}.lexeme l ON l.id = f.lexeme_id \
             WHERE f.form = ?1 AND l.lang = ?2 \
             ORDER BY f.features IS NOT NULL, l.id",
            s = db.schema
        ))?;
        let rows = statement.query_map(params![form, lang.code()], |row| {
            Ok((
                row.get(0)?,
                Word {
                    lang,
                    lemma: row.get(1)?,
                    pos: row.get(2)?,
                },
                row.get(3)?,
            ))
        })?;
        rows.collect()
    }

    /// The lexemes of one language a typed word may be: its stored forms
    /// first (the lemma, then the exceptions), then what the rules read it
    /// as. One entry per lexeme, the first way found.
    fn find(&self, conn: &Connection, text: &str, lang: Lang) -> Result<Vec<Found>, Error> {
        let form = key(text);
        if form.is_empty() {
            return Ok(Vec::new());
        }
        let mut found: Vec<Found> = Vec::new();
        let mut push = |db: usize, id: i64, word: Word, features: Option<String>| {
            if !found
                .iter()
                .any(|f| f.db == db && f.word.lemma == word.lemma && f.word.pos == word.pos)
            {
                found.push(Found {
                    db,
                    id,
                    word,
                    features,
                });
            }
        };
        for (i, db) in self.dbs.iter().enumerate() {
            for (id, word, features) in
                Self::stored_forms(conn, db, &form, lang).map_err(|e| sql_error(&e))?
            {
                push(i, id, word, features);
            }
        }
        for analysis in analyses(&form, lang) {
            for (i, db) in self.dbs.iter().enumerate() {
                for (id, word, features) in Self::stored_forms(conn, db, &analysis.lemma, lang)
                    .map_err(|e| sql_error(&e))?
                {
                    if features.is_none() && analysis.class.admits(word.pos.as_deref()) {
                        push(i, id, word, Some(analysis.features.to_owned()));
                    }
                }
            }
        }
        Ok(found)
    }

    /// The senses of a lexeme of database `db`, in that database.
    fn senses_of(conn: &Connection, db: &Db, found: &Found) -> rusqlite::Result<Vec<Sense>> {
        let mut statement = conn.prepare_cached(&format!(
            "SELECT s.concept_id, s.rank, {SOURCE_COLUMNS} FROM {s}.sense s \
             JOIN {s}.source src ON src.id = s.source_id \
             WHERE s.lexeme_id = ?1 ORDER BY s.rank, s.id",
            s = db.schema
        ))?;
        let rows = statement.query_map([found.id], |row| {
            Ok(Sense {
                concept: ConceptId(row.get(0)?),
                word: found.word.clone(),
                features: found.features.clone(),
                rank: row.get(1)?,
                source: source_at(row, 2, &db.name)?,
            })
        })?;
        rows.collect()
    }

    /// The senses of a typed word among the words of one language: the
    /// word as typed or inflected ("guitarras", "violins", "llueve"), in
    /// the match form (case and accents do not matter). Ordered by rank,
    /// then store, then order of writing; one per concept.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn senses_in(&self, text: &str, lang: Lang) -> Result<Vec<Sense>, Error> {
        self.with(|conn| {
            let found = self.find(conn, text, lang)?;
            let mut senses: Vec<(usize, Sense)> = Vec::new();
            for (order, f) in found.iter().enumerate() {
                for sense in Self::senses_of(conn, &self.dbs[f.db], f).map_err(|e| sql_error(&e))? {
                    senses.push((f.db * 1_000_000 + order, sense));
                }
            }
            senses.sort_by_key(|(order, s)| (s.rank, *order));
            let mut seen = BTreeSet::new();
            Ok(senses
                .into_iter()
                .map(|(_, s)| s)
                .filter(|s| seen.insert(s.concept.clone()))
                .collect())
        })
    }

    /// What a typed word means (2.1, steps 1 and 3): its senses among the
    /// words of `lang`, or, when `lang` has no such word, of the other
    /// language. `None` when neither has it: an unknown word (step 5), or a
    /// near miss ([`Knowledge::near_misses`], step 4).
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn lookup(&self, text: &str, lang: Lang) -> Result<Option<Lookup>, Error> {
        for l in [lang, lang.other()] {
            let senses = self.senses_in(text, l)?;
            if !senses.is_empty() {
                return Ok(Some(Lookup { lang: l, senses }));
            }
        }
        Ok(None)
    }

    /// Known lemmas one edit away from an unknown word (one insertion,
    /// deletion, substitution or swap of adjacent letters, in the match
    /// form), in `lang` or, when `lang` has none, in the other (2.1, step
    /// 4). The caller suggests one only when there is exactly one.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn near_misses(&self, text: &str, lang: Lang) -> Result<Vec<Word>, Error> {
        let form = key(text);
        if form.chars().count() < 3 {
            return Ok(Vec::new());
        }
        let variants = one_edit(&form);
        self.with(|conn| {
            for l in [lang, lang.other()] {
                let mut words: Vec<Word> = Vec::new();
                for variant in &variants {
                    for db in &self.dbs {
                        for (_, word, features) in
                            Self::stored_forms(conn, db, variant, l).map_err(|e| sql_error(&e))?
                        {
                            if features.is_none() && !words.iter().any(|w| w.lemma == word.lemma) {
                                words.push(word);
                            }
                        }
                    }
                }
                if !words.is_empty() {
                    return Ok(words);
                }
            }
            Ok(Vec::new())
        })
    }

    /// The words of a concept in a language, best first (by rank, then
    /// store, then order of writing): "rank 0 if several" names it (2.7),
    /// the others are its synonyms.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn words(&self, concept: &ConceptId, lang: Lang) -> Result<Vec<Word>, Error> {
        self.with(|conn| {
            let mut words: Vec<(i64, usize, Word)> = Vec::new();
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT l.lemma, l.pos, s.rank FROM {s}.sense s \
                         JOIN {s}.lexeme l ON l.id = s.lexeme_id \
                         WHERE s.concept_id = ?1 AND l.lang = ?2 ORDER BY s.rank, s.id",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let rows = statement
                    .query_map(params![concept.as_str(), lang.code()], |row| {
                        Ok((
                            row.get::<_, i64>(2)?,
                            Word {
                                lang,
                                lemma: row.get(0)?,
                                pos: row.get(1)?,
                            },
                        ))
                    })
                    .map_err(|e| sql_error(&e))?;
                for row in rows {
                    let (rank, word) = row.map_err(|e| sql_error(&e))?;
                    if !words.iter().any(|(_, _, w)| *w == word) {
                        let order = words.len();
                        words.push((rank, order, word));
                    }
                }
            }
            words.sort_by_key(|(rank, order, _)| (*rank, *order));
            Ok(words.into_iter().map(|(_, _, w)| w).collect())
        })
    }

    /// The word that names a concept in a language: its first word there,
    /// or, when it has none, its first word in the other language (2.7; the
    /// caller marks the language). `None` for a concept with no word.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn name(&self, concept: &ConceptId, lang: Lang) -> Result<Option<Word>, Error> {
        for l in [lang, lang.other()] {
            if let Some(word) = self.words(concept, l)?.into_iter().next() {
                return Ok(Some(word));
            }
        }
        Ok(None)
    }

    /// A concept's definition in a language: the gloss of its best-ranked
    /// sense that has one there, from `user.db` first, then the packs in
    /// order. `None` when there is none in that language (the caller says
    /// so; it does not show the other language's unasked).
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn gloss(&self, concept: &ConceptId, lang: Lang) -> Result<Option<Gloss>, Error> {
        self.with(|conn| {
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT s.gloss, {SOURCE_COLUMNS} FROM {s}.sense s \
                         JOIN {s}.lexeme l ON l.id = s.lexeme_id \
                         JOIN {s}.source src ON src.id = s.source_id \
                         WHERE s.concept_id = ?1 AND l.lang = ?2 AND s.gloss IS NOT NULL \
                         ORDER BY s.rank, s.id LIMIT 1",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let gloss = statement
                    .query_row(params![concept.as_str(), lang.code()], |row| {
                        Ok(Gloss {
                            text: row.get(0)?,
                            lang,
                            source: source_at(row, 1, &db.name)?,
                        })
                    })
                    .optional()
                    .map_err(|e| sql_error(&e))?;
                if gloss.is_some() {
                    return Ok(gloss);
                }
            }
            Ok(None)
        })
    }

    /// A word's form with `features` (`pl`, `pres,3sg`…), so a translation
    /// keeps the number of what was typed: a stored exception first
    /// ("fish"), in the match form; else, for the plural of a noun or
    /// adjective, the regular plural in the lemma's spelling ("violines",
    /// "guitarras eléctricas"). `None` when neither gives one.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn form_of(&self, word: &Word, features: &str) -> Result<Option<String>, Error> {
        let stored = self.with(|conn| -> Result<Option<String>, Error> {
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT f.form FROM {s}.form f JOIN {s}.lexeme l ON l.id = f.lexeme_id \
                         WHERE l.lang = ?1 AND l.lemma = ?2 AND l.pos IS ?3 AND f.features = ?4 \
                         LIMIT 1",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let form: Option<String> = statement
                    .query_row(
                        params![word.lang.code(), word.lemma, word.pos, features],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| sql_error(&e))?;
                if form.is_some() {
                    return Ok(form);
                }
            }
            Ok(None)
        })?;
        if stored.is_some() {
            return Ok(stored);
        }
        Ok(
            (features == "pl" && Class::Nominal.admits(word.pos.as_deref()))
                .then(|| inflect::plural(&word.lemma, word.lang)),
        )
    }

    /// The stock phrases a text is (docs/spec/knowledge.md 1.6), compared in
    /// the match form without end punctuation ("salud" is «¡salud!»), in
    /// either language. Several when the same words are two phrases
    /// («¡salud!» sneezing and toasting).
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn stock_phrases(&self, text: &str) -> Result<Vec<StockPhrase>, Error> {
        let form = key(text);
        if form.is_empty() {
            return Ok(Vec::new());
        }
        self.with(|conn| {
            let mut out: Vec<StockPhrase> = Vec::new();
            for db in &self.dbs {
                let matched =
                    Self::phrase_rows(conn, db, "p.form = ?1", &form).map_err(|e| sql_error(&e))?;
                for (_, entry, text, approximate, source) in matched {
                    if out.iter().any(|p| p.entry == entry) {
                        continue;
                    }
                    let rows = Self::phrase_rows(conn, db, "p.entry = ?1", &entry)
                        .map_err(|e| sql_error(&e))?;
                    let (mut es, mut en) = (Vec::new(), Vec::new());
                    for (side_text, _, _, _, _) in rows {
                        match side_text.lang {
                            Lang::Es => es.push(side_text),
                            Lang::En => en.push(side_text),
                        }
                    }
                    out.push(StockPhrase {
                        entry,
                        matched: text,
                        es,
                        en,
                        approximate,
                        source,
                    });
                }
            }
            Ok(out)
        })
    }

    /// Phrase rows of one database matching `condition` on `?1`, with
    /// their notes: (wording, entry, wording again for the match, whether
    /// approximate, source).
    #[allow(clippy::type_complexity)]
    fn phrase_rows(
        conn: &Connection,
        db: &Db,
        condition: &str,
        value: &str,
    ) -> rusqlite::Result<Vec<(PhraseText, String, PhraseText, bool, Source)>> {
        let mut statement = conn.prepare_cached(&format!(
            "SELECT p.id, p.entry, p.lang, p.text, p.approximate, {SOURCE_COLUMNS} \
             FROM {s}.phrase p JOIN {s}.source src ON src.id = p.source_id \
             WHERE {condition} ORDER BY p.lang DESC, p.rank, p.id",
            s = db.schema
        ))?;
        let rows: Vec<(i64, String, String, String, bool, Source)> = statement
            .query_map([value], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    source_at(row, 5, &db.name)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut notes = conn.prepare_cached(&format!(
            "SELECT lang, note FROM {s}.phrase_note WHERE phrase_id = ?1 ORDER BY lang DESC",
            s = db.schema
        ))?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, entry, code, text, approximate, source) in rows {
            let Some(lang) = parse_lang(&code) else {
                continue;
            };
            let notes: Vec<(Lang, String)> = notes
                .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter_map(|(code, note)| parse_lang(&code).map(|l| (l, note)))
                .collect();
            let wording = PhraseText { text, lang, notes };
            out.push((wording.clone(), entry, wording, approximate, source));
        }
        Ok(out)
    }

    /// The word-pair lexicon's entry for a word of `lang` (KNOWLEDGE.md
    /// section 7, role 1): candidate translations into the other language,
    /// best first, from the first store that has the word. The lexicon is
    /// never reasoned from; callers consult it only when the concepts give
    /// no word (docs/spec/knowledge.md, translate).
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn word_pairs(&self, text: &str, lang: Lang) -> Result<Option<WordPairs>, Error> {
        let form = key(text);
        self.with(|conn| {
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT w.word, w.target, {SOURCE_COLUMNS} FROM {s}.word_pair w \
                         JOIN {s}.source src ON src.id = w.source_id \
                         WHERE w.form = ?1 AND w.lang = ?2 ORDER BY w.rank, w.rowid",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let rows: Vec<(String, String, Source)> = statement
                    .query_map(params![form, lang.code()], |row| {
                        Ok((row.get(0)?, row.get(1)?, source_at(row, 2, &db.name)?))
                    })
                    .map_err(|e| sql_error(&e))?
                    .collect::<rusqlite::Result<_>>()
                    .map_err(|e| sql_error(&e))?;
                if let Some((word, _, source)) = rows.first().cloned() {
                    return Ok(Some(WordPairs {
                        word,
                        lang,
                        translations: rows.into_iter().map(|(_, t, _)| t).collect(),
                        source,
                    }));
                }
            }
            Ok(None)
        })
    }

    /// The objects of a concept's claims with `predicate`, held (not
    /// negated, not retracted), in store and claim order: its direct
    /// classes for `is_a`, what it is part of for `part_of`. Validity
    /// intervals are not applied here.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn objects(&self, concept: &ConceptId, predicate: &str) -> Result<Vec<ConceptId>, Error> {
        self.related(concept, predicate, true)
    }

    /// The subjects of claims with `predicate` whose object is `concept`:
    /// its direct kinds for `is_a`, its parts for `part_of`.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn subjects(&self, concept: &ConceptId, predicate: &str) -> Result<Vec<ConceptId>, Error> {
        self.related(concept, predicate, false)
    }

    fn related(
        &self,
        concept: &ConceptId,
        predicate: &str,
        forward: bool,
    ) -> Result<Vec<ConceptId>, Error> {
        let (from, to) = if forward {
            ("subject", "object")
        } else {
            ("object", "subject")
        };
        self.with(|conn| {
            let mut out: Vec<ConceptId> = Vec::new();
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT c.{to} FROM {s}.claim c JOIN {s}.predicate p ON p.id = c.predicate \
                         WHERE c.{from} = ?1 AND p.name = ?2 AND c.negated = 0 \
                         AND c.retracted_at IS NULL AND c.object IS NOT NULL ORDER BY c.id",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let rows = statement
                    .query_map(params![concept.as_str(), predicate], |row| {
                        row.get::<_, String>(0)
                    })
                    .map_err(|e| sql_error(&e))?;
                for row in rows {
                    let id = ConceptId(row.map_err(|e| sql_error(&e))?);
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
            Ok(out)
        })
    }

    /// The concepts one held claim away from `concept`, either direction,
    /// any predicate.
    pub(crate) fn neighbours(&self, concept: &ConceptId) -> Result<BTreeSet<ConceptId>, Error> {
        self.with(|conn| {
            let mut out = BTreeSet::new();
            for db in &self.dbs {
                let mut statement = conn
                    .prepare_cached(&format!(
                        "SELECT object FROM {s}.claim WHERE subject = ?1 AND object IS NOT NULL \
                         AND negated = 0 AND retracted_at IS NULL \
                         UNION SELECT subject FROM {s}.claim WHERE object = ?1 \
                         AND negated = 0 AND retracted_at IS NULL",
                        s = db.schema
                    ))
                    .map_err(|e| sql_error(&e))?;
                let rows = statement
                    .query_map([concept.as_str()], |row| row.get::<_, String>(0))
                    .map_err(|e| sql_error(&e))?;
                for row in rows {
                    out.insert(ConceptId(row.map_err(|e| sql_error(&e))?));
                }
            }
            Ok(out)
        })
    }

    /// Whether `concept` has any held claim with `predicate`, as its
    /// subject or as its object.
    pub(crate) fn has_claim(
        &self,
        concept: &ConceptId,
        predicate: &str,
        as_subject: bool,
    ) -> Result<bool, Error> {
        Ok(!self.related(concept, predicate, as_subject)?.is_empty())
    }
}

/// Every string one edit from `word`: one letter deleted, inserted or
/// substituted (from the match form's alphabet), or two adjacent letters
/// swapped. Without `word` itself; sorted, no repeats.
fn one_edit(word: &str) -> Vec<String> {
    const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz -'";
    let chars: Vec<char> = word.chars().collect();
    let mut out = BTreeSet::new();
    let join = |v: &[char]| v.iter().collect::<String>();
    for i in 0..chars.len() {
        let mut v = chars.clone();
        v.remove(i);
        out.insert(join(&v));
        if i + 1 < chars.len() {
            let mut v = chars.clone();
            v.swap(i, i + 1);
            out.insert(join(&v));
        }
        for c in ALPHABET.chars() {
            let mut v = chars.clone();
            v[i] = c;
            out.insert(join(&v));
        }
    }
    for i in 0..=chars.len() {
        for c in ALPHABET.chars() {
            let mut v = chars.clone();
            v.insert(i, c);
            out.insert(join(&v));
        }
    }
    out.remove(word);
    out.into_iter()
        .filter(|w| !w.is_empty() && !w.starts_with(' ') && !w.ends_with(' '))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = include_str!("../../../../knowledge/seed.toml");

    fn seed() -> Knowledge {
        Knowledge::from_source(SEED).unwrap()
    }

    fn concepts(k: &Knowledge, word: &str, lang: Lang) -> Vec<String> {
        k.senses_in(word, lang)
            .unwrap()
            .into_iter()
            .map(|s| s.concept.0)
            .collect()
    }

    #[test]
    fn forms_resolve_to_their_lemma() {
        let k = seed();
        for (word, lang, concept) in [
            ("guitarras", Lang::Es, "core:guitar"),
            ("Guitarra", Lang::Es, "core:guitar"),
            ("violines", Lang::Es, "core:violin"),
            ("violin", Lang::Es, "core:violin"),
            ("cuerdas", Lang::Es, "core:rope"),
            ("guitars", Lang::En, "core:guitar"),
            ("violins", Lang::En, "core:violin"),
            ("drums", Lang::En, "core:drum"),
            ("fish", Lang::En, "core:fish"),
            ("peces", Lang::Es, "core:fish"),
            ("llueve", Lang::Es, "core:rain"),
            ("llovía", Lang::Es, "core:rain"),
            ("rains", Lang::En, "core:rain"),
            ("raining", Lang::En, "core:rain"),
            ("guitarras eléctricas", Lang::Es, "core:electric_guitar"),
            ("cajas de resonancia", Lang::Es, "core:sound_box"),
            ("string instruments", Lang::En, "core:string_instrument"),
            ("plutón", Lang::Es, "core:pluto"),
        ] {
            assert_eq!(
                concepts(&k, word, lang).first().map(String::as_str),
                Some(concept),
                "{word}"
            );
        }
        let sense = &k.senses_in("guitarras", Lang::Es).unwrap()[0];
        assert_eq!(sense.word.lemma, "guitarra");
        assert_eq!(sense.features.as_deref(), Some("pl"));
        assert_eq!(sense.source.name, "core");
        assert_eq!(sense.source.store, "core");
        let rain = &k.senses_in("llueve", Lang::Es).unwrap()[0];
        assert_eq!(
            (rain.word.lemma.as_str(), rain.features.as_deref()),
            ("llover", Some("pres,3sg"))
        );
        // A verb rule never reads a noun, nor a noun rule a verb.
        assert_eq!(k.senses_in("guitarraba", Lang::Es).unwrap().len(), 0);
    }

    #[test]
    fn senses_come_in_rank_order_one_per_concept() {
        let k = seed();
        assert_eq!(
            concepts(&k, "banco", Lang::Es),
            ["core:bank", "core:bench", "core:school_of_fish"]
        );
        let ranks: Vec<i64> = k
            .senses_in("banco", Lang::Es)
            .unwrap()
            .iter()
            .map(|s| s.rank)
            .collect();
        assert_eq!(ranks, [0, 0, 2]);
        assert_eq!(
            concepts(&k, "neck", Lang::En),
            ["core:neck_body", "core:neck"]
        );
        // "rain" is a noun and a verb of one concept: one sense.
        assert_eq!(concepts(&k, "rain", Lang::En), ["core:rain"]);
        assert_eq!(
            concepts(&k, "cold", Lang::En),
            ["core:cold", "core:common_cold"]
        );
        // The longest word is a lexeme of its own.
        assert_eq!(
            concepts(&k, "banco de peces", Lang::Es),
            ["core:school_of_fish"]
        );
    }

    #[test]
    fn a_word_only_the_other_language_has_is_found_there() {
        let k = seed();
        let found = k.lookup("bench", Lang::Es).unwrap().unwrap();
        assert_eq!(found.lang, Lang::En);
        assert_eq!(found.senses[0].concept.as_str(), "core:bench");
        let found = k.lookup("bandurria", Lang::En).unwrap().unwrap();
        assert_eq!(found.lang, Lang::Es);
        // "animal" is in both: the conversation's language first.
        assert_eq!(
            k.lookup("animal", Lang::En).unwrap().unwrap().lang,
            Lang::En
        );
        for unknown in ["zanfona", "theremin", "zorongo", "glarb", "pandereta", ""] {
            assert!(k.lookup(unknown, Lang::Es).unwrap().is_none(), "{unknown}");
        }
    }

    #[test]
    fn near_misses_look_in_the_conversation_language_first() {
        let k = seed();
        let lemmas = |w: &str, l: Lang| -> Vec<String> {
            k.near_misses(w, l)
                .unwrap()
                .into_iter()
                .map(|w| w.lemma)
                .collect()
        };
        assert_eq!(lemmas("guitara", Lang::Es), ["guitarra"]);
        assert_eq!(lemmas("guitara", Lang::En), ["guitar"]);
        assert_eq!(lemmas("violni", Lang::En), ["violin"]);
        assert_eq!(lemmas("zorongo", Lang::Es).len(), 0);
    }

    #[test]
    fn glosses_words_and_names_by_language() {
        let k = seed();
        let guitar = ConceptId::new("core:guitar");
        let gloss = k.gloss(&guitar, Lang::Es).unwrap().unwrap();
        assert!(gloss.text.starts_with("instrumento de cuerda con mástil"));
        assert_eq!(gloss.source.name, "core");
        assert!(
            k.gloss(&ConceptId::new("core:fret"), Lang::Es)
                .unwrap()
                .is_none()
        );
        assert!(
            k.gloss(&ConceptId::new("core:capo"), Lang::En)
                .unwrap()
                .is_none()
        );
        assert!(
            k.gloss(&ConceptId::new("core:fret"), Lang::En)
                .unwrap()
                .is_some()
        );
        let lemmas = |c: &str, l: Lang| -> Vec<String> {
            k.words(&ConceptId::new(c), l)
                .unwrap()
                .into_iter()
                .map(|w| w.lemma)
                .collect()
        };
        assert_eq!(
            lemmas("core:string_instrument", Lang::En),
            ["string instrument", "stringed instrument"]
        );
        assert_eq!(lemmas("core:riverbank", Lang::En), ["riverbank", "bank"]);
        assert_eq!(
            lemmas("core:school_of_fish", Lang::Es),
            ["banco de peces", "banco"]
        );
        assert_eq!(lemmas("core:bandurria", Lang::En).len(), 0);
        let named = k
            .name(&ConceptId::new("core:dulcimer"), Lang::Es)
            .unwrap()
            .unwrap();
        assert_eq!((named.lemma.as_str(), named.lang), ("dulcimer", Lang::En));
        assert_eq!(
            k.objects(&guitar, "is_a").unwrap(),
            [ConceptId::new("core:string_instrument")]
        );
        assert_eq!(k.subjects(&guitar, "part_of").unwrap().len(), 3);
    }

    #[test]
    fn a_translation_keeps_the_number_of_what_was_typed() {
        let k = seed();
        let typed = &k.senses_in("guitarras", Lang::Es).unwrap()[0];
        let target = &k.words(&typed.concept, Lang::En).unwrap()[0];
        assert_eq!(
            k.form_of(target, typed.features.as_deref().unwrap())
                .unwrap()
                .as_deref(),
            Some("guitars")
        );
        let fish = &k.words(&ConceptId::new("core:fish"), Lang::En).unwrap()[0];
        assert_eq!(k.form_of(fish, "pl").unwrap().as_deref(), Some("fish"));
        let violin = &k.words(&ConceptId::new("core:violin"), Lang::Es).unwrap()[0];
        assert_eq!(
            k.form_of(violin, "pl").unwrap().as_deref(),
            Some("violines")
        );
        let llover = &k.senses_in("llover", Lang::Es).unwrap()[0].word;
        assert_eq!(
            k.form_of(llover, "pres,3sg").unwrap().as_deref(),
            Some("llueve")
        );
        assert_eq!(k.form_of(llover, "pl").unwrap(), None);
    }

    #[test]
    fn stock_phrases_both_sides_with_notes() {
        let k = seed();
        let found = k.stock_phrases("Buenas noches.").unwrap();
        assert_eq!(found.len(), 1);
        let p = &found[0];
        assert_eq!(p.matched.lang, Lang::Es);
        let en: Vec<&str> = p.en.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(en, ["good evening", "good night"]);
        assert_eq!(p.en[1].note(Lang::Es), Some("al despedirse"));
        let salud = k.stock_phrases("salud").unwrap();
        let entries: Vec<&str> = salud.iter().map(|p| p.entry.as_str()).collect();
        assert_eq!(entries, ["salud_estornudo", "salud_brindis"]);
        assert!(k.stock_phrases("buen provecho").unwrap()[0].approximate);
        let back = k.stock_phrases("Nice to meet you!").unwrap();
        assert_eq!(back[0].es.len(), 4);
        assert_eq!(k.stock_phrases("buenas").unwrap().len(), 0);
    }

    #[test]
    fn the_lexicon_gives_candidates_best_first() {
        let k = seed();
        let gaita = k.word_pairs("gaita", Lang::Es).unwrap().unwrap();
        assert_eq!(gaita.translations, ["bagpipes", "bagpipe"]);
        assert_eq!(gaita.source.name, "lexicon");
        assert!(k.word_pairs("gaita", Lang::En).unwrap().is_none());
        assert_eq!(
            k.word_pairs("Cord", Lang::En)
                .unwrap()
                .unwrap()
                .translations,
            ["cuerda", "cordón"]
        );
        // Lexicon-only words have no concept.
        assert!(k.lookup("pandereta", Lang::Es).unwrap().is_none());
    }

    #[test]
    fn the_user_store_is_asked_first_and_merged_by_word() {
        let mut k = Knowledge::with_user_tables().unwrap();
        k.execute_main(
            "INSERT INTO source (id, kind, name, added_at) VALUES (1, 'user', 'user', 0);
             INSERT INTO concept (id) VALUES ('user:zanfona');
             INSERT INTO lexeme (id, lang, lemma, pos) VALUES (1, 'es', 'zanfona', NULL);
             INSERT INTO form (lexeme_id, form) VALUES (1, 'zanfona');
             INSERT INTO sense (lexeme_id, concept_id, rank, source_id) VALUES (1, 'user:zanfona', 0, 1);
             INSERT INTO lexeme (id, lang, lemma, pos) VALUES (2, 'es', 'guitarra', 'n');
             INSERT INTO form (lexeme_id, form) VALUES (2, 'guitarra');
             INSERT INTO sense (lexeme_id, concept_id, gloss, rank, source_id)
               VALUES (2, 'core:guitar', 'mi guitarra', 0, 1);",
        );
        k.attach_source(SEED).unwrap();
        assert_eq!(k.stores(), ["user", "core"]);
        let zanfonas = k.senses_in("zanfonas", Lang::Es).unwrap();
        assert_eq!(zanfonas[0].concept.as_str(), "user:zanfona");
        assert_eq!(zanfonas[0].source.store, "user");
        // One sense for the guitar, though both stores have the word.
        let guitar = k.senses_in("guitarra", Lang::Es).unwrap();
        assert_eq!(guitar.len(), 1);
        let gloss = k.gloss(&guitar[0].concept, Lang::Es).unwrap().unwrap();
        assert_eq!(
            (gloss.text.as_str(), gloss.source.store.as_str()),
            ("mi guitarra", "user")
        );
        assert_eq!(k.words(&guitar[0].concept, Lang::Es).unwrap().len(), 1);
    }

    #[test]
    fn one_edit_covers_the_four_edits() {
        let edits = one_edit("ab");
        for e in ["a", "b", "ba", "xb", "abc", "cab"] {
            assert!(edits.contains(&e.to_owned()), "{e}");
        }
        assert!(!edits.contains(&"ab".to_owned()));
    }
}
