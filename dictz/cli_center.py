"""Command line of the Dictionary Center (same commands and output as the
Rust edition's ``dictz-rs/src/cli_center.rs``):

    dct sources [list|add|edit|remove|enable|disable|default|up|down|refresh|export|import|reset]
    dct catalog [search TEXT|show ID|languages|updates|build FOLDER|check FILE|URL]
    dct install ID|NAME|URL...        dct uninstall ID|NAME...
    dct downloads [list|clear]        dct licenses
    dct setup export|import|show FILE
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .center.model import human_size

CENTER_COMMANDS = [
    ("sources", "dictionary servers / sources: list, add, edit, remove, refresh, export, import"),
    ("catalog", "search the catalog of downloadable dictionaries; build or check a catalog"),
    ("install", "download and install dictionaries from the catalog or a link"),
    ("uninstall", "remove dictionaries installed from the catalog"),
    ("downloads", "download history"),
    ("licenses", "licences dictz knows and what they allow"),
    ("setup", "export / import the whole setup (list, settings, sources, history, files)"),
]
SOURCE_ACTIONS = ("list", "add", "edit", "remove", "enable", "disable", "default", "up", "down", "refresh",
                  "export", "import", "reset")
CATALOG_ACTIONS = ("search", "show", "languages", "updates", "build", "check")


def _print_json(obj) -> None:
    print(json.dumps(obj, ensure_ascii=False, indent=2, sort_keys=True))


def add_parsers(add) -> None:
    """Add the center's sub-commands (``add(name)`` makes a sub-parser)."""
    def source_opts(p):
        p.add_argument("--name", metavar="NAME")
        p.add_argument("--type", metavar="TYPE", help="dictz-catalog, freedict, github-releases, github-folder, "
                                                       "http-index, webdav, nextcloud-share, archive-org, local, telegram")
        p.add_argument("--url", metavar="URL", help="(edit) a new address")
        p.add_argument("--filter", metavar="REGEX", help="file names to list (archive.org: the search query)")
        p.add_argument("--name-pattern", metavar="REGEX", help="languages from names: (?P<src>…) (?P<dst>…)")
        p.add_argument("--from", dest="src", metavar="LANG", help="source language when the source does not say")
        p.add_argument("--to", dest="dst", metavar="LANG", help="target language when the source does not say")
        p.add_argument("--license", metavar="ID", help="licence when the source does not say")
        p.add_argument("--depth", type=int, metavar="N", help="folders deep (listings) / pages (Telegram)")
        p.add_argument("--auth", choices=["none", "basic", "bearer", "header"])
        p.add_argument("--user", metavar="USER", help="user name (basic) or header name (header)")
        p.add_argument("--password", metavar="PASSWORD", help="kept in credentials.json, never exported")
        p.add_argument("--token", metavar="TOKEN", help="kept in credentials.json, never exported")
        p.add_argument("--description", metavar="TEXT")
        p.add_argument("--disabled", action="store_true", help="(add) add switched off")

    so = add("sources")
    so.add_argument("action", nargs="?", default="list", choices=SOURCE_ACTIONS)
    so.add_argument("args", nargs="*", metavar="ID|URL|FILE")
    source_opts(so)
    so.add_argument("--replace", action="store_true", help="(import) drop your own sources first")
    so.add_argument("--mine", action="store_true", help="(export) only your own sources")
    so.add_argument("--json", action="store_true")

    ca = add("catalog")
    ca.add_argument("action", nargs="?", default="search", metavar="ACTION",
                    help="search (default) | show | languages | updates | build | check")
    ca.add_argument("args", nargs="*", metavar="TEXT|ID|FOLDER|FILE")
    ca.add_argument("--from", dest="src", metavar="LANG", help="source language (en, eng, English …)")
    ca.add_argument("--to", dest="dst", metavar="LANG", help="target language")
    ca.add_argument("--source", metavar="ID", help="only this source")
    ca.add_argument("--license", default="all", choices=["all", "open", "free", "unknown", "nonfree"],
                    help="open = open licence, free = open or free to use")
    ca.add_argument("--format", dest="fmt", metavar="FORMAT", help="stardict, dsl, babylon, mdict, dictd …")
    ca.add_argument("--supported", action="store_true", help="only what dictz opens today")
    ca.add_argument("--refresh", action="store_true", help="read the sources first")
    ca.add_argument("-n", "--limit", type=int, default=0, metavar="N")
    ca.add_argument("--base-url", metavar="URL", help="(build) where the files will be served")
    ca.add_argument("-o", "--output", metavar="FILE", help="(build) default: FOLDER/catalog.json")
    ca.add_argument("--name", metavar="NAME", help="(build) the catalog's name")
    ca.add_argument("--default-license", metavar="ID", help="(build) licence when a dictionary has none")
    ca.add_argument("--no-read", action="store_true", help="(build) do not open the dictionaries")
    ca.add_argument("--json", action="store_true")

    ins = add("install")
    ins.add_argument("targets", nargs="+", metavar="ID|NAME|URL")
    ins.add_argument("--no-verify", action="store_true", help="skip checksums")
    ins.add_argument("--keep-archive", action="store_true", help="keep the downloaded archive")
    ins.add_argument("--no-add", action="store_true", help="install only, do not add to the list")
    ins.add_argument("--refresh", action="store_true", help="read the sources first")

    un = add("uninstall")
    un.add_argument("targets", nargs="+", metavar="ID|NAME")
    un.add_argument("--keep-files", action="store_true", help="remove from dictz, keep the folder")

    dl = add("downloads")
    dl.add_argument("action", nargs="?", default="list", choices=["list", "clear"])
    dl.add_argument("-n", "--limit", type=int, default=50, metavar="N")
    dl.add_argument("--json", action="store_true")

    li = add("licenses")
    li.add_argument("--json", action="store_true")

    st = add("setup")
    st.add_argument("action", choices=["export", "import", "show"])
    st.add_argument("file", metavar="FILE")
    st.add_argument("--with-files", action="store_true", help="(export) put the dictionary files in too")
    st.add_argument("--no-settings", action="store_true")
    st.add_argument("--no-sources", action="store_true")
    st.add_argument("--no-history", action="store_true")
    st.add_argument("--no-list", action="store_true", help="leave out the dictionary list")
    st.add_argument("--replace", action="store_true", help="(import) replace instead of merging")
    st.add_argument("--json", action="store_true")


