"""Central SQLite FTS5 index for fast cross-dictionary lookup."""
from __future__ import annotations

import sqlite3
from contextlib import contextmanager, suppress
from pathlib import Path
from typing import Iterator

from ..parsers.base import Dictionary
from .registry import REGISTRY_FILE, Registry, norm_path, unique_name


def file_mtime(path: Path | str) -> float:
    """Modification time of a dictionary file (0 when missing)."""
    try:
        return Path(path).stat().st_mtime
    except OSError:
        return 0.0


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

    def __init__(self, db_path: Path | str, registry_path: Path | str | None = None):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._conn: sqlite3.Connection | None = None
        # the dictionary list lives in dictionaries.json next to the database
        self.registry = Registry(registry_path if registry_path is not None
                                 else self.db_path.parent / REGISTRY_FILE)

    def open(self) -> None:
        if self._conn is not None:
            return
        self._conn = sqlite3.connect(str(self.db_path))
        self._conn.row_factory = sqlite3.Row
        self._conn.executescript(self.SCHEMA)
        with suppress(sqlite3.OperationalError):
            self._conn.execute("ALTER TABLE dictionaries ADD COLUMN sort_order INTEGER DEFAULT 0")
        with suppress(sqlite3.OperationalError):
            self._conn.execute("ALTER TABLE dictionaries ADD COLUMN mtime REAL DEFAULT 0")
        self._conn.commit()
        self._migrate_to_registry()

    def _migrate_to_registry(self) -> None:
        """First start after 0.1.0: copy the old SQLite list into dictionaries.json (order kept)."""
        if self.registry.exists() or self.registry.path is None:
            return
        assert self._conn is not None
        rows = self._conn.execute("SELECT * FROM dictionaries ORDER BY sort_order, id").fetchall()
        indexed = {r[0] for r in self._conn.execute("SELECT DISTINCT dictionary FROM entries")}
        seen: set[str] = set()
        for r in rows:
            if norm_path(r["path"]) in seen:
                continue
            seen.add(norm_path(r["path"]))
            keys = r.keys()
            self.registry.items.append(Registry._clean({
                "path": norm_path(r["path"]), "name": r["name"], "format": r["format"],
                "enabled": bool(r["enabled"]), "word_count": r["word_count"] or 0,
                "source_lang": r["source_lang"] or "", "target_lang": r["target_lang"] or "",
                "description": r["description"] or "",
                "mtime": (r["mtime"] or 0.0) if "mtime" in keys else 0.0,
                "indexed": r["name"] in indexed,
            }))
        self.registry.save()

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

    def register_dictionary(self, dictionary: Dictionary) -> str:
        """Add or update ``dictionary`` in the list; returns its unique name.

        The dictionary is renamed (``Name (2)``) when another file already uses
        its name. An existing entry keeps its name, place and enabled flag.
        """
        if self._conn is None:
            self.open()
        from .articles import source_stamp

        meta = dictionary.metadata
        stamp = source_stamp(meta.file_path)
        old = self.registry.by_path(meta.file_path)
        fields = {}
        if old is not None and old.get("stamp") and old.get("stamp") != stamp:
            fields["icon"] = ""  # the files changed: look for the icon again
        name = self.registry.register(
            meta.name, meta.file_path, format=meta.format, word_count=meta.word_count,
            source_lang=meta.source_language, target_lang=meta.target_language,
            description=meta.description, mtime=file_mtime(meta.file_path), stamp=stamp,
            author=meta.author or "", **fields)
        meta.name = name
        return name

    def icon_path(self, name: str) -> Path | None:
        """The dictionary's own icon, cached in ``DATA/icons`` (looked for once,
        then kept in ``dictionaries.json``: no folder listing or Babylon file
        read at the next start)."""
        row = self.registry.get(name)
        if row is None:
            return None
        icons = self.db_path.parent / "icons"
        mark = row.get("icon") or ""
        if mark == "-":
            return None
        if mark and (icons / mark).is_file():
            return icons / mark
        from ..core.dictident import icon_bytes

        found = icon_bytes(row["path"])
        mark = "-"
        if found:
            data, ext = found
            try:
                icons.mkdir(parents=True, exist_ok=True)
                mark = f"{row['id']}{ext}"
                (icons / mark).write_bytes(data)
            except OSError:
                mark = "-"
        self.registry.set_icon(name, mark)
        return icons / mark if mark != "-" else None

    def unique_name_for(self, name: str, path: Path | str) -> str:
        """The name ``path`` has (or would get) in the list."""
        hit = self.registry.by_path(path)
        if hit is not None:
            return hit["name"]
        return unique_name(name, {d["name"] for d in self.registry.items})

    def get_dictionary(self, name: str) -> dict | None:
        return self.registry.get(name)

    def headwords(self, dictionaries: list[str]) -> list[str]:
        """Headwords of ``dictionaries`` in that order (each in index order =
        case-insensitive sorted), duplicates removed."""
        seen: set[str] = set()
        out: list[str] = []
        with self._cursor() as cur:
            for name in dictionaries:
                cur.execute("SELECT headword FROM entries WHERE dictionary = ? ORDER BY rowid", (name,))
                for (w,) in cur.fetchall():
                    if w not in seen:
                        seen.add(w)
                        out.append(w)
        return out

    def stats(self) -> dict[str, int]:
        with self._cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM entries")
            e = cur.fetchone()[0]
        return {"dictionaries": len(self.registry.rows()), "entries": e}

    def list_dictionaries(self, enabled_only: bool = True) -> list[dict]:
        if self._conn is None:
            self.open()
        return self.registry.rows(enabled_only=enabled_only)

    def set_enabled(self, name: str, enabled: bool) -> None:
        self.registry.set_enabled(name, enabled)

    def set_sort_order(self, name: str, order: int) -> None:
        """Move ``name`` to position ``order`` of the list."""
        self.registry.move_to(name, order)

    def set_order(self, names: list[str]) -> None:
        self.registry.set_order(names)

    def remove_dictionary(self, name: str) -> None:
        with self._cursor() as cur:
            cur.execute("DELETE FROM entries WHERE dictionary = ?", (name,))
        row = self.registry.get(name)
        self.registry.remove(name)
        if row is not None and self.registry.by_path(row["path"]) is None:
            import contextlib

            from .articles import store_path

            with contextlib.suppress(OSError):
                store_path(self.db_path.parent, row["id"]).unlink()

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
            for word in dictionary.headwords():
                batch.append((word, dictionary.metadata.name))
                if len(batch) >= BATCH_SIZE:
                    cur.executemany("INSERT INTO entries (headword, dictionary) VALUES (?, ?)", batch)
                    count += len(batch)
                    batch.clear()
            if batch:
                cur.executemany("INSERT INTO entries (headword, dictionary) VALUES (?, ?)", batch)
                count += len(batch)
        self.registry.set_indexed(dictionary.metadata.name, True)
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
        row = self.registry.get(name)
        return bool(row and row["indexed"])
