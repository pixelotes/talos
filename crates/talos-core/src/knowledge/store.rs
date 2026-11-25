//! The store as the engine holds it: one SQLite connection whose `main` is
//! `user.db` (read-only here; the user store writes it on its own
//! connection) or an empty in-memory database, with every pack attached
//! next to it (ADR 0006, docs/research/pack-attachment.md section 9):
//!
//! - packs are attached read-only and `immutable=1`, each with a small page
//!   cache of its own ([`PACK_CACHE_KIB`]); no mmap; foreign keys off;
//! - queries are generated per attached database and their results combined
//!   in Rust, never written against `UNION ALL` views;
//! - a pack that fails the structural check, repeats an attached pack's id,
//!   or does not fit (SQLite attaches at most ten databases) is refused with
//!   a reason and Talos runs without it.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OpenFlags};

use super::pack::{EXTENSION, PackInfo, Refusal, file_uri, read_info};
use super::schema::{self, SCHEMA_VERSION, sql_error};
use crate::Error;

/// The page cache of each attached pack, in KiB. The spike measured no
/// latency difference between 256 KiB and 64 MiB with the file in the OS
/// cache, and 2.8 MB per pack at SQLite's default.
pub const PACK_CACHE_KIB: u32 = 256;

/// How many prepared statements the connection keeps: a few per query kind
/// and attached database.
const STATEMENT_CACHE: usize = 96;

/// The name `user.db` answers to in sources and results.
pub(crate) const USER: &str = "user";

/// One database queries run against, in the order they are asked: `user.db`
/// first, then the packs in the order they were attached.
#[derive(Debug, Clone)]
pub(crate) struct Db {
    /// The schema name in SQL (`main`, `p1`…).
    pub(crate) schema: String,
    /// `user`, or the pack's id.
    pub(crate) name: String,
}

/// The knowledge store: `user.db` and the attached packs, queried as one
/// (the dictionary queries are in `query.rs`, sense choice in `senses.rs`).
/// Read-only: what the user teaches is written by the user store. Handlers
/// reach it through [`crate::Context::knowledge`].
#[derive(Debug)]
pub struct Knowledge {
    conn: Mutex<Connection>,
    pub(crate) dbs: Vec<Db>,
    packs: Vec<PackInfo>,
    refused: Vec<Refusal>,
    next_alias: usize,
}

impl Knowledge {
    fn configure(conn: &Connection) -> Result<(), Error> {
        conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
        conn.execute_batch(&format!(
            "PRAGMA foreign_keys = OFF; PRAGMA main.cache_size = -{PACK_CACHE_KIB};"
        ))
        .map_err(|e| sql_error(&e))
    }