# ---------------------------------------------------------------- helpers

def _prefs_http(ctx):
    from .center.net import Http

    p = ctx.prefs
    return Http(timeout=float(p["center.timeout"]), proxy=str(p["center.proxy"]))


def _catalog(ctx):
    from .center.catalog import Catalog
    from .center.sources import Sources

    return Catalog(ctx.data_dir, Sources(ctx.data_dir), _prefs_http(ctx))


def _installer(ctx, verify: bool = True, keep: bool = False):
    from .center.install import Installer
    from .center.sources import Sources

    p = ctx.prefs
    root = str(p["center.install_folder"]).strip() or None
    return Installer(ctx.data_dir, Sources(ctx.data_dir), _prefs_http(ctx), root,
                     verify=verify and bool(p["center.verify_checksums"]),
                     keep_archives=keep or bool(p["center.keep_archives"]))


def _refresh(ctx, cat, ids=None, quiet=False) -> None:
    if not ctx.prefs["center.online"]:
        print("offline: center.online is off (dct config set center.online true)", file=sys.stderr)
        return
    def prog(sid, res):
        if not quiet:
            state = f"{len(res['entries'])} dictionaries" if res["ok"] else f"error: {res['error']}"
            print(f"  {sid}: {state}", file=sys.stderr)

    if not quiet:
        print("reading sources…", file=sys.stderr)
    cat.refresh(ids, progress=prog)


def _pair(e: dict) -> str:
    from .center.model import pair_text

    return pair_text(e) or "-"


# ---------------------------------------------------------------- sources

