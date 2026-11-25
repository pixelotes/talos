//! Where the front ends keep what Talos remembers about its user
//! (preferences, corrections; `talos_core::user`) and find the knowledge
//! packs: the data directory (docs/DESIGN.md section 10). With the
//! `knowledge` feature the user's data is `user.db` (an existing
//! `user.toml` is imported once and renamed), and every
//! `packs/*.talospack` is attached; without it, `user.toml` as before. The
//! CLI and the server share them: the "user" is whoever owns the
//! installation, whatever conversation or API `user` a turn comes from
//! (docs/spec/decisions.md, ruling 10).

use std::path::PathBuf;

use talos_core::Engine;

/// The data directory: `$TALOS_DATA_DIR` when set, else
/// `$XDG_DATA_HOME/talos`, else `$HOME/.local/share/talos`. `None` when
/// none of these is known, or when `TALOS_DATA_DIR` is set but empty: then
/// nothing is kept between runs.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name);
    if let Some(dir) = var("TALOS_DATA_DIR") {
        return (!dir.is_empty()).then(|| PathBuf::from(dir));
    }
    if let Some(dir) = var("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir).join("talos"));
    }
    var("HOME")
        .filter(|d| !d.is_empty())
        .map(|home| PathBuf::from(home).join(".local/share/talos"))
}

/// Gives the engine its user store in the data directory. Nothing is
/// written until a turn changes a preference (except the one-time import
/// of `user.toml` into `user.db`). When there is no data directory, or its
/// store cannot be read, the engine keeps what it is told in memory for
/// this run only, and the reason is returned, to be shown.
#[must_use]
pub fn attach_user_store(engine: &mut Engine) -> Option<String> {
    let Some(dir) = data_dir() else {
        return Some("no data directory; preferences last for this run only".to_owned());
    };
    #[cfg(feature = "knowledge")]
    let store = talos_core::UserDb::in_dir(&dir);
    #[cfg(not(feature = "knowledge"))]
    let store = talos_core::FileStore::in_dir(&dir);
    engine
        .use_user_store(Box::new(store))
        .err()
        .map(|error| format!("{error}; preferences last for this run only"))
}

/// Gives the engine the knowledge store of the data directory: `user.db`
/// and the packs in `packs/`. Packs that cannot be used are left out; the
/// reasons are returned, to be shown. Call it after [`attach_user_store`],
/// which brings `user.db` to the current schema. Without a data directory,
/// or with neither `user.db` nor a pack in it, the engine gets no store:
/// nothing of SQLite is opened.
#[cfg(feature = "knowledge")]
#[must_use]
pub fn attach_knowledge(engine: &mut Engine) -> Vec<String> {
    use talos_core::knowledge::{Knowledge, USER_DB, pack_files};
    let Some(dir) = data_dir() else {
        return Vec::new();
    };
    if !dir.join(USER_DB).exists() && pack_files(&dir.join("packs")).is_empty() {
        return Vec::new();
    }
    match Knowledge::open_dir(&dir) {
        Ok(knowledge) => {
            let warnings = knowledge
                .refused()
                .iter()
                // An unreadable user.db was reported by the user store.
                .filter(|r| r.path.extension().is_some_and(|e| e != "db"))
                .map(|r| format!("pack {} left out: {}", r.path.display(), r.reason))
                .collect();
            engine.use_knowledge(knowledge);
            warnings
        }
        Err(error) => vec![format!("{error}; no knowledge for this run")],
    }
}
