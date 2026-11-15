"""Persistent history and favorites, stored in SQLite."""
from __future__ import annotations

import sqlite3
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


class HistoryStore:
    SCHEMA = """
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
    """

    def __init__(self, db_path: Path | str):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._conn: sqlite3.Connection | None = None

    def open(self) -> None:
        if self._conn is not None:
            return
        self._conn = sqlite3.connect(str(self.db_path))
        self._conn.row_factory = sqlite3.Row
        self._conn.executescript(self.SCHEMA)
        self._conn.commit()

    def close(self) -> None:
        if self._conn is not None:
            self._conn.close()
            self._conn = None

    @contextmanager
    def _cursor(self) -> Iterator[sqlite3.Cursor]:
        if self._conn is None:
            self.open()
        assert self._conn is not None
        cur = self._conn.cursor()
        try:
            yield cur
            self._conn.commit()
        except Exception:
            self._conn.rollback()
            raise
        finally:
            cur.close()

    def record(self, word: str, dictionary: str = "") -> None:
        if not word:
            return
        with self._cursor() as cur:
            cur.execute(
                "INSERT INTO history (word, dictionary, looked_up) VALUES (?, ?, ?)",
                (word, dictionary, time.time()),
            )

    def recent(self, limit: int = 100) -> list[str]:
        with self._cursor() as cur:
            cur.execute(
                "SELECT word FROM history GROUP BY word ORDER BY MAX(looked_up) DESC LIMIT ?",
                (limit,),
            )
            return [row["word"] for row in cur.fetchall()]

    def remove_history(self, word: str) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM history WHERE word = ?", (word,))

    def count(self) -> int:
        with self._cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM history")
            return cur.fetchone()[0]

    def purge(self, max_entries: int) -> int:
        """Keep only the newest ``max_entries`` lookups. Returns rows deleted."""
        with self._cursor() as cur:
            cur.execute(
                "DELETE FROM history WHERE id NOT IN "
                "(SELECT id FROM history ORDER BY looked_up DESC, id DESC LIMIT ?)",
                (max_entries,),
            )
            return cur.rowcount

    def clear_favorites(self) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM favorites")

    def clear_history(self) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM history")

    def add_favorite(self, word: str, note: str = "") -> None:
        if not word:
            return
        with self._cursor() as cur:
            cur.execute(
                "INSERT OR IGNORE INTO favorites (word, note, added_at) VALUES (?, ?, ?)",
                (word, note, time.time()),
            )

    def remove_favorite(self, word: str) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM favorites WHERE word = ?", (word,))

    def toggle_favorite(self, word: str) -> bool:
        if self.is_favorite(word):
            self.remove_favorite(word)
            return False
        self.add_favorite(word)
        return True

    def is_favorite(self, word: str) -> bool:
        with self._cursor() as cur:
            cur.execute("SELECT 1 FROM favorites WHERE word = ?", (word,))
            return cur.fetchone() is not None

    def favorites(self, limit: int = 500) -> list[str]:
        with self._cursor() as cur:
            cur.execute("SELECT word FROM favorites ORDER BY added_at DESC LIMIT ?", (limit,))
            return [row["word"] for row in cur.fetchall()]

    def set_note(self, word: str, note: str) -> None:
        with self._cursor() as cur:
            cur.execute("UPDATE favorites SET note = ? WHERE word = ?", (note, word))

    def get_note(self, word: str) -> str:
        with self._cursor() as cur:
            cur.execute("SELECT note FROM favorites WHERE word = ?", (word,))
            row = cur.fetchone()
            return row["note"] if row else ""
