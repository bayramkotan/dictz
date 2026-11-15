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


def test_sort_order(tmp_path: Path) -> None:
    store = IndexStore(tmp_path / "index.db")
    store.open()
    with store._cursor() as cur:
        cur.execute("INSERT INTO dictionaries (name, path, format) VALUES ('a', 'x', 'stardict')")
        cur.execute("INSERT INTO dictionaries (name, path, format) VALUES ('b', 'y', 'stardict')")
    store.set_sort_order("b", -1)
    store.set_sort_order("a", 1)
    names = [r["name"] for r in store.list_dictionaries()]
    assert names == ["b", "a"]
    store.close()
