"""Reading each kind of source into catalog entries.

One function per source type; each reads only through ``http`` (``get`` /
``request``), so tests answer from files (``tests/data/center/``) and the
Rust edition (``dictz-rs/src/center/adapters.rs``) is checked against the
same answers and the same expected entries (``center_golden.json``).

Parsing is done with the same simple rules in both editions (regular
expressions for HTML listings and WebDAV answers), never a full HTML/XML
library, so both read a page the same way.
"""
from __future__ import annotations

import html
import json
import re
from collections.abc import Iterable
from pathlib import PurePosixPath
from urllib.parse import quote, unquote, urljoin, urlsplit

from ..core.dictident import languages_from_name
from . import langs, licenses
from .model import archive_of, format_of, normalize, sort_key

MAX_ENTRIES = 20000  # per source, a guard against a runaway listing
DICT_EXT = re.compile(
    r"\.(ifo|dsl|dsl\.dz|bgl|mdx|xdxf|slob|zim|tar\.gz|tgz|tar\.xz|txz|tar\.bz2|tbz2?|tar|zip|7z)$", re.I)


class AdapterError(Exception):
    pass


# ---------------------------------------------------------------- helpers

def _json(raw: bytes, what: str):
    try:
        return json.loads(raw.decode("utf-8-sig"))
    except (UnicodeDecodeError, ValueError) as e:
        raise AdapterError(f"{what}: not JSON ({e})") from e


def _wanted(name: str, flt: str) -> bool:
    if flt:
        try:
            return re.search(flt, name) is not None
        except re.error:
            return False
    return DICT_EXT.search(name) is not None


def stem(name: str) -> str:
    """File name without its dictionary / archive extensions and a trailing
    format word: ``X Wiktionary dictionary stardict.tar.gz`` → ``X Wiktionary
    dictionary``."""
    low = name.lower()
    out = name
    for ext in (".tar.gz", ".tar.xz", ".tar.bz2", ".dsl.dz", ".dict.dz"):
        if low.endswith(ext):
            out = name[: -len(ext)]
            break
    else:
        out = name.rsplit(".", 1)[0] if "." in name else name
    trimmed = re.sub(r"[\s._-]+(stardict|dictd|slob)$", "", out, flags=re.I)
    return trimmed or out


def license_from_url(url: str) -> str:
    """``…/licenses/by-sa/4.0/`` → ``CC-BY-SA-4.0``; '' when not recognised."""
    u = (url or "").lower()
    m = re.search(r"creativecommons\.org/licenses/([a-z-]+)/(\d\.\d)", u)
    if m:
        return licenses.norm("CC-" + m.group(1).upper() + "-" + m.group(2))
    if "publicdomain/zero" in u:
        return "CC0-1.0"
    if "publicdomain/mark" in u:
        return "public-domain"
    if "gnu.org/licenses/gpl" in u or "gnu.org/copyleft/gpl" in u:
        return "GPL-3.0-or-later"
    if "gnu.org/licenses/fdl" in u:
        return "GFDL-1.3-or-later"
    return ""


def parse_size(text: str) -> int:
    """``12.3 MB`` → bytes (0 when not a size)."""
    m = re.search(r"([\d.,]+)\s*([KMGT]?i?B)\b", text or "", re.I)
    if not m:
        return 0
    try:
        n = float(m.group(1).replace(",", "."))
    except ValueError:
        return 0
    unit = m.group(2).upper().replace("I", "")
    mult = {"B": 1, "KB": 1 << 10, "MB": 1 << 20, "GB": 1 << 30, "TB": 1 << 40}.get(unit, 1)
    return int(n * mult)


def _languages(source: dict, name: str) -> tuple[str, list[str]]:
    """Languages from the source's name pattern, the name itself, or the
    source's own languages — in that order."""
    pat = source.get("name_pattern") or ""
    if pat:
        try:
            m = re.search(pat, name)
        except re.error:
            m = None
        if m:
            g = m.groupdict()
            s, d = langs.norm(g.get("src") or ""), langs.norm(g.get("dst") or "")
            if s:
                return s, [d] if d else ([langs.norm(source["target_lang"])] if source.get("target_lang") else [])
    pair = languages_from_name(name)
    if pair:
        return langs.norm(pair[0]), [langs.norm(pair[1])]
    s = langs.norm(source.get("source_lang") or "")
    d = langs.norm(source.get("target_lang") or "")
    return s, [d] if d else []


