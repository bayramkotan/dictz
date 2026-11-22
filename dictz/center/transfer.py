"""Import / export: the whole setup, the dictionary list, and catalogs.

* **Setup** (``.dictz-setup.zip``) — everything needed to move dictz to
  another computer: the dictionary list (order, on/off, names), settings,
  sources (never passwords), installed-from-catalog records, history and
  favourites (``history.json``), and — when asked — the dictionary files
  themselves (with their media). Read by both editions.
* **List** — the dictionaries with languages, word counts, licences and
  sources as CSV, JSON or plain text.
* **Catalog** — ``build_catalog`` writes a ``dictz-catalog.json`` for a
  folder of dictionaries (sizes, SHA-256, languages, word counts, licence),
  ready to put on a server; ``check_catalog`` reports problems in one.
"""
from __future__ import annotations

import contextlib
import csv
import hashlib
import io
import json
import zipfile
from collections.abc import Callable
from pathlib import Path, PurePosixPath
from urllib.parse import quote

from .. import __version__
from . import CATALOG_SCHEMA, langs, licenses
from .catalog import now_iso
from .model import ENTRY_KEYS, archive_of, format_of, normalize
from .sources import Sources, center_dir
from .store import JsonFile, dumps

SETUP_SCHEMA = 1
PARTS = ("dictionaries", "settings", "sources", "installed", "history", "files")


def _media_of(path: Path) -> list[Path]:
    """Files that belong to a dictionary besides its own: media zips and folders."""
    from ..core.media import MediaResolver

    out = []
    r = MediaResolver(path)
    for loc in r.search_locations()[:-1]:  # never the whole folder
        if loc.is_file():
            out.append(loc)
        elif loc.is_dir() and loc.name.lower() != path.parent.name.lower():
            out += [p for p in sorted(loc.rglob("*")) if p.is_file()]
    return out


def own_files(path: Path | str) -> list[Path]:
    """The files that make one dictionary — what it is read from (a StarDict
    is three or four files) and its media zips: what "delete with its files"
    removes. Media folders and anything else in the folder stay."""
    from ..core.media import MediaResolver
    from ..storage.articles import source_files

    p = Path(path)
    out = [f for f in source_files(p) if f.is_file()]
    if p.suffix.lower() == ".ifo":
        syn = p.with_suffix(".syn")
        if syn.is_file():
            out.append(syn)
    for loc in MediaResolver(p).search_locations()[:-1]:
        if loc.is_file() and loc not in out:
            out.append(loc)
    return out


def export_setup(zip_path: Path | str, data_dir: Path | str, parts: tuple[str, ...] = PARTS[:-1],
                 progress: Callable[[str, int, int], None] | None = None) -> dict:
    """Write a setup archive; returns its manifest."""
    from ..storage.articles import source_files
    from ..storage.registry import Registry

    data_dir = Path(data_dir)
    reg = Registry(data_dir / "dictionaries.json")
    reg.reload(force=True)
    rows = reg.rows()
    manifest = {"dictz-setup": SETUP_SCHEMA, "created": now_iso(), "app": f"dictz {__version__} (python)",
                "parts": [p for p in parts if p in PARTS], "dictionaries": []}
    zip_path = Path(zip_path)
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        if "dictionaries" in parts:
            z.writestr("dictionaries.json", dumps({"version": 1, "dictionaries": rows}))
        if "settings" in parts and (data_dir / "settings.json").is_file():
            z.write(data_dir / "settings.json", "settings.json")
        if "sources" in parts:
            src = Sources(data_dir)
            z.writestr("center/sources.json", dumps({"dictz-sources": 1, "default": src.default_id(),
                                                     "sources": src.all()}))
        cdir = center_dir(data_dir)
        if "installed" in parts and (cdir / "installed.json").is_file():
            z.write(cdir / "installed.json", "center/installed.json")
        if "history" in parts:
            z.writestr("history.json", dumps(_history_dump(data_dir)))
        files_total = len(rows)
        for i, row in enumerate(rows):
            item = {"id": row["id"], "name": row["name"], "path": row["path"], "files": []}
            if "files" in parts:
                p = Path(row["path"])
                for f in [*source_files(p), *_media_of(p)]:
                    if not f.is_file():
                        continue
                    rel = f.relative_to(p.parent).as_posix() if p.parent in f.parents else f.name
                    arc = f"files/{row['id']}/{rel}"
                    if arc not in item["files"]:
                        z.write(f, arc)
                        item["files"].append(arc)
                if progress is not None:
                    progress("files", i + 1, files_total)
            manifest["dictionaries"].append(item)
        z.writestr("dictz-setup.json", dumps(manifest))
    return manifest


