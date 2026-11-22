"""Sources (servers) the catalog is read from: built-in ones and the user's.

``DATA/center/sources.json``::

    {"version": 1, "default": "dictz-data", "removed": [...built-in ids...],
     "sources": [{...SOURCE_KEYS...}, ...]}

The list order is the catalog's order (the first source wins a duplicate).
Built-in sources are written into the file the first time; a newer dictz adds
its new built-in sources unless the user removed them (``removed``). Every
built-in source can be edited (its address may move), switched off or
removed, and "reset" brings them back.

Passwords and tokens never go into this file: ``credentials.json`` next to it
(only readable by the user where the system allows it), or environment
variables ``DICTZ_SOURCE_<ID>_PASSWORD`` / ``DICTZ_SOURCE_<ID>_TOKEN``
(``<ID>`` upper-case, ``-`` → ``_``), which win over the file.
"""
from __future__ import annotations

import contextlib
import os
import re
from pathlib import Path
from typing import Any

from . import CENTER_DIR
from .store import JsonFile

TYPES = ("dictz-catalog", "freedict", "github-releases", "github-folder", "http-index", "webdav",
         "nextcloud-share", "archive-org", "local", "telegram")
AUTH = ("none", "basic", "bearer", "header")
SOURCE_KEYS = ("id", "name", "type", "url", "enabled", "builtin", "homepage", "description", "license",
               "source_lang", "target_lang", "filter", "name_pattern", "depth", "auth", "user", "notes")
DEFAULT_SERVER = "dictz-data"


def _src(**kw: Any) -> dict:
    s = dict.fromkeys(SOURCE_KEYS, "")
    s.update(enabled=True, builtin=True, depth=2, auth="none")
    s.update(kw)
    return s


# The built-in sources. Only the first is ours; the rest are other people's
# servers, read the way they publish (their addresses may change: every one
# can be edited in the Sources page).
BUILTIN: tuple[dict, ...] = (
    _src(id="dictz-data", name="dictz-data (default server)", type="dictz-catalog",
         url="https://raw.githubusercontent.com/bayramkotan/dictz/main/docs/dictz-data/catalog.json",
         homepage="https://github.com/bayramkotan/dictz/tree/main/docs/dictz-data",
         description="dictz's own dictionary server: every dictionary dictz offers, with its licence."),
    _src(id="freedict", name="FreeDict", type="freedict", url="https://freedict.org/freedict-database.json",
         homepage="https://freedict.org/", license="freedict",
         description="Free bilingual dictionaries, about 200 language pairs (StarDict, dictd, slob)."),
    _src(id="wiktionary-vuizur", name="Wiktionary Dictionaries (Vuizur)", type="github-folder",
         url="https://github.com/Vuizur/Wiktionary-Dictionaries", homepage="https://github.com/Vuizur/Wiktionary-Dictionaries",
         license="CC-BY-SA-3.0", filter=r" stardict\.tar\.gz$", target_lang="en", depth=1,
         name_pattern=r"^(?P<src>.+?)-(?P<dst>English) Wiktionary dictionary",
         description="Wiktionary turned into StarDict: almost every language → English (kaikki.org / wiktextract)."),
    _src(id="wiktionary-xxyzz", name="Wiktionary StarDict (xxyzz)", type="github-releases",
         url="https://github.com/xxyzz/wiktionary_stardict", homepage="https://github.com/xxyzz/wiktionary_stardict",
         license="wiktionary", filter=r"\.(zip|tar\.gz|tar\.xz|tar\.bz2)$",
         description="Wiktionary dictionaries for KOReader and e-readers, rebuilt monthly."),
    _src(id="ecdict", name="ECDICT (English → Chinese)", type="github-releases",
         url="https://github.com/skywind3000/ECDICT", homepage="https://github.com/skywind3000/ECDICT",
         license="MIT", source_lang="en", target_lang="zh", filter=r"(?i)stardict.*\.(zip|7z|tar\.gz)$",
         description="Free English–Chinese dictionary database, with a StarDict build."),
    _src(id="huzheng", name="StarDict archive (download.huzheng.org)", type="http-index",
         url="http://download.huzheng.org/", homepage="http://download.huzheng.org/", license="unknown",
         filter=r"\.(tar\.bz2|tar\.gz|tgz|zip)$", depth=2,
         description="The oldest, largest StarDict archive, many languages. Licences are mostly not stated."),
    _src(id="archive-org", name="Internet Archive", type="archive-org", url="https://archive.org",
         homepage="https://archive.org/", license="unknown",
         filter=r'(stardict OR "babylon bgl" OR "lingvo dsl" OR goldendict) AND mediatype:(data OR software OR texts)',
         description="Dictionary packs uploaded by the community (StarDict, Babylon, DSL). Licences vary."),
    _src(id="freemdict", name="FreeMdict cloud", type="nextcloud-share",
         url="https://cloud.freemdict.com/index.php/s/pgKcDcbSDTCzXCs", homepage="https://forum.freemdict.com/",
         license="unknown", depth=2,
         description="The FreeMdict community's shared folder: MDict, StarDict and DSL packs. Licences vary."),
    _src(id="telegram-goldendict", name="Telegram: Dictionaries_GoldenDict", type="telegram",
         url="https://t.me/s/Dictionaries_GoldenDict", homepage="https://t.me/Dictionaries_GoldenDict",
         license="unknown",
         description="A channel with packs in 40+ languages. Listed here; the files open in Telegram."),
)