def cmd_sources(ctx, args) -> int:
    from .center.catalog import Catalog
    from .center.sources import Sources, guess_type

    src = Sources(ctx.data_dir)
    a = args.action
    if a == "list":
        cat = Catalog(ctx.data_dir, src)
        rows = []
        for s in src.all():
            c = cat.cached(s["id"])
            rows.append({**s, "default": s["id"] == src.default_id(), "entries": len(c["entries"]),
                         "checked": c["fetched"][:10], "ok": c["ok"], "error": c["error"]})
        if args.json:
            _print_json(rows)
            return 0
        from .cli import _table

        table = [[str(i + 1), "yes" if r["enabled"] else "no", r["id"] + (" *" if r["default"] else ""),
                  r["type"], str(r["entries"]), r["checked"] or "-",
                  "ok" if r["ok"] else ("error" if r["error"] else "-"), r["url"]] for i, r in enumerate(rows)]
        print(_table(table, ["#", "on", "id", "type", "entries", "checked", "state", "url"]))
        print("\n* = default server · dct sources refresh · dct sources add URL")
        return 0
    if a == "add":
        if not args.args:
            raise SystemExit("error: dct sources add URL [--name NAME] [--type TYPE] …")
        url = args.args[0]
        fields = _source_fields(args)
        fields.setdefault("type", guess_type(url))
        fields.setdefault("name", fields.get("name") or url.rstrip("/").rsplit("/", 1)[-1] or url)
        s = src.add({"url": url, "enabled": not args.disabled, **fields})
        if args.password or args.token:
            src.set_secret(s["id"], args.password, args.token)
        print(f"added: {s['id']} ({s['type']}) {s['url']}")
        return 0
    if a == "reset":
        src.reset()
        print("built-in sources reset")
        return 0
    if a == "refresh":
        ids = [_source_id(src, x) for x in args.args] or None
        if not ctx.prefs["center.online"]:
            raise SystemExit("error: offline: center.online is off (dct config set center.online true)")
        cat = Catalog(ctx.data_dir, src, _prefs_http(ctx))
        res = cat.refresh(ids)
        rc = 0
        for sid, r in res.items():
            if r["ok"]:
                print(f"{sid}: {len(r['entries'])} dictionaries")
            else:
                print(f"{sid}: error: {r['error']}")
                rc = 1
        return rc
    if a == "export":
        if not args.args:
            raise SystemExit("error: dct sources export FILE")
        n = src.export(args.args[0], include_builtin=not args.mine)
        print(f"exported {n} sources → {args.args[0]}")
        return 0
    if a == "import":
        if not args.args:
            raise SystemExit("error: dct sources import FILE")
        try:
            n = src.import_(args.args[0], replace=args.replace)
        except ValueError as e:
            raise SystemExit(f"error: {e}") from e
        print(f"imported {n} sources")
        return 0
    if not args.args:
        raise SystemExit(f"error: dct sources {a} ID")
    for key in args.args:
        sid = _source_id(src, key)
        if a == "edit":
            fields = _source_fields(args)
            if args.url:
                fields["url"] = args.url
            src.update(sid, **fields)
            if args.password is not None or args.token is not None:
                src.set_secret(sid, args.password, args.token)
            print(f"changed: {sid}")
        elif a == "remove":
            src.remove(sid)
            print(f"removed: {sid}")
        elif a in ("enable", "disable"):
            src.set_enabled(sid, a == "enable")
            print(f"{a}d: {sid}")
        elif a == "default":
            src.set_default(sid)
            print(f"default server: {sid}")
        elif a in ("up", "down"):
            src.move(sid, -1 if a == "up" else 1)
            print(f"moved {a}: {sid}")
    return 0


def _source_id(src, key: str) -> str:
    s = src.find(key)
    if s is None:
        raise SystemExit(f"error: no source {key!r} (see `dct sources`)")
    return s["id"]


def _source_fields(args) -> dict:
    m = {"name": args.name, "type": args.type, "filter": args.filter, "name_pattern": args.name_pattern,
         "source_lang": args.src, "target_lang": args.dst, "license": args.license, "depth": args.depth,
         "auth": args.auth, "user": args.user, "description": args.description}
    return {k: v for k, v in m.items() if v is not None}


