//! Lookup history and favorites — same schema and file as the Python
//! edition (`dictz/storage/history.py`).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::DictzError;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    word        TEXT NOT NULL,
    dictionary  TEXT DEFAULT '',
    looked_up   REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_history_word ON history(word);
CREATE INDEX IF NOT EXISTS idx_history_time ON history(looked_up DESC);
CREATE TABLE IF NOT EXISTS favorites (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    word        TEXT NOT NULL UNIQUE,
    note        TEXT DEFAULT '',
    added_at    REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fav_word ON favorites(word);
";

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// History + favorites database.
pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    /// Open (and create) the database at `path`.
    pub fn open(path: &Path) -> Result<Self, DictzError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// In-memory database.
    pub fn open_in_memory() -> Result<Self, DictzError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Record a lookup.
    pub fn record(&self, word: &str, dictionary: &str) -> Result<(), DictzError> {
        if word.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO history (word, dictionary, looked_up) VALUES (?1, ?2, ?3)",
            params![word, dictionary, now()],
        )?;
        Ok(())
    }

    /// Most recent distinct words, newest first.
    pub fn recent(&self, limit: usize) -> Result<Vec<String>, DictzError> {
        let mut st = self.conn.prepare(
            "SELECT word FROM history GROUP BY word ORDER BY MAX(looked_up) DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Delete all history.
    pub fn clear_history(&self) -> Result<(), DictzError> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Add a favorite (no-op when present).
    pub fn add_favorite(&self, word: &str) -> Result<(), DictzError> {
        if word.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO favorites (word, note, added_at) VALUES (?1, '', ?2)",
            params![word, now()],
        )?;
        Ok(())
    }

    /// Remove a favorite.
    pub fn remove_favorite(&self, word: &str) -> Result<(), DictzError> {
        self.conn
            .execute("DELETE FROM favorites WHERE word = ?1", params![word])?;
        Ok(())
    }

    /// Toggle; returns the new state.
    pub fn toggle_favorite(&self, word: &str) -> Result<bool, DictzError> {
        if self.is_favorite(word)? {
            self.remove_favorite(word)?;
            Ok(false)
        } else {
            self.add_favorite(word)?;
            Ok(true)
        }
    }

    /// `true` when `word` is a favorite.
    pub fn is_favorite(&self, word: &str) -> Result<bool, DictzError> {
        let mut st = self
            .conn
            .prepare("SELECT 1 FROM favorites WHERE word = ?1")?;
        Ok(st.exists(params![word])?)
    }

    /// Favorites, newest first.
    pub fn favorites(&self, limit: usize) -> Result<Vec<String>, DictzError> {
        let mut st = self
            .conn
            .prepare("SELECT word FROM favorites ORDER BY added_at DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_and_favorites() {
        let h = HistoryStore::open_in_memory().unwrap();
        h.record("a", "").unwrap();
        h.record("b", "").unwrap();
        h.record("a", "").unwrap();
        let r = h.recent(10).unwrap();
        assert_eq!(r.len(), 2);
        h.clear_history().unwrap();
        assert!(h.recent(10).unwrap().is_empty());
        assert!(h.toggle_favorite("x").unwrap());
        assert!(h.is_favorite("x").unwrap());
        assert_eq!(h.favorites(10).unwrap(), vec!["x".to_string()]);
        assert!(!h.toggle_favorite("x").unwrap());
    }
}