def _file_entry(source: dict, rel: str, url: str, size: int = 0, date: str = "", **extra) -> dict:
    name = PurePosixPath(rel).name
    title = stem(name)
    s, d = _languages(source, title)
    e = {"id": rel.lower(), "name": title, "files": [{"name": name, "url": url, "size": size}],
         "source_lang": s, "target_lang": d, "date": date[:10]}
    e.update(extra)
    return e


def _group_sets(source: dict, files: list[tuple[str, str, int, str]]) -> list[dict]:
    """(relative path, url, size, date) of a listing → entries. A StarDict set
    (``.ifo`` + ``.idx`` + ``.dict``) or an MDict pair (``.mdx`` + ``.mdd``) in one
    folder is one entry; every other dictionary file or archive is its own."""
    by_dir: dict[str, list[tuple[str, str, int, str]]] = {}
    for f in files:
        by_dir.setdefault(str(PurePosixPath(f[0]).parent), []).append(f)
    out = []
    flt = source.get("filter") or ""
    for _d, items in sorted(by_dir.items()):
        names = {PurePosixPath(r).name.lower(): (r, u, s, t) for r, u, s, t in items}
        used: set[str] = set()
        for low, (rel, _url, _size, date) in sorted(names.items()):
            if low.endswith((".ifo", ".mdx")):
                base = low[:-4]
                sib = [n for n in names if n != low and n.startswith(base + ".") and n not in used
                       and re.search(r"\.(idx|idx\.gz|dict|dict\.dz|syn|syn\.dz|mdd)$", n)]
                group = [names[n] for n in [low, *sorted(sib)]]
                used.update([low, *sib])
                title = PurePosixPath(rel).name[:-4]
                s, dl = _languages(source, title)
                out.append({"id": rel.lower(), "name": title, "source_lang": s, "target_lang": dl,
                            "date": date[:10],
                            "files": [{"name": PurePosixPath(r).name, "url": u, "size": z} for r, u, z, _t in group]})
        for low, (rel, url, size, date) in sorted(names.items()):
            if low in used or not _wanted(PurePosixPath(rel).name, flt):
                continue
            if re.search(r"\.(idx|idx\.gz|dict|dict\.dz|syn|mdd)$", low):
                continue  # part of a set without its .ifo / .mdx
            out.append(_file_entry(source, rel, url, size, date))
    return out


def finish(source: dict, raw: Iterable[dict]) -> list[dict]:
    """Normalised, de-duplicated (first wins), sorted entries of a source."""
    seen: set[str] = set()
    out = []
    for r in raw:
        e = normalize(r, source["id"], source.get("license") or "")
        if e["id"] in seen:
            continue
        seen.add(e["id"])
        out.append(e)
        if len(out) >= MAX_ENTRIES:
            break
    out.sort(key=sort_key)
    return out


# ---------------------------------------------------------------- types

def read_dictz_catalog(source: dict, http, _depth: int = 0) -> list[dict]:
    """A ``dictz-catalog.json`` (docs/CATALOG.md): ``{"catalog": {...},
    "dictionaries": [...]}``. Relative file addresses are relative to the
    catalog; ``catalog.include`` lists more catalogs (one level)."""
    url = source["url"]
    data = _json(http.get(url), url)
    if isinstance(data, list):
        head, rows = {}, data
    elif isinstance(data, dict):
        head = data.get("catalog") or {}
        rows = data.get("dictionaries") or []
    else:
        raise AdapterError(f"{url}: not a dictz catalog")
    base = head.get("base_url") or url
    out = []
    for r in rows:
        if not isinstance(r, dict):
            continue
        r = dict(r)
        files = []
        for f in r.get("files") or []:
            f = {"url": f} if isinstance(f, str) else dict(f)
            f["url"] = urljoin(base, f.get("url", ""))
            files.append(f)
        r["files"] = files
        if not r.get("license") and head.get("license"):
            r["license"] = head["license"]
        out.append(r)
    if _depth == 0:
        for inc in head.get("include") or []:
            sub = dict(source, url=urljoin(url, str(inc)))
            try:
                out += read_dictz_catalog(sub, http, 1)
            except Exception:  # noqa: BLE001 — an included catalog that fails is skipped
                continue
    return out


