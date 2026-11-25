//! Packs (ADR 0006): read-only SQLite files with the knowledge schema and a
//! manifest in `meta`. What a pack says about itself ([`PackInfo`]), the
//! structural check every pack passes before it is attached, and the full
//! check of `talos packs check`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::schema::{self, APPLICATION_ID, SCHEMA_VERSION, TABLES, sql_error};
use crate::{Error, Lang};

/// The extension of a pack file.
pub const EXTENSION: &str = "talospack";

/// Where a fact came from (the `source` table): shown when Talos says how
/// it knows something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// `core`, `lexicon`, `user`…
    pub name: String,
    /// `user`, `pack` or `import`.
    pub kind: String,
    /// The licence, as an SPDX id, when known.
    pub license: Option<String>,
    /// Where it can be found.
    pub url: Option<String>,
    /// The database it was read from: `user`, or a pack's id.
    pub store: String,
}

/// A pack's manifest: the `meta` table plus its sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackInfo {
    /// The file, or `None` for a pack built in memory.
    pub path: Option<PathBuf>,
    /// Short, unique among attached packs (`core`).
    pub id: String,
    /// The pack's own version (`1.0`).
    pub version: String,
    /// A human title.
    pub title: String,
    /// The languages it has words in.
    pub languages: Vec<Lang>,
    /// The pack's licence (SPDX).
    pub license: String,
    /// The notices its licence and its sources' licences ask for.
    pub notice: String,
    /// The schema it was built for; always [`SCHEMA_VERSION`] once attached.
    pub schema_version: u32,
    /// Where its contents came from.
    pub sources: Vec<Source>,
}

/// A pack that was found but not attached, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The file.
    pub path: PathBuf,
    /// Why, in a sentence for a person.
    pub reason: String,
}

/// How much a pack holds (`talos packs info`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    /// Concepts.
    pub concepts: i64,
    /// Lexemes in Spanish.
    pub lexemes_es: i64,
    /// Lexemes in English.
    pub lexemes_en: i64,
    /// Stored forms (lemmas and the exceptions no rule gives).
    pub forms: i64,
    /// Senses.
    pub senses: i64,
    /// Senses with a gloss.
    pub glosses: i64,
    /// Claims.
    pub claims: i64,
    /// Stock phrases (entries, both sides together).
    pub phrases: i64,
    /// Word pairs of the lexicon (one per translation).
    pub word_pairs: i64,
}

/// A `file:` URI for `path` with query parameters (SQLite URI filenames:
/// `?`, `#` and `%` must be escaped in the path).
pub(crate) fn file_uri(path: &Path, query: &str) -> Result<String, Error> {
    let text = path
        .to_str()
        .ok_or_else(|| Error::Knowledge(format!("{}: not a UTF-8 path", path.display())))?;
    let mut uri = String::from("file:");
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri.push('?');
    uri.push_str(query);
    Ok(uri)
}

/// Opens a pack file read-only and immutable, on its own connection.
pub(crate) fn open(path: &Path) -> Result<Connection, Error> {
    Connection::open_with_flags(
        file_uri(path, "mode=ro&immutable=1")?,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| sql_error(&e))
}

fn meta(conn: &Connection, schema: &str, key: &str) -> Result<String, Error> {
    conn.query_row(
        &format!("SELECT value FROM {schema}.meta WHERE key = ?1"),
        [key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| sql_error(&e))?
    .filter(|v| !v.trim().is_empty())
    .ok_or_else(|| Error::Knowledge(format!("its manifest has no `{key}`")))
}

/// The sources of a database, in id order.
pub(crate) fn sources(conn: &Connection, schema: &str, store: &str) -> Result<Vec<Source>, Error> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT name, kind, license, url FROM {schema}.source ORDER BY id"
        ))
        .map_err(|e| sql_error(&e))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Source {
                name: row.get(0)?,
                kind: row.get(1)?,
                license: row.get(2)?,
                url: row.get(3)?,
                store: store.to_owned(),
            })
        })
        .map_err(|e| sql_error(&e))?;
    rows.collect::<Result<_, _>>().map_err(|e| sql_error(&e))
}

