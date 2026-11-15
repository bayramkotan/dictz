"""Python edition ↔ Rust edition parity.

The golden files in ``tests/data/parity`` are what the Python edition reads
from the shared samples. ``dictz-rs/tests/parity.rs`` asserts the Rust crate
produces exactly the same; these tests make sure the Python side still does
(so a PyGlossary upgrade that changes output is noticed in both editions).
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from dictz.core.fuzzy import ratio
from dictz.core.media import MediaResolver
from dictz.core.query_manager import parser_for

PARITY = Path(__file__).resolve().parent / "data" / "parity"


def _read(sample: str) -> dict:
    d = parser_for(PARITY / sample)(PARITY / sample)
    d.load()
    m = d.metadata
    return {
        "meta": {"name": m.name, "author": m.author, "description": m.description,
                 "source": m.source_language, "target": m.target_language, "count": m.word_count},
        "entries": [[e.headword, e.definition] for e in d.iter_entries()],
    }


@pytest.mark.parametrize(("sample", "golden"), [
    ("sample.dsl", "dsl_golden.json"),
    ("sample.bgl", "bgl_golden.json"),
])
def test_python_edition_matches_golden(sample: str, golden: str) -> None:
    expected = json.loads((PARITY / golden).read_text(encoding="utf-8"))
    assert _read(sample) == expected


def test_dsl_headwords_are_unescaped() -> None:
    words = [e[0] for e in _read("sample.dsl")["entries"]]
    assert "rock & roll" in words
    assert not any("&amp;" in w for w in words)


def test_fuzzy_golden_matches() -> None:
    for a, b, want in json.loads((PARITY / "fuzzy_golden.json").read_text(encoding="utf-8")):
        assert ratio(a, b) == want, (a, b)


def test_sample_media_found() -> None:
    r = MediaResolver(PARITY / "sample.dsl")
    assert r.read("apple.png").startswith(b"\x89PNG")
    assert r.read("apple.wav").startswith(b"RIFF")  # sounds/Apple.WAV by base name


def test_rust_parity_test_exists() -> None:
    rs = (PARITY.parent.parent.parent / "dictz-rs" / "tests" / "parity.rs").read_text(encoding="utf-8")
    for name in ("dsl_golden.json", "bgl_golden.json", "fuzzy_golden.json"):
        assert name in rs


def _golden(name: str):
    return json.loads((PARITY / name).read_text(encoding="utf-8"))


def test_article_text_golden() -> None:
    """HTML → text (CLI output, copy article) — the Rust edition checks the same file."""
    from dictz.core.htmltext import to_text

    for html, want in _golden("text_golden.json"):
        assert to_text(html) == want, html


def test_settings_schema_golden() -> None:
    from dictz.storage.settings import schema_as_json

    assert json.loads(json.dumps(schema_as_json(), ensure_ascii=False)) == _golden("settings_schema.json")


def test_cli_commands_golden() -> None:
    from dictz.cli import COMMANDS

    assert [list(c) for c in COMMANDS] == _golden("cli_commands.json")


@pytest.mark.parametrize("shell", ["bash", "zsh", "fish", "powershell"])
def test_completion_golden(shell: str) -> None:
    from dictz.cli import completion_script

    want = (PARITY / f"completion_{shell}.txt").read_text(encoding="utf-8")
    assert completion_script(shell) == want
    rs = Path(__file__).resolve().parent.parent / "dictz-rs" / "src" / "completion" / f"{shell}.txt"
    assert rs.read_text(encoding="utf-8") == want, "run tests/data/parity/make_samples.py"


def test_shortcuts_golden() -> None:
    from dictz.core.shortcuts import SHORTCUTS

    assert [list(s) for s in SHORTCUTS] == _golden("shortcuts.json")
