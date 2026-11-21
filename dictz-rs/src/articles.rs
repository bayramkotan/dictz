//! Article store: a dictionary's articles saved once, read on demand.
//!
//! Same file format and rules as the Python edition
//! (`dictz/storage/articles.py`); a store written by one edition is read by
//! the other.
//!
//! Parsing a dictionary is done once: the articles go into
//! `articles/<id>.db` in the data folder (`id` = the dictionary's id in
//! `dictionaries.json`). Later starts open that file and read a few metadata
//! rows; an article is read (and unpacked) only when it is shown — like
//! GoldenDict. The store is rebuilt when the dictionary's files change
//! ([`source_stamp`]) or [`STORE_VERSION`] changes.
//!
//! Tables:
//!
//! ```text
//! meta(key TEXT PRIMARY KEY, value TEXT)
//! articles(id INTEGER PRIMARY KEY, word TEXT, folded TEXT, z INTEGER, body BLOB)
//! ```
//!
//! Rows are in the parser's sorted order, so `ORDER BY id` is that order;
//! `folded` is the lower-cased word; `z = 1` means `body` is zlib-compressed
//! UTF-8, `z = 0` plain UTF-8.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OpenFlags};

use crate::{Dictionary, DictionaryEntry, DictionaryMetadata, DictzError, WordTable};

/// Format version; a store with another version is rebuilt.
pub const STORE_VERSION: &str = "1";
/// Folder inside the data folder.
pub const STORE_DIR: &str = "articles";
/// Prefix lookups return at most this many articles.
pub const PREFIX_LIMIT: usize = 50;

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE articles (
    id INTEGER PRIMARY KEY,
    word TEXT NOT NULL,
    folded TEXT NOT NULL,
    z INTEGER NOT NULL,
    body BLOB NOT NULL
);
";
const INDEXES: &str = "
CREATE UNIQUE INDEX articles_word ON articles(word);
CREATE INDEX articles_folded ON articles(folded, id);
";

/// The files a dictionary is read from (a StarDict is three files).
pub fn source_files(path: &Path) -> Vec<PathBuf> {
    let is_ifo = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("ifo"))
        .unwrap_or(false);
    let mut out = vec![path.to_path_buf()];
    if is_ifo {
        let base = path.with_extension("");
        let base = base.to_string_lossy();
        for group in [[".idx", ".idx.gz"], [".dict", ".dict.dz"]] {
            for ext in group {
                let q = PathBuf::from(format!("{}{}", base, ext));
                if q.is_file() {
                    out.push(q);
                    break;
                }
            }
        }
    }
    out
}

