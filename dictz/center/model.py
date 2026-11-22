"""Catalog entries: one downloadable dictionary.

An entry is a plain dict with fixed keys (``ENTRY_KEYS``), so it is written
to and read from JSON the same way in both editions. ``normalize`` fills
missing keys, fixes types, normalises languages and licences and works out
the format, kind and whether dictz can open it.
"""
from __future__ import annotations

import re
from typing import Any

from . import langs, licenses

ENTRY_KEYS = (
    "id", "source", "name", "description", "homepage", "page", "source_lang", "target_lang",
    "kind", "format", "supported", "headwords", "version", "date", "size", "files", "license",
    "license_url", "author", "tags", "sample", "resolve",
)
FILE_KEYS = ("name", "url", "size", "sha256", "sha512", "md5", "archive")

# formats dictz opens today; the rest are listed (and can be downloaded) but
# are opened by later versions
SUPPORTED_FORMATS = ("stardict", "dsl", "babylon")
KNOWN_FORMATS = ("stardict", "dsl", "babylon", "mdict", "dictd", "xdxf", "slob", "tabfile", "mobi",
                 "epub", "zim", "lingoes", "json", "archive", "unknown")

# archive by name, longest first
ARCHIVES = ((".tar.gz", "tar.gz"), (".tgz", "tar.gz"), (".tar.xz", "tar.xz"), (".txz", "tar.xz"),
            (".tar.bz2", "tar.bz2"), (".tbz2", "tar.bz2"), (".tbz", "tar.bz2"), (".tar", "tar"),
            (".zip", "zip"), (".7z", "7z"), (".rar", "rar"))


def archive_of(name: str) -> str:
    """``x.tar.xz`` → ``tar.xz``; '' when the file is not an archive."""
    low = name.lower()
    for ext, kind in ARCHIVES:
        if low.endswith(ext):
            return kind
    return ""


_FORMAT_WORDS = (
    ("stardict", "stardict"), ("startdict", "stardict"), ("babylon", "babylon"), ("bgl", "babylon"),
    ("lingvo", "dsl"), ("dsl", "dsl"), ("mdict", "mdict"), ("mdx", "mdict"), ("dictd", "dictd"),
    ("xdxf", "xdxf"), ("slob", "slob"), ("tabfile", "tabfile"), ("tsv", "tabfile"), ("mobi", "mobi"),
    ("kindle", "mobi"), ("epub", "epub"), ("zim", "zim"), ("lingoes", "lingoes"), ("ld2", "lingoes"),
    ("json", "json"),
)


def format_of(name: str) -> str:
    """The dictionary format a file name says (``.ifo`` → stardict, ``…stardict.tar.xz``
    → stardict, ``x.bgl`` → babylon); ``archive`` for an archive that does not say,
    ``unknown`` otherwise."""
    low = name.lower()
    for ext, fmt in ((".ifo", "stardict"), (".dict.dz", "stardict"), (".idx", "stardict"),
                     (".dsl.dz", "dsl"), (".dsl", "dsl"), (".bgl", "babylon"), (".mdx", "mdict"),
                     (".mdd", "mdict"), (".index", "dictd"), (".xdxf", "xdxf"), (".slob", "slob"),
                     (".mobi", "mobi"), (".azw3", "mobi"), (".epub", "epub"), (".zim", "zim"),
                     (".ld2", "lingoes"), (".tsv", "tabfile")):
        if low.endswith(ext):
            return fmt
    words = re.split(r"[^a-z0-9]+", low)
    for word, fmt in _FORMAT_WORDS:
        if word in words:
            return fmt
    return "archive" if archive_of(name) else "unknown"


def is_supported(fmt: str) -> bool | None:
    """``True`` = dictz opens it, ``False`` = not yet, ``None`` = cannot tell
    before unpacking (an archive that does not say)."""
    if fmt in SUPPORTED_FORMATS:
        return True
    if fmt in ("archive", "unknown"):
        return None
    return False


def _int(v: Any) -> int:
    try:
        return max(0, int(str(v).strip().replace(",", "") or 0))
    except (TypeError, ValueError):
        return 0


def _str(v: Any) -> str:
    return "" if v is None else str(v).strip()


def _list(v: Any) -> list:
    if v is None or v == "":
        return []
    if isinstance(v, (list, tuple)):
        return list(v)
    return [v]


def norm_file(f: dict | str) -> dict:
    if isinstance(f, str):
        f = {"url": f}
    url = _str(f.get("url"))
    name = _str(f.get("name")) or url.rstrip("/").rsplit("/", 1)[-1].split("?", 1)[0]
    from urllib.parse import unquote

    name = unquote(name)
    return {
        "name": name,
        "url": url,
        "size": _int(f.get("size")),
        "sha256": _str(f.get("sha256")).lower(),
        "sha512": _str(f.get("sha512")).lower(),
        "md5": _str(f.get("md5")).lower(),
        "archive": _str(f.get("archive")) or archive_of(name),
    }


