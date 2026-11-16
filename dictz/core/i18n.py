"""Interface languages (same catalogue and rules as the Rust edition, ``dictz-rs/src/i18n.rs``).

The interface is written in Turkish; ``t("Sözlük ekle…")`` returns the text in
the chosen language. Catalogues are ``dictz/resources/i18n/<code>.json``
({Turkish text: translation}); a missing entry falls back to English, then to
the Turkish text itself. ``{name}`` placeholders are filled by ``t(...).format()``
or ``t(text, name=value)``. An ``&`` (Qt menu mnemonic) is ignored when looking
up and put back in front of the translation.

The setting ``general.language`` is a code from :data:`LANGUAGES` or ``auto``
(the system language when it is in the list, otherwise English).
Nothing here imports Qt.
"""
from __future__ import annotations

import json
import locale
import os
import sys
from pathlib import Path

I18N_DIR = Path(__file__).resolve().parent.parent / "resources" / "i18n"
FLAGS_DIR = Path(__file__).resolve().parent.parent / "resources" / "flags"

# (code, name in its own language, English name, right-to-left)
LANGUAGES: list[tuple[str, str, str, bool]] = [
    ("tr", "Türkçe", "Turkish", False),
    ("en", "English", "English", False),
    ("az", "Azərbaycan türkcəsi", "Azerbaijani Turkish", False),
    ("de", "Deutsch", "German", False),
    ("fr", "Français", "French", False),
    ("es", "Español", "Spanish", False),
    ("it", "Italiano", "Italian", False),
    ("pt", "Português", "Portuguese", False),
    ("nl", "Nederlands", "Dutch", False),
    ("pl", "Polski", "Polish", False),
    ("ru", "Русский", "Russian", False),
    ("uk", "Українська", "Ukrainian", False),
    ("ar", "العربية", "Arabic", True),
    ("fa", "فارسی", "Persian", True),
    ("ur", "اردو", "Urdu", True),
    ("kk", "Қазақша", "Kazakh", False),
    ("ky", "Кыргызча", "Kyrgyz", False),
    ("uz", "Oʻzbekcha", "Uzbek", False),
    ("tk", "Türkmençe", "Turkmen", False),
    ("zh", "中文", "Chinese", False),
    ("ja", "日本語", "Japanese", False),
    ("ko", "한국어", "Korean", False),
    ("id", "Bahasa Indonesia", "Indonesian", False),
    ("ms", "Bahasa Melayu", "Malay", False),
    ("hi", "हिन्दी", "Hindi", False),
]
CODES = [c for c, *_ in LANGUAGES]

_lang = "tr"
_catalog: dict[str, str] = {}
_fallback: dict[str, str] = {}


def load_catalog(code: str) -> dict[str, str]:
    if code == "tr":
        return {}
    try:
        return json.loads((I18N_DIR / f"{code}.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def _code_of(name: str) -> str:
    """``tr_TR.UTF-8`` / ``Turkish_Türkiye`` / ``tr-TR`` → ``tr`` (empty when unknown)."""
    head = name.split(":")[0].split(".")[0].replace("-", "_").split("_")[0].strip().lower()
    if head in CODES:
        return head
    by_english = {e.lower(): c for c, _n, e, _r in LANGUAGES}
    return by_english.get(head, "")


def system_language() -> str:
    """The system's language code if dictz has it, otherwise ``en``."""
    for var in ("LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"):
        v = os.environ.get(var, "")
        if v and v not in ("C", "POSIX", "C.UTF-8"):
            return _code_of(v) or "en"
    try:
        loc = locale.getlocale()[0] or ""
    except ValueError:
        loc = ""
    if not loc and sys.platform == "darwin":  # GUI apps on macOS get no LANG
        import subprocess

        try:
            loc = subprocess.run(["defaults", "read", "-g", "AppleLocale"], capture_output=True, text=True,
                                 timeout=5).stdout.strip()
        except (OSError, subprocess.SubprocessError):
            loc = ""
    return _code_of(loc) or "en"


def resolve(setting: str) -> str:
    """``auto`` → the system language; unknown codes → ``en``."""
    if setting in CODES:
        return setting
    return system_language()


def set_language(setting: str) -> str:
    """Switch the interface language; returns the code in use."""
    global _lang, _catalog, _fallback
    _lang = resolve(setting)
    _catalog = load_catalog(_lang)
    _fallback = load_catalog("en") if _lang not in ("tr", "en") else {}
    return _lang


def current() -> str:
    return _lang


def is_rtl(code: str | None = None) -> bool:
    code = code or _lang
    return any(c == code and rtl for c, _n, _e, rtl in LANGUAGES)


def native_name(code: str) -> str:
    return next((n for c, n, _e, _r in LANGUAGES if c == code), code)


def flag_path(code: str) -> Path:
    return FLAGS_DIR / f"{code}.png"


def t(text: str, **kwargs) -> str:
    """``text`` (Turkish) in the interface language."""
    if "&" in text and "&&" not in text and "&lt;" not in text and "&gt;" not in text:
        found = _catalog.get(text.replace("&", "")) or _fallback.get(text.replace("&", ""))
        out = "&" + found if found else text
    else:
        out = _catalog.get(text) or _fallback.get(text) or text
    return out.format(**kwargs) if kwargs else out
