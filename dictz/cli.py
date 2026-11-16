"""dictz command line (Python edition).

    dictz                       open the window
    dictz FILE...               add dictionaries, open the window
    dictz <command> [options]   see `dictz help` / `dictz <command> --help`

The Rust edition (`cargo install dictz`) has the same commands, options and
output. Nothing here imports Qt.
"""
from __future__ import annotations

import argparse
import contextlib
import json
import os
import sys
import time
from pathlib import Path

from . import __version__

COMMANDS = [
    ("gui", "open the window (default)"),
    ("lookup", "look a word up and print the articles"),
    ("suggest", "headwords starting with a prefix (+ fuzzy fill)"),
    ("fuzzy", "\"did you mean\" candidates for a word"),
    ("random", "a random article"),
    ("dicts", "list dictionaries"),
    ("add", "add dictionary files or folders"),
    ("remove", "remove dictionaries from dictz (files are kept)"),
    ("enable", "enable dictionaries"),
    ("disable", "disable dictionaries"),
    ("move", "change a dictionary's position"),
    ("info", "details of a dictionary (name or file)"),
    ("reindex", "rebuild the search index"),
    ("scan", "find dictionary files in folders"),
    ("dump", "export a dictionary as tsv / json / txt"),
    ("history", "lookup history"),
    ("favorites", "favorite words"),
    ("config", "show and change settings"),
    ("paths", "where dictz keeps its files"),
    ("doctor", "check the installation"),
    ("selftest", "headless self-test (exit 0 = OK)"),
    ("update", "update this copy of dictz"),
    ("version", "print the version"),
    ("completion", "shell completion script"),
    ("shell", "interactive lookup prompt"),
    ("show", "bring the running window to the front (starts dictz if needed)"),
    ("popup", "look a word up in the running dictz's scan popup"),
    ("watch", "turn clipboard / selection scanning on or off"),
    ("help", "show help"),
]
ALIASES = {"l": "lookup", "s": "suggest", "ls": "dicts", "rm": "remove", "fav": "favorites",
           "upgrade": "update", "f": "fuzzy"}
COMMAND_NAMES = {c for c, _ in COMMANDS} | set(ALIASES)

EDITION = "Python edition"


# --------------------------------------------------------------------- output

class Out:
    def __init__(self, color: bool):
        self.color = color

    def c(self, text: str, code: str) -> str:
        return f"\x1b[{code}m{text}\x1b[0m" if self.color else text

    def bold(self, t: str) -> str:
        return self.c(t, "1")

    def head(self, t: str) -> str:
        return self.c(t, "1;36")

    def dim(self, t: str) -> str:
        return self.c(t, "2")

    def err(self, t: str) -> str:
        return self.c(t, "1;31")


def _use_color(args) -> bool:
    if getattr(args, "no_color", False) or os.environ.get("NO_COLOR"):
        return False
    return sys.stdout.isatty()


def _print_json(obj) -> None:
    print(json.dumps(obj, ensure_ascii=False, indent=2))


def _table(rows: list[list[str]], header: list[str]) -> str:
    widths = [len(h) for h in header]
    for r in rows:
        for i, cell in enumerate(r):
            widths[i] = max(widths[i], len(cell))
    lines = ["  ".join(h.ljust(widths[i]) for i, h in enumerate(header)).rstrip()]
    for r in rows:
        lines.append("  ".join(cell.ljust(widths[i]) for i, cell in enumerate(r)).rstrip())
    return "\n".join(lines)


def _group(n: int) -> str:
    return f"{n:,}"


# --------------------------------------------------------------------- context

class Ctx:
    """Lazily opened stores."""

    def __init__(self, args):
        from .app import user_data_dir

        self.args = args
        self.data_dir = user_data_dir()
        self._index = None
        self._history = None
        self._qm = None
        self._prefs = None

    @property
    def prefs(self):
        if self._prefs is None:
            from .storage.settings import SETTINGS_FILE, Preferences

            self._prefs = Preferences.load(self.data_dir / SETTINGS_FILE)
        return self._prefs

    @property
    def index(self):
        if self._index is None:
            from .storage.indexer import IndexStore

            self._index = IndexStore(self.data_dir / "dictz_index.db")
            self._index.open()
        return self._index

    @property
    def history(self):
        if self._history is None:
            from .storage.history import HistoryStore

            self._history = HistoryStore(self.data_dir / "dictz_history.db")
            self._history.open()
        return self._history

    def qm(self, names: list[str] | None = None, load: bool = True):
        """QueryManager with the enabled (or the given) dictionaries loaded."""
        if self._qm is None:
            from .core.query_manager import QueryManager

            self._qm = QueryManager(self.index)
            if load:
                _, errors = self._qm.restore_from_index(names=names, enabled_only=names is None)
                for e in errors:
                    print(f"warning: {e}", file=sys.stderr)
        return self._qm

    def rows(self):
        return self.index.list_dictionaries(enabled_only=False)

    def resolve_names(self, wanted: list[str]) -> list[str]:
        """Exact name, else case-insensitive, else unique substring."""
        names = [r["name"] for r in self.rows()]
        out = []
        for w in wanted:
            if w in names:
                out.append(w)
                continue
            ci = [n for n in names if n.lower() == w.lower()]
            sub = [n for n in names if w.lower() in n.lower()]
            if len(ci) == 1:
                out.append(ci[0])
            elif len(sub) == 1:
                out.append(sub[0])
            elif not sub:
                raise SystemExit(f"error: no dictionary named {w!r} (see `dictz dicts`)")
            else:
                raise SystemExit(f"error: {w!r} matches several dictionaries: " + ", ".join(sub))
        return out


