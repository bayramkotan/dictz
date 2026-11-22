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
//! dictz = { path = "../dictz-rs", default-features = false }
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

pub mod articles;
pub mod audioplay;
pub mod autostart;
pub mod babylon;
#[cfg(feature = "center")]
pub mod center;
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
pub mod loading;
pub mod media;
pub mod paths;
pub mod query;
pub mod registry;
pub mod scancache;
pub mod scantext;
pub mod selftest;
pub mod settings;
pub mod shortcuts;
pub mod stardict;
pub mod suggest;
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

/// Synonym pairs without duplicates, ordered by (lower-cased synonym,
/// synonym, headword) — the order kept in the article store. Same as
/// `sort_synonyms` in `dictz/parsers/base.py`.
pub fn sort_synonyms(pairs: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut v: Vec<(String, (String, String))> =
        pairs.into_iter().map(|p| (p.0.to_lowercase(), p)).collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|(_, p)| p).collect()
}

/// The headwords `word` is a synonym of: the pairs whose synonym is exactly
/// `word`, otherwise those of the first synonym equal to it ignoring case.
/// Each headword once, in pair order.
pub fn synonym_targets(pairs: &[(String, String)], word: &str) -> Vec<String> {
    let mut hits: Vec<&str> = pairs
        .iter()
        .filter(|(s, _)| s == word)
        .map(|(_, t)| t.as_str())
        .collect();
    if hits.is_empty() {
        let low = word.to_lowercase();
        if let Some((first, _)) = pairs.iter().find(|(s, _)| s.to_lowercase() == low) {
            hits = pairs
                .iter()
                .filter(|(s, _)| s == first)
                .map(|(_, t)| t.as_str())
                .collect();
        }
    }
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|t| seen.insert(*t))
        .map(str::to_string)
        .collect()
}

/// Headwords plus the synonyms that are not headwords, sorted
/// case-insensitively (headwords first among equals).
pub fn merge_search_words(headwords: &[String], pairs: &[(String, String)]) -> Vec<String> {
    if pairs.is_empty() {
        return headwords.to_vec();
    }
    let have: std::collections::HashSet<&str> = headwords.iter().map(String::as_str).collect();
    let mut seen = std::collections::HashSet::new();
    let mut all: Vec<(String, String)> = headwords
        .iter()
        .map(|w| (w.to_lowercase(), w.clone()))
        .collect();
    for (s, _) in pairs {
        if !have.contains(s.as_str()) && seen.insert(s.as_str()) {
            all.push((s.to_lowercase(), s.clone()));
        }
    }
    all.sort_by(|a, b| a.0.cmp(&b.0)); // stable: headwords first among equals
    all.into_iter().map(|(_, w)| w).collect()
}

/// Trait every format parser implements.
///
/// Mirrors the Python `dictz.parsers.base.Dictionary` class, including the
/// lookup rules: exact match first, then the first case-insensitive match,
/// then the synonyms (StarDict `.syn`), then (when `exact == false`) up to
/// 50 case-insensitive prefix matches.
pub trait Dictionary: Send {
    /// Read the dictionary into memory. Idempotent.
    fn load(&mut self) -> Result<(), DictzError>;

    /// `true` once [`Dictionary::load`] succeeded.
    fn is_loaded(&self) -> bool;

    /// Dictionary metadata.
    fn metadata(&self) -> &DictionaryMetadata;

    /// Mutable metadata (the list renames a dictionary to its unique name).
    fn metadata_mut(&mut self) -> &mut DictionaryMetadata;

    /// Headword table (insertion order + case-insensitive sorted order).
    fn words(&self) -> &WordTable;

    /// Definition of an indexed headword.
    fn definition(&self, headword: &str) -> Option<String>;

    /// Release the in-memory data.
    fn close(&mut self);

    /// Close and answer nothing until replaced: its files are being rebuilt
    /// by a worker (on Windows an open file cannot be replaced). Only the
    /// article store needs more than [`Dictionary::close`].
    fn suspend(&mut self) {
        self.close();
    }

    /// `(synonym, headword)` pairs in [`sort_synonyms`] order (StarDict's
    /// `.syn`: "went" → "go"); empty for other formats.
    fn synonyms(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Headwords and synonyms: what the index, the word list and "did you
    /// mean" offer ([`merge_search_words`]).
    fn search_words(&self) -> Vec<String> {
        merge_search_words(self.words().sorted(), &self.synonyms())
    }

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
        if hits.is_empty() {
            let syn = self.synonyms();
            if !syn.is_empty() {
                hits = synonym_targets(&syn, word);
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
