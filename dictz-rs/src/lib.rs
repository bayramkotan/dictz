//! # dictz
//!
//! Rust edition of [bayramkotan/dictz](https://github.com/bayramkotan/dictz):
//! a cross-platform, multi-format offline dictionary.
//!
//! The crate ships both a library and the `dictz` desktop application.
//! It has the same features as the Python edition (`pip install dictz`) and is
//! released with the same version number from the same tag.
//!
//! * Formats: StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`),
//!   Babylon (`.bgl`) — all parsed natively.
//! * [`index::IndexStore`]: SQLite FTS5 headword index (same schema and file
//!   as the Python edition, so both editions share one dictionary list).
//! * [`history::HistoryStore`]: lookup history and favorites.
//! * [`fuzzy`]: "did you mean" matching, scores identical to RapidFuzz `fuzz.ratio`.
//! * [`media::MediaResolver`]: pictures and sounds referenced by DSL / StarDict
//!   articles (`*.dsl.files/`, `*.dsl.files.zip`, `res/`).
//!
//! Library-only use (no GUI dependencies):
//!
//! ```toml
//! dictz = { version = "0.0.9", default-features = false }
//! ```
//!
//! ```no_run
//! use dictz::query::QueryManager;
//! let mut qm = QueryManager::new(None);
//! qm.add_dictionary("en-tr.ifo", false).unwrap();
//! for e in qm.search("apple", true) {
//!     println!("{}: {}", e.headword, e.definition);
//! }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;
use std::path::{Path, PathBuf};

pub mod babylon;
#[cfg(feature = "cli")]
pub mod cli;
pub mod desktop;
pub mod dictident;
pub mod dsl;
#[cfg(feature = "cli")]
pub mod entry;
pub mod fuzzy;
pub mod history;
pub mod hotkeys;
pub mod html;
pub mod i18n;
pub mod icon;
pub mod index;
pub mod instance;
pub mod media;
pub mod paths;
pub mod query;
pub mod scantext;
pub mod selftest;
pub mod settings;
pub mod shortcuts;
pub mod stardict;
pub mod text;
#[cfg(feature = "update")]
pub mod updater;
mod wordtable;

#[cfg(feature = "gui")]
pub mod gui;

pub use wordtable::WordTable;

/// Current crate version — always identical to the Python package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A single headword + definition pair returned from a dictionary lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryEntry {
    /// The headword being defined (case preserved).
    pub headword: String,
    /// The definition body. Plain text or HTML depending on the format.
    pub definition: String,
    /// Name of the dictionary this entry came from.
    pub dictionary_id: String,
    /// Match score — 1.0 for exact matches.
    pub score: f32,
}

impl DictionaryEntry {
    /// Build an exact-match entry.
    pub fn exact(headword: impl Into<String>, definition: impl Into<String>) -> Self {
        Self {
            headword: headword.into(),
            definition: definition.into(),
            dictionary_id: String::new(),
            score: 1.0,
        }
    }
}

/// Descriptor metadata about a loaded dictionary.
#[derive(Debug, Clone, Default)]
pub struct DictionaryMetadata {
    /// Human-readable dictionary name.
    pub name: String,
    /// Source language (free-form).
    pub source_language: String,
    /// Target language (free-form).
    pub target_language: String,
    /// Short description from the dictionary's own header.
    pub description: String,
    /// Dictionary author / publisher.
    pub author: String,
    /// Headword count.
    pub word_count: usize,
    /// Path of the file that was opened.
    pub file_path: PathBuf,
    /// Format identifier — "stardict", "dsl" or "babylon".
    pub format: String,
}

impl DictionaryMetadata {
    /// `name (src → dst)` when both languages are known, otherwise `name`.
    pub fn display_name(&self) -> String {
        if !self.source_language.is_empty() && !self.target_language.is_empty() {
            format!(
                "{} ({} → {})",
                self.name, self.source_language, self.target_language
            )
        } else {
            self.name.clone()
        }
    }
}

/// Trait every format parser implements.
///
/// Mirrors the Python `dictz.parsers.base.Dictionary` class, including the
/// lookup rules: exact match first, then the first case-insensitive match,
/// then (when `exact == false`) up to 50 case-insensitive prefix matches.
pub trait Dictionary: Send {
    /// Read the dictionary into memory. Idempotent.
    fn load(&mut self) -> Result<(), DictzError>;

    /// `true` once [`Dictionary::load`] succeeded.
    fn is_loaded(&self) -> bool;