_PLATFORM_ORDER = ("stardict", "slob", "dictd", "src")


def read_freedict(source: dict, http) -> list[dict]:
    """``freedict-database.json``: one entry per dictionary, its StarDict
    release (else slob / dictd), with size and SHA-512."""
    data = _json(http.get(source["url"]), source["url"])
    if not isinstance(data, list):
        raise AdapterError("FreeDict: not a list")
    out = []
    for d in data:
        if not isinstance(d, dict) or not d.get("name") or not isinstance(d.get("releases"), list):
            continue
        rels = {str(r.get("platform", "")): r for r in d["releases"] if isinstance(r, dict) and r.get("URL")}
        plat = next((p for p in _PLATFORM_ORDER if p in rels), "")
        if not plat:
            continue
        rel = rels[plat]
        pair = str(d["name"])
        a, _, b = pair.partition("-")
        src, dst = langs.norm(a), langs.norm(b)
        title = f"FreeDict {langs.name(src) or a} → {langs.name(dst) or b}"
        out.append({
            "id": pair, "name": title, "source_lang": src, "target_lang": [dst] if dst else [],
            "headwords": d.get("headwords"), "version": d.get("edition") or rel.get("version"),
            "date": d.get("date") or rel.get("date"), "author": d.get("maintainerName", ""),
            "homepage": "https://freedict.org/", "page": d.get("sourceURL", ""),
            "format": {"stardict": "stardict", "slob": "slob", "dictd": "dictd"}.get(plat, ""),
            "tags": sorted(rels), "description": d.get("status", ""),
            "files": [{"url": rel["URL"], "size": rel.get("size"), "sha512": rel.get("checksum", "")}],
        })
    return out


def _github(url: str) -> tuple[str, str, str, str]:
    """``https://github.com/o/r[/tree/ref/path]`` → (owner, repo, ref, path)."""
    p = urlsplit(url)
    parts = [x for x in p.path.split("/") if x]
    if len(parts) < 2:
        raise AdapterError(f"not a GitHub repository: {url}")
    owner, repo = parts[0], parts[1].removesuffix(".git")
    ref, path = "", ""
    if len(parts) > 3 and parts[2] in ("tree", "blob"):
        ref, path = parts[3], "/".join(parts[4:])
    return owner, repo, ref, path


def read_github_releases(source: dict, http) -> list[dict]:
    """Assets of the newest release (``depth`` > 2: of the last ``depth`` releases)."""
    owner, repo, _ref, _path = _github(source["url"])
    api = f"https://api.github.com/repos/{owner}/{repo}/releases?per_page=30"
    rels = _json(http.get(api, {"Accept": "application/vnd.github+json"}), api)
    if not isinstance(rels, list):
        raise AdapterError(f"{api}: unexpected answer")
    rels = [r for r in rels if isinstance(r, dict) and not r.get("draft")]
    keep = rels[: max(1, int(source.get("depth") or 0) - 1)] if int(source.get("depth") or 0) > 2 else rels[:1]
    out = []
    for r in keep:
        tag = str(r.get("tag_name") or "")
        for a in r.get("assets") or []:
            name = str(a.get("name") or "")
            if not name or not _wanted(name, source.get("filter") or ""):
                continue
            e = _file_entry(source, name, str(a.get("browser_download_url") or ""), int(a.get("size") or 0),
                            str(r.get("published_at") or ""), version=tag,
                            page=str(r.get("html_url") or ""), homepage=source.get("homepage", ""))
            if len(keep) > 1:
                e["id"] = f"{tag}/{name}".lower()
                e["name"] = f"{e['name']} ({tag})"
            out.append(e)
    return out


