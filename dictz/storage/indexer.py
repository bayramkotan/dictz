"""Central SQLite FTS5 index for fast cross-dictionary lookup."""
from __future__ import annotations

import sqlite3
from contextlib import contextmanager, suppress
from pathlib import Path
from typing import Iterator

from ..parsers.base import Dictionary


class IndexStore:
    SCHEMA = """
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
        with suppress(sqlite3.OperationalError):
            self._conn.execute("ALTER TABLE dictionaries ADD COLUMN sort_order INTEGER DEFAULT 0")
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

    def register_dictionary(self, dictionary: Dictionary) -> int:
        meta = dictionary.metadata
        with self._cursor() as cur:
            cur.execute(
                """
                INSERT OR REPLACE INTO dictionaries
                    (name, path, format, word_count, source_lang, target_lang, description, enabled)
                VALUES (?, ?, ?, ?, ?, ?, ?, 1)
                """,
                (meta.name, str(meta.file_path), meta.format, meta.word_count,
                 meta.source_language, meta.target_language, meta.description),
            )
            return cur.lastrowid or 0

    def list_dictionaries(self, enabled_only: bool = True) -> list[sqlite3.Row]:
        with self._cursor() as cur:
            if enabled_only:
                cur.execute("SELECT * FROM dictionaries WHERE enabled = 1 ORDER BY sort_order, name")
            else:
                cur.execute("SELECT * FROM dictionaries ORDER BY sort_order, name")
            return cur.fetchall()

    def set_enabled(self, name: str, enabled: bool) -> None:
        with self._cursor() as cur:
            cur.execute("UPDATE dictionaries SET enabled = ? WHERE name = ?", (1 if enabled else 0, name))

    def set_sort_order(self, name: str, order: int) -> None:
        with self._cursor() as cur:
            cur.execute("UPDATE dictionaries SET sort_order = ? WHERE name = ?", (order, name))

    def remove_dictionary(self, name: str) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM entries WHERE dictionary = ?", (name,))
            cur.execute("DELETE FROM dictionaries WHERE name = ?", (name,))

    def index_dictionary(self, dictionary: Dictionary) -> int:
        if not dictionary.loaded:
            dictionary.load()
        self.register_dictionary(dictionary)
        with self._cursor() as cur:
            cur.execute("DELETE FROM entries WHERE dictionary = ?", (dictionary.metadata.name,))
        batch: list[tuple[str, str]] = []
        count = 0
        BATCH_SIZE = 5000
        with self._cursor() as cur:
            for entry in dictionary.iter_entries():
                batch.append((entry.headword, dictionary.metadata.name))
                if len(batch) >= BATCH_SIZE:
                    cur.executemany("INSERT INTO entries (headword, dictionary) VALUES (?, ?)", batch)
                    count += len(batch)
                    batch.clear()
            if batch:
                cur.executemany("INSERT INTO entries (headword, dictionary) VALUES (?, ?)", batch)
                count += len(batch)
        return count

    def search_headword(
        self, query: str, limit: int = 100, dictionaries: list[str] | None = None
    ) -> list[tuple[str, str]]:
        """Prefix search (``"query"*``) ranked by FTS5.

        ``dictionaries`` limits the search; an empty list means "none".
        """
        if dictionaries is not None and not dictionaries:
            return []
        safe = query.replace('"', '""')
        fts_query = f'"{safe}"*'
        sql = "SELECT headword, dictionary FROM entries WHERE entries MATCH ?"
        args: list = [fts_query]
        if dictionaries is not None:
            sql += f" AND dictionary IN ({','.join('?' * len(dictionaries))})"
            args.extend(dictionaries)
        sql += " ORDER BY rank LIMIT ?"
        args.append(limit)
        with self._cursor() as cur:
            cur.execute(sql, args)
            rows = cur.fetchall()
        return [(r["headword"], r["dictionary"]) for r in rows]

    def is_indexed(self, name: str) -> bool:
        with self._cursor() as cur:
            cur.execute("SELECT 1 FROM dictionaries WHERE name = ?", (name,))
            return cur.fetchone() is not None