/// `name:size:mtime_ns` of every source file, joined by `;`.
pub fn source_stamp(path: &Path) -> String {
    source_files(path)
        .iter()
        .map(|f| {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match std::fs::metadata(f) {
                Ok(m) => {
                    let ns = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    format!("{}:{}:{}", name, m.len(), ns)
                }
                Err(_) => format!("{}:-:-", name),
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// `DATA/articles/<id>.db`.
pub fn store_path(data_dir: &Path, dict_id: &str) -> PathBuf {
    data_dir.join(STORE_DIR).join(format!("{}.db", dict_id))
}

fn pack(text: &str) -> (i64, Vec<u8>) {
    let raw = text.as_bytes();
    if raw.len() < 64 {
        return (0, raw.to_vec());
    }
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    if enc.write_all(raw).is_ok() {
        if let Ok(packed) = enc.finish() {
            if packed.len() < raw.len() {
                return (1, packed);
            }
        }
    }
    (0, raw.to_vec())
}

fn unpack(z: i64, body: &[u8]) -> String {
    if z != 0 {
        let mut out = Vec::new();
        if flate2::read::ZlibDecoder::new(body)
            .read_to_end(&mut out)
            .is_ok()
        {
            return String::from_utf8_lossy(&out).into_owned();
        }
        return String::new();
    }
    String::from_utf8_lossy(body).into_owned()
}

fn open_ro(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

/// The store's metadata, or `None` when it is missing or unreadable.
pub fn read_meta(path: &Path) -> Option<HashMap<String, String>> {
    if !path.is_file() {
        return None;
    }
    let con = open_ro(path).ok()?;
    let mut st = con.prepare("SELECT key, value FROM meta").ok()?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .ok()?;
    rows.collect::<Result<HashMap<_, _>, _>>().ok()
}

/// `true` when `store` was built from the current `source` files.
pub fn is_fresh(store: &Path, source: &Path) -> bool {
    match read_meta(store) {
        Some(m) => {
            m.get("version").map(String::as_str) == Some(STORE_VERSION)
                && m.get("stamp") == Some(&source_stamp(source))
        }
        None => false,
    }
}

/// Write every article of a loaded `dict` to `store`; returns the count.
/// Written to a temporary file first and renamed, so a half-written store is
/// never used.
pub fn build(dict: &dyn Dictionary, store: &Path) -> Result<usize, DictzError> {
    build_with(dict, store, &mut |_, _| {})
}

/// [`build`] that calls `progress(done, total)` every few thousand articles.
pub fn build_with(
    dict: &dyn Dictionary,
    store: &Path,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<usize, DictzError> {
    let dir = store.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&dir)?;
    let name = store
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{}.{}.tmp", name, std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let res = (|| -> Result<usize, DictzError> {
        let mut con = Connection::open(&tmp)?;
        con.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;")?;
        con.execute_batch(SCHEMA)?;
        let tx = con.transaction()?;
        let mut count = 0usize;
        let total = dict.words().len();
        {
            let mut ins =
                tx.prepare("INSERT INTO articles (word, folded, z, body) VALUES (?1, ?2, ?3, ?4)")?;
            for w in dict.words().sorted() {
                let Some(defi) = dict.definition(w) else {
                    continue;
                };
                let (z, body) = pack(&defi);
                ins.execute(params![w, w.to_lowercase(), z, body])?;
                count += 1;
                if count % 5000 == 0 {
                    progress(count, total.max(count));
                }
            }
        }
        tx.execute_batch(INDEXES)?;
        let m = dict.metadata();
        let mut meta = vec![
            ("version", STORE_VERSION.to_string()),
            ("stamp", source_stamp(&m.file_path)),
            ("builder", "rust".to_string()),
            ("name", m.name.clone()),
            ("description", m.description.clone()),
            ("author", m.author.clone()),
            ("source_lang", m.source_language.clone()),
            ("target_lang", m.target_language.clone()),
            ("word_count", count.to_string()),
            ("format", m.format.clone()),
        ];
        meta.sort();
        for (k, v) in meta {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        tx.commit()?;
        drop(con);
        Ok(count)
    })();
    match res {
        Ok(n) => {
            std::fs::rename(&tmp, store)?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Remove temp files of stores whose writer was stopped (the program quit
/// while a dictionary was being read). Only files older than an hour: a
/// younger one may belong to the other edition, writing right now.
pub fn remove_stale_temp(data_dir: &Path) {
    let Ok(dir) = std::fs::read_dir(data_dir.join(STORE_DIR)) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with('.') && name.ends_with(".tmp")) {
            continue;
        }
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() >= 3600);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// A dictionary served from its article store (nothing parsed).
pub struct StoredDictionary {
    store: PathBuf,
    // opened on first use: a dictionary opened from dictionaries.json alone
    // reads nothing until it is searched
    con: std::cell::OnceCell<Option<Connection>>,
    meta: DictionaryMetadata,
    words: OnceLock<WordTable>,
    empty: WordTable,
    // being rebuilt by a worker: closed, answers nothing until replaced
    suspended: bool,
}

impl StoredDictionary {
    /// Open `store`, the article store of the dictionary file `source`.
    pub fn open(store: &Path, source: &Path) -> Result<Self, DictzError> {
        let m = read_meta(store).ok_or_else(|| {
            DictzError::Malformed(format!("{}: no article store", store.display()))
        })?;
        let get = |k: &str| m.get(k).cloned().unwrap_or_default();
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = get("name");
        let meta = DictionaryMetadata {
            name: if name.is_empty() { stem } else { name },
            source_language: get("source_lang"),
            target_language: get("target_lang"),
            description: get("description"),
            author: get("author"),
            word_count: get("word_count").parse().unwrap_or(0),
            file_path: source.to_path_buf(),
            format: get("format"),
        };
        let con = std::cell::OnceCell::new();
        let _ = con.set(Some(open_ro(store)?));
        Ok(Self {
            store: store.to_path_buf(),
            con,
            meta,
            words: OnceLock::new(),
            empty: WordTable::new(),
            suspended: false,
        })
    }

    /// A dictionary from `dictionaries.json` alone: neither the store nor the
    /// dictionary file is opened until the first lookup.
    pub fn from_meta(store: &Path, meta: DictionaryMetadata) -> Self {
        Self {
            store: store.to_path_buf(),
            con: std::cell::OnceCell::new(),
            meta,
            words: OnceLock::new(),
            empty: WordTable::new(),
            suspended: false,
        }
    }

    fn con(&self) -> Option<&Connection> {
        if self.suspended {
            return None;
        }
        self.con.get_or_init(|| open_ro(&self.store).ok()).as_ref()
    }

    /// The store file.
    pub fn store(&self) -> &Path {
        &self.store
    }

    fn rows(&self, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Vec<(String, i64, Vec<u8>)> {
        let Some(con) = self.con() else {
            return Vec::new();
        };
        let Ok(mut st) = con.prepare_cached(sql) else {
            return Vec::new();
        };
        st.query_map(args, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|it| it.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn to_entries(&self, rows: Vec<(String, i64, Vec<u8>)>) -> Vec<DictionaryEntry> {
        rows.into_iter()
            .map(|(w, z, b)| DictionaryEntry {
                headword: w,
                definition: unpack(z, &b),
                dictionary_id: self.meta.name.clone(),
                score: 1.0,
            })
            .collect()
    }
}

impl Dictionary for StoredDictionary {
    fn load(&mut self) -> Result<(), DictzError> {
        self.con()
            .map(|_| ())
            .ok_or_else(|| DictzError::Malformed(format!("{}: cannot open", self.store.display())))
    }

    fn is_loaded(&self) -> bool {
        true // opened on first use
    }

    fn metadata(&self) -> &DictionaryMetadata {
        &self.meta
    }

    fn metadata_mut(&mut self) -> &mut DictionaryMetadata {
        &mut self.meta
    }

    fn words(&self) -> &WordTable {
        let Some(con) = self.con() else {
            return &self.empty;
        };
        self.words.get_or_init(|| {
            let mut t = WordTable::new();
            if let Ok(mut st) = con.prepare("SELECT word FROM articles ORDER BY id") {
                if let Ok(rows) = st.query_map([], |r| r.get::<_, String>(0)) {
                    for w in rows.flatten() {
                        t.insert(&w);
                    }
                }
            }
            t.finish();
            t
        })
    }

    fn definition(&self, headword: &str) -> Option<String> {
        self.rows(
            "SELECT word, z, body FROM articles WHERE word = ?1",
            &[&headword],
        )
        .into_iter()
        .next()
        .map(|(_, z, b)| unpack(z, &b))
    }

    fn close(&mut self) {
        self.con = std::cell::OnceCell::new();
        self.words = OnceLock::new();
    }

    fn suspend(&mut self) {
        self.suspended = true;
        self.close();
    }

    fn lookup(&self, word: &str, exact: bool) -> Vec<DictionaryEntry> {
        let mut rows = self.rows(
            "SELECT word, z, body FROM articles WHERE word = ?1",
            &[&word],
        );
        let folded = word.to_lowercase();
        if rows.is_empty() {
            rows = self.rows(
                "SELECT word, z, body FROM articles WHERE folded = ?1 ORDER BY id LIMIT 1",
                &[&folded],
            );
        }
        if !exact && rows.is_empty() {
            let hi = format!("{}\u{10ffff}", folded);
            let limit = PREFIX_LIMIT as i64;
            rows = self.rows(
                "SELECT word, z, body FROM articles WHERE folded >= ?1 AND folded < ?2 \
                 ORDER BY id LIMIT ?3",
                &[&folded, &hi, &limit],
            );
            rows.retain(|(w, _, _)| w.to_lowercase().starts_with(&folded));
        }
        self.to_entries(rows)
    }

    fn entries(&self) -> Vec<DictionaryEntry> {
        let rows = self.rows("SELECT word, z, body FROM articles ORDER BY id", &[]);
        self.to_entries(rows)
    }
}

/// Open a dictionary from its article store; parse it and build the store
/// first when the store is missing, old or `rebuild` is set. Returns
/// `(dictionary, built)`. When the store cannot be written (read-only data
/// folder) the parsed dictionary is returned instead.
pub fn open_cached(
    path: &Path,
    data_dir: &Path,
    rebuild: bool,
) -> Result<(Box<dyn Dictionary>, bool), DictzError> {
    open_cached_row(path, data_dir, rebuild, None).map(|(d, built, _)| (d, built))
}

/// [`open_cached`] with the dictionary's row of `dictionaries.json`: when
/// its files still have the stamp kept there, nothing but the list is read
/// (the third value is then `true`).
pub fn open_cached_row(
    path: &Path,
    data_dir: &Path,
    rebuild: bool,
    row: Option<&crate::index::DictionaryRow>,
) -> Result<(Box<dyn Dictionary>, bool, bool), DictzError> {
    let store = store_path(data_dir, &crate::registry::path_id(path));
    if let Some(r) = row.filter(|r| !rebuild && !r.stamp.is_empty()) {
        if r.stamp == source_stamp(path) && store.is_file() {
            // the fast path: everything from dictionaries.json, nothing opened
            let meta = DictionaryMetadata {
                name: r.name.clone(),
                source_language: r.source_language.clone(),
                target_language: r.target_language.clone(),
                description: r.description.clone(),
                author: r.author.clone(),
                word_count: r.word_count.max(0) as usize,
                file_path: path.to_path_buf(),
                format: r.format.clone(),
            };
            return Ok((
                Box::new(StoredDictionary::from_meta(&store, meta)),
                false,
                true,
            ));
        }
    }
    let (d, built) = open_cached_full(path, &store, rebuild)?;
    Ok((d, built, false))
}

fn open_cached_full(
    path: &Path,
    store: &Path,
    rebuild: bool,
) -> Result<(Box<dyn Dictionary>, bool), DictzError> {
    let store = store.to_path_buf();
    if !rebuild && is_fresh(&store, path) {
        if let Ok(d) = StoredDictionary::open(&store, path) {
            return Ok((Box::new(d), false));
        }
    }
    let mut parsed = crate::query::QueryManager::open_file(path)?;
    if build(parsed.as_ref(), &store).is_err() {
        return Ok((parsed, true));
    }
    parsed.close();
    match StoredDictionary::open(&store, path) {
        Ok(d) => Ok((Box::new(d), true)),
        Err(_) => Ok((crate::query::QueryManager::open_file(path)?, true)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip() {
        for s in ["", "kısa", &"uzun metin ".repeat(40)] {
            let (z, b) = pack(s);
            assert_eq!(unpack(z, &b), s);
        }
        assert_eq!(pack(&"a".repeat(200)).0, 1);
    }
}
