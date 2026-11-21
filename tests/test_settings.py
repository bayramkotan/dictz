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


def test_autostart_entry_on_linux(tmp_path, monkeypatch) -> None:
    import sys

    import pytest

    if sys.platform in ("win32", "darwin"):
        pytest.skip("Linux autostart file")
    from dictz.core import autostart

    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    assert autostart.sync(True, tmp_path) == ""
    text = (tmp_path / "autostart" / "dictz.desktop").read_text(encoding="utf-8")
    assert "--autostart" in text and "X-GNOME-Autostart-enabled=true" in text
    assert autostart.is_enabled()
    assert autostart.sync(False) == ""
    assert not autostart.is_enabled()


def test_autostart_flag_is_not_a_cli_command(monkeypatch) -> None:
    import dictz.app as app

    seen = {}
    monkeypatch.setattr(app, "run_gui", lambda files, autostart=False: seen.update(f=files, a=autostart) or 0)
    assert app.main(["--autostart"]) == 0
    assert seen == {"f": [], "a": True}


def test_old_default_sizes_move_to_the_compact_ones_once(tmp_path) -> None:
    import json

    from dictz.storage.settings import Preferences

    f = tmp_path / "settings.json"
    f.write_text(json.dumps({"appearance": {"ui_font_size": 14}, "article": {"font_size": 18, "line_spacing": 1.6}}))
    p = Preferences.load(f)
    assert p["appearance.ui_font_size"] == 13  # old default → new default
    assert p["article.font_size"] == 18  # the user's own value stays
    assert p["article.line_spacing"] == 1.45
    p.save()
    p.set("appearance.ui_font_size", 14, save=True)
    assert Preferences.load(f)["appearance.ui_font_size"] == 14  # chosen again: kept