# --------------------------------------------------------------------- commands

def _article_text(definition: str) -> str:
    from .core.htmltext import to_text

    return to_text(definition)


def print_articles(entries, out: Out) -> None:
    last = None
    for e in entries:
        if e.dictionary_id != last:
            last = e.dictionary_id
            print(out.head(f"== {e.dictionary_id} =="))
        print(out.bold(e.headword))
        for line in _article_text(e.definition).split("\n"):
            print(f"  {line}" if line else "")
        print()


def cmd_lookup(ctx: Ctx, args) -> int:
    word = " ".join(args.word).strip()
    names = ctx.resolve_names(args.dict) if args.dict else None
    qm = ctx.qm(names)
    p = ctx.prefs
    limit = args.limit or int(p["search.max_articles"])
    started = time.perf_counter()
    entries = qm.search(word, exact=not args.prefix, dictionaries=names, max_articles=limit)
    if not entries and not args.prefix and p["search.prefix_fallback"]:
        entries = qm.search(word, exact=False, dictionaries=names, max_articles=limit)
    if not entries and args.fuzzy is not False and p["search.fuzzy"]:
        cands = qm.fuzzy(word, int(p["search.fuzzy_limit"]), float(p["search.fuzzy_cutoff"]))
        if args.fuzzy and cands:
            entries = qm.search(cands[0][0], exact=True, dictionaries=names, max_articles=limit)
    else:
        cands = []
    elapsed = (time.perf_counter() - started) * 1000
    if p["history.enabled"] and entries and not args.no_history:
        ctx.history.record(word)
    out = Out(_use_color(args))
    if args.json:
        _print_json([{"dictionary": e.dictionary_id, "headword": e.headword, "definition": e.definition,
                      "text": _article_text(e.definition)} for e in entries])
        return 0 if entries else 1
    if not entries:
        print(out.err(f"No results for '{word}'."), file=sys.stderr)
        if cands:
            print("Did you mean: " + ", ".join(f"{w} ({s:.0f}%)" for w, s in cands), file=sys.stderr)
        return 1
    if args.html:
        for e in entries:
            print(f"<!-- {e.dictionary_id} -->\n<h3>{e.headword}</h3>\n{e.definition}\n")
    else:
        print_articles(entries, out)
    if args.verbose:
        n = len({e.dictionary_id for e in entries})
        print(out.dim(f"{len(entries)} articles in {n} dictionaries, {elapsed:.0f} ms"), file=sys.stderr)
    return 0


def cmd_suggest(ctx: Ctx, args) -> int:
    from .core import fuzzy

    p = ctx.prefs
    enabled = [r["name"] for r in ctx.rows() if r["enabled"]]
    prefix = " ".join(args.prefix).strip()
    words = [w for w, _ in ctx.index.search_headword(prefix, limit=args.limit, dictionaries=enabled)]
    words = fuzzy.rank(prefix, list(dict.fromkeys(words)))
    items = [{"word": w, "fuzzy": False} for w in words]
    if p["search.fuzzy"] and not args.no_fuzzy and len(items) < int(p["search.fuzzy_fill_below"]):
        seen = set(words)
        for w, _s in fuzzy.extract(prefix, ctx.index.headwords(enabled), int(p["search.fuzzy_limit"]),
                                   float(p["search.fuzzy_cutoff"])):
            if len(items) >= args.limit:
                break
            if w not in seen:
                seen.add(w)
                items.append({"word": w, "fuzzy": True})
    if args.json:
        _print_json(items)
    else:
        for it in items:
            print(("≈ " if it["fuzzy"] else "") + it["word"])
    return 0 if items else 1


def cmd_fuzzy(ctx: Ctx, args) -> int:
    from .core import fuzzy

    p = ctx.prefs
    enabled = [r["name"] for r in ctx.rows() if r["enabled"]]
    word = " ".join(args.word).strip()
    limit = args.limit if args.limit is not None else int(p["search.fuzzy_limit"])
    cutoff = args.cutoff if args.cutoff is not None else float(p["search.fuzzy_cutoff"])
    hits = fuzzy.extract(word, ctx.index.headwords(enabled), limit, cutoff)
    if args.json:
        _print_json([{"word": w, "score": s} for w, s in hits])
    else:
        for w, s in hits:
            print(f"{s:6.2f}  {w}")
    return 0 if hits else 1


def cmd_random(ctx: Ctx, args) -> int:
    names = ctx.resolve_names([args.dict]) if args.dict else None
    qm = ctx.qm(names)
    e = qm.random_entry(names[0] if names else None)
    if e is None:
        print("No dictionaries.", file=sys.stderr)
        return 1
    if args.json:
        _print_json({"dictionary": e.dictionary_id, "headword": e.headword, "definition": e.definition,
                     "text": _article_text(e.definition)})
    else:
        print_articles([e], Out(_use_color(args)))
    return 0


