//! `user.db`: the preferences and the user-correction table, kept in the
//! user's SQLite store from v0.3 on (DESIGN.md section 7), next to what the
//! user teaches.
//!
//! The guarantees of the `user.toml` it replaces hold:
//!
//! - **nothing is written on a turn that changes nothing**: the engine
//!   saves only when its data changed, and the file is not even created
//!   until the first save. Reading opens it read-only.
//! - **an unreadable store is never overwritten**: [`UserDb::load`] fails,
//!   and the engine then keeps the user's data in memory for the run.
//! - each save is one transaction, so a crash leaves the old data or the
//!   new, never half of either.
//!
//! **Migration from `user.toml`.** When there is no `user.db` and there is a
//! `user.toml` beside it, the file is read once, written to a new `user.db`,
//! and renamed `user.toml.migrated` (never deleted). A `user.toml` that
//! cannot be read stops the migration: no `user.db` is created, and the
//! error is reported like any unreadable store. Once `user.db` exists a
//! `user.toml` beside it is ignored.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use super::schema::{self, DbKind, SCHEMA_VERSION, sql_error};
use crate::Error;
use crate::skill::{Preferences, Units};
use crate::user::{Correction, FILE_NAME as TOML_NAME, FileStore, UserData, UserStore};

/// The file's name inside the data directory.
pub const FILE_NAME: &str = "user.db";

/// What a migrated `user.toml` is renamed to.
pub const MIGRATED_NAME: &str = "user.toml.migrated";

/// The user store in `user.db` (see the module documentation).
#[derive(Debug, Clone)]
pub struct UserDb {
    path: PathBuf,
}

fn store_error(path: &Path, e: &impl std::fmt::Display) -> Error {
    Error::Store(format!("{}: {e}", path.display()))
}

impl UserDb {
    /// A store at `path`; nothing is read until [`UserStore::load`].
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// A store at [`FILE_NAME`] inside a data directory.
    #[must_use]
    pub fn in_dir(dir: &Path) -> Self {
        Self::new(dir.join(FILE_NAME))
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn error(&self, e: &impl std::fmt::Display) -> Error {
        store_error(&self.path, e)
    }

    /// Opens the file for writing, creating it and its directory, and
    /// brings its schema forward.
    fn open_writable(&self) -> Result<Connection, Error> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|e| self.error(&e))?;
        }
        let mut conn = Connection::open(&self.path).map_err(|e| self.error(&e))?;
        conn.execute_batch("PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL;")
            .map_err(|e| self.error(&e))?;
        let tx = conn.transaction().map_err(|e| self.error(&e))?;
        schema::migrate(&tx, "main", DbKind::User).map_err(|e| self.error(&e))?;
        tx.commit().map_err(|e| self.error(&e))?;
        Ok(conn)
    }

    /// Reads the data of an existing file, migrating its schema forward
    /// first when it is older (a write, at start, once per version).
    fn read(&self) -> Result<UserData, Error> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| self.error(&e))?;
        let version = schema::version(&conn, "main").map_err(|e| self.error(&e))?;
        if version > SCHEMA_VERSION {
            return Err(self.error(&format!(
                "schema version {version} is newer than {SCHEMA_VERSION}, the one this Talos knows"
            )));
        }
        let conn = if version < SCHEMA_VERSION {
            drop(conn);
            self.open_writable()?
        } else {
            conn
        };
        if schema::application_id(&conn, "main").map_err(|e| self.error(&e))?
            != schema::APPLICATION_ID
        {
            return Err(self.error(&"not a Talos user database"));
        }
        read_data(&conn).map_err(|e| self.error(&e))
    }

    /// The `user.toml` beside the database.
    fn toml_path(&self) -> PathBuf {
        self.path.with_file_name(TOML_NAME)
    }

    /// Imports `user.toml` into a new `user.db` and renames it. `None` when
    /// there is no `user.toml`.
    fn migrate_toml(&self) -> Result<Option<UserData>, Error> {
        let toml = self.toml_path();
        if !toml.exists() {
            return Ok(None);
        }
        let data = FileStore::new(&toml).load()?;
        self.save(&data)?;
        let migrated = self.path.with_file_name(MIGRATED_NAME);
        if !migrated.exists() {
            fs::rename(&toml, &migrated).map_err(|e| store_error(&toml, &e))?;
        }
        Ok(Some(data))
    }
}

/// Reads preferences and corrections from a user database.
fn read_data(conn: &Connection) -> Result<UserData, Error> {
    let mut preferences = Preferences::NONE;
    let mut statement = conn
        .prepare("SELECT key, value FROM preference ORDER BY key")
        .map_err(|e| sql_error(&e))?;
    let rows: Vec<(String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| sql_error(&e))?
        .collect::<Result<_, _>>()
        .map_err(|e| sql_error(&e))?;
    for (key, value) in rows {
        preferences = match key.as_str() {
            "name" => preferences.with_name(value),
            "place" => preferences.with_place(value),
            "units" => preferences.with_units(
                Units::parse(&value)
                    .ok_or_else(|| Error::Store(format!("unknown units `{value}`")))?,
            ),
            other => return Err(Error::Store(format!("unknown preference `{other}`"))),
        };
    }
    let mut data = UserData {
        preferences,
        corrections: Vec::new(),
    };
    let mut corrections = conn
        .prepare("SELECT id, phrase, intent FROM correction ORDER BY id")
        .map_err(|e| sql_error(&e))?;
    let mut slots = conn
        .prepare("SELECT name, value FROM correction_slot WHERE correction_id = ?1 ORDER BY name")
        .map_err(|e| sql_error(&e))?;
    let rows: Vec<(i64, String, String)> = corrections
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| sql_error(&e))?
        .collect::<Result<_, _>>()
        .map_err(|e| sql_error(&e))?;
    for (id, phrase, intent) in rows {
        let values: BTreeMap<String, String> = slots
            .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| sql_error(&e))?
            .collect::<Result<_, _>>()
            .map_err(|e| sql_error(&e))?;
        data.correct(Correction::new(&phrase, intent, values));
    }
    Ok(data)
}