    fn with_connection(conn: Connection, dbs: Vec<Db>) -> Result<Self, Error> {
        Self::configure(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            dbs,
            packs: Vec::new(),
            refused: Vec::new(),
            next_alias: 1,
        })
    }

    /// A store with nothing in it: no `user.db`, no pack. Every lookup
    /// finds nothing.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] if SQLite cannot open an in-memory database.
    pub fn empty() -> Result<Self, Error> {
        let conn = Connection::open_in_memory().map_err(|e| sql_error(&e))?;
        Self::with_connection(conn, Vec::new())
    }

    /// The store of a data directory: its `user.db` when there is one at
    /// the current schema version (read-only; migrating it is the user
    /// store's job, done before this), and every `*.talospack` in its
    /// `packs/` directory, attached in file-name order. What cannot be
    /// used is refused with a reason ([`Knowledge::refused`]), never an
    /// error.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] only if SQLite cannot open a database at all.
    pub fn open_dir(dir: &Path) -> Result<Self, Error> {
        let user = dir.join(super::userdb::FILE_NAME);
        let mut refused = Vec::new();
        let mut store = match Self::open_user(&user) {
            Ok(Some(store)) => store,
            Ok(None) => Self::empty()?,
            Err(error) => {
                refused.push(Refusal {
                    path: user,
                    reason: error.to_string(),
                });
                Self::empty()?
            }
        };
        store.refused = refused;
        for path in pack_files(&dir.join("packs")) {
            store.attach(&path);
        }
        Ok(store)
    }

    fn open_user(path: &Path) -> Result<Option<Self>, Error> {
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| sql_error(&e))?;
        let version = schema::version(&conn, "main")?;
        if version != SCHEMA_VERSION {
            return Err(Error::Knowledge(format!(
                "user.db is at schema version {version}, not {SCHEMA_VERSION}"
            )));
        }
        let db = Db {
            schema: "main".to_owned(),
            name: USER.to_owned(),
        };
        Self::with_connection(conn, vec![db]).map(Some)
    }

    fn alias(&mut self) -> String {
        let alias = format!("p{}", self.next_alias);
        self.next_alias += 1;
        alias
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `f` with the connection, for the query layer.
    pub(crate) fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.lock())
    }

    /// Checks the attached schema `alias` and keeps it, or detaches it.
    fn adopt(&mut self, alias: String, path: Option<&Path>) -> Result<(), String> {
        let conn = self.conn.get_mut().unwrap_or_else(PoisonError::into_inner);
        let checked = read_info(conn, &alias, path).and_then(|info| {
            if self.packs.iter().any(|p| p.id == info.id) {
                return Err(Error::Knowledge(format!(
                    "a pack with the id `{}` is already attached",
                    info.id
                )));
            }
            conn.execute_batch(&format!("PRAGMA {alias}.cache_size = -{PACK_CACHE_KIB}"))
                .map_err(|e| sql_error(&e))?;
            Ok(info)
        });
        match checked {
            Ok(info) => {
                self.dbs.push(Db {
                    schema: alias,
                    name: info.id.clone(),
                });
                self.packs.push(info);
                Ok(())
            }
            Err(error) => {
                let _ = conn.execute_batch(&format!("DETACH DATABASE {alias}"));
                Err(match error {
                    Error::Knowledge(message) => message,
                    other => other.to_string(),
                })
            }
        }
    }

    /// Attaches a pack file, read-only. Whether it was attached; when not,
    /// the reason is in [`Knowledge::refused`].
    pub fn attach(&mut self, path: &Path) -> bool {
        let alias = self.alias();
        let attached = file_uri(path, "mode=ro&immutable=1").and_then(|uri| {
            let conn = self.conn.get_mut().unwrap_or_else(PoisonError::into_inner);
            conn.execute(&format!("ATTACH DATABASE ?1 AS {alias}"), [uri])
                .map_err(|e| sql_error(&e))
        });
        let result = match attached {
            Ok(_) => self.adopt(alias, Some(path)),
            Err(Error::Knowledge(message)) => Err(message),
            Err(other) => Err(other.to_string()),
        };
        match result {
            Ok(()) => true,
            Err(reason) => {
                self.refused.push(Refusal {
                    path: path.to_path_buf(),
                    reason,
                });
                false
            }
        }
    }

    /// Builds a pack from its source (`knowledge/seed.toml`) into an
    /// in-memory database and attaches it: how tests and golden files get
    /// the seed pack without touching the disk.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when the source does not build, or the pack is
    /// refused (its id is already attached).
    pub fn attach_source(&mut self, source: &str) -> Result<(), Error> {
        let alias = self.alias();
        {
            let conn = self.conn.get_mut().unwrap_or_else(PoisonError::into_inner);
            conn.execute_batch(&format!("ATTACH DATABASE ':memory:' AS {alias}"))
                .map_err(|e| sql_error(&e))?;
            if let Err(error) = super::build::build_into(conn, &alias, source) {
                let _ = conn.execute_batch(&format!("DETACH DATABASE {alias}"));
                return Err(error);
            }
        }
        self.adopt(alias, None).map_err(Error::Knowledge)
    }

    /// A store holding only the pack built from `source`: what engine tests
    /// inject ([`crate::Engine::use_knowledge`]).
    ///
    /// # Errors
    ///
    /// As [`Knowledge::attach_source`].
    pub fn from_source(source: &str) -> Result<Self, Error> {
        let mut store = Self::empty()?;
        store.attach_source(source)?;
        Ok(store)
    }

    /// The attached packs, in the order they are asked.
    #[must_use]
    pub fn packs(&self) -> &[PackInfo] {
        &self.packs
    }

    /// What was found and not attached, with the reason.
    #[must_use]
    pub fn refused(&self) -> &[Refusal] {
        &self.refused
    }

    /// Whether `user.db` is part of the store.
    #[must_use]
    pub fn has_user_db(&self) -> bool {
        self.dbs.iter().any(|d| d.name == USER)
    }

    /// The stores asked, in order: `user`, then each pack's id.
    #[must_use]
    pub fn stores(&self) -> Vec<&str> {
        self.dbs.iter().map(|d| d.name.as_str()).collect()
    }

    /// Creates an empty `user.db`-shaped database in memory as `main`; for
    /// tests of what is read from `user.db`.
    #[cfg(test)]
    pub(crate) fn with_user_tables() -> Result<Self, Error> {
        let mut conn = Connection::open_in_memory().map_err(|e| sql_error(&e))?;
        let tx = conn.transaction().map_err(|e| sql_error(&e))?;
        schema::migrate(&tx, "main", schema::DbKind::User)?;
        tx.commit().map_err(|e| sql_error(&e))?;
        let db = Db {
            schema: "main".to_owned(),
            name: USER.to_owned(),
        };
        Self::with_connection(conn, vec![db])
    }

    /// Runs statements against `main`, for tests that put rows in an
    /// in-memory `user.db`.
    #[cfg(test)]
    pub(crate) fn execute_main(&self, sql: &str) {
        self.lock().execute_batch(sql).unwrap();
    }
}

