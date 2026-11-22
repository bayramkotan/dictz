"""The shared theme table (dictz/resources/themes.json)."""
from __future__ import annotations

import json
import re
from pathlib import Path

from dictz.core import themes
from dictz.storage.settings import SCHEMA_BY_KEY, Preferences

ROOT = Path(__file__).resolve().parents[1]


def test_every_theme_is_complete() -> None:
    ts = themes.all_themes()
    assert len(ts) >= 20
    assert len({t["id"] for t in ts}) == len(ts)
    assert {"light", "dark"} <= set(themes.ids())
    hexc = re.compile(r"^#[0-9a-f]{6}$")
    for t in ts:
        assert hexc.match(t["accent"]), t["id"]
        for k in themes.UI_KEYS:
            assert hexc.match(t["ui"][k]), (t["id"], k)
        for k in themes.ARTICLE_KEYS:
            assert hexc.match(t["article"][k]), (t["id"], k)


def test_choices_and_rust_copy() -> None:
    assert SCHEMA_BY_KEY["appearance.theme"].choices == themes.choices()
    rust = ROOT / "dictz-rs" / "src" / "gui" / "themes.json"
    assert json.loads(rust.read_text("utf-8")) == json.loads((ROOT / "dictz" / "resources" / "themes.json")
                                                             .read_text("utf-8"))


def test_palette_and_accent(tmp_path: Path) -> None:
    from dictz.ui import themes as ui

    p = Preferences.load(tmp_path / "settings.json")
    p.set("appearance.theme", "nord")
    c = ui.palette(p)
    assert c["bg"] == "#2e3440" and c["accent"] == "#88c0d0"  # the theme's own accent
    p.set("appearance.accent_color", "#ff0000")
    assert ui.palette(p)["accent"] == "#ff0000"  # the user's wins
    p.set("appearance.theme", "light")
    assert "#ffffff" in ui.definition_css(p)
    assert themes.resolve("nope") == "light" and themes.resolve("auto", True) == "dark"
