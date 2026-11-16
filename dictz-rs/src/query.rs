//! Lookups across all loaded dictionaries — mirrors
//! `dictz/core/query_manager.py`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::babylon::BabylonDictionary;
use crate::dsl::DslDictionary;
use crate::fuzzy;
use crate::index::IndexStore;
use crate::media::MediaResolver;
use crate::stardict::StarDictDictionary;
use crate::{Dictionary, DictionaryEntry, DictzError};

/// File extensions that can be opened.
pub fn supported_extensions() -> &'static [&'static str] {
    &[".bgl", ".dsl", ".ifo"]
}

/// Create the right parser for `path` (`.ifo`, `.dsl`, `.dsl.dz`, `.bgl`).
pub fn open_dictionary(path: &Path) -> Result<Box<dyn Dictionary>, DictzError> {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = |e: &str| name.ends_with(e);
    if ext(".ifo") || ext(".idx") || ext(".dict") || ext(".dict.dz") || ext(".idx.gz") {
        Ok(Box::new(StarDictDictionary::new(path)?))
    } else if ext(".dsl") || ext(".dsl.dz") {
        Ok(Box::new(DslDictionary::new(path)))
    } else if ext(".bgl") {
        Ok(Box::new(BabylonDictionary::new(path)))
    } else {
        Err(DictzError::Unsupported(path.display().to_string()))
    }
}

/// `true` for `.ifo`, `.dsl`, `.dsl.dz` and `.bgl` files.
pub fn is_dictionary_file(path: &Path) -> bool {
    let n = path
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    n.ends_with(".ifo") || n.ends_with(".dsl") || n.ends_with(".dsl.dz") || n.ends_with(".bgl")
}

/// Dictionary files under `folder`, sorted (same rules as the Python
/// edition's `scan_folder`): hidden folders and `*.files` media folders are
/// skipped.
pub fn scan_folder(folder: &Path, recursive: bool) -> Vec<PathBuf> {
    if folder.is_file() {
        return if is_dictionary_file(folder) {
            vec![folder.to_path_buf()]
        } else {
            Vec::new()
        };
    }
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for e in entries {
            let name = e
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if e.is_dir() {
                if recursive && !name.starts_with('.') && !name.to_lowercase().ends_with(".files") {
                    stack.push(e);
                }
            } else if is_dictionary_file(&e) {
                out.push(e);
            }
        }
    }
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    out
}

/// Options for [`QueryManager::restore_from_index`].
#[derive(Debug, Clone, Default)]
pub struct RestoreOptions {
    /// Only these dictionaries.
    pub names: Option<Vec<String>>,
    /// Skip disabled dictionaries.
    pub enabled_only: bool,
    /// Rebuild the index of files modified since they were indexed.
    pub reindex_changed: bool,
}

/// A suggestion under the search box.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Headword.
    pub word: String,
    /// `true` when it came from fuzzy matching (not a prefix hit).
    pub fuzzy: bool,
}

struct Loaded {
    dict: Box<dyn Dictionary>,
    enabled: bool,
    media: MediaResolver,
}

/// Coordinates lookups across dictionaries, in the user's order.
pub struct QueryManager {
    dicts: Vec<Loaded>,
    /// Optional FTS5 index (suggestions + persistence of the dictionary list).
    pub index: Option<IndexStore>,
    headword_cache: Option<Vec<String>>,
}

/// Suggestions are filled up with fuzzy matches when fewer prefix hits exist.
pub const FUZZY_FILL_BELOW: usize = 10;

impl QueryManager {
    /// New manager, optionally backed by an index.
    pub fn new(index: Option<IndexStore>) -> Self {
        Self {
            dicts: Vec::new(),
            index,
            headword_cache: None,
        }
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.dicts
            .iter()
            .position(|d| d.dict.metadata().name == name)
    }

    /// Load a dictionary file and append it to the list.
    /// Returns its name.
    pub fn add_dictionary(
        &mut self,
        path: impl AsRef<Path>,
        reindex: bool,
    ) -> Result<String, DictzError> {
        let path = path.as_ref();
        let mut dict = open_dictionary(path)?;
        dict.load()?;
        // the list gives every file a unique name ("Name (2)")
        let name = match self.index.as_mut() {
            Some(index) => {
                let name = index.register_dictionary(dict.as_ref())?;
                dict.metadata_mut().name = name.clone();
                if reindex || !index.is_indexed(&name)? {
                    index.index_dictionary(dict.as_ref())?;
                }
                name
            }
            None => {
                let base = dict.metadata().name.clone();
                let me = crate::registry::norm_path(path);
                let taken: std::collections::HashSet<String> = self
                    .dicts
                    .iter()
                    .filter(|d| crate::registry::norm_path(&d.dict.metadata().file_path) != me)
                    .map(|d| d.dict.metadata().name.clone())
                    .collect();
                let name = crate::registry::unique_name(&base, &taken);
                dict.metadata_mut().name = name.clone();
                name
            }
        };
        if let Some(i) = self.position(&name) {
            self.dicts.remove(i);
        }
        let media = MediaResolver::for_dictionary(path);
        self.dicts.push(Loaded {
            dict,
            enabled: true,
            media,
        });
        self.persist_order();
        self.headword_cache = None;
        Ok(name)
    }