def _row_dict(i: int, r) -> dict:
    return {"position": i + 1, "name": r["name"], "enabled": bool(r["enabled"]), "format": r["format"],
            "words": int(r["word_count"] or 0), "path": r["path"],
            "source_language": r["source_lang"] or "", "target_language": r["target_lang"] or "",
            "exists": Path(r["path"]).exists()}


def cmd_dicts(ctx: Ctx, args) -> int:
    rows = [_row_dict(i, r) for i, r in enumerate(ctx.rows())]
    if args.names:
        for r in rows:
            print(r["name"])
        return 0
    if args.json:
        _print_json(rows)
        return 0
    if not rows:
        print("No dictionaries. Add one with: dictz add FILE")
        return 0
    table = [[str(r["position"]), "yes" if r["enabled"] else "no", r["name"], r["format"], _group(r["words"]),
              r["path"] + ("" if r["exists"] else "  (missing)")] for r in rows]
    print(_table(table, ["#", "on", "name", "format", "words", "path"]))
    total = sum(r["words"] for r in rows if r["enabled"])
    print(f"\n{sum(1 for r in rows if r['enabled'])} of {len(rows)} enabled · {_group(total)} words")
    return 0


def _expand_paths(paths: list[str], recursive: bool) -> list[Path]:
    from .core.query_manager import scan_folder

    out: list[Path] = []
    for p in paths:
        pp = Path(p).expanduser().absolute()
        if pp.is_dir():
            out.extend(scan_folder(pp, recursive))
        else:
            out.append(pp)
    return out


def cmd_add(ctx: Ctx, args) -> int:
    qm = ctx.qm(load=False)
    files = _expand_paths(args.paths, args.recursive)
    if not files:
        print("No dictionary files found.", file=sys.stderr)
        return 1
    # keep the existing order: attach rows first without loading
    rc = 0
    for f in files:
        flags = {r["name"]: r["enabled"] for r in ctx.rows()}
        try:
            d = qm.add_dictionary(f, reindex=args.reindex)
        except Exception as exc:  # noqa: BLE001
            print(f"error: {f}: {exc}", file=sys.stderr)
            rc = 1
            continue
        name = d.metadata.name
        # add_dictionary appends; restore true positions/flags of the others
        order = [r["name"] for r in ctx.rows() if r["name"] != name] + [name]
        for i, n in enumerate(order):
            ctx.index.set_sort_order(n, i)
            if n != name and n in flags:
                ctx.index.set_enabled(n, bool(flags[n]))
        if args.disabled:
            ctx.index.set_enabled(name, False)
        print(f"added: {name} ({d.metadata.format}, {_group(d.metadata.word_count)} words)")
    return rc


def cmd_remove(ctx: Ctx, args) -> int:
    for n in ctx.resolve_names(args.names):
        ctx.index.remove_dictionary(n)
        print(f"removed: {n}")
    _renumber(ctx)
    return 0


def _renumber(ctx: Ctx) -> None:
    for i, r in enumerate(ctx.rows()):
        ctx.index.set_sort_order(r["name"], i)


def cmd_enable(ctx: Ctx, args, on: bool = True) -> int:
    for n in ctx.resolve_names(args.names):
        ctx.index.set_enabled(n, on)
        print(f"{'enabled' if on else 'disabled'}: {n}")
    return 0


def cmd_move(ctx: Ctx, args) -> int:
    (name,) = ctx.resolve_names([args.name])
    names = [r["name"] for r in ctx.rows()]
    i = names.index(name)
    where = args.where.lower()
    if where == "up":
        j = i - 1
    elif where == "down":
        j = i + 1
    elif where == "top":
        j = 0
    elif where == "bottom":
        j = len(names) - 1
    else:
        try:
            j = int(where) - 1
        except ValueError:
            raise SystemExit("error: position must be up, down, top, bottom or a number") from None
    j = max(0, min(len(names) - 1, j))
    names.insert(j, names.pop(i))
    for k, n in enumerate(names):
        ctx.index.set_sort_order(n, k)
    print(f"{name}: position {j + 1}")
    return 0


def cmd_info(ctx: Ctx, args) -> int:
    target = " ".join(args.target)
    from .core.query_manager import QueryManager
    from .storage.indexer import file_mtime

    if Path(target).expanduser().exists():
        path = Path(target).expanduser()
    else:
        (name,) = ctx.resolve_names([target])
        path = Path(ctx.index.get_dictionary(name)["path"])
    d = QueryManager.open_file(path)
    m = d.metadata
    row = ctx.index.get_dictionary(m.name)
    info = {"name": m.name, "format": m.format, "words": m.word_count, "path": str(path),
            "source_language": m.source_language, "target_language": m.target_language,
            "author": m.author, "description": m.description,
            "size_bytes": path.stat().st_size if path.exists() else 0,
            "modified": time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(file_mtime(path))),
            "in_dictz": row is not None, "enabled": bool(row["enabled"]) if row else False}
    if args.json:
        _print_json(info)
        return 0
    for k, v in info.items():
        if isinstance(v, bool):
            v = "yes" if v else "no"
        if v not in ("", None):
            print(f"{k.replace('_', ' '):16} {v}")
    return 0


