"""Interface languages: every text of both editions is translated, catalogues match."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "data" / "parity"))

from dictz.core import i18n  # noqa: E402


def _catalog(code: str) -> dict[str, str]:
    return json.loads((i18n.I18N_DIR / f"{code}.json").read_text(encoding="utf-8"))


def test_every_language_has_a_catalogue_and_a_flag() -> None:
    for code, *_ in i18n.LANGUAGES:
        assert i18n.flag_path(code).is_file(), code
        if code != "tr":
            assert (i18n.I18N_DIR / f"{code}.json").is_file(), code
    assert len(i18n.LANGUAGES) >= 20


@pytest.mark.parametrize("code", [c for c, *_ in i18n.LANGUAGES if c != "tr"])
def test_catalogue_is_complete(code: str) -> None:
    import i18n_keys

    cat = _catalog(code)
    missing = [k for k in i18n_keys.all_keys() if k not in cat]
    assert not missing, f"{code}: {len(missing)} texts not translated, e.g. {missing[:5]}"
    for k, v in cat.items():
        assert v.strip(), (code, k)
        assert sorted(re.findall(r"\{\w+\}", k)) == sorted(re.findall(r"\{\w+\}", v)), (code, k, v)
        assert k.count("\n") == v.count("\n"), (code, k)


def test_rust_catalogue_is_the_same() -> None:
    rust = json.loads((ROOT / "dictz-rs" / "src" / "i18n" / "catalog.json").read_text(encoding="utf-8"))
    for code, *_ in i18n.LANGUAGES:
        if code != "tr":
            assert rust[code] == _catalog(code), code
    for f in (ROOT / "dictz" / "resources" / "flags").glob("*.png"):
        assert (ROOT / "dictz-rs" / "src" / "flags" / f.name).read_bytes() == f.read_bytes()
    rs_langs = re.findall(r'\("(\w+)", "[^"]+", "[^"]+", (?:true|false)\)',
                          (ROOT / "dictz-rs" / "src" / "i18n.rs").read_text(encoding="utf-8"))
    assert rs_langs == [c for c, *_ in i18n.LANGUAGES]


def test_translate_and_fallback(monkeypatch) -> None:
    try:
        assert i18n.set_language("en") == "en"
        assert i18n.t("&Dosya") == "&File"
        assert i18n.t("{n} sözlük yüklendi", n=3) == "3 dictionaries loaded"
        assert i18n.t("Hiç çevrilmemiş metin") == "Hiç çevrilmemiş metin"
        i18n.set_language("de")
        assert i18n.t("Tercihler") == "Einstellungen"
        i18n.set_language("ar")
        assert i18n.is_rtl() and i18n.t("Tercihler") == "التفضيلات"
        monkeypatch.setenv("LC_ALL", "kk_KZ.UTF-8")
        assert i18n.set_language("auto") == "kk"
        monkeypatch.setenv("LC_ALL", "xx_YY.UTF-8")
        assert i18n.set_language("auto") == "en"
        assert i18n._code_of("Turkish_Türkiye") == "tr"
    finally:
        i18n.set_language("tr")
    assert i18n.t("Tercihler") == "Tercihler"


def test_language_setting() -> None:
    from dictz.storage.settings import SCHEMA

    s = next(s for s in SCHEMA if s.key == "general.language")
    assert s.default == "auto" and [c for c, _ in s.choices][1:] == [c for c, *_ in i18n.LANGUAGES]
