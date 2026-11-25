//! What Talos keeps about its user across restarts: the preferences (name,
//! place, units; docs/spec/state.md) and the user-correction table ("when I
//! say X, I mean Y"; ADR 0008). One set per installation (DESIGN.md, non-
//! goals; docs/spec/decisions.md ruling 10).
//!
//! The engine holds the data in memory and hands every change to a
//! [`UserStore`]. Its home is the SQLite `user.db` (`knowledge::UserDb`,
//! with the `knowledge` feature; DESIGN.md section 7). Without that feature
//! the durable form is the small TOML file of v0.2 ([`FileStore`]), replaced
//! whole and atomically on each change, whose few fields map one to one
//! onto `user.db`'s tables; `UserDb` imports it once. [`MemoryStore`], the
//! engine's default, keeps nothing on disk: tests and golden files use it,
//! so they never read or write the user's home.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use crate::normalise::normalise;
use crate::skill::{Preferences, Units};
use crate::{Error, IntentId};

/// One entry of the user-correction table: a message that **is** `phrase`
/// (after [`Correction::key`]) is handled as `intent` with `slots`, at the
/// rule stage and ahead of the patterns (ADR 0008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    /// The phrase, in its key form ([`Correction::key`]).
    pub phrase: String,
    /// What it means.
    pub intent: IntentId,
    /// Slot values, as text; each is read by its slot's extractor when the
    /// phrase is used, like a rule's capture.
    pub slots: BTreeMap<String, String>,
}

/// Characters a phrase key does not keep at either end: punctuation and
/// quotes ("Tira." is "tira").
const KEY_EDGES: &[char] = &[
    '.', ',', ';', ':', '!', '?', '¡', '¿', '"', '\'', '«', '»', '(', ')', '…',
];

impl Correction {
    /// A correction; the phrase is reduced to its key form.
    #[must_use]
    pub fn new(
        phrase: &str,
        intent: impl Into<String>,
        slots: BTreeMap<String, String>,
    ) -> Correction {
        Correction {
            phrase: Correction::key(phrase),
            intent: IntentId(intent.into()),
            slots,
        }
    }

    /// The form a message and a phrase are compared in: the match form
    /// (lower case, no accents, spaces collapsed), without punctuation or
    /// quotes at either end (docs/spec/knowledge.md, teach.phrase).
    #[must_use]
    pub fn key(text: &str) -> String {
        normalise(text)
            .folded
            .trim_matches(|c: char| c.is_whitespace() || KEY_EDGES.contains(&c))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Everything kept about the user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserData {
    /// Name, place and units.
    pub preferences: Preferences,
    /// The user-correction table, in the order the entries were made; a
    /// later entry for the same phrase replaces an earlier one.
    pub corrections: Vec<Correction>,
}

/// The version of the file format written by [`UserData::to_toml`].
const VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUser {
    version: u32,
    #[serde(default)]
    preferences: RawPreferences,
    #[serde(default)]
    correction: Vec<RawCorrection>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawPreferences {
    name: Option<String>,
    place: Option<String>,
    units: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCorrection {
    phrase: String,
    intent: String,
    #[serde(default)]
    slots: BTreeMap<String, String>,
}

/// A TOML basic string.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML key: bare when it can be, quoted otherwise.
fn key(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        text.to_owned()
    } else {
        quoted(text)
    }
}

impl UserData {
    /// Nothing kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.preferences.is_empty() && self.corrections.is_empty()
    }

    /// The correction for a message, if the message is one of the phrases.
    #[must_use]
    pub fn correction(&self, message: &str) -> Option<&Correction> {
        let key = Correction::key(message);
        if key.is_empty() {
            return None;
        }
        self.corrections.iter().rev().find(|c| c.phrase == key)
    }

    /// Adds a correction, replacing any for the same phrase.
    pub fn correct(&mut self, correction: Correction) {
        self.corrections.retain(|c| c.phrase != correction.phrase);
        self.corrections.push(correction);
    }

    /// Removes the correction for a phrase; whether there was one.
    pub fn uncorrect(&mut self, phrase: &str) -> bool {
        let key = Correction::key(phrase);
        let before = self.corrections.len();
        self.corrections.retain(|c| c.phrase != key);
        self.corrections.len() != before
    }

    /// The file form: TOML, every value a string, in a fixed order, so the
    /// same data is always the same bytes.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut out = String::from(
            "# What Talos remembers about its user. Written by Talos; the whole\n\
             # file is replaced on every change.\n",
        );
        let _ = writeln!(out, "version = {VERSION}");
        let p = &self.preferences;
        if !p.is_empty() {
            out.push_str("\n[preferences]\n");
            if let Some(name) = p.name() {
                let _ = writeln!(out, "name = {}", quoted(name));
            }
            if let Some(place) = p.place() {
                let _ = writeln!(out, "place = {}", quoted(place));
            }
            if let Some(units) = p.units() {
                let _ = writeln!(out, "units = {}", quoted(units.as_str()));
            }
        }
        for c in &self.corrections {
            out.push_str("\n[[correction]]\n");
            let _ = writeln!(out, "phrase = {}", quoted(&c.phrase));
            let _ = writeln!(out, "intent = {}", quoted(c.intent.as_str()));
            if !c.slots.is_empty() {
                let slots: Vec<String> = c
                    .slots
                    .iter()
                    .map(|(k, v)| format!("{} = {}", key(k), quoted(v)))
                    .collect();
                let _ = writeln!(out, "slots = {{ {} }}", slots.join(", "));
            }
        }
        out
    }