def center_dir(data_dir: Path | str) -> Path:
    return Path(data_dir) / CENTER_DIR


def clean(s: dict) -> dict:
    """A source with every key, the right types."""
    out = dict.fromkeys(SOURCE_KEYS, "")
    out.update(enabled=True, builtin=False, depth=2, auth="none")
    for k in SOURCE_KEYS:
        if k in s and s[k] is not None:
            out[k] = s[k]
    out["id"] = slug(str(out["id"]) or str(out["name"]) or str(out["url"]))
    for k in ("name", "type", "url", "homepage", "description", "license", "source_lang", "target_lang",
              "filter", "name_pattern", "auth", "user", "notes"):
        out[k] = str(out[k]).strip()
    out["enabled"] = bool(out["enabled"])
    out["builtin"] = bool(out["builtin"])
    try:
        out["depth"] = max(0, min(8, int(out["depth"])))
    except (TypeError, ValueError):
        out["depth"] = 2
    if out["auth"] not in AUTH:
        out["auth"] = "none"
    out["name"] = out["name"] or out["id"]
    return out


def slug(text: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return s[:48] or "source"


def guess_type(url: str) -> str:
    """The type a pasted address most likely is."""
    u = url.strip().lower()
    if u.endswith("freedict-database.json"):
        return "freedict"
    if u.endswith(".json"):
        return "dictz-catalog"
    if "github.com/" in u and "/releases" in u:
        return "github-releases"
    if "github.com/" in u:
        return "github-folder"
    if "/index.php/s/" in u or "/s/" in u and "cloud" in u:
        return "nextcloud-share"
    if "archive.org" in u:
        return "archive-org"
    if "t.me/" in u:
        return "telegram"
    if u.startswith(("webdav://", "webdavs://")) or "/remote.php/dav" in u:
        return "webdav"
    if u.startswith(("http://", "https://")):
        return "http-index"
    return "local"


class Sources:
    """``sources.json`` + ``credentials.json``."""

    def __init__(self, data_dir: Path | str):
        self.dir = center_dir(data_dir)
        self.file = JsonFile(self.dir / "sources.json", self._empty)
        self.secrets = JsonFile(self.dir / "credentials.json", dict)

    @staticmethod
    def _empty() -> dict:
        return {"version": 1, "default": DEFAULT_SERVER, "removed": [], "sources": []}

    @staticmethod
    def _complete(data: dict) -> tuple[dict, bool]:
        """Add built-in sources the file does not have yet (unless removed)."""
        if not isinstance(data, dict):
            data = Sources._empty()
        data.setdefault("version", 1)
        data.setdefault("default", DEFAULT_SERVER)
        data.setdefault("removed", [])
        rows = [clean(s) for s in data.get("sources") or [] if isinstance(s, dict)]
        have = {s["id"] for s in rows}
        changed = False
        for i, b in enumerate(BUILTIN):
            if b["id"] not in have and b["id"] not in data["removed"]:
                rows.insert(min(i, len(rows)), clean(b))
                changed = True
        data["sources"] = rows
        return data, changed

    def data(self) -> dict:
        raw = self.file.read()
        data, changed = self._complete(raw)
        if changed or not self.file.path.exists():
            with contextlib.suppress(OSError):
                self.file.update(lambda d: self._complete(d)[0])
        return data

    def all(self) -> list[dict]:
        return self.data()["sources"]

    def enabled(self) -> list[dict]:
        return [s for s in self.all() if s["enabled"]]

    def get(self, sid: str) -> dict | None:
        return next((s for s in self.all() if s["id"] == sid), None)

    def find(self, key: str) -> dict | None:
        """By id or (case-insensitive) name."""
        k = key.strip().lower()
        return next((s for s in self.all() if s["id"] == k or s["name"].lower() == k), None)

    def default_id(self) -> str:
        return str(self.data().get("default") or DEFAULT_SERVER)

    # ------------------------------------------------------------ changes
    def _change(self, fn) -> dict:
        def go(d):
            d, _ = self._complete(d)
            fn(d)
            return d

        return self.file.update(go)

    def add(self, source: dict) -> dict:
        s = clean({**source, "builtin": False})
        taken = {x["id"] for x in self.all()}
        base, n = s["id"], 2
        while s["id"] in taken:
            s["id"] = f"{base}-{n}"
            n += 1
        self._change(lambda d: d["sources"].append(s))
        return s

    def update(self, sid: str, **fields: Any) -> dict | None:
        out: list[dict] = []

        def fn(d):
            for i, s in enumerate(d["sources"]):
                if s["id"] == sid:
                    new = clean({**s, **{k: v for k, v in fields.items() if k in SOURCE_KEYS and k != "id"}})
                    new["builtin"] = s["builtin"]
                    d["sources"][i] = new
                    out.append(new)

        self._change(fn)
        return out[0] if out else None

    def set_enabled(self, sid: str, on: bool) -> None:
        self.update(sid, enabled=on)

    def remove(self, sid: str) -> bool:
        found = []

        def fn(d):
            for s in list(d["sources"]):
                if s["id"] == sid:
                    d["sources"].remove(s)
                    found.append(s)
                    if s["builtin"] and sid not in d["removed"]:
                        d["removed"].append(sid)
            if d.get("default") == sid:
                d["default"] = d["sources"][0]["id"] if d["sources"] else DEFAULT_SERVER

        self._change(fn)
        self.set_secret(sid, None, None)
        return bool(found)

    def move(self, sid: str, delta: int) -> None:
        def fn(d):
            rows = d["sources"]
            i = next((k for k, s in enumerate(rows) if s["id"] == sid), None)
            if i is None:
                return
            j = max(0, min(len(rows) - 1, i + delta))
            rows.insert(j, rows.pop(i))

        self._change(fn)

    def set_default(self, sid: str) -> None:
        """The default server: listed first, read first."""
        def fn(d):
            if any(s["id"] == sid for s in d["sources"]):
                d["default"] = sid
                rows = d["sources"]
                i = next(k for k, s in enumerate(rows) if s["id"] == sid)
                rows.insert(0, rows.pop(i))

        self._change(fn)

    def reset(self) -> None:
        """Built-in sources back as shipped (the user's own stay)."""
        def fn(d):
            mine = [s for s in d["sources"] if not s["builtin"]]
            d["sources"] = [clean(b) for b in BUILTIN] + mine
            d["removed"] = []
            d["default"] = DEFAULT_SERVER

        self._change(fn)

    # ------------------------------------------------------------ secrets
    def secret(self, sid: str) -> tuple[str, str]:
        """(password, token) for a source: environment first, then the file."""
        env = "DICTZ_SOURCE_" + sid.upper().replace("-", "_")
        stored = self.secrets.read().get(sid, {}) if isinstance(self.secrets.read(), dict) else {}
        pw = os.environ.get(env + "_PASSWORD", stored.get("password", ""))
        tok = os.environ.get(env + "_TOKEN", stored.get("token", ""))
        return pw, tok

    def set_secret(self, sid: str, password: str | None, token: str | None) -> None:
        def fn(d):
            if not isinstance(d, dict):
                d = {}
            if not password and not token:
                d.pop(sid, None)
            else:
                d[sid] = {"password": password or "", "token": token or ""}
            return d

        if not password and not token and not self.secrets.path.exists():
            return
        self.secrets.update(fn)
        with contextlib.suppress(OSError):
            os.chmod(self.secrets.path, 0o600)

    # ------------------------------------------------------------ share
    def export(self, path: Path | str, include_builtin: bool = True) -> int:
        """Write the list (never passwords) to ``path``; returns the count."""
        rows = [s for s in self.all() if include_builtin or not s["builtin"]]
        data = {"dictz-sources": 1, "default": self.default_id(), "sources": rows}
        JsonFile(Path(path), dict).write(data)
        return len(rows)

    def import_(self, path: Path | str, replace: bool = False) -> int:
        """Add the sources of an exported list (same id = updated); ``replace``
        drops the user's own sources first. Returns how many were added or updated."""
        raw = JsonFile(Path(path), dict).read()
        rows = raw.get("sources") if isinstance(raw, dict) else None
        if not isinstance(rows, list):
            raise ValueError("not a dictz source list")
        incoming = [clean(s) for s in rows if isinstance(s, dict)]

        def fn(d):
            if replace:
                d["sources"] = [s for s in d["sources"] if s["builtin"]]
            for s in incoming:
                i = next((k for k, x in enumerate(d["sources"]) if x["id"] == s["id"]), None)
                if i is None:
                    d["sources"].append(s)
                else:
                    s["builtin"] = d["sources"][i]["builtin"]
                    d["sources"][i] = s

        self._change(fn)
        return len(incoming)
