"""Central SQLite FTS5 index for fast cross-dictionary lookup.

Storing every entry in a single FTS5 table lets us search across all loaded
dictionaries in a single query, with prefix matching out of the box.

Schema:
    entries (FTS5 virtual table)
        headword     — the word being defined (indexed)
        dictionary   — name of the source dictionary
        rowid        — auto

    dictionaries (regular table)
        id, name, path, format, word_count, source_lang, target_lang, description
"""
from __future__ import annotations

import sqlite3
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

from ..parsers.base import Dictionary, DictionaryEntry


class IndexStore:
    """Wraps a SQLite database with FTS5 for headword search."""

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
        enabled      INTEGER DEFAULT 1
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

    # ----------------------------------------------------------- connection

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

    # ------------------------------------------------------- dictionary CRUD

    def register_dictionary(self, dictionary: Dictionary) -> int:
        """Insert dictionary metadata. Returns the row id."""
        meta = dictionary.metadata
        with self._cursor() as cur:
            cur.execute(
                """
                INSERT OR REPLACE INTO dictionaries
                    (name, path, format, word_count, source_lang, target_lang, description, enabled)
                VALUES (?, ?, ?, ?, ?, ?, ?, 1)
                """,
                (
                    meta.name,
                    str(meta.file_path),
                    meta.format,
                    meta.word_count,
                    meta.source_language,
                    meta.target_language,
                    meta.description,
                ),
            )
            return cur.lastrowid or 0

    def list_dictionaries(self, enabled_only: bool = True) -> list[sqlite3.Row]:
        with self._cursor() as cur:
            if enabled_only:
                cur.execute("SELECT * FROM dictionaries WHERE enabled = 1 ORDER BY name")
            else:
                cur.execute("SELECT * FROM dictionaries ORDER BY name")
            return cur.fetchall()

    def set_enabled(self, name: str, enabled: bool) -> None:
        with self._cursor() as cur:
            cur.execute(
                "UPDATE dictionaries SET enabled = ? WHERE name = ?",
                (1 if enabled else 0, name),
            )

    def remove_dictionary(self, name: str) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM entries WHERE dictionary = ?", (name,))
            cur.execute("DELETE FROM dictionaries WHERE name = ?", (name,))

    # ------------------------------------------------------------ indexing

    def index_dictionary(self, dictionary: Dictionary) -> int:
        """Index all headwords of a dictionary into the FTS5 table.

        We only index the headwords, not full definitions — definitions are
        re-fetched from the source dictionary on demand. This keeps the
        index DB small even for huge sources.

        Returns the number of entries indexed.
        """
        if not dictionary.loaded:
            dictionary.load()

        self.register_dictionary(dictionary)

        # Wipe any prior entries for this dictionary (in case of re-index)
        with self._cursor() as cur:
            cur.execute("DELETE FROM entries WHERE dictionary = ?", (dictionary.metadata.name,))

        # Bulk-insert in batches for speed
        batch: list[tuple[str, str]] = []
        count = 0
        BATCH_SIZE = 5000
        with self._cursor() as cur:
            for entry in dictionary.iter_entries():
                batch.append((entry.headword, dictionary.metadata.name))
                if len(batch) >= BATCH_SIZE:
                    cur.executemany(
                        "INSERT INTO entries (headword, dictionary) VALUES (?, ?)",
                        batch,
                    )
                    count += len(batch)
                    batch.clear()
            if batch:
                cur.executemany(
                    "INSERT INTO entries (headword, dictionary) VALUES (?, ?)",
                    batch,
                )
                count += len(batch)

        return count

    # ------------------------------------------------------------- search

    def search_headword(
        self,
        query: str,
        limit: int = 100,
        dictionaries: list[str] | None = None,
    ) -> list[tuple[str, str]]:
        """Search for a headword (exact or prefix).

        Returns a list of (headword, dictionary_name) tuples.
        """
        # FTS5 prefix match: append * to the term
        # Escape FTS5 special characters by quoting
        safe = query.replace('"', '""')
        fts_query = f'"{safe}"*'

        sql = """
            SELECT headword, dictionary FROM entries
            WHERE entries MATCH ?
            ORDER BY rank
            LIMIT ?
        """
        with self._cursor() as cur:
            cur.execute(sql, (fts_query, limit))
            rows = cur.fetchall()

        if dictionaries:
            allowed = set(dictionaries)
            rows = [r for r in rows if r["dictionary"] in allowed]

        return [(r["headword"], r["dictionary"]) for r in rows]

    def is_indexed(self, name: str) -> bool:
        with self._cursor() as cur:
            cur.execute("SELECT 1 FROM dictionaries WHERE name = ?", (name,))
            return cur.fetchone() is not None
