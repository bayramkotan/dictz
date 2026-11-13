"""Tests for the SQLite FTS5 IndexStore."""
from __future__ import annotations

import struct
from pathlib import Path

from dictz.parsers.stardict import StarDictDictionary
from dictz.storage.indexer import IndexStore


def _build_tiny_stardict(tmp_path: Path, name: str = "tiny") -> Path:
    base = tmp_path / name
    entries = [
        ("apple", "elma"),
        ("application", "uygulama"),
        ("apply", "uygulamak"),
        ("book", "kitap"),
    ]

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
    ifo_path = base.with_suffix(".ifo")
    ifo_path.write_text(ifo, encoding="utf-8")
    return ifo_path


def test_register_and_list(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    store = IndexStore(tmp_path / "index.db")
    store.open()

    d = StarDictDictionary(ifo_path)
    d.load()
    store.register_dictionary(d)

    rows = store.list_dictionaries()
    assert len(rows) == 1
    assert rows[0]["name"] == "tiny-book"

    store.close()


def test_index_and_search(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    store = IndexStore(tmp_path / "index.db")
    store.open()

    d = StarDictDictionary(ifo_path)
    d.load()
    count = store.index_dictionary(d)
    assert count == 4

    results = store.search_headword("app")
    headwords = [r[0] for r in results]
    assert "apple" in headwords
    assert "application" in headwords
    assert "apply" in headwords
    assert "book" not in headwords

    store.close()


def test_remove_dictionary(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    store = IndexStore(tmp_path / "index.db")
    store.open()

    d = StarDictDictionary(ifo_path)
    d.load()
    store.index_dictionary(d)
    assert store.is_indexed(d.metadata.name)

    store.remove_dictionary(d.metadata.name)
    assert not store.is_indexed(d.metadata.name)
    assert store.search_headword("app") == []

    store.close()