/// The structural check every pack passes before it is attached: a Talos
/// database, of this schema version, a pack, with every table and a
/// complete manifest. Cheap (a few small reads), so it runs at every start;
/// the full integrity check is [`check`].
///
/// # Errors
///
/// [`Error::Knowledge`] saying what is wrong, for a person.
pub(crate) fn read_info(
    conn: &Connection,
    schema: &str,
    path: Option<&Path>,
) -> Result<PackInfo, Error> {
    let not_pack = || Error::Knowledge("not a Talos pack".to_owned());
    let id = schema::application_id(conn, schema)
        .map_err(|e| Error::Knowledge(format!("not a readable SQLite database ({e})")))?;
    if id != APPLICATION_ID {
        return Err(not_pack());
    }
    let version = schema::version(conn, schema)?;
    if version != SCHEMA_VERSION {
        return Err(Error::Knowledge(format!(
            "built for schema version {version}; this Talos reads version {SCHEMA_VERSION}"
        )));
    }
    if meta(conn, schema, "kind")? != "pack" {
        return Err(not_pack());
    }
    for table in TABLES {
        let present: bool = conn
            .query_row(
                &format!(
                    "SELECT EXISTS (SELECT 1 FROM {schema}.sqlite_master \
                     WHERE type = 'table' AND name = ?1)"
                ),
                [table],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(&e))?;
        if !present {
            return Err(Error::Knowledge(format!("the table `{table}` is missing")));
        }
    }
    let id = meta(conn, schema, "id")?;
    let mut languages = Vec::new();
    for code in meta(conn, schema, "languages")?.split_whitespace() {
        languages.push(
            code.parse::<Lang>()
                .map_err(|_| Error::Knowledge(format!("unknown language `{code}`")))?,
        );
    }
    Ok(PackInfo {
        path: path.map(Path::to_path_buf),
        sources: sources(conn, schema, &id)?,
        version: meta(conn, schema, "version")?,
        title: meta(conn, schema, "title")?,
        license: meta(conn, schema, "license")?,
        notice: meta(conn, schema, "notice")?,
        id,
        languages,
        schema_version: version,
    })
}

/// What a pack file says about itself, after the structural check.
///
/// # Errors
///
/// [`Error::Knowledge`] when the file cannot be opened or fails the check.
pub fn inspect(path: &Path) -> Result<PackInfo, Error> {
    let conn = open(path)?;
    read_info(&conn, "main", Some(path))
}

/// Counts what a database holds.
///
/// # Errors
///
/// [`Error::Knowledge`] when it cannot be read.
pub(crate) fn counts(conn: &Connection, schema: &str) -> Result<Counts, Error> {
    let n = |sql: &str| -> Result<i64, Error> {
        conn.query_row(&sql.replace("@.", &format!("{schema}.")), [], |row| {
            row.get(0)
        })
        .map_err(|e| sql_error(&e))
    };
    Ok(Counts {
        concepts: n("SELECT count(*) FROM @.concept")?,
        lexemes_es: n("SELECT count(*) FROM @.lexeme WHERE lang = 'es'")?,
        lexemes_en: n("SELECT count(*) FROM @.lexeme WHERE lang = 'en'")?,
        forms: n("SELECT count(*) FROM @.form")?,
        senses: n("SELECT count(*) FROM @.sense")?,
        glosses: n("SELECT count(*) FROM @.sense WHERE gloss IS NOT NULL")?,
        claims: n("SELECT count(*) FROM @.claim")?,
        phrases: n("SELECT count(DISTINCT entry) FROM @.phrase")?,
        word_pairs: n("SELECT count(*) FROM @.word_pair")?,
    })
}

/// What a pack holds, after the structural check (`talos packs info`).
///
/// # Errors
///
/// [`Error::Knowledge`] when the file cannot be opened or fails the check.
pub fn info(path: &Path) -> Result<(PackInfo, Counts), Error> {
    let conn = open(path)?;
    let info = read_info(&conn, "main", Some(path))?;
    Ok((info, counts(&conn, "main")?))
}

