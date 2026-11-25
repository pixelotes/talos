//! The knowledge schema (docs/KNOWLEDGE.md section 2) and its forward-only
//! migrations.
//!
//! Every Talos database, `user.db` and each pack alike, has the knowledge
//! tables; `user.db` also has the tables of what is kept about the user
//! (preferences, the correction table). The version lives in `meta`
//! (`schema_version`); `PRAGMA application_id` marks the file as Talos's.
//!
//! The statements are written with `@.` before every table and index name
//! and are run with `@` replaced by the schema they are created in (`main`,
//! or an attached in-memory database for a pack built in a test).

use rusqlite::{Connection, OptionalExtension};

use crate::Error;

/// The schema version this Talos writes and reads.
pub const SCHEMA_VERSION: u32 = 1;

/// `PRAGMA application_id` of every Talos database: "TLOS".
pub const APPLICATION_ID: i32 = 0x544C_4F53;

/// The knowledge tables, version 1: KNOWLEDGE.md section 2 as written,
/// except `gloss_fts` (full-text search is not part of v0.3, and FTS5 is
/// compiled out; a later version adds the table by migration), plus three
/// tables the section names but does not spell out: the stock phrases
/// (docs/spec/knowledge.md 1.6), their notes, and the word-pair lexicon
/// (KNOWLEDGE.md section 7, "its own table of word pairs with rank and
/// source"), and one more index, `form_by_lexeme`: a word's stored form
/// with given features ([`super::Knowledge::form_of`]) is found through its
/// lexeme, which without it scans every form of a pack (10 ms on the
/// vocabulary pack). The `REFERENCES` clauses are documentation: foreign keys are
/// off, since a pack may name another pack's concepts; the pack builder
/// checks references instead.
const KNOWLEDGE_V1: &str = "
CREATE TABLE @.meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE @.source (
  id        INTEGER PRIMARY KEY,
  kind      TEXT NOT NULL,
  name      TEXT NOT NULL,
  license   TEXT,
  url       TEXT,
  added_at  INTEGER NOT NULL
);

CREATE TABLE @.concept (
  id        TEXT PRIMARY KEY,
  kind      TEXT
);

CREATE TABLE @.lexeme (
  id        INTEGER PRIMARY KEY,
  lang      TEXT NOT NULL,
  lemma     TEXT NOT NULL,
  pos       TEXT,
  UNIQUE (lang, lemma, pos)
);

CREATE TABLE @.form (
  lexeme_id INTEGER NOT NULL REFERENCES lexeme(id),
  form      TEXT NOT NULL,
  features  TEXT
);
CREATE INDEX @.form_lookup ON form(form);
CREATE INDEX @.form_by_lexeme ON form(lexeme_id);

CREATE TABLE @.sense (
  id         INTEGER PRIMARY KEY,
  lexeme_id  INTEGER NOT NULL REFERENCES lexeme(id),
  concept_id TEXT    NOT NULL REFERENCES concept(id),
  gloss      TEXT,
  rank       INTEGER NOT NULL DEFAULT 0,
  source_id  INTEGER NOT NULL REFERENCES source(id)
);
CREATE INDEX @.sense_by_lexeme  ON sense(lexeme_id);
CREATE INDEX @.sense_by_concept ON sense(concept_id);