def read_github_folder(source: dict, http) -> list[dict]:
    """Files of a repository folder (GitHub contents API), ``depth`` folders deep."""
    owner, repo, ref, path = _github(source["url"])
    files: list[tuple[str, str, int, str]] = []

    def walk(p: str, depth: int) -> None:
        api = f"https://api.github.com/repos/{owner}/{repo}/contents" + (f"/{quote(p)}" if p else "") \
            + (f"?ref={quote(ref)}" if ref else "")
        items = _json(http.get(api, {"Accept": "application/vnd.github+json"}), api)
        if not isinstance(items, list):
            raise AdapterError(f"{api}: not a folder")
        for it in sorted(items, key=lambda x: str(x.get("path", ""))):
            kind, ipath = it.get("type"), str(it.get("path") or "")
            if kind == "dir" and depth > 1:
                walk(ipath, depth - 1)
            elif kind == "file" and it.get("download_url"):
                files.append((ipath, str(it["download_url"]), int(it.get("size") or 0), ""))

    walk(path, max(1, int(source.get("depth") or 1)))
    out = _group_sets(source, files)
    for e in out:
        e["homepage"] = source.get("homepage", "")
    return out


_HREF = re.compile(r"""<a\s[^>]*?href\s*=\s*["']([^"'#]+)["']""", re.I)


def read_http_index(source: dict, http) -> list[dict]:
    """An Apache / nginx / lighttpd folder listing, ``depth`` folders deep."""
    root = source["url"] if source["url"].endswith("/") else source["url"] + "/"
    host = urlsplit(root).netloc
    files: list[tuple[str, str, int, str]] = []
    seen: set[str] = set()

    def walk(url: str, depth: int) -> None:
        if url in seen:
            return
        seen.add(url)
        try:
            page = http.get(url).decode("utf-8", "replace")
        except Exception:  # noqa: BLE001 — a folder that fails is skipped (the root raises below)
            if url == root:
                raise
            return
        for href in _HREF.findall(page):
            href = html.unescape(href)
            if href.startswith(("?", "mailto:", "javascript:")) or href in ("../", "./", "/"):
                continue
            full = urljoin(url, href)
            if urlsplit(full).netloc != host or not full.startswith(root) or full == url:
                continue
            rel = unquote(full[len(root):])
            if full.endswith("/"):
                if depth > 1:
                    walk(full, depth - 1)
            elif "?" not in full:
                files.append((rel, full, 0, ""))

    walk(root, max(1, int(source.get("depth") or 1)))
    return _group_sets(source, files)


_DAV_RESP = re.compile(r"<(?:[\w-]+:)?response\b.*?</(?:[\w-]+:)?response>", re.S | re.I)
_DAV_HREF = re.compile(r"<(?:[\w-]+:)?href>\s*(.*?)\s*</(?:[\w-]+:)?href>", re.S | re.I)
_DAV_LEN = re.compile(r"<(?:[\w-]+:)?getcontentlength>\s*(\d+)\s*<", re.I)
_DAV_COLL = re.compile(r"<(?:[\w-]+:)?collection\s*/?>", re.I)
_DAV_DATE = re.compile(r"<(?:[\w-]+:)?getlastmodified>\s*(.*?)\s*<", re.I)
PROPFIND = (b'<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop>'
            b"<d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>")


def _http_date(text: str) -> str:
    from email.utils import parsedate_to_datetime

    try:
        return parsedate_to_datetime(text).strftime("%Y-%m-%d")
    except (TypeError, ValueError, IndexError):
        return ""


def _read_dav(source: dict, http, root: str, headers: dict) -> list[dict]:
    root = root if root.endswith("/") else root + "/"
    base = urlsplit(root)
    files: list[tuple[str, str, int, str]] = []

    def walk(url: str, depth: int) -> None:
        body = http.request("PROPFIND", url, PROPFIND, {**headers, "Depth": "1",
                                                        "Content-Type": "application/xml"})
        for block in _DAV_RESP.findall(body.decode("utf-8", "replace")):
            m = _DAV_HREF.search(block)
            if not m:
                continue
            href = html.unescape(m.group(1))
            full = urljoin(f"{base.scheme}://{base.netloc}/", href) if href.startswith("/") else urljoin(url, href)
            if full.rstrip("/") == url.rstrip("/"):
                continue
            rel = unquote(full[len(root):]) if full.startswith(root) else unquote(href.rsplit("/", 1)[-1])
            if _DAV_COLL.search(block):
                if depth > 1:
                    walk(full if full.endswith("/") else full + "/", depth - 1)
                continue
            n = _DAV_LEN.search(block)
            d = _DAV_DATE.search(block)
            files.append((rel, full, int(n.group(1)) if n else 0, _http_date(d.group(1)) if d else ""))

    walk(root, max(1, int(source.get("depth") or 1)))
    return _group_sets(source, files)