    /// Reload every dictionary recorded in the index (start-up).
    /// Returns (loaded names, errors).
    pub fn restore_from_index(&mut self) -> (Vec<String>, Vec<String>) {
        self.restore_with(&RestoreOptions::default(), |_, _, _| {})
    }

    /// Reload dictionaries recorded in the index. `progress(i, total, name)`
    /// is called before each one.
    pub fn restore_with(
        &mut self,
        opts: &RestoreOptions,
        mut progress: impl FnMut(usize, usize, &str),
    ) -> (Vec<String>, Vec<String>) {
        let rows = match self.index.as_ref().map(|i| i.list_dictionaries(false)) {
            Some(Ok(r)) => r,
            Some(Err(e)) => return (Vec::new(), vec![e.to_string()]),
            None => return (Vec::new(), Vec::new()),
        };
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|r| {
                opts.names
                    .as_ref()
                    .map(|n| n.contains(&r.name))
                    .unwrap_or(true)
            })
            .filter(|r| !opts.enabled_only || r.enabled)
            .filter(|r| self.position(&r.name).is_none())
            .collect();
        let mut ok = Vec::new();
        let mut errors = Vec::new();
        let total = rows.len();
        for (i, row) in rows.into_iter().enumerate() {
            progress(i, total, &row.name);
            match Self::open_file(&row.path) {
                Ok(mut dict) => {
                    dict.metadata_mut().name = row.name.clone();
                    if opts.reindex_changed && row.changed() {
                        if let Some(index) = self.index.as_mut() {
                            let _ = index.index_dictionary(dict.as_ref());
                        }
                    }
                    ok.push(dict.metadata().name.clone());
                    self.attach(dict, &row.path, row.enabled);
                }
                Err(e) => errors.push(format!("{}: {}", row.path.display(), e)),
            }
        }
        (ok, errors)
    }

    /// Parse a dictionary file (safe to call from a worker thread).
    pub fn open_file(path: &Path) -> Result<Box<dyn Dictionary>, DictzError> {
        let mut dict = open_dictionary(path)?;
        dict.load()?;
        Ok(dict)
    }

    /// Append an already-loaded dictionary (no indexing).
    pub fn attach(&mut self, dict: Box<dyn Dictionary>, path: &Path, enabled: bool) {
        let name = dict.metadata().name.clone();
        if let Some(i) = self.position(&name) {
            self.dicts.remove(i);
        }
        let media = MediaResolver::for_dictionary(path);
        self.dicts.push(Loaded {
            dict,
            enabled,
            media,
        });
        self.headword_cache = None;
    }

    /// Write list positions and enabled flags to the index.
    pub fn save_order(&self) {
        self.persist_order();
    }

    /// Unload a dictionary and drop it from the index.
    pub fn remove_dictionary(&mut self, name: &str) -> Result<(), DictzError> {
        if let Some(i) = self.position(name) {
            self.dicts[i].dict.close();
            self.dicts.remove(i);
        }
        if let Some(index) = self.index.as_ref() {
            index.remove_dictionary(name)?;
        }
        self.persist_order();
        self.headword_cache = None;
        Ok(())
    }

    /// Enable or disable a dictionary.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(i) = self.position(name) {
            self.dicts[i].enabled = enabled;
        }
        if let Some(index) = self.index.as_ref() {
            let _ = index.set_enabled(name, enabled);
        }
        self.headword_cache = None;
    }

    /// `true` when loaded and enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.position(name)
            .map(|i| self.dicts[i].enabled)
            .unwrap_or(false)
    }

    /// Move a dictionary up (`delta < 0`) or down in the list.
    pub fn move_dictionary(&mut self, name: &str, delta: isize) {
        let Some(i) = self.position(name) else { return };
        let j = (i as isize + delta).clamp(0, self.dicts.len() as isize - 1) as usize;
        if i != j {
            let d = self.dicts.remove(i);
            self.dicts.insert(j, d);
            self.persist_order();
            self.headword_cache = None;
        }
    }

    /// Write the loaded order into the list; dictionaries that are in the
    /// list but not loaded (missing file, failed) keep their place.
    fn persist_order(&self) {
        let Some(index) = self.index.as_ref() else {
            return;
        };
        let all: Vec<String> = index
            .list_dictionaries(false)
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.name)
            .collect();
        let order: Vec<String> = self
            .dicts
            .iter()
            .map(|d| d.dict.metadata().name.clone())
            .collect();
        let loaded: std::collections::HashSet<&String> = order.iter().collect();
        let known: std::collections::HashSet<&String> = all.iter().collect();
        let mut it = order.iter().filter(|n| known.contains(n));
        let mut merged: Vec<String> = all
            .iter()
            .map(|n| {
                if loaded.contains(n) {
                    it.next().cloned().unwrap_or_else(|| n.clone())
                } else {
                    n.clone()
                }
            })
            .collect();
        merged.extend(order.iter().filter(|n| !known.contains(n)).cloned());
        if merged != all {
            let _ = index.set_order(&merged);
        }
    }

    /// Names in list order.
    pub fn loaded_names(&self) -> Vec<String> {
        self.dicts
            .iter()
            .map(|d| d.dict.metadata().name.clone())
            .collect()
    }

    /// A loaded dictionary by name.
    pub fn dictionary(&self, name: &str) -> Option<&dyn Dictionary> {
        self.position(name).map(|i| self.dicts[i].dict.as_ref())
    }

    /// Media resolver of a loaded dictionary.
    pub fn media(&self, name: &str) -> Option<&MediaResolver> {
        self.position(name).map(|i| &self.dicts[i].media)
    }

    /// Path of a loaded dictionary.
    pub fn path_of(&self, name: &str) -> Option<PathBuf> {
        self.dictionary(name)
            .map(|d| d.metadata().file_path.clone())
    }

    /// Total headwords in enabled dictionaries.
    pub fn word_count(&self) -> usize {
        self.dicts
            .iter()
            .filter(|d| d.enabled)
            .map(|d| d.dict.metadata().word_count)
            .sum()
    }

    /// Look `word` up in every enabled dictionary, in list order.
    pub fn search(&self, word: &str, exact: bool) -> Vec<DictionaryEntry> {
        self.search_in(word, exact, None, None)
    }

    /// [`QueryManager::search`] limited to `dictionaries` and `max_articles`.
    pub fn search_in(
        &self,
        word: &str,
        exact: bool,
        dictionaries: Option<&[String]>,
        max_articles: Option<usize>,
    ) -> Vec<DictionaryEntry> {
        if word.trim().is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for d in self.dicts.iter().filter(|d| d.enabled) {
            if let Some(names) = dictionaries {
                if !names.iter().any(|n| *n == d.dict.metadata().name) {
                    continue;
                }
            }
            if max_articles.map(|m| out.len() >= m).unwrap_or(false) {
                break;
            }
            out.extend(d.dict.lookup(word, exact));
        }
        if let Some(m) = max_articles {
            out.truncate(m);
        }
        out
    }

    /// Enabled dictionaries that have `word` (exact or case-insensitive).
    pub fn found_in(&self, word: &str) -> Vec<String> {
        self.dicts
            .iter()
            .filter(|d| d.enabled && !d.dict.lookup(word, true).is_empty())
            .map(|d| d.dict.metadata().name.clone())
            .collect()
    }

    /// A random article from one (or any enabled) dictionary.
    pub fn random_entry(&self, dictionary: Option<&str>) -> Option<DictionaryEntry> {
        let pool: Vec<&Loaded> = self
            .dicts
            .iter()
            .filter(|d| match dictionary {
                Some(n) => d.dict.metadata().name == n,
                None => d.enabled,
            })
            .filter(|d| !d.dict.words().is_empty())
            .collect();
        if pool.is_empty() {
            return None;
        }
        let mut r = random_u64();
        let d = pool[(r % pool.len() as u64) as usize];
        r = r.rotate_left(29) ^ 0x9E37_79B9_7F4A_7C15;
        let words = d.dict.words().sorted();
        let w = &words[(r % words.len() as u64) as usize];
        d.dict.lookup(w, true).into_iter().next()
    }

    fn enabled_names(&self) -> Vec<String> {
        self.dicts
            .iter()
            .filter(|d| d.enabled)
            .map(|d| d.dict.metadata().name.clone())
            .collect()
    }

    /// Headwords of all enabled dictionaries (list order, each dictionary
    /// sorted, duplicates removed) — the fuzzy candidate list.
    pub fn all_headwords(&mut self) -> &[String] {
        if self.headword_cache.is_none() {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for d in self.dicts.iter().filter(|d| d.enabled) {
                for w in d.dict.words().sorted() {
                    if seen.insert(w.as_str()) {
                        out.push(w.clone());
                    }
                }
            }
            self.headword_cache = Some(out);
        }
        self.headword_cache.as_deref().unwrap_or(&[])
    }

    /// "Did you mean" candidates for `word`, best first.
    pub fn fuzzy(&mut self, word: &str, limit: usize, cutoff: f64) -> Vec<(String, f64)> {
        let word = word.trim().to_string();
        if word.is_empty() {
            return Vec::new();
        }
        let words = self.all_headwords();
        fuzzy::extract(&word, words.iter().map(String::as_str), limit, cutoff)
    }

    /// Suggestions for the search box: prefix hits ranked closest-first,
    /// filled up with fuzzy matches when there are fewer than
    /// [`FUZZY_FILL_BELOW`].
    pub fn suggest(&mut self, prefix: &str, limit: usize) -> Vec<Suggestion> {
        self.suggest_with(
            prefix,
            limit,
            FUZZY_FILL_BELOW,
            fuzzy::DEFAULT_LIMIT,
            fuzzy::DEFAULT_CUTOFF,
        )
    }

    /// [`QueryManager::suggest`] with explicit fuzzy settings.
    pub fn suggest_with(
        &mut self,
        prefix: &str,
        limit: usize,
        fuzzy_fill_below: usize,
        fuzzy_limit: usize,
        fuzzy_cutoff: f64,
    ) -> Vec<Suggestion> {
        let prefix = prefix.trim().to_string();
        if prefix.is_empty() {
            return Vec::new();
        }
        let enabled = self.enabled_names();
        let mut words: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        if let Some(index) = self.index.as_ref() {
            if let Ok(rows) = index.search_headword(&prefix, limit, Some(&enabled)) {
                for (w, _) in rows {
                    if seen.insert(w.clone()) {
                        words.push(w);
                    }
                }
            }
        } else {
            'outer: for d in self.dicts.iter().filter(|d| d.enabled) {
                for e in d.dict.lookup(&prefix, false) {
                    if seen.insert(e.headword.clone()) {
                        words.push(e.headword);
                        if words.len() >= limit {
                            break 'outer;
                        }
                    }
                }
            }
        }
        fuzzy::rank(&prefix, &mut words);
        let mut out: Vec<Suggestion> = words
            .into_iter()
            .map(|word| Suggestion { word, fuzzy: false })
            .collect();
        if out.len() < fuzzy_fill_below {
            for (w, _) in self.fuzzy(&prefix, fuzzy_limit, fuzzy_cutoff) {
                if out.len() >= limit {
                    break;
                }
                if seen.insert(w.clone()) {
                    out.push(Suggestion {
                        word: w,
                        fuzzy: true,
                    });
                }
            }
        }
        out
    }

    /// Close everything.
    pub fn close_all(&mut self) {
        for d in &mut self.dicts {
            d.dict.close();
        }
        self.dicts.clear();
        self.headword_cache = None;
    }
}

fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stardict::write_stardict;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dictz-qm-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn search_suggest_fuzzy_order_restore() {
        let d = tmp("all");
        let a =
            write_stardict(&d.join("a"), "A", &[("apple", "elma"), ("apply", "uygula")]).unwrap();
        let b =
            write_stardict(&d.join("b"), "B", &[("apple", "Apfel"), ("banana", "muz")]).unwrap();
        let db = d.join("index.db");
        let mut qm = QueryManager::new(Some(IndexStore::open(&db).unwrap()));
        qm.add_dictionary(&a, false).unwrap();
        qm.add_dictionary(&b, false).unwrap();
        assert_eq!(qm.search("apple", true).len(), 2);
        assert_eq!(qm.loaded_names(), vec!["A", "B"]);
        qm.move_dictionary("B", -1);
        assert_eq!(qm.loaded_names(), vec!["B", "A"]);
        assert_eq!(qm.search("apple", true)[0].definition, "Apfel");
        qm.set_enabled("B", false);
        assert_eq!(qm.search("apple", true).len(), 1);

        let s = qm.suggest("app", 30);
        assert_eq!(s[0].word, "apple");
        assert!(!s[0].fuzzy);
        let f = qm.fuzzy("aple", 5, 60.0);
        assert_eq!(f[0].0, "apple");
        assert!(qm
            .suggest("aplpe", 30)
            .iter()
            .any(|s| s.fuzzy && s.word == "apple"));

        // restart: same order and enabled flags come back
        drop(qm);
        let mut qm2 = QueryManager::new(Some(IndexStore::open(&db).unwrap()));
        let (ok, errors) = qm2.restore_from_index();
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(ok, vec!["B", "A"]);
        assert!(!qm2.is_enabled("B"));
        assert!(qm2.is_enabled("A"));
    }

    #[test]
    fn unsupported_extension() {
        assert!(matches!(
            open_dictionary(Path::new("x.txt")),
            Err(DictzError::Unsupported(_))
        ));
    }
}