/// The pack files of a directory, in file-name order; none when it does not
/// exist.
#[must_use]
pub fn pack_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION) && p.is_file())
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::build::build_file;

    const SEED: &str = include_str!("../../../../knowledge/seed.toml");

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("talos-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        dir
    }

    #[test]
    fn packs_in_the_data_directory_are_attached_and_bad_ones_refused() {
        let dir = temp("attach");
        build_file(SEED, &dir.join("packs/a-core.talospack")).unwrap();
        build_file(SEED, &dir.join("packs/b-again.talospack")).unwrap();
        std::fs::write(dir.join("packs/c-junk.talospack"), b"not a database at all").unwrap();
        std::fs::write(dir.join("packs/ignored.txt"), b"x").unwrap();
        let store = Knowledge::open_dir(&dir).unwrap();
        assert_eq!(store.stores(), ["core"]);
        assert!(!store.has_user_db());
        let reasons: Vec<&str> = store.refused().iter().map(|r| r.reason.as_str()).collect();
        assert_eq!(reasons.len(), 2, "{reasons:?}");
        assert!(reasons[0].contains("already attached"), "{reasons:?}");
        assert!(reasons[1].contains("not a database"), "{reasons:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pack_for_another_schema_version_is_refused_with_a_reason() {
        let dir = temp("version");
        let path = dir.join("packs/old.talospack");
        build_file(SEED, &path).unwrap();
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "UPDATE meta SET value = '0' WHERE key = 'schema_version'",
                [],
            )
            .unwrap();
        }
        let store = Knowledge::open_dir(&dir).unwrap();
        assert_eq!(store.packs().len(), 0);
        assert!(
            store.refused()[0].reason.contains("schema version 0"),
            "{:?}",
            store.refused()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_more_packs_than_sqlite_attaches() {
        let dir = temp("many");
        for i in 0..11 {
            let source = SEED.replacen("id = \"core\"", &format!("id = \"core{i:02}\""), 1);
            build_file(&source, &dir.join(format!("packs/{i:02}.talospack"))).unwrap();
        }
        let store = Knowledge::open_dir(&dir).unwrap();
        assert_eq!(store.packs().len(), 10);
        assert_eq!(store.refused().len(), 1);
        assert!(
            store.refused()[0].reason.contains("too many"),
            "{:?}",
            store.refused()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
