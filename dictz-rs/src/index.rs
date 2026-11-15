//! SQLite FTS5 headword index — same schema and file as the Python edition
//! (`dictz/storage/indexer.py`).

use std::path::{Path, PathBuf};

use rusqlite::{params, params_from_iter, Connection};

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

/// One row of the `dictionaries` table.
#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryRow {
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
}

impl IndexStore {
    /// Open (and create) the database at `path`.
    pub fn open(path: &Path) -> Result<Self, DictzError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::with_connection(Connection::open(path)?)
    }

    /// In-memory database (tests, `--selftest`).
    pub fn open_in_memory() -> Result<Self, DictzError> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(conn: Connection) -> Result<Self, DictzError> {
        conn.execute_batch(SCHEMA)?;
        let _ = conn.execute(
            "ALTER TABLE dictionaries ADD COLUMN sort_order INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE dictionaries ADD COLUMN mtime REAL DEFAULT 0",
            [],
        );
        Ok(Self { conn })
    }

    /// Insert or update the row of `dict`. An existing row keeps its
    /// `enabled` flag and `sort_order`.
    pub fn register_dictionary(&self, dict: &dyn Dictionary) -> Result<(), DictzError> {
        let m = dict.metadata();
        self.conn.execute(
            "INSERT INTO dictionaries
                (name, path, format, word_count, source_lang, target_lang, description, enabled, mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)
             ON CONFLICT(name) DO UPDATE SET
                path = excluded.path, format = excluded.format, word_count = excluded.word_count,
                source_lang = excluded.source_lang, target_lang = excluded.target_lang,
                description = excluded.description, mtime = excluded.mtime",
            params![
                m.name,
                m.file_path.to_string_lossy(),
                m.format,
                m.word_count as i64,
                m.source_language,
                m.target_language,
                m.description,
                file_mtime(&m.file_path)
            ],
        )?;
        Ok(())
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
        let d: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM dictionaries", [], |r| r.get(0))?;
        let e: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        Ok((d, e))
    }

    /// Rows ordered by `sort_order, name`.
    pub fn list_dictionaries(&self, enabled_only: bool) -> Result<Vec<DictionaryRow>, DictzError> {
        let sql = if enabled_only {
            "SELECT name, path, format, word_count, enabled, sort_order, mtime, source_lang, target_lang FROM dictionaries WHERE enabled = 1 ORDER BY sort_order, name"
        } else {
            "SELECT name, path, format, word_count, enabled, sort_order, mtime, source_lang, target_lang FROM dictionaries ORDER BY sort_order, name"
        };
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map([], |r| {
            Ok(DictionaryRow {
                name: r.get(0)?,
                path: PathBuf::from(r.get::<_, String>(1)?),
                format: r.get(2)?,
                word_count: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                enabled: r.get::<_, Option<i64>>(4)?.unwrap_or(1) != 0,
                sort_order: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                mtime: r.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                source_language: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                target_language: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Enable / disable a dictionary.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<(), DictzError> {
        self.conn.execute(
            "UPDATE dictionaries SET enabled = ?1 WHERE name = ?2",
            params![enabled as i64, name],
        )?;
        Ok(())
    }

    /// Set the list position of a dictionary.
    pub fn set_sort_order(&self, name: &str, order: i64) -> Result<(), DictzError> {
        self.conn.execute(
            "UPDATE dictionaries SET sort_order = ?1 WHERE name = ?2",
            params![order, name],
        )?;
        Ok(())
    }

    /// Delete a dictionary and its headwords.
    pub fn remove_dictionary(&self, name: &str) -> Result<(), DictzError> {
        self.conn
            .execute("DELETE FROM entries WHERE dictionary = ?1", params![name])?;
        self.conn
            .execute("DELETE FROM dictionaries WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// (Re)build the headword index of `dict`. Returns the number of rows.
    pub fn index_dictionary(&mut self, dict: &dyn Dictionary) -> Result<usize, DictzError> {
        self.register_dictionary(dict)?;
        let name = dict.metadata().name.clone();
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

    /// `true` when the dictionary has a row.
    pub fn is_indexed(&self, name: &str) -> Result<bool, DictzError> {
        let mut st = self
            .conn
            .prepare("SELECT 1 FROM dictionaries WHERE name = ?1")?;
        Ok(st.exists(params![name])?)
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
        assert_eq!(rows[0].sort_order, 3);
        assert!(!rows[0].enabled);
        assert!(idx.list_dictionaries(true).unwrap().is_empty());
        idx.remove_dictionary("Idx").unwrap();
        assert!(!idx.is_indexed("Idx").unwrap());
    }
}
