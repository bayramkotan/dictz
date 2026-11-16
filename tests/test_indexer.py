"""Tests for the SQLite FTS5 IndexStore."""
from __future__ import annotations

import struct
from pathlib import Path

from dictz.parsers.stardict import StarDictDictionary
from dictz.storage.indexer import IndexStore


def _build_tiny_stardict(tmp_path: Path, name: str = "tiny") -> Path:
    base = tmp_path / name
    entries = [("apple", "elma"), ("application", "uygulama"),
               ("apply", "uygulamak"), ("book", "kitap")]
    dict_bytes = b""
    idx_records = []
    offset = 0
    for word, definition in entries:
        body = definition.encode("utf-8")
        idx_records.append((word, offset, len(body)))
        dict_bytes += body
        offset += len(body)
    base.with_suffix(".dict").write_bytes(dict_bytes)
    idx_bytes = b""
    for word, off, size in idx_records:
        idx_bytes += word.encode("utf-8") + b"\x00"
        idx_bytes += struct.pack(">I", off)
        idx_bytes += struct.pack(">I", size)
    base.with_suffix(".idx").write_bytes(idx_bytes)
    ifo = (
        "StarDict's dict ifo file\n"
        "version=2.4.2\n"
        f"bookname={name}-book\n"
        f"wordcount={len(entries)}\n"
        f"idxfilesize={len(idx_bytes)}\n"
    )
    base.with_suffix(".ifo").write_text(ifo, encoding="utf-8")
    return base.with_suffix(".ifo")


def test_register_and_list(tmp_path: Path) -> None:
    store = IndexStore(tmp_path / "index.db")
    store.open()
    d = StarDictDictionary(_build_tiny_stardict(tmp_path))
    d.load()
    store.register_dictionary(d)
    rows = store.list_dictionaries()
    assert len(rows) == 1 and rows[0]["name"] == "tiny-book"
    store.close()


def test_index_and_search(tmp_path: Path) -> None:
    store = IndexStore(tmp_path / "index.db")
    store.open()
    d = StarDictDictionary(_build_tiny_stardict(tmp_path))
    d.load()
    assert store.index_dictionary(d) == 4
    headwords = [r[0] for r in store.search_headword("app")]
    assert "apple" in headwords and "application" in headwords and "book" not in headwords
    store.close()


def test_list_lives_in_json_and_keeps_order(tmp_path: Path) -> None:
    import json

    store = IndexStore(tmp_path / "index.db")
    store.open()
    for n in ("a", "b", "c"):
        store.registry.register(n, tmp_path / f"{n}.ifo", format="stardict")
    store.set_order(["c", "a"])
    assert [r["name"] for r in store.list_dictionaries()] == ["c", "a", "b"]
    store.set_enabled("a", False)
    store.close()
    data = json.loads((tmp_path / "dictionaries.json").read_text(encoding="utf-8"))
    assert [d["name"] for d in data["dictionaries"]] == ["c", "a", "b"]
    assert [d["enabled"] for d in data["dictionaries"]] == [True, False, True]
    again = IndexStore(tmp_path / "index.db")
    again.open()
    assert [r["name"] for r in again.list_dictionaries(enabled_only=True)] == ["c", "b"]
    again.close()


def test_same_name_twice_is_kept_apart(tmp_path: Path) -> None:
    store = IndexStore(tmp_path / "index.db")
    store.open()
    one = tmp_path / "one"
    two = tmp_path / "two"
    one.mkdir()
    two.mkdir()
    d1 = StarDictDictionary(_build_tiny_stardict(one))
    d1.load()
    d2 = StarDictDictionary(_build_tiny_stardict(two))
    d2.load()
    store.index_dictionary(d1)
    store.index_dictionary(d2)
    assert d1.metadata.name == "tiny-book" and d2.metadata.name == "tiny-book (2)"
    rows = store.list_dictionaries()
    assert [r["name"] for r in rows] == ["tiny-book", "tiny-book (2)"]
    assert {r["path"] for r in rows} == {str(d1.metadata.file_path.absolute()), str(d2.metadata.file_path.absolute())}
    # registering the same file again keeps its name and place
    d1b = StarDictDictionary(_build_tiny_stardict(one))
    d1b.load()
    assert store.register_dictionary(d1b) == "tiny-book"
    assert {r[1] for r in store.search_headword("app")} == {"tiny-book", "tiny-book (2)"}
    store.close()


def test_migrates_the_old_sqlite_list(tmp_path: Path) -> None:
    import sqlite3

    db = tmp_path / "index.db"
    conn = sqlite3.connect(db)
    conn.executescript(IndexStore.SCHEMA)
    conn.execute("ALTER TABLE dictionaries ADD COLUMN mtime REAL DEFAULT 0")
    conn.execute("INSERT INTO dictionaries (name, path, format, enabled, sort_order) VALUES ('z', '/d/z.ifo', 'stardict', 1, 0)")
    conn.execute("INSERT INTO dictionaries (name, path, format, enabled, sort_order) VALUES ('a', '/d/a.dsl', 'dsl', 0, 1)")
    conn.execute("INSERT INTO entries (headword, dictionary) VALUES ('w', 'z')")
    conn.commit()
    conn.close()
    store = IndexStore(db)
    store.open()
    rows = store.list_dictionaries(enabled_only=False)
    assert [(r["name"], r["enabled"], r["indexed"]) for r in rows] == [("z", True, True), ("a", False, False)]
    assert (tmp_path / "dictionaries.json").is_file()
    store.close()


def test_failed_dictionaries_keep_their_place(tmp_path: Path) -> None:
    from dictz.core.query_manager import QueryManager

    store = IndexStore(tmp_path / "index.db")
    store.open()
    real = StarDictDictionary(_build_tiny_stardict(tmp_path))
    real.load()
    store.registry.register("gone", tmp_path / "missing.ifo", format="stardict", indexed=True)
    store.index_dictionary(real)
    qm = QueryManager(index_store=store)
    loaded, errors = qm.restore_from_index()
    assert loaded == ["tiny-book"] and errors
    qm._persist_order()
    assert [r["name"] for r in store.list_dictionaries(enabled_only=False)] == ["gone", "tiny-book"]
    store.close()