def read_webdav(source: dict, http, headers: dict | None = None) -> list[dict]:
    url = re.sub(r"^webdav(s?)://", lambda m: "https://" if m.group(1) else "http://", source["url"])
    return _read_dav(source, http, url, headers or {})


def nextcloud_endpoint(share_url: str) -> tuple[str, str]:
    """``https://host/index.php/s/TOKEN`` → (WebDAV root, token)."""
    p = urlsplit(share_url)
    m = re.search(r"/s/([A-Za-z0-9]+)", p.path)
    if not m:
        raise AdapterError(f"not a share link: {share_url}")
    prefix = p.path[: p.path.find("/index.php/s/")] if "/index.php/s/" in p.path else p.path[: m.start()]
    return f"{p.scheme}://{p.netloc}{prefix}/public.php/webdav/", m.group(1)


def read_nextcloud_share(source: dict, http, password: str = "") -> list[dict]:
    """A public Nextcloud / ownCloud share: WebDAV with the token as the user."""
    import base64

    root, token = nextcloud_endpoint(source["url"])
    auth = {"Authorization": "Basic " + base64.b64encode(f"{token}:{password}".encode()).decode()}
    return _read_dav(source, http, root, auth)


ARCHIVE_ORG_ROWS = 200


def read_archive_org(source: dict, http) -> list[dict]:
    """Internet Archive search; each item's files are looked up when it is installed."""
    from urllib.parse import urlencode

    q = source.get("filter") or "stardict"
    params = [("q", q), ("fl[]", "identifier"), ("fl[]", "title"), ("fl[]", "description"),
              ("fl[]", "licenseurl"), ("fl[]", "date"), ("fl[]", "item_size"), ("fl[]", "language"),
              ("sort[]", "downloads desc"), ("rows", str(ARCHIVE_ORG_ROWS)), ("page", "1"), ("output", "json")]
    url = "https://archive.org/advancedsearch.php?" + urlencode(params)
    data = _json(http.get(url), url)
    docs = (data.get("response") or {}).get("docs") if isinstance(data, dict) else None
    if not isinstance(docs, list):
        raise AdapterError("archive.org: unexpected answer")
    out = []
    for d in docs:
        ident = str(d.get("identifier") or "")
        if not ident:
            continue
        title = d.get("title") or ident
        title = title[0] if isinstance(title, list) else str(title)
        desc = d.get("description") or ""
        desc = " ".join(desc) if isinstance(desc, list) else str(desc)
        desc = re.sub(r"<[^>]+>", " ", desc)
        desc = re.sub(r"\s+", " ", html.unescape(desc)).strip()[:400]
        s, dl = _languages(source, title)
        lang = d.get("language")
        if not s and lang:
            first = lang[0] if isinstance(lang, list) else str(lang)
            s = langs.norm(first)
        out.append({"id": ident, "name": title, "description": desc, "source_lang": s, "target_lang": dl,
                    "license": license_from_url(str(d.get("licenseurl") or "")) or "",
                    "date": str(d.get("date") or "")[:10], "size": d.get("item_size") or 0,
                    "page": f"https://archive.org/details/{ident}", "format": "archive",
                    "resolve": f"archive-org:{ident}"})
    return out


def resolve_archive_org(ident: str, http) -> list[dict]:
    """An item's dictionary files (for installing)."""
    url = f"https://archive.org/metadata/{quote(ident)}"
    data = _json(http.get(url), url)
    out = []
    for f in (data.get("files") or []) if isinstance(data, dict) else []:
        name = str(f.get("name") or "")
        if not name or f.get("source") == "metadata" or not DICT_EXT.search(name):
            continue
        out.append({"name": PurePosixPath(name).name, "url": f"https://archive.org/download/{quote(ident)}/{quote(name)}",
                    "size": f.get("size") or 0, "md5": f.get("md5") or ""})
    return out


