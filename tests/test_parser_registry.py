"""Tests for the parser-registry dispatch in query_manager.parser_for."""
from __future__ import annotations

from pathlib import Path

import pytest

from dictz.core.query_manager import parser_for, supported_extensions
from dictz.parsers.babylon import BabylonDictionary
from dictz.parsers.dsl import DSLDictionary
from dictz.parsers.stardict import StarDictDictionary


def test_parser_for_ifo() -> None:
    assert parser_for(Path("foo.ifo")) is StarDictDictionary


def test_parser_for_dsl() -> None:
    assert parser_for(Path("foo.dsl")) is DSLDictionary


def test_parser_for_dsl_dz() -> None:
    assert parser_for(Path("foo.dsl.dz")) is DSLDictionary


def test_parser_for_bgl() -> None:
    assert parser_for(Path("foo.bgl")) is BabylonDictionary


def test_parser_for_unknown_raises() -> None:
    with pytest.raises(ValueError):
        parser_for(Path("foo.xyz"))


def test_supported_extensions_has_all() -> None:
    exts = supported_extensions()
    assert ".ifo" in exts and ".dsl" in exts and ".bgl" in exts