def _history_dump(data_dir: Path) -> dict:
    import sqlite3

    db = data_dir / "dictz_history.db"
    out: dict = {"history": [], "favorites": []}
    if not db.is_file():
        return out
    con = sqlite3.connect(f"file:{db.as_posix()}?mode=ro", uri=True)
    try:
        with contextlib.suppress(sqlite3.Error):
            out["history"] = [{"word": w, "dictionary": d or "", "looked_up": t}
                              for w, d, t in con.execute("SELECT word, dictionary, looked_up FROM history ORDER BY id")]
        with contextlib.suppress(sqlite3.Error):
            out["favorites"] = [{"word": w, "note": n or "", "added_at": t}
                                for w, n, t in con.execute("SELECT word, note, added_at FROM favorites ORDER BY id")]
    finally:
        con.close()
    return out


def read_manifest(zip_path: Path | str) -> dict:
    try:
        with zipfile.ZipFile(zip_path) as z:
            m = json.loads(z.read("dictz-setup.json").decode("utf-8"))
    except (OSError, KeyError, ValueError, zipfile.BadZipFile) as e:
        raise ValueError(f"not a dictz setup archive: {zip_path} ({e})") from e
    if not isinstance(m, dict) or "dictz-setup" not in m:
        raise ValueError(f"not a dictz setup archive: {zip_path}")
    return m


def import_setup(zip_path: Path | str, data_dir: Path | str, replace: bool = False,
                 parts: tuple[str, ...] = PARTS, progress: Callable[[str, int, int], None] | None = None) -> dict:
    """Bring a setup archive in. ``replace`` = the list, sources and history
    become the archive's; otherwise they are merged (what is already here
    stays; the same file is not added twice). Returns what was done:
    ``{"added": [rows], "settings": bool, "sources": n, "history": n,
    "favorites": n, "files": n, "missing": [paths]}``."""
    from ..storage.registry import Registry, norm_path

    data_dir = Path(data_dir)
    manifest = read_manifest(zip_path)
    have = set(manifest.get("parts") or [])
    want = set(parts) & have
    res = {"added": [], "settings": False, "sources": 0, "history": 0, "favorites": 0, "files": 0,
           "missing": []}
    moved: dict[str, str] = {}  # original path → where it was unpacked
    with zipfile.ZipFile(zip_path) as z:
        names = set(z.namelist())
        if "files" in want:
            from .install import _safe_target

            dest_root = data_dir / "dictionaries" / "imported"
            items = manifest.get("dictionaries") or []
            for i, item in enumerate(items):
                written = []
                for arc in item.get("files") or []:
                    if arc not in names:
                        continue
                    rel = arc.split("/", 2)[2] if arc.count("/") >= 2 else PurePosixPath(arc).name
                    target = _safe_target(dest_root / item["id"], rel)
                    if target is None:
                        continue
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(z.read(arc))
                    written.append(target)
                    res["files"] += 1
                main = dest_root / item["id"] / Path(item["path"]).name
                if main.is_file():
                    moved[norm_path(item["path"])] = str(main)
                if progress is not None:
                    progress("files", i + 1, len(items))
        if "dictionaries" in want and "dictionaries.json" in names:
            rows = json.loads(z.read("dictionaries.json").decode("utf-8")).get("dictionaries") or []
            reg = Registry(data_dir / "dictionaries.json")
            reg.reload(force=True)
            if replace:
                for r in list(reg.rows()):
                    reg.remove(r["name"])
            known = {norm_path(r["path"]) for r in reg.rows()}
            for r in rows:
                path = moved.get(norm_path(r["path"]), r["path"])
                if norm_path(path) in known:
                    continue
                known.add(norm_path(path))
                fields = {k: r[k] for k in ("format", "word_count", "source_lang", "target_lang", "description",
                                            "author") if k in r}
                name = reg.register(r.get("name") or Path(path).stem, path, **fields)
                if not r.get("enabled", True):
                    reg.set_enabled(name, False)
                row = reg.get(name)
                if row is not None:
                    res["added"].append(row)
                if not Path(path).exists():
                    res["missing"].append(path)
        if "settings" in want and "settings.json" in names:
            from ..storage.settings import SETTINGS_FILE, Preferences

            prefs = Preferences.load(data_dir / SETTINGS_FILE)
            data = json.loads(z.read("settings.json").decode("utf-8"))
            prefs.import_dict(data if isinstance(data, dict) else {})
            prefs.save()
            res["settings"] = True
        if "sources" in want and "center/sources.json" in names:
            tmp = center_dir(data_dir) / ".import-sources.json"
            tmp.parent.mkdir(parents=True, exist_ok=True)
            tmp.write_bytes(z.read("center/sources.json"))
            try:
                res["sources"] = Sources(data_dir).import_(tmp, replace=replace)
            finally:
                tmp.unlink(missing_ok=True)
        if "installed" in want and "center/installed.json" in names:
            data = json.loads(z.read("center/installed.json").decode("utf-8"))
            recs = data.get("installed") or {}
            for rec in recs.values():
                rec["files"] = [moved.get(norm_path(f), f) for f in rec.get("files") or []]
                if rec["files"]:
                    rec["folder"] = str(Path(rec["files"][0]).parent)
            f = JsonFile(center_dir(data_dir) / "installed.json", lambda: {"version": 1, "installed": {}})

            def fn(d):
                if not isinstance(d, dict):
                    d = {"version": 1, "installed": {}}
                if replace:
                    d["installed"] = {}
                for k, v in recs.items():
                    d.setdefault("installed", {}).setdefault(k, v)
                return d

            f.update(fn)
        if "history" in want and "history.json" in names:
            h = json.loads(z.read("history.json").decode("utf-8"))
            res["history"], res["favorites"] = _history_load(data_dir, h, replace)
    return res


