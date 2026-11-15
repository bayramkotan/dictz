//! # dictz
//!
//! Rust port of [bayramkotan/dictz](https://github.com/bayramkotan/dictz) — a
//! cross-platform multi-format offline dictionary.
//!
//! This is the v0.0.5 scaffold: metadata types, a semver-tracked VERSION
//! constant, and a stable public surface (`Dictionary`, `DictionaryEntry`,
//! `DictionaryMetadata`) that the native StarDict / DSL / Babylon parsers
//! will fill in over the next few point releases.
//!
//! The Python edition (`pip install dictz`) is the full, shipping version;
//! the Rust port exists to benchmark, learn, and eventually deliver a
//! single-static-binary alternative with the same dictionary-format support.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

/// Current crate version, mirroring the Python package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A single headword + definition pair returned from a dictionary lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryEntry {
    /// The headword being defined (case preserved).
    pub headword: String,
    /// The definition body. May be plain text or HTML depending on the format.
    pub definition: String,
    /// Name of the dictionary this entry came from (populated by the manager).
    pub dictionary_id: String,
    /// Match score — 1.0 for exact, <1.0 for fuzzy matches.
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
    /// Source language (free-form; usually an ISO code or an English name).
    pub source_language: String,
    /// Target language (free-form).
    pub target_language: String,
    /// Short description from the dictionary's own header.
    pub description: String,
    /// Dictionary author / publisher.
    pub author: String,
    /// Headword count.
    pub word_count: usize,
    /// Format identifier — "stardict", "dsl", "babylon", etc.
    pub format: String,
}

/// Trait every format parser will implement.
///
/// Mirrors the Python `dictz.parsers.base.Dictionary` ABC so the two
/// implementations evolve together.
pub trait Dictionary {
    /// Load headers + index into memory. Idempotent.
    fn load(&mut self) -> Result<(), DictzError>;

    /// Look up a headword. Returns an empty vector when nothing matches.
    fn lookup(&self, word: &str, exact: bool) -> Vec<DictionaryEntry>;

    /// Dictionary metadata — safe to call before `load()`.
    fn metadata(&self) -> &DictionaryMetadata;
}

/// Errors every parser may emit.
#[derive(Debug)]
pub enum DictzError {
    /// Dictionary file not found on disk.
    NotFound(String),
    /// File exists but is not a valid dictionary of the expected format.
    Malformed(String),
    /// Underlying I/O error.
    Io(std::io::Error),
}

impl fmt::Display for DictzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictzError::NotFound(path) => write!(f, "dictionary file not found: {}", path),
            DictzError::Malformed(msg) => write!(f, "malformed dictionary: {}", msg),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        assert!(!VERSION.is_empty());
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION {} is not major.minor.patch", VERSION);
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
    fn dictionary_metadata_defaults() {
        let m = DictionaryMetadata::default();
        assert!(m.name.is_empty());
        assert_eq!(m.word_count, 0);
    }

    #[test]
    fn error_display() {
        let e = DictzError::NotFound("foo.ifo".into());
        assert_eq!(format!("{}", e), "dictionary file not found: foo.ifo");
    }
}