def cmd_reindex(ctx: Ctx, args) -> int:
    names = ctx.resolve_names(args.names) if args.names else [r["name"] for r in ctx.rows()]
    from .core.query_manager import QueryManager

    rc = 0
    for n in names:
        row = ctx.index.get_dictionary(n)
        try:
            d = QueryManager.open_file(row["path"])
            count = ctx.index.index_dictionary(d)
            print(f"reindexed: {n} ({_group(count)} headwords)")
        except Exception as exc:  # noqa: BLE001
            print(f"error: {n}: {exc}", file=sys.stderr)
            rc = 1
    return rc


def cmd_scan(ctx: Ctx, args) -> int:
    files = _expand_paths(args.folders, not args.no_recursive)
    known = {str(Path(r["path"]).resolve()) for r in ctx.rows()}
    for f in files:
        mark = "added" if str(f.resolve()) in known else "new"
        print(f"{mark:6} {f}")
    if args.add:
        new = [f for f in files if str(f.resolve()) not in known]
        if new:
            ns = argparse.Namespace(paths=[str(f) for f in new], recursive=False, reindex=False, disabled=False)
            return cmd_add(ctx, ns)
    return 0


def cmd_dump(ctx: Ctx, args) -> int:
    from .core.query_manager import QueryManager

    target = args.target
    if Path(target).expanduser().exists():
        path = Path(target).expanduser()
    else:
        (name,) = ctx.resolve_names([target])
        path = Path(ctx.index.get_dictionary(name)["path"])
    d = QueryManager.open_file(path)
    fh = open(args.output, "w", encoding="utf-8", newline="\n") if args.output else sys.stdout  # noqa: SIM115
    try:
        if args.format == "json":
            json.dump([{"headword": e.headword, "definition": e.definition} for e in d.iter_entries()],
                      fh, ensure_ascii=False, indent=1)
            fh.write("\n")
        else:
            for e in d.iter_entries():
                if args.format == "tsv":
                    defi = e.definition.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")
                    fh.write(f"{e.headword}\t{defi}\n")
                else:
                    fh.write(e.headword + "\n")
                    for line in _article_text(e.definition).split("\n"):
                        fh.write(f"  {line}\n" if line else "\n")
                    fh.write("\n")
    finally:
        if fh is not sys.stdout:
            fh.close()
    if args.output:
        print(f"wrote {_group(d.metadata.word_count)} entries to {args.output}", file=sys.stderr)
    return 0


def cmd_history(ctx: Ctx, args) -> int:
    if args.clear:
        ctx.history.clear_history()
        print("history cleared")
        return 0
    if args.remove:
        ctx.history.remove_history(args.remove)
        return 0
    words = ctx.history.recent(args.limit)
    if args.json:
        _print_json(words)
    else:
        for w in words:
            print(w)
    return 0


def cmd_favorites(ctx: Ctx, args) -> int:
    action = args.action or "list"
    if action == "add":
        for w in args.words:
            ctx.history.add_favorite(w)
            print(f"added: {w}")
    elif action == "remove":
        for w in args.words:
            ctx.history.remove_favorite(w)
            print(f"removed: {w}")
    elif action == "clear":
        ctx.history.clear_favorites()
        print("favorites cleared")
    else:
        words = ctx.history.favorites(args.limit)
        if args.json:
            _print_json(words)
        else:
            for w in words:
                print(w)
    return 0


def _fmt_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, list):
        return json.dumps(v, ensure_ascii=False)
    if isinstance(v, float):
        return f"{v:g}"
    return str(v)


def cmd_config(ctx: Ctx, args) -> int:
    from .storage.settings import SCHEMA, SCHEMA_BY_KEY, SECTIONS

    p = ctx.prefs
    action = args.action or "list"
    if action == "path":
        print(p.path)
    elif action == "get":
        if args.key not in p.values:
            raise SystemExit(f"error: unknown setting {args.key!r} (see `dictz config list`)")
        print(_fmt_value(p[args.key]))
    elif action == "set":
        if args.value is None:
            raise SystemExit("error: usage: dictz config set KEY VALUE")
        value = args.value
        s = SCHEMA_BY_KEY.get(args.key)
        if s is not None and s.kind == "list" and value.strip().startswith("["):
            value = json.loads(value)
        try:
            v = p.set(args.key, value, save=True)
        except (KeyError, ValueError) as exc:
            raise SystemExit(f"error: {exc}") from None
        print(f"{args.key} = {_fmt_value(v)}")
    elif action == "reset":
        try:
            p.reset(args.key)
        except KeyError as exc:
            raise SystemExit(f"error: {exc}") from None
        p.save()
        print(f"reset: {args.key or 'all settings'}")
    elif action == "export":
        data = json.dumps(p.export(), ensure_ascii=False, indent=2)
        if args.key:
            Path(args.key).write_text(data + "\n", encoding="utf-8")
            print(f"exported to {args.key}")
        else:
            print(data)
    elif action == "import":
        if not args.key:
            raise SystemExit("error: usage: dictz config import FILE")
        errors = p.import_dict(json.loads(Path(args.key).read_text(encoding="utf-8")))
        p.save()
        for e in errors:
            print(f"warning: {e}", file=sys.stderr)
        print(f"imported {args.key}")
    else:  # list
        if args.json:
            _print_json([{"key": s.key, "value": p[s.key], "default": s.default, "kind": s.kind,
                          "label": s.label} for s in SCHEMA])
            return 0
        out = Out(_use_color(args))
        for sec_id, title in SECTIONS:
            print(out.head(f"[{sec_id}] {title}"))
            for s in SCHEMA:
                if s.key.split(".")[0] != sec_id:
                    continue
                v = _fmt_value(p[s.key])
                changed = p[s.key] != s.default
                extra = ""
                if s.kind == "choice":
                    extra = " (" + "|".join(c[0] for c in s.choices) + ")"
                elif s.min is not None:
                    lo, hi = (int(s.min), int(s.max)) if s.kind == "int" else (f"{s.min:g}", f"{s.max:g}")
                    extra = f" ({lo}–{hi})"
                line = f"  {s.key:34} {v:22} {s.label}{extra}"
                print(out.bold(line) if changed else line)
            print()
    return 0