CREATE TABLE @.predicate (
  id         INTEGER PRIMARY KEY,
  name       TEXT UNIQUE NOT NULL,
  inverse    TEXT,
  transitive INTEGER NOT NULL DEFAULT 0,
  symmetric  INTEGER NOT NULL DEFAULT 0,
  functional INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE @.claim (
  id          INTEGER PRIMARY KEY,
  subject     TEXT    NOT NULL REFERENCES concept(id),
  predicate   INTEGER NOT NULL REFERENCES predicate(id),
  object      TEXT    REFERENCES concept(id),
  literal     TEXT,
  negated     INTEGER NOT NULL DEFAULT 0,
  confidence  REAL    NOT NULL,
  source_id   INTEGER NOT NULL REFERENCES source(id),
  valid_from  INTEGER,
  valid_to    INTEGER,
  created_at  INTEGER NOT NULL,
  retracted_at INTEGER
);
CREATE INDEX @.claim_forward ON claim(subject, predicate);
CREATE INDEX @.claim_reverse ON claim(object, predicate);

CREATE TABLE @.audit (
  id INTEGER PRIMARY KEY, at INTEGER NOT NULL,
  action TEXT NOT NULL, claim_id INTEGER, detail TEXT
);

CREATE TABLE @.phrase (
  id          INTEGER PRIMARY KEY,
  entry       TEXT    NOT NULL,
  lang        TEXT    NOT NULL,
  text        TEXT    NOT NULL,
  form        TEXT    NOT NULL,
  rank        INTEGER NOT NULL DEFAULT 0,
  approximate INTEGER NOT NULL DEFAULT 0,
  source_id   INTEGER NOT NULL REFERENCES source(id)
);
CREATE INDEX @.phrase_lookup   ON phrase(form);
CREATE INDEX @.phrase_by_entry ON phrase(entry);

CREATE TABLE @.phrase_note (
  phrase_id INTEGER NOT NULL REFERENCES phrase(id),
  lang      TEXT    NOT NULL,
  note      TEXT    NOT NULL,
  PRIMARY KEY (phrase_id, lang)
);

CREATE TABLE @.word_pair (
  lang      TEXT    NOT NULL,
  word      TEXT    NOT NULL,
  form      TEXT    NOT NULL,
  target    TEXT    NOT NULL,
  rank      INTEGER NOT NULL DEFAULT 0,
  source_id INTEGER NOT NULL REFERENCES source(id)
);
CREATE INDEX @.word_pair_lookup ON word_pair(form, lang);
";

/// What only `user.db` holds, version 1: the preferences and the
/// user-correction table that were `user.toml` until v0.3, one row per
/// value, so that a change rewrites rows, not a file.
const USER_V1: &str = "
CREATE TABLE @.preference (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE @.correction (
  id     INTEGER PRIMARY KEY,
  phrase TEXT NOT NULL UNIQUE,
  intent TEXT NOT NULL
);

CREATE TABLE @.correction_slot (
  correction_id INTEGER NOT NULL REFERENCES correction(id),
  name          TEXT    NOT NULL,
  value         TEXT    NOT NULL,
  PRIMARY KEY (correction_id, name)
);
";

/// One step forward: the statements that take a database from `to - 1` to
/// `to`. Steps are never edited once released; a change is a new step.
struct Migration {
    to: u32,
    knowledge: &'static str,
    user: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    to: 1,
    knowledge: KNOWLEDGE_V1,
    user: USER_V1,
}];

/// Which kind of Talos database a file is (`meta.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbKind {
    /// `user.db`: read-write, migrated forward at start.
    User,
    /// A pack: read-only, built for exactly one schema version.
    Pack,
}

impl DbKind {
    /// The `meta.kind` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            DbKind::User => "user",
            DbKind::Pack => "pack",
        }
    }
}

pub(crate) fn sql_error(e: &rusqlite::Error) -> Error {
    Error::Knowledge(e.to_string())
}

/// Runs `sql` with `@` naming `schema`.
fn run(conn: &Connection, schema: &str, sql: &str) -> Result<(), Error> {
    conn.execute_batch(&sql.replace("@.", &format!("{schema}.")))
        .map_err(|e| sql_error(&e))
}