    /// Dictionary metadata.
    fn metadata(&self) -> &DictionaryMetadata;

    /// Headword table (insertion order + case-insensitive sorted order).
    fn words(&self) -> &WordTable;

    /// Definition of an indexed headword.
    fn definition(&self, headword: &str) -> Option<String>;

    /// Release the in-memory data.
    fn close(&mut self);

    /// Look up a headword. Returns an empty vector when nothing matches.
    fn lookup(&self, word: &str, exact: bool) -> Vec<DictionaryEntry> {
        let table = self.words();
        let mut hits: Vec<String> = Vec::new();
        if table.contains(word) {
            hits.push(word.to_string());
        }
        if hits.is_empty() {
            if let Some(w) = table.find_case_insensitive(word) {
                hits.push(w.to_string());
            }
        }
        if !exact && hits.is_empty() {
            hits.extend(
                table
                    .prefix_case_insensitive(word, 50)
                    .into_iter()
                    .map(str::to_string),
            );
        }
        hits.into_iter()
            .filter_map(|w| {
                self.definition(&w).map(|d| DictionaryEntry {
                    headword: w,
                    definition: d,
                    dictionary_id: self.metadata().name.clone(),
                    score: 1.0,
                })
            })
            .collect()
    }

    /// All entries, headwords in case-insensitive sorted order.
    fn entries(&self) -> Vec<DictionaryEntry> {
        let name = self.metadata().name.clone();
        self.words()
            .sorted()
            .iter()
            .filter_map(|w| {
                self.definition(w).map(|d| DictionaryEntry {
                    headword: w.clone(),
                    definition: d,
                    dictionary_id: name.clone(),
                    score: 1.0,
                })
            })
            .collect()
    }
}

/// Errors every parser may emit.
#[derive(Debug)]
pub enum DictzError {
    /// Dictionary file not found on disk.
    NotFound(String),
    /// File exists but is not a valid dictionary of the expected format.
    Malformed(String),
    /// No parser is registered for the file extension.
    Unsupported(String),
    /// SQLite error from the index or history database.
    Database(String),
    /// Underlying I/O error.
    Io(std::io::Error),
}

impl fmt::Display for DictzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictzError::NotFound(path) => write!(f, "dictionary file not found: {}", path),
            DictzError::Malformed(msg) => write!(f, "malformed dictionary: {}", msg),
            DictzError::Unsupported(path) => write!(f, "no parser registered for file: {}", path),
            DictzError::Database(msg) => write!(f, "database error: {}", msg),
            DictzError::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for DictzError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DictzError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DictzError {
    fn from(err: std::io::Error) -> Self {
        DictzError::Io(err)
    }
}

impl From<rusqlite::Error> for DictzError {
    fn from(err: rusqlite::Error) -> Self {
        DictzError::Database(err.to_string())
    }
}

/// Read a whole file, transparently gunzipping `.gz` / `.dz` (dictzip) files.
pub fn read_maybe_gzip(path: &Path) -> Result<Vec<u8>, DictzError> {
    let raw = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            DictzError::NotFound(path.display().to_string())
        } else {
            DictzError::Io(e)
        }
    })?;
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .read_to_end(&mut out)
            .map_err(|e| DictzError::Malformed(format!("{}: gzip: {}", path.display(), e)))?;
        Ok(out)
    } else {
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "VERSION {} is not major.minor.patch",
            VERSION
        );
        for p in &parts {
            assert!(p.parse::<u32>().is_ok(), "non-numeric component in VERSION");
        }
    }

    #[test]
    fn dictionary_entry_exact_defaults() {
        let e = DictionaryEntry::exact("apple", "elma");
        assert_eq!(e.headword, "apple");
        assert_eq!(e.definition, "elma");
        assert_eq!(e.score, 1.0);
        assert!(e.dictionary_id.is_empty());
    }

    #[test]
    fn metadata_display_name() {
        let mut m = DictionaryMetadata {
            name: "X".into(),
            ..Default::default()
        };
        assert_eq!(m.display_name(), "X");
        m.source_language = "English".into();
        m.target_language = "Turkish".into();
        assert_eq!(m.display_name(), "X (English → Turkish)");
    }

    #[test]
    fn error_display() {
        let e = DictzError::NotFound("foo.ifo".into());
        assert_eq!(format!("{}", e), "dictionary file not found: foo.ifo");
    }
}