    /// Reads the file form.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the text is not TOML of this format, has
    /// another version, or names units that do not exist.
    pub fn from_toml(text: &str) -> Result<UserData, Error> {
        let raw: RawUser = toml::from_str(text).map_err(|e| Error::Store(e.to_string()))?;
        if raw.version != VERSION {
            return Err(Error::Store(format!(
                "version {} is not {VERSION}, the one this Talos reads",
                raw.version
            )));
        }
        let mut preferences = Preferences::NONE;
        if let Some(name) = raw.preferences.name {
            preferences = preferences.with_name(name);
        }
        if let Some(place) = raw.preferences.place {
            preferences = preferences.with_place(place);
        }
        if let Some(units) = raw.preferences.units {
            let units = Units::parse(&units)
                .ok_or_else(|| Error::Store(format!("unknown units `{units}`")))?;
            preferences = preferences.with_units(units);
        }
        let mut data = UserData {
            preferences,
            corrections: Vec::new(),
        };
        for c in raw.correction {
            data.correct(Correction::new(&c.phrase, c.intent, c.slots));
        }
        Ok(data)
    }
}

/// Where the user's data is kept between runs.
pub trait UserStore: Send + Sync + fmt::Debug {
    /// What was kept, or nothing when nothing was.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when it exists but cannot be read.
    fn load(&self) -> Result<UserData, Error>;

    /// Keeps `data`, replacing what was kept.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when it cannot be written; what was kept before is
    /// then unchanged.
    fn save(&self, data: &UserData) -> Result<(), Error>;
}

/// A store in memory only: nothing survives the process, nothing touches
/// the disk. The engine's default, and what tests use.
#[derive(Debug, Default)]
pub struct MemoryStore {
    data: Mutex<UserData>,
}

impl MemoryStore {
    /// A store holding `data` to begin with.
    #[must_use]
    pub fn new(data: UserData) -> Self {
        Self {
            data: Mutex::new(data),
        }
    }

    /// What was saved last.
    #[must_use]
    pub fn saved(&self) -> UserData {
        self.data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl UserStore for MemoryStore {
    fn load(&self) -> Result<UserData, Error> {
        Ok(self.saved())
    }

    fn save(&self, data: &UserData) -> Result<(), Error> {
        data.clone_into(
            &mut self
                .data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        Ok(())
    }
}

/// A store in one TOML file ([`UserData::to_toml`]). A missing file is no
/// data. Saving writes a sibling temporary file, flushes it to disk and
/// renames it over the old one, so a crash leaves the old file or the new
/// one, never half of either.
#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
}

/// The file's name inside the data directory.
pub const FILE_NAME: &str = "user.toml";

impl FileStore {
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

    fn error(&self, e: &io::Error) -> Error {
        Error::Store(format!("{}: {e}", self.path.display()))
    }
}

impl UserStore for FileStore {
    fn load(&self) -> Result<UserData, Error> {
        match fs::read_to_string(&self.path) {
            Ok(text) => UserData::from_toml(&text)
                .map_err(|e| Error::Store(format!("{}: {e}", self.path.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(UserData::default()),
            Err(e) => Err(self.error(&e)),
        }
    }

    fn save(&self, data: &UserData) -> Result<(), Error> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|e| self.error(&e))?;
        }
        let mut temporary = self.path.clone().into_os_string();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        let write = || -> io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(data.to_toml().as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)
        };
        write().map_err(|e| {
            let _ = fs::remove_file(&temporary);
            self.error(&e)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            BTreeMap::from([("place".to_owned(), "Tokio".to_owned())]),
        ));
        data
    }

    #[test]
    fn the_file_form_round_trips_byte_for_byte() {
        let data = sample();
        let text = data.to_toml();
        assert!(text.contains("phrase = \"tira\""), "{text}");
        let back = UserData::from_toml(&text).unwrap();
        assert_eq!(back, data);
        assert_eq!(back.to_toml(), text);
        assert_eq!(
            UserData::from_toml("version = 1\n").unwrap(),
            UserData::default()
        );
        assert!(UserData::from_toml("version = 2\n").is_err());
        assert!(UserData::from_toml("version = 1\n[preferences]\nunits = \"cubits\"\n").is_err());
        assert!(UserData::from_toml("version = 1\ncolour = \"red\"\n").is_err());
    }

    #[test]
    fn phrases_match_the_whole_message_after_normalisation() {
        let data = sample();
        assert_eq!(
            data.correction("Tira.").map(|c| c.intent.as_str()),
            Some("random.dice")
        );
        assert!(data.correction("  TIRA  ").is_some());
        assert!(data.correction("tira dos veces").is_none());
        assert!(data.correction("La de Tokió?").is_some());
        assert!(data.correction("?").is_none());
        let mut data = data;
        data.correct(Correction::new("tira", "random.coin", BTreeMap::new()));
        assert_eq!(data.corrections.len(), 2);
        assert_eq!(
            data.correction("tira").map(|c| c.intent.as_str()),
            Some("random.coin")
        );
        assert!(data.uncorrect("TIRA"));
        assert!(!data.uncorrect("tira"));
    }

    #[test]
    fn a_file_store_writes_whole_files_and_reads_them_back() {
        let dir = std::env::temp_dir().join(format!("talos-user-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = FileStore::in_dir(&dir.join("nested"));
        assert_eq!(store.load().unwrap(), UserData::default());
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap(), sample());
        store.save(&UserData::default()).unwrap();
        assert_eq!(store.load().unwrap(), UserData::default());
        let entries: Vec<_> = fs::read_dir(dir.join("nested")).unwrap().collect();
        assert_eq!(entries.len(), 1, "no temporary file is left behind");
        fs::write(store.path(), "not toml [").unwrap();
        assert!(store.load().is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_memory_store_keeps_the_last_save() {
        let store = MemoryStore::new(sample());
        assert_eq!(store.load().unwrap(), sample());
        store.save(&UserData::default()).unwrap();
        assert!(store.saved().is_empty());
    }
}
