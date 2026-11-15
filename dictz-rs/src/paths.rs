//! Per-user data folder — the same folder the Python edition uses, so both
//! editions share the dictionary list, index, history and favorites.

use std::path::PathBuf;

/// Application folder name.
pub const APP_NAME: &str = "dictz";
/// SQLite index file name (shared with the Python edition).
pub const INDEX_DB: &str = "dictz_index.db";
/// SQLite history / favorites file name (shared with the Python edition).
pub const HISTORY_DB: &str = "dictz_history.db";
/// Preferences file of the Rust edition.
pub const PREFS_FILE: &str = "dictz-rs.ini";

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Windows: `%APPDATA%\dictz`; macOS: `~/Library/Application Support/dictz`;
/// Linux: `$XDG_DATA_HOME/dictz` or `~/.local/share/dictz`.
pub fn user_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DICTZ_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData").join("Roaming"));
        base.join(APP_NAME)
    } else if cfg!(target_os = "macos") {
        home()
            .join("Library")
            .join("Application Support")
            .join(APP_NAME)
    } else {
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local").join("share"));
        base.join(APP_NAME)
    }
}
