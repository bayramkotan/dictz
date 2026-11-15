"""settings.json — shared by both editions."""
from __future__ import annotations

import json

import pytest

from dictz.storage.settings import SCHEMA, SECTIONS, Preferences, coerce


def setting(key):
    return next(s for s in SCHEMA if s.key == key)


def test_every_setting_belongs_to_a_section() -> None:
    ids = {s for s, _ in SECTIONS}
    assert all(s.key.split(".")[0] in ids for s in SCHEMA)
    assert len({s.key for s in SCHEMA}) == len(SCHEMA)
    assert len(SCHEMA) >= 50


def test_coerce_rules() -> None:
    assert coerce(setting("search.fuzzy"), "evet") is True
    assert coerce(setting("search.fuzzy"), "off") is False
    assert coerce(setting("article.font_size"), "99") == 48  # clamped
    assert coerce(setting("article.font_size"), 16.5) == 16  # banker's rounding, like Rust
    assert coerce(setting("appearance.accent_color"), "#AABBCC") == "#aabbcc"
    with pytest.raises(ValueError, match="got 'x'"):
        coerce(setting("search.fuzzy"), "x")
    with pytest.raises(ValueError):
        coerce(setting("appearance.theme"), "blue")


def test_roundtrip_keeps_unknown_keys(tmp_path) -> None:
    f = tmp_path / "settings.json"
    f.write_text(json.dumps({"other": {"keep": 1}, "appearance": {"theme": "bogus"}}), encoding="utf-8")
    p = Preferences.load(f)
    assert p["appearance.theme"] == "auto"
    p.set("appearance.theme", "dark", save=True)
    data = json.loads(f.read_text(encoding="utf-8"))
    assert data["other"] == {"keep": 1}
    assert data["appearance"]["theme"] == "dark"
    assert list(data) == sorted(data), "keys are sorted (same file as the Rust edition)"
    p.reset("appearance.theme")
    assert p["appearance.theme"] == "auto"


def test_export_import(tmp_path) -> None:
    p = Preferences.load(tmp_path / "a.json")
    p.set("search.debounce_ms", 300)
    q = Preferences.load(tmp_path / "b.json")
    assert q.import_dict(p.export()) == []
    assert q["search.debounce_ms"] == 300
