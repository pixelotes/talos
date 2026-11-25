//! Where the front ends keep what Talos remembers about its user
//! (preferences, corrections; `talos_core::user`): one file per
//! installation, `user.toml` in the data directory (docs/DESIGN.md section
//! 10). The CLI and the server share it: the "user" is whoever owns the
//! installation, whatever conversation or API `user` a turn comes from
//! (docs/spec/decisions.md, ruling 10).

use std::path::PathBuf;

use talos_core::{Engine, FileStore};

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
/// written until a turn changes a preference. When there is no data
/// directory, or its file cannot be read, the engine keeps what it is told
/// in memory for this run only, and the reason is returned, to be shown.
#[must_use]
pub fn attach_user_store(engine: &mut Engine) -> Option<String> {
    let Some(dir) = data_dir() else {
        return Some("no data directory; preferences last for this run only".to_owned());
    };
    engine
        .use_user_store(Box::new(FileStore::in_dir(&dir)))
        .err()
        .map(|error| format!("{error}; preferences last for this run only"))
}
