"""The interface themes, shared by both editions.

``dictz/resources/themes.json`` lists them (``dictz-rs/src/gui/themes.json``
is a copy made by ``tests/data/parity/make_samples.py``). Each theme has an
id, a name (a Turkish interface text, translated like the others), whether
it is dark, its own accent colour, the window colours (``ui``) and,
optionally, the article page colours (``article``; otherwise taken from
``ui``). "auto" is not a theme: it follows the desktop, light or dark.
"""
from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path

_FILE = Path(__file__).resolve().parent.parent / "resources" / "themes.json"
DEFAULT_ACCENT = "#2563eb"
UI_KEYS = ("bg", "panel", "border", "text", "muted", "hover", "header", "icon")
ARTICLE_KEYS = ("bg", "fg", "muted", "card", "border", "strip")


def _mix(a: str, b: str, t: float) -> str:
    x = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    y = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(p + (q - p) * t):02x}" for p, q in zip(x, y, strict=True))


def article_colors(t: dict) -> dict[str, str]:
    """The article page colours of a theme (its own, or made from ``ui``)."""
    if t.get("article"):
        return dict(t["article"])
    u = t["ui"]
    bg = _mix(u["bg"], "#000000", 0.12) if t["dark"] else u["panel"]
    return {"bg": bg, "fg": u["text"], "muted": u["muted"], "card": u["header"], "border": u["border"],
            "strip": u["header"]}


@lru_cache(maxsize=1)
def all_themes() -> tuple[dict, ...]:
    raw = json.loads(_FILE.read_text(encoding="utf-8"))
    out = []
    for t in raw["themes"]:
        t = dict(t)
        t["article"] = article_colors(t)
        out.append(t)
    return tuple(out)


def ids() -> list[str]:
    return [t["id"] for t in all_themes()]


def resolve(theme_id: str, system_dark: bool = False) -> str:
    """"auto" → "light" / "dark"; an unknown id → "light"."""
    if theme_id == "auto":
        return "dark" if system_dark else "light"
    return theme_id if theme_id in ids() else "light"


def theme(theme_id: str) -> dict:
    tid = resolve(theme_id)
    return next(t for t in all_themes() if t["id"] == tid)


def choices() -> tuple[tuple[str, str], ...]:
    """The ``appearance.theme`` choices: auto, then every theme (light ones first)."""
    ts = all_themes()
    return (("auto", "Otomatik (sistem)"),) + tuple((t["id"], t["name"]) for t in ts if not t["dark"]) \
        + tuple((t["id"], t["name"]) for t in ts if t["dark"])
