"""Languages: ISO 639 codes and names (``dictz/resources/languages.json``,
the ISO 639-3 table; the Rust edition embeds the same file).

:func:`norm` turns whatever a catalog says — ``eng``, ``en``, ``English``,
``İngilizce``, ``Greek, Modern (1453-)`` — into one code: the two-letter
ISO 639-1 code when the language has one, else the ISO 639-3 code, else ''.
"""
from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path

_FILE = Path(__file__).resolve().parent.parent / "resources" / "languages.json"

# other names a catalog may use (Turkish names, common short forms)
_ALIASES = {
    "ingilizce": "en", "türkçe": "tr", "turkce": "tr", "almanca": "de", "fransızca": "fr",
    "fransizca": "fr", "ispanyolca": "es", "italyanca": "it", "rusça": "ru", "rusca": "ru",
    "arapça": "ar", "arapca": "ar", "farsça": "fa", "farsca": "fa", "farsi": "fa", "osmanlıca": "ota",
    "osmanlica": "ota", "japonca": "ja", "çince": "zh", "cince": "zh", "korece": "ko",
    "azerice": "az", "kürtçe": "ku", "kurtce": "ku", "yunanca": "el", "latince": "la",
    "greek": "el", "modern greek": "el", "ancient greek": "grc", "chinese": "zh", "mandarin": "zh",
    "malay": "ms", "persian": "fa", "kurdish": "ku", "swahili": "sw", "nepali": "ne",
    "serbo-croatian": "sh", "norwegian": "no", "albanian": "sq", "estonian": "et", "latvian": "lv",
    "uzbek": "uz", "mongolian": "mn", "quechua": "qu", "pashto": "ps", "punjabi": "pa",
    "panjabi": "pa", "oriya": "or", "old english": "ang", "middle english": "enm",
    "old norse": "non", "old french": "fro", "middle french": "frm", "old high german": "goh",
    "middle high german": "gmh", "classical syriac": "syc", "ottoman turkish": "ota",
    "abkhaz": "ab", "kyrgyz": "ky", "kirghiz": "ky", "uyghur": "ug", "uighur": "ug",
    "tatar": "tt", "bashkir": "ba", "chuvash": "cv", "turkmen": "tk", "kazakh": "kk",
    "azerbaijani": "az", "crimean tatar": "crh", "gagauz": "gag", "yakut": "sah", "tuvan": "tyv",
}


@lru_cache(maxsize=1)
def table() -> tuple[dict[str, tuple[str, str, str]], dict[str, str]]:
    """(code → (639-3, 639-1, English name), folded name → code)."""
    rows = json.loads(_FILE.read_text(encoding="utf-8"))
    by_code: dict[str, tuple[str, str, str]] = {}
    by_name: dict[str, str] = {}
    for a3, a2, name in rows:
        code = a2 or a3
        by_code[a3] = (a3, a2, name)
        if a2:
            by_code[a2] = (a3, a2, name)
        for key in _name_keys(name):
            # a language with a two-letter code wins a shared name
            if key not in by_name or (a2 and len(by_name[key]) == 3):
                by_name[key] = code
    for k, v in _ALIASES.items():
        by_name[k] = v
    return by_code, by_name


def _name_keys(name: str) -> list[str]:
    keys = []
    for n in name.lower().split("; "):  # "Ossetian; Ossetic"
        keys.append(n)
        base = n.split(" (", 1)[0]  # "Modern Greek (1453-)"
        keys.append(base)
        if ", " in base:  # "Greek, Modern" → "modern greek", "greek"
            a, b = base.split(", ", 1)
            keys += [f"{b} {a}", a]
    return keys


def norm(value: str) -> str:
    """One language code for ``value`` ('' when unknown)."""
    v = (value or "").strip()
    if not v:
        return ""
    by_code, by_name = table()
    low = v.lower().replace("_", "-")
    if low in by_code:
        a3, a2, _n = by_code[low]
        return a2 or a3
    head = low.split("-", 1)[0]  # en-US, pt-BR
    if len(head) in (2, 3) and head in by_code:
        a3, a2, _n = by_code[head]
        return a2 or a3
    folded = low.replace("̇", "")
    return by_name.get(folded, by_name.get(folded.split(" (", 1)[0], ""))


def name(code: str) -> str:
    """English name of a code, without the table's notes ('' when unknown):
    ``el`` → ``Modern Greek``."""
    hit = table()[0].get((code or "").lower())
    return hit[2].split(" (", 1)[0].split("; ", 1)[0] if hit else ""


def label(code: str) -> str:
    """``English (en)``; the code itself when the name is unknown."""
    n = name(code)
    return f"{n} ({code})" if n else code