def _paths(ctx: Ctx) -> dict:
    from .storage.settings import SETTINGS_FILE

    d = ctx.data_dir
    return {"data_dir": str(d), "settings": str(d / SETTINGS_FILE), "index_db": str(d / "dictz_index.db"),
            "history_db": str(d / "dictz_history.db"), "media_cache": str(d / "media-cache"),
            "executable": sys.executable if getattr(sys, "frozen", False) else str(Path(sys.argv[0]).resolve())}


def cmd_paths(ctx: Ctx, args) -> int:
    paths = _paths(ctx)
    if args.json:
        _print_json(paths)
    else:
        for k, v in paths.items():
            print(f"{k:12} {v}")
    return 0


def cmd_doctor(ctx: Ctx, args) -> int:
    import platform

    from .core.updater import detect_install
    from .selftest import run as selftest

    out = Out(_use_color(args))
    inst = detect_install()
    print(out.head("dictz doctor"))
    print(f"  version      {__version__} ({EDITION})")
    print(f"  install      {inst.kind} — {inst.detail}")
    print(f"  python       {platform.python_version()} ({sys.executable})")
    print(f"  platform     {platform.platform()}")
    try:
        import pyglossary

        print(f"  pyglossary   {getattr(pyglossary, '__version__', '?')}")
    except ImportError:
        print(f"  pyglossary   {out.err('missing')} (DSL / Babylon need it)")
    try:
        import PySide6

        print(f"  PySide6      {PySide6.__version__}")
    except ImportError:
        print(f"  PySide6      {out.err('missing')} (needed for the window)")
    for k, v in _paths(ctx).items():
        print(f"  {k:12} {v}")
    st = ctx.index.stats()
    print(f"  index        {st['dictionaries']} dictionaries, {_group(st['entries'])} headwords")
    print(f"  history      {_group(ctx.history.count())} lookups, {len(ctx.history.favorites(100000))} favorites")
    problems = 0
    for r in ctx.rows():
        if not Path(r["path"]).exists():
            problems += 1
            print(out.err(f"  missing file: {r['name']} -> {r['path']}"))
    print(out.head("selftest"))
    rc = selftest()
    return 1 if (rc or problems) else 0


def cmd_version(ctx: Ctx, args) -> int:
    if getattr(args, "json", False):
        import platform

        _print_json({"version": __version__, "edition": "python", "python": platform.python_version()})
    else:
        print(f"dictz {__version__} ({EDITION})")
    return 0


