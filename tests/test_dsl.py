"""Tests for the DSL (ABBYY Lingvo) parser.

Uses a minimal hand-written .dsl file. DSL is a text markup format that
PyGlossary parses.
"""
from __future__ import annotations

from pathlib import Path

import pytest

pyglossary = pytest.importorskip("pyglossary")

from dictz.parsers.dsl import DSLDictionary  # noqa: E402


DSL_SAMPLE = (
    "﻿"  # BOM
    "#NAME \"Tiny Test DSL\"\n"
    "#INDEX_LANGUAGE \"English\"\n"
    "#CONTENTS_LANGUAGE \"Turkish\"\n"
    "\n"
    "apple\n"
    "\t[trn]elma[/trn]\n"
    "\n"
    "book\n"
    "\t[trn]kitap[/trn]\n"
    "\n"
    "cat\n"
    "\t[trn]kedi[/trn]\n"
)


def _write_dsl(tmp_path: Path) -> Path:
    dsl_path = tmp_path / "tiny.dsl"
    dsl_path.write_text(DSL_SAMPLE, encoding="utf-16")
    return dsl_path


def test_load_dsl(tmp_path: Path) -> None:
    dsl_path = _write_dsl(tmp_path)
    d = DSLDictionary(dsl_path)
    d.load()

    assert d.loaded
    assert d.metadata.format == "dsl"
    assert d.metadata.word_count >= 3


def test_dsl_lookup_exact(tmp_path: Path) -> None:
    dsl_path = _write_dsl(tmp_path)
    d = DSLDictionary(dsl_path)
    d.load()

    entries = d.lookup("book")
    assert len(entries) == 1
    assert entries[0].headword == "book"
    assert "kitap" in entries[0].definition


def test_dsl_iter_entries(tmp_path: Path) -> None:
    dsl_path = _write_dsl(tmp_path)
    d = DSLDictionary(dsl_path)
    d.load()

    headwords = sorted(e.headword for e in d.iter_entries())
    assert "apple" in headwords
    assert "book" in headwords
    assert "cat" in headwords
