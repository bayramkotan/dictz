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


def test_scan_golden() -> None:
    from dictz.core.hotkeys import parse_hotkey
    from dictz.core.icon import app_icon_rgba
    from dictz.core.scantext import DoubleCopy, clean_scan_text

    g = _golden("scan_golden.json")
    for text, want in g["clean"]:
        assert clean_scan_text(text, 80) == want, text
    d = DoubleCopy(500)
    for text, ms, want in g["double_copy_500"]:
        assert d.feed(text, ms) == want
    for text, want in g["hotkeys"]:
        try:
            h = parse_hotkey(text)
            assert want["ok"] and str(h) == want["text"] and h.key == want["key"]
            assert sorted(h.mods) == want["mods"]
        except ValueError as exc:
            assert not want["ok"] and str(exc) == want["error"]
    h = 0xcbf29ce484222325
    for b in app_icon_rgba(64):
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    assert f"{h:016x}" == g["icon_fnv1a64"]


def test_dictident_golden() -> None:
    from dictz.core import dictident as di

    g = _golden("dictident_golden.json")
    for name, mono, col, langs in g["names"]:
        assert di.monogram(name) == mono, name
        assert di.color(name) == col, name
        assert list(di.languages_from_name(name) or []) == langs, name
    for c, want in g["language_code"]:
        assert di.language_code(c) == want, c
    for a, b, n, want in g["pair"]:
        assert list(di.language_pair(a, b, n) or []) == want
    icon = di.bgl_icon(PARITY / "sample.bgl") or b""
    assert f"{di.fnv1a64(icon):016x}" == g["bgl_icon_fnv1a64"]
    assert di.icon_bytes(PARITY / "sample.bgl")[1] == ".ico"


def test_dictionary_icon_next_to_file(tmp_path) -> None:
    from dictz.core.dictident import icon_bytes, icon_file

    (tmp_path / "Lingvo.dsl.dz").write_bytes(b"x")
    assert icon_file(tmp_path / "Lingvo.dsl.dz") is None
    (tmp_path / "Lingvo.PNG").write_bytes(b"png")
    (tmp_path / "Lingvo.bmp").write_bytes(b"bmp")
    assert icon_bytes(tmp_path / "Lingvo.dsl.dz") == (b"png", ".png")
    (tmp_path / "Lingvo.ico").write_bytes(b"ico")
    assert icon_bytes(tmp_path / "Lingvo.dsl.dz")[1] == ".ico"