def read_local(source: dict, _http=None) -> list[dict]:
    """A folder on this computer, a USB disk or a network share."""
    from pathlib import Path

    root = Path(source["url"]).expanduser()
    if not root.is_dir():
        raise AdapterError(f"no such folder: {root}")
    files: list[tuple[str, str, int, str]] = []
    depth = max(1, int(source.get("depth") or 1))
    for p in sorted(root.rglob("*")):
        rel = p.relative_to(root).as_posix()
        if p.is_file() and rel.count("/") < depth:
            files.append((rel, p.resolve().as_uri(), p.stat().st_size, ""))
    return _group_sets(source, files)


_TG_WRAP = '<div class="tgme_widget_message_wrap'  # one post per piece (split, no look-ahead: same in Rust)
_TG_POST = re.compile(r'data-post="([^"]+)"')
_TG_DOC = re.compile(r'tgme_widget_message_document_title[^>]*>(.*?)</div>', re.S)
_TG_EXTRA = re.compile(r'tgme_widget_message_document_extra[^>]*>(.*?)</div>', re.S)
_TG_TIME = re.compile(r'<time[^>]*datetime="([^"]+)"')
_TG_TEXT = re.compile(r'tgme_widget_message_text[^>]*>(.*?)</div>', re.S)


def read_telegram(source: dict, http) -> list[dict]:
    """A public channel's web preview (``t.me/s/<channel>``): its files are
    listed with a link to the post; downloading needs Telegram itself."""
    url = source["url"]
    out: list[dict] = []
    pages = max(1, int(source.get("depth") or 1))
    before = ""
    for _ in range(pages):
        page = http.get(url + (f"?before={before}" if before else "")).decode("utf-8", "replace")
        posts = []
        for block in page.split(_TG_WRAP)[1:]:
            post = _TG_POST.search(block)
            doc = _TG_DOC.search(block)
            if not post:
                continue
            posts.append(post.group(1))
            if not doc:
                continue
            title = html.unescape(re.sub(r"<[^>]+>", "", doc.group(1))).strip()
            extra = _TG_EXTRA.search(block)
            when = _TG_TIME.search(block)
            text = _TG_TEXT.search(block)
            desc = html.unescape(re.sub(r"<[^>]+>", " ", text.group(1))) if text else ""
            s, dl = _languages(source, title)
            out.append({"id": post.group(1).lower(), "name": stem(title), "description": " ".join(desc.split())[:400],
                        "size": parse_size(extra.group(1)) if extra else 0, "source_lang": s, "target_lang": dl,
                        "date": (when.group(1) if when else "")[:10], "page": f"https://t.me/{post.group(1)}",
                        "format": format_of(title), "tags": ["telegram"]})
        nums = [int(p.rsplit("/", 1)[-1]) for p in posts if p.rsplit("/", 1)[-1].isdigit()]
        if not nums:
            break
        before = str(min(nums))
    return out


def read(source: dict, http, password: str = "", token: str = "") -> list[dict]:
    """Every entry of ``source`` (normalised)."""
    from .net import auth_headers

    t = source["type"]
    if t == "dictz-catalog":
        raw = read_dictz_catalog(source, http)
    elif t == "freedict":
        raw = read_freedict(source, http)
    elif t == "github-releases":
        raw = read_github_releases(source, http)
    elif t == "github-folder":
        raw = read_github_folder(source, http)
    elif t == "http-index":
        raw = read_http_index(source, http)
    elif t == "webdav":
        raw = read_webdav(source, http, auth_headers(source, password, token))
    elif t == "nextcloud-share":
        raw = read_nextcloud_share(source, http, password)
    elif t == "archive-org":
        raw = read_archive_org(source, http)
    elif t == "local":
        raw = read_local(source)
    elif t == "telegram":
        raw = read_telegram(source, http)
    else:
        raise AdapterError(f"unknown source type: {t}")
    return finish(source, raw)


def entry_from_url(url: str) -> dict:
    """"Add from link…": one entry for any address of a file or archive."""
    name = unquote(urlsplit(url).path.rstrip("/").rsplit("/", 1)[-1]) or url
    src = {"id": "link", "license": "unknown", "source_lang": "", "target_lang": "", "name_pattern": ""}
    e = _file_entry(src, name, url, 0)
    e["id"] = re.sub(r"[^a-z0-9._-]+", "-", name.lower()).strip("-") or "download"
    e["page"] = url
    if not archive_of(name) and format_of(name) == "unknown":
        e["format"] = "archive"  # look inside after downloading
    return normalize(e, "link", "unknown")
