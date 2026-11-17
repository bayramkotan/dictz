//! SQLite FTS5 headword index — same schema and file as the Python edition
//! (`dictz/storage/indexer.py`).

//!
//! The dictionary list itself (order, enabled flags, unique names) lives in
//! `dictionaries.json` next to the database ([`crate::registry`]); the old
//! SQLite `dictionaries` table is only read once to migrate it.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{params, params_from_iter, Connection};

use crate::registry::{norm_path, unique_name, Entry, Registry, REGISTRY_FILE};
use crate::{Dictionary, DictzError};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS dictionaries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL UNIQUE,
    path         TEXT NOT NULL,
    format       TEXT NOT NULL,
    word_count   INTEGER DEFAULT 0,
    source_lang  TEXT DEFAULT '',
    target_lang  TEXT DEFAULT '',
    description  TEXT DEFAULT '',
    enabled      INTEGER DEFAULT 1,
    sort_order   INTEGER DEFAULT 0
);
CREATE VIRTUAL TABLE IF NOT EXISTS entries USING fts5(
    headword,
    dictionary UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE INDEX IF NOT EXISTS idx_dict_name ON dictionaries(name);
";

/// One dictionary of the list (`dictionaries.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryRow {
    /// FNV-1a 64 of the absolute path, hex.
    pub id: String,
    /// Dictionary name (unique).
    pub name: String,
    /// File the dictionary was loaded from.
    pub path: PathBuf,
    /// "stardict", "dsl" or "babylon".
    pub format: String,
    /// Headword count.
    pub word_count: i64,
    /// Enabled in searches.
    pub enabled: bool,
    /// Position in the dictionary list.
    pub sort_order: i64,
    /// File modification time when indexed (0 = unknown).
    pub mtime: f64,
    /// Source language.
    pub source_language: String,
    /// Target language.
    pub target_language: String,
    /// Description.
    pub description: String,
    /// Headwords are in the index.
    pub indexed: bool,
    /// Source files' stamp when the article store was built.
    pub stamp: String,
    /// Author.
    pub author: String,
    /// Cached icon (`""` not looked for, `"-"` none).
    pub icon: String,
}

impl DictionaryRow {
    fn from_entry(i: usize, e: Entry) -> Self {
        Self {
            id: e.id,
            name: e.name,
            path: PathBuf::from(e.path),
            format: e.format,
            word_count: e.word_count,
            enabled: e.enabled,
            sort_order: i as i64,
            mtime: e.mtime,
            source_language: e.source_lang,
            target_language: e.target_lang,
            description: e.description,
            indexed: e.indexed,
            stamp: e.stamp,
            author: e.author,
            icon: e.icon,
        }
    }
}

impl DictionaryRow {
    /// `true` when the file changed since it was indexed.
    pub fn changed(&self) -> bool {
        self.mtime > 0.0 && (file_mtime(&self.path) - self.mtime).abs() > 1e-6
    }
}

