"""Article store: a dictionary's articles saved once, read on demand.

Same file format and rules as the Rust edition (``dictz-rs/src/articles.rs``).

Parsing a dictionary (above all DSL and Babylon) is slow, so it is done once:
the articles go into ``articles/<id>.db`` in the data folder (``id`` = the
dictionary's id in ``dictionaries.json``). Later starts open that file, read
a few metadata lines and nothing else; an article is read (and unpacked) only
when it is shown. This is how GoldenDict works too.

The store is rebuilt when the dictionary's files change (size or mtime, see
:func:`source_stamp`) or when :data:`STORE_VERSION` changes.

Lookup rules are the parsers' rules: the exact headword, otherwise the first
case-insensitive match in sorted order, otherwise (``exact=False``) up to 50
case-insensitive prefix matches. Rows are stored in the parsers' sorted order
(``sorted(words, key=str.lower)``), so ``ORDER BY id`` is that order.

Tables::

    meta(key TEXT PRIMARY KEY, value TEXT)
    articles(id INTEGER PRIMARY KEY, word TEXT, folded TEXT, z INTEGER, body BLOB)

``folded`` is ``word.lower()``; ``z = 1`` means ``body`` is zlib-compressed
UTF-8, ``z = 0`` plain UTF-8.
"""
from __future__ import annotations

import contextlib
import os
import sqlite3
import zlib
from pathlib import Path
from typing import Iterator

from ..parsers.base import Dictionary, DictionaryEntry

STORE_VERSION = "1"
STORE_DIR = "articles"
PREFIX_LIMIT = 50
_MAX_CHAR = "\U0010ffff"

_SCHEMA = """
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE articles (
    id INTEGER PRIMARY KEY,
    word TEXT NOT NULL,
    folded TEXT NOT NULL,
    z INTEGER NOT NULL,
    body BLOB NOT NULL
);
"""
_INDEXES = """
CREATE UNIQUE INDEX articles_word ON articles(word);
CREATE INDEX articles_folded ON articles(folded, id);
"""
META_KEYS = ("name", "description", "author", "source_lang", "target_lang", "word_count", "format")


def source_files(path: Path | str) -> list[Path]:
    """The files a dictionary is read from (a StarDict is three files)."""
    p = Path(path)
    if p.suffix.lower() == ".ifo":
        base = p.with_suffix("")
        out = [p]
        for group in ((".idx", ".idx.gz"), (".dict", ".dict.dz")):
            for ext in group:
                q = Path(str(base) + ext)
                if q.is_file():
                    out.append(q)
                    break
        return out
    return [p]


def source_stamp(path: Path | str) -> str:
    """``name:size:mtime_ns`` of every source file, joined by ``;``."""
    parts = []
    for f in source_files(path):
        try:
            st = f.stat()
        except OSError:
            parts.append(f"{f.name}:-:-")
            continue
        parts.append(f"{f.name}:{st.st_size}:{st.st_mtime_ns}")
    return ";".join(parts)


def store_path(data_dir: Path | str, dict_id: str) -> Path:
    return Path(data_dir) / STORE_DIR / f"{dict_id}.db"


def _pack(text: str) -> tuple[int, bytes]:
    raw = text.encode("utf-8")
    if len(raw) < 64:
        return 0, raw
    packed = zlib.compress(raw, 6)
    return (1, packed) if len(packed) < len(raw) else (0, raw)


def _unpack(z: int, body: bytes) -> str:
    data = zlib.decompress(body) if z else body
    return data.decode("utf-8", errors="replace")


def read_meta(path: Path | str) -> dict[str, str] | None:
    """The store's metadata, or ``None`` when it is missing or unreadable."""
    p = Path(path)
    if not p.is_file():
        return None
    try:
        con = sqlite3.connect(f"file:{p.as_posix()}?mode=ro", uri=True)
        try:
            return dict(con.execute("SELECT key, value FROM meta").fetchall())
        finally:
            con.close()
    except sqlite3.Error:
        return None


def is_fresh(store: Path | str, source: Path | str) -> bool:
    """``True`` when ``store`` was built from the current ``source`` files."""
    meta = read_meta(store)
    return bool(meta) and meta.get("version") == STORE_VERSION and meta.get("stamp") == source_stamp(source)