# ---------------------------------------------------------------- catalog

def cmd_catalog(ctx, args) -> int:
    from .center import langs, licenses

    action, rest = args.action, list(args.args)
    if action not in CATALOG_ACTIONS:
        action, rest = "search", [action, *rest]
    if action == "build":
        return _catalog_build(ctx, args, rest)
    if action == "check":
        return _catalog_check(ctx, rest)
    cat = _catalog(ctx)
    if args.refresh or not any(cat.cached(s["id"])["fetched"] for s in cat.sources.enabled()):
        _refresh(ctx, cat)
    if action == "show":
        if not rest:
            raise SystemExit("error: dct catalog show ID")
        hits = cat.find(" ".join(rest))
        if not hits:
            raise SystemExit(f"error: no catalog entry {' '.join(rest)!r}")
        e = hits[0]
        if args.json:
            _print_json(e)
            return 0
        lic = licenses.get(e["license"])
        lines = [("id", e["id"]), ("name", e["name"]), ("languages", _pair(e)), ("kind", e["kind"]),
                 ("format", e["format"] + ("" if e["supported"] is not False else " (not opened by dictz yet)")),
                 ("headwords", f"{e['headwords']:,}" if e["headwords"] else "-"), ("size", human_size(e["size"])),
                 ("version", e["version"] or "-"), ("date", e["date"] or "-"),
                 ("license", f"{lic.name} [{lic.status}]" + (f" {e['license_url']}" if e["license_url"] else "")),
                 ("author", e["author"] or "-"), ("source", e["source"]), ("homepage", e["homepage"] or "-"),
                 ("page", e["page"] or "-"), ("description", e["description"] or "-")]
        for k, v in lines:
            print(f"{k + ':':13}{v}")
        for f in e["files"]:
            print(f"{'file:':13}{f['url']} ({human_size(f['size'])})")
        return 0
    if action == "languages":
        s, t = cat.languages()
        if args.json:
            _print_json({"from": s, "to": t})
            return 0
        print("from: " + ", ".join(f"{langs.label(c)}" for c in s))
        print("to:   " + ", ".join(f"{langs.label(c)}" for c in t))
        return 0
    if action == "updates":
        ups = _installer(ctx).updates(cat.entries(include_disabled=True))
        if not ups:
            print("all installed dictionaries are up to date")
            return 0
        for rec, e in ups:
            print(f"{e['id']}: {rec.get('version') or rec.get('date') or '?'} → {e['version'] or e['date']}")
        return 0
    filters = {"text": " ".join(rest), "src": langs.norm(args.src) if args.src else "",
               "dst": langs.norm(args.dst) if args.dst else "", "source": args.source or "",
               "license": args.license, "fmt": (args.fmt or "").lower(), "supported_only": args.supported}
    hits = cat.search(**filters)
    if args.limit:
        hits = hits[: args.limit]
    if args.json:
        _print_json(hits)
        return 0
    if not hits:
        print("nothing found (dct sources refresh to read the sources again)")
        return 0
    from .cli import _table

    inst = _installer(ctx).installed()
    table = [[e["id"], e["name"][:48], _pair(e), e["format"], human_size(e["size"]),
              licenses.get(e["license"]).id, "yes" if e["id"] in inst else ""] for e in hits]
    print(_table(table, ["id", "name", "languages", "format", "size", "license", "installed"]))
    print(f"\n{len(hits)} dictionaries · dct install ID · dct catalog show ID")
    return 0


def _catalog_build(ctx, args, rest) -> int:
    from .center.store import dumps
    from .center.transfer import build_catalog

    if not rest or not args.base_url:
        raise SystemExit("error: dct catalog build FOLDER --base-url URL [-o FILE]")
    folder = Path(rest[0]).expanduser()
    if not folder.is_dir():
        raise SystemExit(f"error: no such folder: {folder}")
    cat = build_catalog(folder, args.base_url, args.name or "", args.default_license or "",
                        read_dictionaries=not args.no_read)
    out = Path(args.output) if args.output else folder / "catalog.json"
    out.write_text(dumps(cat), encoding="utf-8")
    print(f"{len(cat['dictionaries'])} dictionaries → {out}")
    return 0