def kind_of(source: str, targets: list[str]) -> str:
    if not source or not targets:
        return "unknown"
    if len(targets) > 1:
        return "multilingual"
    return "monolingual" if targets[0] == source else "bilingual"


def normalize(e: dict, source_id: str = "", default_license: str = "") -> dict:
    """A complete, canonical entry from what a catalog / adapter gave."""
    files = [norm_file(f) for f in _list(e.get("files"))]
    name = _str(e.get("name")) or (files[0]["name"] if files else "")
    src = langs.norm(_str(e.get("source_lang")))
    targets = []
    for t in _list(e.get("target_lang")):
        code = langs.norm(_str(t))
        if code and code not in targets:
            targets.append(code)
    fmt = _str(e.get("format")).lower()
    if fmt in ("bgl",):
        fmt = "babylon"
    if fmt not in KNOWN_FORMATS:
        fmt = ""
    if not fmt:
        guesses = [format_of(f["name"]) for f in files] + [format_of(name)]
        fmt = next((g for g in guesses if g not in ("archive", "unknown")), "")
        fmt = fmt or ("archive" if any(g == "archive" for g in guesses) else "unknown")
    lic = licenses.norm(_str(e.get("license")) or default_license or "unknown")
    sup = e.get("supported")
    supported = is_supported(fmt) if not isinstance(sup, bool) else sup
    size = _int(e.get("size")) or sum(f["size"] for f in files)
    eid = _str(e.get("id")) or re.sub(r"[^a-z0-9._-]+", "-", name.lower()).strip("-")
    if source_id and not eid.startswith(source_id + ":"):
        eid = f"{source_id}:{eid}"
    out = {
        "id": eid,
        "source": source_id or _str(e.get("source")),
        "name": name,
        "description": _str(e.get("description")),
        "homepage": _str(e.get("homepage")),
        "page": _str(e.get("page")),
        "source_lang": src,
        "target_lang": targets,
        "kind": _str(e.get("kind")) or kind_of(src, targets),
        "format": fmt,
        "supported": supported,
        "headwords": _int(e.get("headwords")),
        "version": _str(e.get("version")),
        "date": _str(e.get("date"))[:10],
        "size": size,
        "files": files,
        "license": lic,
        "license_url": _str(e.get("license_url")) or licenses.get(lic).url,
        "author": _str(e.get("author")),
        "tags": sorted({_str(t) for t in _list(e.get("tags")) if _str(t)}),
        "sample": _str(e.get("sample")),
        "resolve": _str(e.get("resolve")),
    }
    return out


def installable(e: dict) -> bool:
    """Something to download: files, or a link resolved at install time."""
    return bool(e["files"]) or bool(e["resolve"])


def pair_text(e: dict) -> str:
    """``en → tr``, ``en → de, fr``, ``tr`` (monolingual) or ''."""
    if not e["source_lang"]:
        return ""
    if e["kind"] == "monolingual":
        return e["source_lang"]
    return f"{e['source_lang']} → {', '.join(e['target_lang'])}" if e["target_lang"] else e["source_lang"]


def matches(e: dict, *, text: str = "", src: str = "", dst: str = "", source: str = "",
            license: str = "all", fmt: str = "", supported_only: bool = False) -> bool:
    """The catalog filters (both editions, same rules)."""
    if src and e["source_lang"] != src:
        return False
    if dst and dst not in e["target_lang"]:
        return False
    if source and e["source"] != source:
        return False
    if fmt and e["format"] != fmt:
        return False
    if supported_only and e["supported"] is False:
        return False
    if not licenses.status_matches(e["license"], license):
        return False
    if text:
        hay = " ".join([e["name"], e["description"], e["author"], e["id"], " ".join(e["tags"]),
                        pair_text(e)]).lower()
        return all(w in hay for w in text.lower().split())
    return True


def sort_key(e: dict) -> tuple:
    return (e["name"].lower(), e["id"])


def human_size(n: int) -> str:
    """``1536`` → ``1.5 KB``; ``-`` for 0 (both editions print sizes the same way)."""
    if n <= 0:
        return "-"
    if n < 1024:
        return f"{n} B"
    v = float(n)
    for unit in ("KB", "MB", "GB", "TB"):
        v /= 1024
        if v < 1024 or unit == "TB":
            return f"{v:.1f} {unit}"
    return f"{n} B"