/// Modification time of a file in seconds (0 when missing).
pub fn file_mtime(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// The index database.
pub struct IndexStore {
    conn: Connection,
    registry: RefCell<Registry>,
}

impl IndexStore {
    /// Open (and create) the database at `path`.
    pub fn open(path: &Path) -> Result<Self, DictzError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let reg = path.parent().map(|p| p.join(REGISTRY_FILE));
        Self::with_connection(Connection::open(path)?, reg)
    }

    /// Open the database at `db` with the list at `registry`.
    pub fn open_with_registry(db: &Path, registry: &Path) -> Result<Self, DictzError> {
        if let Some(parent) = db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::with_connection(Connection::open(db)?, Some(registry.to_path_buf()))
    }

    /// In-memory database (tests, `--selftest`).
    pub fn open_in_memory() -> Result<Self, DictzError> {
        Self::with_connection(Connection::open_in_memory()?, None)
    }

    fn with_connection(conn: Connection, reg: Option<PathBuf>) -> Result<Self, DictzError> {
        conn.execute_batch(SCHEMA)?;
        let _ = conn.execute(
            "ALTER TABLE dictionaries ADD COLUMN sort_order INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE dictionaries ADD COLUMN mtime REAL DEFAULT 0",
            [],
        );
        let store = Self {
            conn,
            registry: RefCell::new(Registry::new(reg)),
        };
        store.migrate_to_registry()?;
        Ok(store)
    }

    /// First start after 0.1.0: copy the old SQLite list into
    /// `dictionaries.json` (order kept, duplicate paths dropped).
    fn migrate_to_registry(&self) -> Result<(), DictzError> {
        let mut reg = self.registry.borrow_mut();
        if reg.path.is_none() || reg.exists() {
            return Ok(());
        }
        let indexed: HashSet<String> = {
            let mut st = self
                .conn
                .prepare("SELECT DISTINCT dictionary FROM entries")?;
            let r = st.query_map([], |r| r.get::<_, String>(0))?;
            r.collect::<Result<_, _>>()?
        };
        let mut st = self.conn.prepare(
            "SELECT name, path, format, word_count, enabled, mtime, source_lang, target_lang, description \
             FROM dictionaries ORDER BY sort_order, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, Option<i64>>(4)?.unwrap_or(1) != 0,
                r.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            ))
        })?;
        let mut seen = HashSet::new();
        for row in rows {
            let (name, path, format, wc, enabled, mtime, sl, tl, desc) = row?;
            let p = norm_path(Path::new(&path));
            if !seen.insert(p) {
                continue;
            }
            let mut e = Entry::new(&name, Path::new(&path));
            e.format = format;
            e.word_count = wc;
            e.enabled = enabled;
            e.mtime = mtime;
            e.source_lang = sl;
            e.target_lang = tl;
            e.description = desc;
            e.indexed = indexed.contains(&name);
            reg.items.push(e);
        }
        reg.save()?;
        Ok(())
    }

    /// Add or update `dict` in the list; returns its unique name — the
    /// caller renames the dictionary to it (`Name (2)` when another file
    /// already uses the name). An existing entry keeps its name, place and
    /// enabled flag.
    pub fn register_dictionary(&self, dict: &dyn Dictionary) -> Result<String, DictzError> {
        let m = dict.metadata();
        let mtime = file_mtime(&m.file_path);
        let stamp = crate::articles::source_stamp(&m.file_path);
        Ok(self
            .registry
            .borrow_mut()
            .register(&m.name, &m.file_path, |e| {
                e.format = m.format.clone();
                e.word_count = m.word_count as i64;
                e.source_lang = m.source_language.clone();
                e.target_lang = m.target_language.clone();
                e.description = m.description.clone();
                e.mtime = mtime;
                if !e.stamp.is_empty() && e.stamp != stamp {
                    e.icon.clear(); // the files changed: look for the icon again
                }
                e.stamp = stamp;
                e.author = m.author.clone();
            }))
    }

    /// The dictionary's own icon, cached in `DATA/icons` — looked for once,
    /// then kept in `dictionaries.json` (no folder listing or Babylon file
    /// read at the next start).
    pub fn icon_path(&self, name: &str) -> Option<PathBuf> {
        let row = self.registry.borrow_mut().get(name)?;
        let icons = self.data_dir()?.join("icons");
        if row.icon == "-" {
            return None;
        }
        if !row.icon.is_empty() && icons.join(&row.icon).is_file() {
            return Some(icons.join(&row.icon));
        }
        let mut mark = "-".to_string();
        if let Some((data, ext)) = crate::dictident::icon_bytes(Path::new(&row.path)) {
            let file = format!("{}{}", row.id, ext);
            if std::fs::create_dir_all(&icons).is_ok()
                && std::fs::write(icons.join(&file), data).is_ok()
            {
                mark = file;
            }
        }
        self.registry.borrow_mut().set_icon(name, &mark);
        (mark != "-").then(|| icons.join(mark))
    }

    /// The data folder (where `dictionaries.json` and the article stores
    /// live); `None` for an in-memory index.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.registry
            .borrow()
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    /// The name `path` has (or would get) in the list.
    pub fn unique_name_for(&self, name: &str, path: &Path) -> String {
        let reg = self.registry.borrow();
        if let Some(e) = reg.by_path(path) {
            return e.name.clone();
        }
        let taken: HashSet<String> = reg.items.iter().map(|e| e.name.clone()).collect();
        unique_name(name, &taken)
    }

    /// One row by name.
    pub fn get_dictionary(&self, name: &str) -> Result<Option<DictionaryRow>, DictzError> {
        Ok(self
            .list_dictionaries(false)?
            .into_iter()
            .find(|r| r.name == name))
    }

    /// Headwords of `dictionaries` in that order (each in index order),
    /// duplicates removed — the fuzzy candidate list without loading files.
    pub fn headwords(&self, dictionaries: &[String]) -> Result<Vec<String>, DictzError> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let mut st = self
            .conn
            .prepare("SELECT headword FROM entries WHERE dictionary = ?1 ORDER BY rowid")?;
        for name in dictionaries {
            let rows = st.query_map(params![name], |r| r.get::<_, String>(0))?;
            for w in rows {
                let w = w?;
                if seen.insert(w.clone()) {
                    out.push(w);
                }
            }
        }
        Ok(out)
    }

    /// (dictionaries, indexed headwords).
    pub fn stats(&self) -> Result<(i64, i64), DictzError> {
        let d = self.registry.borrow_mut().rows(false).len() as i64;
        let e: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        Ok((d, e))
    }

    /// The list in order.
    pub fn list_dictionaries(&self, enabled_only: bool) -> Result<Vec<DictionaryRow>, DictzError> {
        Ok(self
            .registry
            .borrow_mut()
            .rows(enabled_only)
            .into_iter()
            .map(|(i, e)| DictionaryRow::from_entry(i, e))
            .collect())
    }

    /// Enable / disable a dictionary.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<(), DictzError> {
        self.registry.borrow_mut().set_enabled(name, enabled);
        Ok(())
    }

    /// Move `name` to position `order` of the list.
    pub fn set_sort_order(&self, name: &str, order: i64) -> Result<(), DictzError> {
        self.registry
            .borrow_mut()
            .move_to(name, order.max(0) as usize);
        Ok(())
    }

    /// Put `names` first in this order; the rest keep their order after them.
    pub fn set_order(&self, names: &[String]) -> Result<(), DictzError> {
        self.registry.borrow_mut().set_order(names);
        Ok(())
    }

    /// Delete a dictionary and its headwords.
    pub fn remove_dictionary(&self, name: &str) -> Result<(), DictzError> {
        self.conn
            .execute("DELETE FROM entries WHERE dictionary = ?1", params![name])?;
        let row = self.registry.borrow_mut().get(name);
        self.registry.borrow_mut().remove(name);
        if let (Some(row), Some(dir)) = (row, self.data_dir()) {
            if self
                .registry
                .borrow()
                .by_path(Path::new(&row.path))
                .is_none()
            {
                let _ = std::fs::remove_file(crate::articles::store_path(&dir, &row.id));
            }
        }
        Ok(())
    }

    /// (Re)build the headword index of `dict`. Returns the number of rows.
    pub fn index_dictionary(&mut self, dict: &dyn Dictionary) -> Result<usize, DictzError> {
        let name = self.register_dictionary(dict)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM entries WHERE dictionary = ?1", params![name])?;
        let mut count = 0usize;
        {
            let mut st =
                tx.prepare("INSERT INTO entries (headword, dictionary) VALUES (?1, ?2)")?;
            for w in dict.words().sorted() {
                st.execute(params![w, name])?;
                count += 1;
            }
        }
        tx.commit()?;
        self.registry.borrow_mut().set_indexed(&name, true);
        Ok(count)
    }

    /// Prefix search (`"query"*`) ranked by FTS5, limited to `dictionaries`
    /// when given.
    pub fn search_headword(
        &self,
        query: &str,
        limit: usize,
        dictionaries: Option<&[String]>,
    ) -> Result<Vec<(String, String)>, DictzError> {
        let fts = format!("\"{}\"*", query.replace('"', "\"\""));
        let mut sql =
            String::from("SELECT headword, dictionary FROM entries WHERE entries MATCH ?");
        let mut args: Vec<String> = vec![fts];
        if let Some(d) = dictionaries {
            if d.is_empty() {
                return Ok(Vec::new());
            }
            sql.push_str(&format!(
                " AND dictionary IN ({})",
                vec!["?"; d.len()].join(",")
            ));
            args.extend(d.iter().cloned());
        }
        sql.push_str(&format!(" ORDER BY rank LIMIT {}", limit));
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(args.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// `true` when the dictionary's headwords are in the index.
    pub fn is_indexed(&self, name: &str) -> Result<bool, DictzError> {
        Ok(self
            .registry
            .borrow_mut()
            .get(name)
            .map(|e| e.indexed)
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stardict::{write_stardict, StarDictDictionary};

    #[test]
    fn index_and_search() {
        let d = std::env::temp_dir().join(format!("dictz-idx-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let ifo = write_stardict(
            &d.join("t"),
            "Idx",
            &[("café", "x"), ("cafeteria", "y"), ("dog", "z")],
        )
        .unwrap();
        let mut sd = StarDictDictionary::new(&ifo).unwrap();
        sd.load().unwrap();
        let mut idx = IndexStore::open_in_memory().unwrap();
        assert_eq!(idx.index_dictionary(&sd).unwrap(), 3);
        assert!(idx.is_indexed("Idx").unwrap());
        let hits = idx.search_headword("cafe", 10, None).unwrap();
        assert_eq!(hits.len(), 2, "diacritic-insensitive prefix search");
        assert!(idx
            .search_headword("cafe", 10, Some(&[]))
            .unwrap()
            .is_empty());
        assert_eq!(idx.search_headword("do\"g", 10, None).unwrap().len(), 0);
        idx.set_sort_order("Idx", 3).unwrap();
        idx.set_enabled("Idx", false).unwrap();
        let rows = idx.list_dictionaries(false).unwrap();
        assert_eq!(rows[0].sort_order, 0);
        assert!(!rows[0].enabled);
        assert!(idx.list_dictionaries(true).unwrap().is_empty());
        idx.remove_dictionary("Idx").unwrap();
        assert!(!idx.is_indexed("Idx").unwrap());
    }

    #[test]
    fn migrates_the_old_sqlite_list() {
        let d = std::env::temp_dir().join(format!("dictz-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let db = d.join("dictz_index.db");
        {
            let c = Connection::open(&db).unwrap();
            c.execute_batch(SCHEMA).unwrap();
            for (n, p, en, so) in [
                ("B", "/x/b.ifo", 1, 2),
                ("A", "/x/a.ifo", 0, 1),
                ("A2", "/x/a.ifo", 1, 3),
            ] {
                c.execute(
                    "INSERT INTO dictionaries (name, path, format, enabled, sort_order) VALUES (?1, ?2, 'stardict', ?3, ?4)",
                    params![n, p, en, so],
                )
                .unwrap();
            }
            c.execute(
                "INSERT INTO entries (headword, dictionary) VALUES ('w', 'B')",
                [],
            )
            .unwrap();
        }
        let idx = IndexStore::open(&db).unwrap();
        let rows = idx.list_dictionaries(false).unwrap();
        let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["A", "B"], "order kept, duplicate path dropped");
        assert!(!rows[0].enabled);
        assert!(rows[1].indexed && !rows[0].indexed);
        assert!(d.join(REGISTRY_FILE).is_file());
        let _ = std::fs::remove_dir_all(&d);
    }
}