def build(dictionary: Dictionary, store: Path | str, builder: str = "python") -> int:
    """Write every article of a loaded ``dictionary`` to ``store``; returns the count.

    Written to a temporary file first and renamed, so a half-written store is
    never used.
    """
    if not dictionary.loaded:
        dictionary.load()
    store = Path(store)
    store.parent.mkdir(parents=True, exist_ok=True)
    tmp = store.with_name(f".{store.name}.{os.getpid()}.tmp")
    with contextlib.suppress(OSError):
        tmp.unlink()
    m = dictionary.metadata
    count = 0
    con = sqlite3.connect(str(tmp))
    try:
        con.execute("PRAGMA journal_mode=OFF")
        con.execute("PRAGMA synchronous=OFF")
        con.executescript(_SCHEMA)
        rows = []
        for e in dictionary.iter_entries():
            z, body = _pack(e.definition)
            rows.append((e.headword, e.headword.lower(), z, body))
            if len(rows) >= 5000:
                con.executemany("INSERT INTO articles (word, folded, z, body) VALUES (?, ?, ?, ?)", rows)
                count += len(rows)
                rows.clear()
        if rows:
            con.executemany("INSERT INTO articles (word, folded, z, body) VALUES (?, ?, ?, ?)", rows)
            count += len(rows)
        con.executescript(_INDEXES)
        meta = {
            "version": STORE_VERSION,
            "stamp": source_stamp(m.file_path),
            "builder": builder,
            "name": m.name,
            "description": m.description or "",
            "author": m.author or "",
            "source_lang": m.source_language or "",
            "target_lang": m.target_language or "",
            "word_count": str(count),
            "format": m.format or "",
        }
        con.executemany("INSERT INTO meta (key, value) VALUES (?, ?)", sorted(meta.items()))
        con.commit()
    except BaseException:
        con.close()
        with contextlib.suppress(OSError):
            tmp.unlink()
        raise
    con.close()
    os.replace(tmp, store)
    return count


class StoredDictionary(Dictionary):
    """A dictionary served from its article store (nothing parsed)."""

    def __init__(self, store: Path | str, source: Path | str):
        super().__init__(source)
        self.store = Path(store)
        self._con: sqlite3.Connection | None = None
        self._words: list[str] | None = None
        meta = read_meta(self.store) or {}
        m = self.metadata
        m.name = meta.get("name") or self.path.stem
        m.description = meta.get("description", "")
        m.author = meta.get("author", "")
        m.source_language = meta.get("source_lang", "")
        m.target_language = meta.get("target_lang", "")
        m.format = meta.get("format", "")
        try:
            m.word_count = int(meta.get("word_count") or 0)
        except ValueError:
            m.word_count = 0

    def load(self) -> None:
        if self._loaded:
            return
        # opened in one thread (the loader), used in another (the GUI); reads only
        self._con = sqlite3.connect(f"file:{self.store.as_posix()}?mode=ro", uri=True, check_same_thread=False)
        self._loaded = True

    def _rows(self, sql: str, args: tuple) -> list[tuple[str, int, bytes]]:
        if not self._loaded:
            self.load()
        assert self._con is not None
        return self._con.execute(sql, args).fetchall()

    def _entries(self, rows) -> list[DictionaryEntry]:
        return [DictionaryEntry(headword=w, definition=_unpack(z, b), dictionary_id=self.metadata.name, score=1.0)
                for w, z, b in rows]

    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        rows = self._rows("SELECT word, z, body FROM articles WHERE word = ?", (word,))
        if not rows:
            rows = self._rows("SELECT word, z, body FROM articles WHERE folded = ? ORDER BY id LIMIT 1",
                              (word.lower(),))
        if not exact and not rows:
            p = word.lower()
            rows = self._rows("SELECT word, z, body FROM articles WHERE folded >= ? AND folded < ? "
                              "ORDER BY id LIMIT ?", (p, p + _MAX_CHAR, PREFIX_LIMIT))
            # the range is a superset only for odd prefixes; keep the parsers' rule
            rows = [r for r in rows if r[0].lower().startswith(p)]
        return self._entries(rows)

    def headwords(self) -> list[str]:
        if self._words is None:
            self._words = [w for (w,) in self._rows("SELECT word FROM articles ORDER BY id", ())]
        return list(self._words)

    def iter_entries(self) -> Iterator[DictionaryEntry]:
        if not self._loaded:
            self.load()
        assert self._con is not None
        cur = self._con.execute("SELECT word, z, body FROM articles ORDER BY id")
        for w, z, b in cur:
            yield DictionaryEntry(headword=w, definition=_unpack(z, b), dictionary_id=self.metadata.name)

    def close(self) -> None:
        if self._con is not None:
            with contextlib.suppress(sqlite3.Error):
                self._con.close()
        self._con = None
        self._words = None
        super().close()