def _catalog_check(ctx, rest) -> int:
    from .center.transfer import check_catalog

    if not rest:
        raise SystemExit("error: dct catalog check FILE|URL")
    target = rest[0]
    try:
        if target.startswith(("http://", "https://")):
            raw = _prefs_http(ctx).get(target)
        else:
            raw = Path(target).read_bytes()
        data = json.loads(raw.decode("utf-8-sig"))
    except Exception as e:  # noqa: BLE001
        print(f"error: {target}: {e}")
        return 1
    n, problems = check_catalog(data)
    for p in problems:
        print(p)
    errors = sum(1 for p in problems if p.startswith("error"))
    print(f"{n} dictionaries · {errors} errors · {len(problems) - errors} warnings")
    return 1 if errors else 0


# ---------------------------------------------------------------- install

def cmd_install(ctx, args) -> int:
    from .center import adapters

    cat = _catalog(ctx)
    if args.refresh:
        _refresh(ctx, cat)
    inst = _installer(ctx, verify=not args.no_verify, keep=args.keep_archive)
    rc = 0
    if not ctx.prefs["center.online"] and any(not t.startswith("file:") for t in args.targets):
        raise SystemExit("error: offline: center.online is off (dct config set center.online true)")
    for t in args.targets:
        if t.startswith(("http://", "https://", "file:")):
            entry = adapters.entry_from_url(t)
        else:
            hits = cat.find(t)
            if not hits and not any(cat.cached(s["id"])["fetched"] for s in cat.sources.enabled()):
                _refresh(ctx, cat)
                hits = cat.find(t)
            if not hits:
                print(f"error: no catalog entry {t!r} (dct catalog {t})", file=sys.stderr)
                rc = 1
                continue
            if len(hits) > 1 and hits[0]["id"] != t:
                print(f"error: {t!r} matches {len(hits)} entries: " + ", ".join(h["id"] for h in hits[:8]),
                      file=sys.stderr)
                rc = 1
                continue
            entry = hits[0]
        last = [-1]

        def prog(stage, done, total, name=entry["name"], last=last):
            if stage == "download" and total:
                pct = int(done * 100 / total)
                if pct != last[0] and pct % 10 == 0:
                    last[0] = pct
                    print(f"  {name}: {pct}%", file=sys.stderr)

        print(f"downloading: {entry['name']}", file=sys.stderr)
        try:
            rec = inst.install(entry, prog)
        except Exception as e:  # noqa: BLE001
            print(f"error: {entry['name']}: {e}", file=sys.stderr)
            rc = 1
            continue
        print(f"installed: {entry['name']} → {rec['folder']} ({len(rec['files'])} dictionaries)")
        if not rec["files"]:
            print("  dictz cannot open this format yet; the files are kept in the folder above")
        if not args.no_add:
            qm = ctx.qm(load=False)
            for f in rec["files"]:
                try:
                    d = qm.add_dictionary(Path(f))
                    print(f"added: {d.metadata.name} ({d.metadata.format}, {d.metadata.word_count:,} words)")
                except Exception as e:  # noqa: BLE001
                    print(f"error: {f}: {e}", file=sys.stderr)
                    rc = 1
    return rc


def cmd_uninstall(ctx, args) -> int:
    from .storage.registry import norm_path

    inst = _installer(ctx)
    recs = inst.installed()
    rc = 0
    for t in args.targets:
        rid = t if t in recs else next((k for k, r in recs.items() if r["name"].lower() == t.lower()), None)
        if rid is None:
            print(f"error: not installed from the catalog: {t!r} (dct downloads)", file=sys.stderr)
            rc = 1
            continue
        rec = recs[rid]
        files = {norm_path(f) for f in rec.get("files") or []}
        for row in ctx.rows():
            if norm_path(row["path"]) in files:
                ctx.index.remove_dictionary(row["name"])
                print(f"removed: {row['name']}")
        inst.uninstall(rid, delete_files=not args.keep_files)
        print(f"uninstalled: {rec['name']}" + ("" if args.keep_files else f" (deleted {rec['folder']})"))
    return rc