def completion_script(shell: str) -> str:
    cmds = " ".join(sorted({c for c, _ in COMMANDS} | set(ALIASES)))
    dict_cmds = "lookup l remove rm enable disable move info reindex dump random"
    if shell == "bash":
        return f"""# dictz bash completion — add to ~/.bashrc:  eval "$(dictz completion bash)"
_dictz() {{
    local cur prev
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[1]}}"
    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "{cmds} --version --help" -- "$cur") )
        return
    fi
    case "$prev" in
        enable|disable|remove|rm|move|info|reindex|dump)
            local IFS=$'\\n'
            COMPREPLY=( $(compgen -W "$(dictz dicts --names 2>/dev/null)" -- "$cur") ) ;;
        config) COMPREPLY=( $(compgen -W "list get set reset path export import" -- "$cur") ) ;;
        favorites|fav) COMPREPLY=( $(compgen -W "list add remove clear" -- "$cur") ) ;;
        completion) COMPREPLY=( $(compgen -W "bash zsh fish powershell" -- "$cur") ) ;;
        watch) COMPREPLY=( $(compgen -W "on off toggle status" -- "$cur") ) ;;
        add|scan) COMPREPLY=( $(compgen -f -- "$cur") ) ;;
    esac
}}
complete -o default -F _dictz dictz
"""
    if shell == "zsh":
        return f"""#compdef dictz
# dictz zsh completion — add to ~/.zshrc:  eval "$(dictz completion zsh)"
_dictz() {{
    if (( CURRENT == 2 )); then
        compadd -- {cmds}
        return
    fi
    case "${{words[2]}}" in
        enable|disable|remove|rm|move|info|reindex|dump) compadd -- ${{(f)"$(dictz dicts --names 2>/dev/null)"}} ;;
        config) compadd -- list get set reset path export import ;;
        favorites|fav) compadd -- list add remove clear ;;
        completion) compadd -- bash zsh fish powershell ;;
        watch) compadd -- on off toggle status ;;
        *) _files ;;
    esac
}}
compdef _dictz dictz
"""
    if shell == "fish":
        lines = ["# dictz fish completion — save as ~/.config/fish/completions/dictz.fish",
                 "complete -c dictz -f"]
        for c, desc in COMMANDS:
            lines.append(f"complete -c dictz -n '__fish_use_subcommand' -a {c} -d '{desc.replace(chr(39), '')}'")
        lines.append(f"complete -c dictz -n '__fish_seen_subcommand_from {dict_cmds}' "
                     "-a '(dictz dicts --names 2>/dev/null)'")
        lines.append("complete -c dictz -n '__fish_seen_subcommand_from config' "
                     "-a 'list get set reset path export import'")
        lines.append("complete -c dictz -n '__fish_seen_subcommand_from favorites fav' -a 'list add remove clear'")
        lines.append("complete -c dictz -n '__fish_seen_subcommand_from completion' -a 'bash zsh fish powershell'")
        lines.append("complete -c dictz -n '__fish_seen_subcommand_from watch' -a 'on off toggle status'")
        lines.append("complete -c dictz -n '__fish_seen_subcommand_from add scan' -F")
        return "\n".join(lines) + "\n"
    if shell == "powershell":
        quoted = ", ".join(f"'{c}'" for c in sorted({c for c, _ in COMMANDS} | set(ALIASES)))
        return f"""# dictz PowerShell completion — add to $PROFILE:  dictz completion powershell | Out-String | Invoke-Expression
Register-ArgumentCompleter -Native -CommandName dictz -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)
    $words = $commandAst.CommandElements | ForEach-Object {{ $_.ToString() }}
    if ($words.Count -le 2) {{
        @({quoted}) | Where-Object {{ $_ -like "$wordToComplete*" }} |
            ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_) }}
        return
    }}
    switch ($words[1]) {{
        {{ $_ -in 'enable','disable','remove','rm','move','info','reindex','dump' }} {{
            dictz dicts --names | Where-Object {{ $_ -like "$wordToComplete*" }} |
                ForEach-Object {{ [System.Management.Automation.CompletionResult]::new("'$_'", $_, 'ParameterValue', $_) }}
        }}
        'config' {{ @('list','get','set','reset','path','export','import') | Where-Object {{ $_ -like "$wordToComplete*" }} }}
        'completion' {{ @('bash','zsh','fish','powershell') | Where-Object {{ $_ -like "$wordToComplete*" }} }}
        'watch' {{ @('on','off','toggle','status') | Where-Object {{ $_ -like "$wordToComplete*" }} }}
    }}
}}
"""
    raise SystemExit("error: shell must be bash, zsh, fish or powershell")


def cmd_completion(ctx: Ctx, args) -> int:
    sys.stdout.write(completion_script(args.shell))
    return 0