/// Replaces preferences and corrections with `data`, in one transaction.
fn write_data(conn: &mut Connection, data: &UserData) -> Result<(), Error> {
    let tx = conn.transaction().map_err(|e| sql_error(&e))?;
    tx.execute_batch(
        "DELETE FROM correction_slot; DELETE FROM correction; DELETE FROM preference;",
    )
    .map_err(|e| sql_error(&e))?;
    let p = &data.preferences;
    let values = [
        ("name", p.name()),
        ("place", p.place()),
        ("units", p.units().map(Units::as_str)),
    ];
    for (key, value) in values {
        if let Some(value) = value {
            tx.execute(
                "INSERT INTO preference (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .map_err(|e| sql_error(&e))?;
        }
    }
    for (i, c) in data.corrections.iter().enumerate() {
        let id = i64::try_from(i + 1).unwrap_or(i64::MAX);
        tx.execute(
            "INSERT INTO correction (id, phrase, intent) VALUES (?1, ?2, ?3)",
            params![id, c.phrase, c.intent.as_str()],
        )
        .map_err(|e| sql_error(&e))?;
        for (name, value) in &c.slots {
            tx.execute(
                "INSERT INTO correction_slot (correction_id, name, value) VALUES (?1, ?2, ?3)",
                params![id, name, value],
            )
            .map_err(|e| sql_error(&e))?;
        }
    }
    tx.commit().map_err(|e| sql_error(&e))
}

impl UserStore for UserDb {
    fn load(&self) -> Result<UserData, Error> {
        if self.path.exists() {
            return self.read();
        }
        Ok(self.migrate_toml()?.unwrap_or_default())
    }

    fn save(&self, data: &UserData) -> Result<(), Error> {
        let mut conn = self.open_writable()?;
        write_data(&mut conn, data).map_err(|e| self.error(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("talos-userdb-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> UserData {
        let mut data = UserData {
            preferences: Preferences::NONE
                .with_name("Dr. \"Sam\" O'Neil")
                .with_place("Nueva York")
                .with_units(Units::Imperial),
            corrections: Vec::new(),
        };
        data.correct(Correction::new("¡Tira!", "random.dice", BTreeMap::new()));
        data.correct(Correction::new(
            "la de Tokio",
            "timezones.time_in",
            BTreeMap::from([
                ("place".to_owned(), "Tokio".to_owned()),
                ("a".to_owned(), "b".to_owned()),
            ]),
        ));
        data.correct(Correction::new("tira", "random.coin", BTreeMap::new()));
        data
    }

    #[test]
    fn nothing_is_created_until_the_first_save() {
        let dir = temp("lazy");
        let store = UserDb::in_dir(&dir.join("nested"));
        assert_eq!(store.load().unwrap(), UserData::default());
        assert!(!dir.exists(), "reading created something");
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap(), sample());
        store.save(&UserData::default()).unwrap();
        assert_eq!(store.load().unwrap(), UserData::default());
        let names: Vec<_> = fs::read_dir(dir.join("nested"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["user.db"], "no journal or temporary file is left");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reading_writes_nothing() {
        let dir = temp("read");
        let store = UserDb::in_dir(&dir);
        store.save(&sample()).unwrap();
        let before = fs::read(store.path()).unwrap();
        for _ in 0..3 {
            assert_eq!(store.load().unwrap(), sample());
        }
        assert_eq!(fs::read(store.path()).unwrap(), before);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_toml_is_migrated_once_and_kept_renamed() {
        let dir = temp("migrate");
        let toml = FileStore::in_dir(&dir);
        toml.save(&sample()).unwrap();
        let text = fs::read_to_string(toml.path()).unwrap();
        let store = UserDb::in_dir(&dir);
        assert_eq!(store.load().unwrap(), sample());
        assert!(!toml.path().exists());
        assert_eq!(
            fs::read_to_string(dir.join(MIGRATED_NAME)).unwrap(),
            text,
            "the old file is renamed, not changed"
        );
        // Later starts read user.db; a user.toml put back is ignored.
        fs::write(toml.path(), "version = 1\n").unwrap();
        assert_eq!(store.load().unwrap(), sample());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_store_is_reported_and_never_overwritten() {
        let dir = temp("bad");
        fs::create_dir_all(&dir).unwrap();
        // A user.toml that cannot be read: no user.db is made.
        fs::write(dir.join(TOML_NAME), "not toml [").unwrap();
        let store = UserDb::in_dir(&dir);
        assert!(store.load().is_err());
        assert!(!store.path().exists());
        assert!(dir.join(TOML_NAME).exists());
        // A user.db that is not a database.
        fs::write(store.path(), b"garbage garbage garbage").unwrap();
        assert!(store.load().is_err());
        assert_eq!(fs::read(store.path()).unwrap(), b"garbage garbage garbage");
        // A user.db from a newer Talos.
        fs::remove_file(store.path()).unwrap();
        store.save(&sample()).unwrap();
        {
            let conn = Connection::open(store.path()).unwrap();
            conn.execute(
                "UPDATE meta SET value = '99' WHERE key = 'schema_version'",
                [],
            )
            .unwrap();
        }
        let error = store.load().unwrap_err();
        assert!(error.to_string().contains("newer"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }
}