/// The result of the full check of a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The manifest.
    pub info: PackInfo,
    /// What it holds.
    pub counts: Counts,
    /// What is wrong; empty for a good pack.
    pub problems: Vec<String>,
    /// Concepts its claims and senses name that it does not define: allowed
    /// (packs compose), worth knowing.
    pub outside_concepts: i64,
}

/// References that must resolve inside the pack: (what, query counting the
/// rows that do not).
const REFERENCES: &[(&str, &str)] = &[
    (
        "forms of no lexeme",
        "SELECT count(*) FROM form f WHERE NOT EXISTS (SELECT 1 FROM lexeme l WHERE l.id = f.lexeme_id)",
    ),
    (
        "senses of no lexeme",
        "SELECT count(*) FROM sense s WHERE NOT EXISTS (SELECT 1 FROM lexeme l WHERE l.id = s.lexeme_id)",
    ),
    (
        "senses of no source",
        "SELECT count(*) FROM sense s WHERE NOT EXISTS (SELECT 1 FROM source x WHERE x.id = s.source_id)",
    ),
    (
        "claims of no predicate",
        "SELECT count(*) FROM claim c WHERE NOT EXISTS (SELECT 1 FROM predicate p WHERE p.id = c.predicate)",
    ),
    (
        "claims of no source",
        "SELECT count(*) FROM claim c WHERE NOT EXISTS (SELECT 1 FROM source x WHERE x.id = c.source_id)",
    ),
    (
        "claims with neither or both of an object and a literal",
        "SELECT count(*) FROM claim WHERE (object IS NULL) = (literal IS NULL)",
    ),
    (
        "lexemes in an unknown language",
        "SELECT count(*) FROM lexeme WHERE lang NOT IN ('es', 'en')",
    ),
    (
        "phrases of no source",
        "SELECT count(*) FROM phrase p WHERE NOT EXISTS (SELECT 1 FROM source x WHERE x.id = p.source_id)",
    ),
    (
        "word pairs of no source",
        "SELECT count(*) FROM word_pair w WHERE NOT EXISTS (SELECT 1 FROM source x WHERE x.id = w.source_id)",
    ),
];

/// The full check (`talos packs check`): the structural check, SQLite's
/// `integrity_check` over every page, and the references the pack builder
/// guarantees. Reads the whole file, so it is not run at start.
///
/// # Errors
///
/// [`Error::Knowledge`] when the file cannot be opened or fails the
/// structural check; other problems are listed in [`Checked::problems`].
pub fn check(path: &Path) -> Result<Checked, Error> {
    let conn = open(path)?;
    let info = read_info(&conn, "main", Some(path))?;
    let mut problems = Vec::new();
    let mut statement = conn
        .prepare("PRAGMA integrity_check")
        .map_err(|e| sql_error(&e))?;
    let lines: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .map_err(|e| sql_error(&e))?
        .collect::<Result<_, _>>()
        .map_err(|e| sql_error(&e))?;
    if lines != ["ok"] {
        problems.extend(lines.into_iter().map(|l| format!("integrity: {l}")));
    }
    for (what, sql) in REFERENCES {
        let n: i64 = conn
            .query_row(sql, [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        if n > 0 {
            problems.push(format!("{n} {what}"));
        }
    }
    let outside_concepts: i64 = conn
        .query_row(
            "SELECT count(*) FROM (SELECT concept_id AS c FROM sense \
             UNION SELECT subject FROM claim UNION SELECT object FROM claim WHERE object IS NOT NULL) \
             WHERE c NOT IN (SELECT id FROM concept)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| sql_error(&e))?;
    Ok(Checked {
        counts: counts(&conn, "main")?,
        info,
        problems,
        outside_concepts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_escape_what_sqlite_would_read_as_syntax() {
        assert_eq!(
            file_uri(Path::new("/a b/c?d#e%f.talospack"), "mode=ro").unwrap(),
            "file:/a%20b/c%3Fd%23e%25f.talospack?mode=ro"
        );
    }
}
