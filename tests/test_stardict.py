"""Tests for the StarDict parser.

We build a tiny StarDict dictionary in a tmp dir and verify that the parser
reads the .ifo, decodes the .idx, and pulls definitions out of the .dict.
"""
from __future__ import annotations

import struct
from pathlib import Path

import pytest

from dictz.parsers.stardict import StarDictDictionary, StarDictError


def _build_tiny_stardict(tmp_path: Path, name: str = "tiny") -> Path:
    """Construct a minimal valid StarDict on disk. Returns the .ifo path."""
    base = tmp_path / name
    entries = [
        ("apple", "elma — a round fruit"),
        ("book", "kitap — a written work"),
        ("cat", "kedi — a small carnivorous mammal"),
    ]

    # Build .dict (concatenated definitions, UTF-8 bytes)
    dict_bytes = b""
    idx_records: list[tuple[str, int, int]] = []
    offset = 0
    for word, definition in entries:
        body = definition.encode("utf-8")
        idx_records.append((word, offset, len(body)))
        dict_bytes += body
        offset += len(body)

    base.with_suffix(".dict").write_bytes(dict_bytes)

    # Build .idx: each record = word + \0 + offset(BE 32) + size(BE 32)
    idx_bytes = b""
    for word, offset, size in idx_records:
        idx_bytes += word.encode("utf-8") + b"\x00"
        idx_bytes += struct.pack(">I", offset)
        idx_bytes += struct.pack(">I", size)
    base.with_suffix(".idx").write_bytes(idx_bytes)

    # Build .ifo
    ifo = (
        "StarDict's dict ifo file\n"
        "version=2.4.2\n"
        f"bookname=Tiny Test Dictionary\n"
        f"wordcount={len(entries)}\n"
        f"idxfilesize={len(idx_bytes)}\n"
        "author=dictz Tests\n"
        "description=A tiny dictionary used in unit tests\n"
    )
    ifo_path = base.with_suffix(".ifo")
    ifo_path.write_text(ifo, encoding="utf-8")

    return ifo_path


# ----------------------------------------------------------------- tests


def test_load_metadata(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    assert d.loaded
    assert d.metadata.name == "Tiny Test Dictionary"
    assert d.metadata.author == "dictz Tests"
    assert d.metadata.word_count == 3


def test_lookup_exact(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    entries = d.lookup("book")
    assert len(entries) == 1
    assert entries[0].headword == "book"
    assert "kitap" in entries[0].definition


def test_lookup_case_insensitive(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    entries = d.lookup("APPLE")
    assert len(entries) == 1
    assert entries[0].headword == "apple"


def test_lookup_prefix(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    entries = d.lookup("ca", exact=False)
    headwords = [e.headword for e in entries]
    assert "cat" in headwords


def test_lookup_missing(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    assert d.lookup("nonexistent") == []


def test_iter_entries(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    d = StarDictDictionary(ifo_path)
    d.load()

    headwords = [e.headword for e in d.iter_entries()]
    assert sorted(headwords) == ["apple", "book", "cat"]


def test_missing_ifo_raises(tmp_path: Path) -> None:
    bogus = tmp_path / "nope.dict"
    bogus.write_bytes(b"")
    with pytest.raises(StarDictError):
        StarDictDictionary(bogus)


def test_context_manager(tmp_path: Path) -> None:
    ifo_path = _build_tiny_stardict(tmp_path)
    with StarDictDictionary(ifo_path) as d:
        assert d.loaded
        assert len(d.lookup("apple")) == 1
    # After __exit__, dictionary is closed
    assert not d.loaded