def _history_load(data_dir: Path, h: dict, replace: bool) -> tuple[int, int]:
    from ..storage.history import HistoryStore

    store = HistoryStore(data_dir / "dictz_history.db")
    store.open()
    nh = nf = 0
    try:
        con = store._conn  # one transaction for speed
        assert con is not None
        if replace:
            con.execute("DELETE FROM history")
            con.execute("DELETE FROM favorites")
        have = {(w, t) for w, t in con.execute("SELECT word, looked_up FROM history")}
        for r in h.get("history") or []:
            key = (str(r.get("word", "")), float(r.get("looked_up") or 0))
            if key[0] and key not in have:
                con.execute("INSERT INTO history (word, dictionary, looked_up) VALUES (?, ?, ?)",
                            (key[0], str(r.get("dictionary") or ""), key[1]))
                have.add(key)
                nh += 1
        for r in h.get("favorites") or []:
            w = str(r.get("word", ""))
            if w:
                cur = con.execute("INSERT OR IGNORE INTO favorites (word, note, added_at) VALUES (?, ?, ?)",
                                  (w, str(r.get("note") or ""), float(r.get("added_at") or 0)))
                nf += cur.rowcount
        con.commit()
    finally:
        store.close()
    return nh, nf


# ---------------------------------------------------------------- the list

LIST_COLUMNS = ("name", "format", "source_lang", "target_lang", "words", "enabled", "license", "source",
                "homepage", "path")


def list_rows(data_dir: Path | str) -> list[dict]:
    from ..storage.registry import Registry
    from .install import Installer

    data_dir = Path(data_dir)
    reg = Registry(data_dir / "dictionaries.json")
    reg.reload(force=True)
    inst = Installer(data_dir)
    out = []
    for r in reg.rows():
        rec = inst.record_for_path(r["path"]) or {}
        tl = rec.get("target_lang") or []
        out.append({"name": r["name"], "format": r["format"], "source_lang": r["source_lang"] or rec.get("source_lang", ""),
                    "target_lang": r["target_lang"] or (", ".join(tl) if isinstance(tl, list) else str(tl)), "words": int(r["word_count"] or 0),
                    "enabled": bool(r["enabled"]), "license": rec.get("license", "unknown"),
                    "source": rec.get("source", ""), "homepage": rec.get("homepage", ""), "path": r["path"]})
    return out


def export_list(path: Path | str, data_dir: Path | str, fmt: str = "") -> int:
    """The dictionary list as ``csv``, ``json`` or ``txt`` (by extension when
    ``fmt`` is empty). Returns the count."""
    path = Path(path)
    fmt = (fmt or path.suffix.lstrip(".") or "csv").lower()
    rows = list_rows(data_dir)
    if fmt == "json":
        text = dumps({"dictz-list": 1, "dictionaries": rows})
    elif fmt == "csv":
        buf = io.StringIO()
        w = csv.DictWriter(buf, fieldnames=LIST_COLUMNS, lineterminator="\n")
        w.writeheader()
        for r in rows:
            w.writerow({**r, "enabled": "yes" if r["enabled"] else "no"})
        text = buf.getvalue()
    else:
        lines = []
        for r in rows:
            pair = f"{r['source_lang']} → {r['target_lang']}" if r["source_lang"] else ""
            lines.append(f"{r['name']}\t{r['format']}\t{pair}\t{r['words']}\t{licenses.get(r['license']).name}")
        text = "\n".join(lines) + ("\n" if lines else "")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="")
    return len(rows)


# ---------------------------------------------------------------- catalogs