/// The schema version of a database: 0 when it has no `meta` table (a new,
/// empty file).
///
/// # Errors
///
/// [`Error::Knowledge`] when the file cannot be read or is not a Talos
/// database of a known shape.
pub fn version(conn: &Connection, schema: &str) -> Result<u32, Error> {
    let has_meta: bool = conn
        .query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM {schema}.sqlite_master \
                 WHERE type = 'table' AND name = 'meta')"
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| sql_error(&e))?;
    if !has_meta {
        return Ok(0);
    }
    let value: Option<String> = conn
        .query_row(
            &format!("SELECT value FROM {schema}.meta WHERE key = 'schema_version'"),
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| sql_error(&e))?;
    value
        .as_deref()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Error::Knowledge("no schema version in `meta`".to_owned()))
}

/// The `PRAGMA application_id` of a schema.
pub(crate) fn application_id(conn: &Connection, schema: &str) -> Result<i32, Error> {
    conn.query_row(&format!("PRAGMA {schema}.application_id"), [], |row| {
        row.get(0)
    })
    .map_err(|e| sql_error(&e))
}

/// Brings a database forward to [`SCHEMA_VERSION`], creating it when it is
/// empty, inside one transaction the caller opened. Whether anything was
/// done.
///
/// # Errors
///
/// [`Error::Knowledge`] when the database is newer than this Talos (it is
/// then left as it is), is not a Talos database, or a statement fails.
pub fn migrate(conn: &Connection, schema: &str, kind: DbKind) -> Result<bool, Error> {
    let from = version(conn, schema)?;
    if from > SCHEMA_VERSION {
        return Err(Error::Knowledge(format!(
            "schema version {from} is newer than {SCHEMA_VERSION}, the one this Talos knows"
        )));
    }
    if from == SCHEMA_VERSION {
        return Ok(false);
    }
    if from == 0 {
        let tables: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM {schema}.sqlite_master"),
                [],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(&e))?;
        if tables != 0 {
            return Err(Error::Knowledge(
                "the file has tables but is not a Talos database".to_owned(),
            ));
        }
    }
    for step in MIGRATIONS.iter().filter(|m| m.to > from) {
        run(conn, schema, step.knowledge)?;
        if kind == DbKind::User {
            run(conn, schema, step.user)?;
        }
    }
    let set = |key: &str, value: &str| {
        conn.execute(
            &format!("INSERT OR REPLACE INTO {schema}.meta (key, value) VALUES (?1, ?2)"),
            [key, value],
        )
        .map_err(|e| sql_error(&e))
    };
    set("schema_version", &SCHEMA_VERSION.to_string())?;
    set("kind", kind.as_str())?;
    run(
        conn,
        schema,
        &format!("PRAGMA @.application_id = {APPLICATION_ID}"),
    )?;
    Ok(true)
}

/// The tables every Talos knowledge database must have, for the structural
/// check of a pack.
pub(crate) const TABLES: &[&str] = &[
    "meta",
    "source",
    "concept",
    "lexeme",
    "form",
    "sense",
    "predicate",
    "claim",
    "audit",
    "phrase",
    "phrase_note",
    "word_pair",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_database_is_created_at_the_current_version_and_left_alone_after() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(version(&conn, "main").unwrap(), 0);
        assert!(migrate(&conn, "main", DbKind::User).unwrap());
        assert_eq!(version(&conn, "main").unwrap(), SCHEMA_VERSION);
        assert_eq!(application_id(&conn, "main").unwrap(), APPLICATION_ID);
        assert!(!migrate(&conn, "main", DbKind::User).unwrap());
        for table in TABLES.iter().chain(&["preference", "correction"]) {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{table}");
        }
    }

    #[test]
    fn a_newer_or_foreign_database_is_refused_untouched() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, "main", DbKind::Pack).unwrap();
        conn.execute(
            "UPDATE meta SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
        let error = migrate(&conn, "main", DbKind::Pack).unwrap_err();
        assert!(error.to_string().contains("newer"), "{error}");
        let other = Connection::open_in_memory().unwrap();
        other.execute_batch("CREATE TABLE t (x)").unwrap();
        assert!(migrate(&other, "main", DbKind::User).is_err());
    }

    #[test]
    fn packs_have_no_user_tables() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, "main", DbKind::Pack).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'preference'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }
}