def cmd_shell(ctx: Ctx, args) -> int:
    with contextlib.suppress(ImportError):
        import readline  # noqa: F401 — line editing + history
    qm = ctx.qm()
    out = Out(_use_color(args))
    n = len(qm.loaded_names())
    print(f"dictz {__version__} — {n} dictionaries. Type a word; :help for commands, :q to quit.")
    while True:
        try:
            line = input("dictz> ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            return 0
        if not line:
            continue
        if line in (":q", ":quit", ":exit"):
            return 0
        if line in (":h", ":help", "?"):
            print(":fuzzy WORD   did-you-mean candidates\n:suggest P    prefix suggestions\n"
                  ":dicts        list dictionaries\n:random       random article\n:q            quit")
            continue
        if line.startswith(":fuzzy "):
            for w, s in qm.fuzzy(line[7:].strip()):
                print(f"{s:6.2f}  {w}")
            continue
        if line.startswith(":suggest "):
            for s in qm.suggest(line[9:].strip(), 30):
                print(("≈ " if s.fuzzy else "") + s.word)
            continue
        if line == ":dicts":
            for name in qm.loaded_names():
                print(("  " if qm.is_enabled(name) else "- ") + name)
            continue
        if line == ":random":
            e = qm.random_entry()
            if e:
                print_articles([e], out)
            continue
        entries = qm.search(line, exact=True) or qm.search(line, exact=False)
        if entries:
            print_articles(entries, out)
            if ctx.prefs["history.enabled"]:
                ctx.history.record(line)
        else:
            cands = qm.fuzzy(line)
            print(out.err(f"No results for '{line}'.")
                  + (" Did you mean: " + ", ".join(w for w, _ in cands) if cands else ""))


def cmd_update(ctx: Ctx, args) -> int:
    from .core.updater import run_update

    return run_update(check_only=args.check, assume_yes=args.yes)


def _start_detached(extra: list[str]) -> bool:
    """Start the window in the background (``dictz gui …``)."""
    import subprocess

    if getattr(sys, "frozen", False):
        cmd = [sys.executable, "gui", *extra]
    else:
        cmd = [sys.executable, "-m", "dictz", "gui", *extra]
    kw: dict = {"stdin": subprocess.DEVNULL, "stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
    if sys.platform == "win32":
        kw["creationflags"] = 0x00000008 | 0x00000200  # DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    else:
        kw["start_new_session"] = True
    try:
        subprocess.Popen(cmd, **kw)  # noqa: S603
        return True
    except OSError as exc:
        print(f"error: cannot start dictz: {exc}", file=sys.stderr)
        return False


def cmd_show(ctx: Ctx, args) -> int:
    from .core.instance import send

    answer = send(ctx.data_dir, {"cmd": "show"})
    if answer and answer.get("ok"):
        return 0
    print("dictz is not running; starting it.")
    return 0 if _start_detached([]) else 1


def cmd_popup(ctx: Ctx, args) -> int:
    from .core.instance import send

    word = " ".join(args.word).strip()
    answer = send(ctx.data_dir, {"cmd": "popup", "word": word})
    if answer and answer.get("ok"):
        return 0
    if answer:
        print(f"error: {answer.get('error', 'failed')}", file=sys.stderr)
        return 1
    print("dictz is not running; starting it.")
    return 0 if _start_detached(["--popup", word]) else 1


def cmd_watch(ctx: Ctx, args) -> int:
    from .core.instance import send

    answer = send(ctx.data_dir, {"cmd": "scan", "state": args.state})
    if not answer:
        print("error: dictz is not running (start it with `dictz` or `dictz show`)", file=sys.stderr)
        return 1
    if not answer.get("ok"):
        print(f"error: {answer.get('error', 'failed')}", file=sys.stderr)
        return 1
    print("scanning: " + ("on" if answer.get("scan") else "off"))
    return 0


def cmd_selftest(ctx: Ctx, args) -> int:
    from .selftest import run

    return run()


# --------------------------------------------------------------------- parser

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="dictz",
        description=f"dictz {__version__} — offline dictionary (StarDict, DSL, Babylon). "
                    "Without a command the window opens.",
        epilog="Same commands in the Rust edition (cargo install dictz). "
               "`dictz <command> --help` for details.",
    )
    p.add_argument("-v", "-V", "--version", action="store_true", help="print the version and exit")
    p.add_argument("--selftest", action="store_true", help=argparse.SUPPRESS)
    p.add_argument("--data-dir", metavar="DIR", help="use another data folder (also DICTZ_DATA_DIR)")
    p.add_argument("--no-color", action="store_true", help="no colors (also NO_COLOR)")
    sub = p.add_subparsers(dest="command", metavar="<command>")

    def add(name: str, aliases: list[str] | None = None):
        helptext = dict(COMMANDS)[name]
        return sub.add_parser(name, aliases=aliases or [], help=helptext, description=helptext)

    g = add("gui")
    g.add_argument("files", nargs="*", help="dictionary files to add first")
    g.add_argument("--popup", metavar="WORD", help="open the scan popup with WORD (empty = clipboard)")

    lk = add("lookup", ["l"])
    lk.add_argument("word", nargs="+")
    lk.add_argument("-d", "--dict", action="append", metavar="NAME", help="only this dictionary (repeatable)")
    lk.add_argument("-p", "--prefix", action="store_true", help="all headwords starting with WORD")
    lk.add_argument("-f", "--fuzzy", action="store_true", default=None,
                    help="when nothing matches, show the closest headword's article")
    lk.add_argument("--no-fuzzy", dest="fuzzy", action="store_false", help="no did-you-mean list")
    lk.add_argument("-n", "--limit", type=int, default=None, metavar="N", help="at most N articles")
    lk.add_argument("--json", action="store_true", help="JSON output")
    lk.add_argument("--html", action="store_true", help="print the raw HTML definitions")
    lk.add_argument("--no-history", action="store_true", help="do not record in the history")
    lk.add_argument("--verbose", action="store_true", help="counts and timing on stderr")
    lk.add_argument("--no-color", action="store_true", default=argparse.SUPPRESS, help=argparse.SUPPRESS)

    sg = add("suggest", ["s"])
    sg.add_argument("prefix", nargs="+")
    sg.add_argument("-n", "--limit", type=int, default=30, metavar="N")
    sg.add_argument("--no-fuzzy", action="store_true", help="prefix matches only")
    sg.add_argument("--json", action="store_true")

    fz = add("fuzzy", ["f"])
    fz.add_argument("word", nargs="+")
    fz.add_argument("-n", "--limit", type=int, default=None, metavar="N")
    fz.add_argument("-c", "--cutoff", type=float, default=None, metavar="PCT", help="minimum similarity 0–100")
    fz.add_argument("--json", action="store_true")

    rd = add("random")
    rd.add_argument("-d", "--dict", metavar="NAME")
    rd.add_argument("--json", action="store_true")
    rd.add_argument("--no-color", action="store_true", default=argparse.SUPPRESS, help=argparse.SUPPRESS)

    ds = add("dicts", ["ls"])
    ds.add_argument("--json", action="store_true")
    ds.add_argument("--names", action="store_true", help="names only, one per line")

    ad = add("add")
    ad.add_argument("paths", nargs="+", metavar="PATH", help="dictionary files or folders")
    ad.add_argument("-r", "--recursive", action="store_true", help="search folders recursively")
    ad.add_argument("--reindex", action="store_true", help="rebuild the index even if known")
    ad.add_argument("--disabled", action="store_true", help="add but keep disabled")

    rm = add("remove", ["rm"])
    rm.add_argument("names", nargs="+", metavar="NAME")

    en = add("enable")
    en.add_argument("names", nargs="+", metavar="NAME")
    di = add("disable")
    di.add_argument("names", nargs="+", metavar="NAME")

    mv = add("move")
    mv.add_argument("name", metavar="NAME")
    mv.add_argument("where", metavar="up|down|top|bottom|POSITION")

    inf = add("info")
    inf.add_argument("target", nargs="+", metavar="NAME|FILE")
    inf.add_argument("--json", action="store_true")

    ri = add("reindex")
    ri.add_argument("names", nargs="*", metavar="NAME", help="default: all")

    sc = add("scan")
    sc.add_argument("folders", nargs="+", metavar="FOLDER")
    sc.add_argument("--no-recursive", action="store_true")
    sc.add_argument("--add", action="store_true", help="add the new ones")

    dp = add("dump")
    dp.add_argument("target", metavar="NAME|FILE")
    dp.add_argument("-F", "--format", choices=["tsv", "json", "txt"], default="tsv")
    dp.add_argument("-o", "--output", metavar="FILE")

    hs = add("history")
    hs.add_argument("-n", "--limit", type=int, default=50, metavar="N")
    hs.add_argument("--json", action="store_true")
    hs.add_argument("--clear", action="store_true", help="delete all history")
    hs.add_argument("--remove", metavar="WORD", help="delete one word from the history")

    fv = add("favorites", ["fav"])
    fv.add_argument("action", nargs="?", choices=["list", "add", "remove", "clear"])
    fv.add_argument("words", nargs="*", metavar="WORD")
    fv.add_argument("-n", "--limit", type=int, default=500, metavar="N")
    fv.add_argument("--json", action="store_true")

    cf = add("config")
    cf.add_argument("action", nargs="?", choices=["list", "get", "set", "reset", "path", "export", "import"])
    cf.add_argument("key", nargs="?", metavar="KEY|FILE")
    cf.add_argument("value", nargs="?", metavar="VALUE")
    cf.add_argument("--json", action="store_true")
    cf.add_argument("--no-color", action="store_true", default=argparse.SUPPRESS, help=argparse.SUPPRESS)

    pa = add("paths")
    pa.add_argument("--json", action="store_true")

    dr = add("doctor")
    dr.add_argument("--no-color", action="store_true", default=argparse.SUPPRESS, help=argparse.SUPPRESS)
    add("selftest")

    up = add("update", ["upgrade"])
    up.add_argument("--check", action="store_true", help="only check, do not install")
    up.add_argument("-y", "--yes", action="store_true", help="do not ask")

    vs = add("version")
    vs.add_argument("--json", action="store_true")

    cp = add("completion")
    cp.add_argument("shell", choices=["bash", "zsh", "fish", "powershell"])

    sh = add("shell")
    sh.add_argument("--no-color", action="store_true", default=argparse.SUPPRESS, help=argparse.SUPPRESS)

    add("show")
    pp = add("popup")
    pp.add_argument("word", nargs="*", help="default: the clipboard text")
    wt = add("watch")
    wt.add_argument("state", nargs="?", choices=["on", "off", "toggle", "status"], default="status")

    hp = add("help")
    hp.add_argument("topic", nargs="?", metavar="COMMAND")
    return p


HANDLERS = {
    "lookup": cmd_lookup, "suggest": cmd_suggest, "fuzzy": cmd_fuzzy, "random": cmd_random,
    "dicts": cmd_dicts, "add": cmd_add, "remove": cmd_remove,
    "enable": lambda c, a: cmd_enable(c, a, True), "disable": lambda c, a: cmd_enable(c, a, False),
    "move": cmd_move, "info": cmd_info, "reindex": cmd_reindex, "scan": cmd_scan, "dump": cmd_dump,
    "history": cmd_history, "favorites": cmd_favorites, "config": cmd_config, "paths": cmd_paths,
    "doctor": cmd_doctor, "selftest": cmd_selftest, "update": cmd_update, "version": cmd_version,
    "completion": cmd_completion, "shell": cmd_shell, "show": cmd_show, "popup": cmd_popup,
    "watch": cmd_watch,
}


def is_cli_invocation(argv: list[str]) -> bool:
    """True when argv asks for a CLI command (not just files for the GUI)."""
    if not argv:
        return False
    first = argv[0]
    return first.startswith("-") or first in COMMAND_NAMES


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.data_dir:
        os.environ["DICTZ_DATA_DIR"] = str(Path(args.data_dir).expanduser())
    if args.version:
        print(f"dictz {__version__} ({EDITION})")
        return 0
    if args.selftest:
        from .selftest import run

        return run()
    cmd = ALIASES.get(args.command, args.command)
    if cmd is None or cmd == "gui":
        from .app import run_gui

        return run_gui(getattr(args, "files", []) or [], popup=getattr(args, "popup", None))
    if cmd == "help":
        if args.topic:
            parser.parse_args([ALIASES.get(args.topic, args.topic), "--help"])
        parser.print_help()
        return 0
    if not hasattr(args, "no_color"):
        args.no_color = False
    ctx = Ctx(args)
    try:
        return HANDLERS[cmd](ctx, args) or 0
    except BrokenPipeError:
        return 0
    except KeyboardInterrupt:
        return 130