def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def build_catalog(folder: Path | str, base_url: str, name: str = "", license_id: str = "",
                  read_dictionaries: bool = True, progress: Callable[[str, int, int], None] | None = None) -> dict:
    """A ``dictz-catalog.json`` for every dictionary (and archive) in ``folder``.

    For each one: files with size and SHA-256, the address ``base_url`` +
    its path, and — read from the dictionary itself — name, languages, word
    count, author, description and a few sample headwords. A sidecar
    ``<file>.json`` next to it overrides any field (licence, homepage,
    description…); ``license_id`` is the default licence.
    """
    from ..core.query_manager import QueryManager
    from .adapters import _group_sets

    folder = Path(folder).expanduser().resolve()
    base = base_url if base_url.endswith("/") else base_url + "/"
    listing = []
    for p in sorted(folder.rglob("*")):
        if p.is_file() and not p.name.endswith(".json") and not p.name.startswith("."):
            rel = p.relative_to(folder).as_posix()
            listing.append((rel, base + quote(rel), p.stat().st_size, ""))
    src = {"id": "", "license": license_id, "filter": "", "name_pattern": "", "source_lang": "", "target_lang": ""}
    groups = _group_sets(src, listing)
    out = []
    for i, e in enumerate(groups):
        if progress is not None:
            progress("catalog", i + 1, len(groups))
        files = []
        for f in e["files"]:
            rel = next(r for r, u, _s, _t in listing if u == f["url"])
            p = folder / rel
            files.append({"name": p.name, "url": f["url"], "size": p.stat().st_size, "sha256": _sha256(p)})
        e["files"] = files
        first = folder / next(r for r, u, _s, _t in listing if u == files[0]["url"])
        e["id"] = e["id"].replace("/", "-")
        if read_dictionaries and not archive_of(first.name) and format_of(first.name) in ("stardict", "dsl", "babylon"):
            try:
                d = QueryManager.open_file(first)
                m = d.metadata
                e.update(name=m.name or e["name"], description=m.description or "", author=m.author or "",
                         headwords=len(d.headwords()), sample=", ".join(d.headwords()[:5]))
                if m.source_language:
                    e["source_lang"] = langs.norm(m.source_language)
                if m.target_language:
                    e["target_lang"] = [langs.norm(m.target_language)]
                d.close()
            except Exception:  # noqa: BLE001 — listed without its own metadata
                pass
        side = first.with_name(first.name + ".json")
        if side.is_file():
            try:
                extra = json.loads(side.read_text(encoding="utf-8"))
                if isinstance(extra, dict):
                    e.update({k: v for k, v in extra.items() if k in ENTRY_KEYS and k not in ("files", "id")})
            except ValueError:
                pass
        n = normalize(e, "", license_id)
        n.pop("source", None)
        out.append(n)
    return {"catalog": {"schema": CATALOG_SCHEMA, "name": name or folder.name, "updated": now_iso(),
                        "base_url": base, "license": licenses.norm(license_id) if license_id else ""},
            "dictionaries": out}


def check_catalog(data: dict | list) -> tuple[int, list[str]]:
    """(entries, problems) of a catalog; problems start with ``error:`` or ``warning:``."""
    problems: list[str] = []
    rows = data if isinstance(data, list) else data.get("dictionaries") if isinstance(data, dict) else None
    if not isinstance(rows, list):
        return 0, ["error: no \"dictionaries\" list"]
    if isinstance(data, dict):
        schema = (data.get("catalog") or {}).get("schema")
        if schema not in (None, CATALOG_SCHEMA):
            problems.append(f"warning: catalog schema {schema} (this dictz reads {CATALOG_SCHEMA})")
    ids: set[str] = set()
    for i, r in enumerate(rows):
        where = f"#{i + 1}"
        if not isinstance(r, dict):
            problems.append(f"error: {where}: not an object")
            continue
        e = normalize(r)
        where = f"#{i + 1} {e['name'] or e['id']}"
        if not e["id"]:
            problems.append(f"error: {where}: no id")
        elif e["id"] in ids:
            problems.append(f"error: {where}: id {e['id']} used twice")
        ids.add(e["id"])
        if not e["files"] and not e["resolve"]:
            problems.append(f"error: {where}: no files")
        for f in e["files"]:
            if not f["url"]:
                problems.append(f"error: {where}: a file without url")
            if not (f["sha256"] or f["sha512"] or f["md5"]):
                problems.append(f"warning: {where}: {f['name']}: no checksum")
            if not f["size"]:
                problems.append(f"warning: {where}: {f['name']}: no size")
        if licenses.get(e["license"]).status == "unknown":
            problems.append(f"warning: {where}: licence unknown")
        if not e["source_lang"]:
            problems.append(f"warning: {where}: no source language")
    return len(rows), problems