def cmd_downloads(ctx, args) -> int:
    inst = _installer(ctx)
    if args.action == "clear":
        inst.clear_history()
        print("download history cleared")
        return 0
    rows = inst.history()[: args.limit]
    if args.json:
        _print_json({"installed": inst.installed(), "history": rows})
        return 0
    from .cli import _table

    recs = inst.installed()
    if recs:
        print(_table([[r["id"], r["name"][:40], r.get("version") or "-", r["installed"][:10], r["license"]]
                      for r in sorted(recs.values(), key=lambda r: r["name"].lower())],
                     ["installed", "name", "version", "date", "license"]))
        print()
    if not rows:
        print("no downloads yet")
        return 0
    print(_table([[h["finished"][:16].replace("T", " "), h["status"], h["name"][:40], human_size(h.get("size", 0)),
                   h.get("error", "")[:60]] for h in rows], ["when", "status", "name", "size", "error"]))
    return 0


def cmd_licenses(ctx, args) -> int:
    from .center.licenses import LICENSES

    if args.json:
        _print_json([{"id": x.id, "name": x.name, "url": x.url, "status": x.status, "attribution": x.attribution,
                      "share_alike": x.share_alike, "commercial": x.commercial} for x in LICENSES])
        return 0
    from .cli import _table

    yn = {True: "yes", False: "no"}
    print(_table([[x.id, x.status, yn[x.attribution], yn[x.share_alike], yn[x.commercial], x.name]
                  for x in LICENSES], ["id", "status", "credit", "share-alike", "commercial", "name"]))
    print("\nopen = open licence · free = free to use, terms not a licence · unknown = the source does not say")
    return 0


def cmd_setup(ctx, args) -> int:
    from .center import transfer

    path = Path(args.file).expanduser()
    if args.action == "show":
        try:
            m = transfer.read_manifest(path)
        except ValueError as e:
            raise SystemExit(f"error: {e}") from e
        if args.json:
            _print_json(m)
            return 0
        print(f"created: {m.get('created', '')} by {m.get('app', '')}")
        print(f"parts:   {', '.join(m.get('parts') or [])}")
        print(f"dictionaries: {len(m.get('dictionaries') or [])}"
              + (f" ({sum(len(d.get('files') or []) for d in m.get('dictionaries') or [])} files inside)"
                 if "files" in (m.get("parts") or []) else ""))
        return 0
    parts = [p for p in transfer.PARTS if not (
        (p == "dictionaries" and args.no_list) or (p == "settings" and args.no_settings)
        or (p == "sources" and args.no_sources) or (p == "history" and args.no_history)
        or (p == "files" and args.action == "export" and not args.with_files))]
    if args.action == "export":
        m = transfer.export_setup(path, ctx.data_dir, tuple(parts))
        print(f"exported {len(m['dictionaries'])} dictionaries ({', '.join(m['parts'])}) → {path}")
        return 0
    try:
        res = transfer.import_setup(path, ctx.data_dir, replace=args.replace, parts=tuple(parts))
    except ValueError as e:
        raise SystemExit(f"error: {e}") from e
    print(f"imported: {len(res['added'])} dictionaries, {res['files']} files, {res['sources']} sources, "
          f"{res['history']} history, {res['favorites']} favorites"
          + (", settings" if res["settings"] else ""))
    for m in res["missing"]:
        print(f"missing: {m}")
    if res["added"]:
        print("the new dictionaries are opened at the next start (or: dct reindex)")
    return 0


HANDLERS = {"sources": cmd_sources, "catalog": cmd_catalog, "install": cmd_install, "uninstall": cmd_uninstall,
            "downloads": cmd_downloads, "licenses": cmd_licenses, "setup": cmd_setup}

__all__ = ["CENTER_COMMANDS", "HANDLERS", "add_parsers", "human_size", "argparse"]
