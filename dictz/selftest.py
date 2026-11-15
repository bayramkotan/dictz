"""Headless self-test: `dictz --selftest`.

Proves a build can actually read dictionaries without opening a window.
CI runs it against every frozen binary (Windows .exe, Linux binary + AppImage,
macOS .app) so a release that cannot open StarDict / DSL / Babylon files is
never published.

Exit code 0 = all checks passed, 1 = at least one failed. The report goes to
stdout and, when DICTZ_SELFTEST_REPORT is set, to that file as well (Windows
GUI executables have no attached console).
"""
from __future__ import annotations

import os
import struct
import sys
import tempfile
import traceback
from pathlib import Path


def _write_stardict(base: Path) -> Path:
    entries = [("apple", "elma"), ("book", "kitap")]
    body = b""
    idx = b""
    for word, definition in entries:
        data = definition.encode("utf-8")
        idx += word.encode("utf-8") + b"\x00" + struct.pack(">II", len(body), len(data))
        body += data
    base.with_suffix(".dict").write_bytes(body)
    base.with_suffix(".idx").write_bytes(idx)
    base.with_suffix(".ifo").write_text(
        "StarDict's dict ifo file\nversion=2.4.2\nbookname=selftest-stardict\n"
        f"wordcount={len(entries)}\nidxfilesize={len(idx)}\n",
        encoding="utf-8",
    )
    return base.with_suffix(".ifo")


def _write_dsl(path: Path) -> Path:
    path.write_text(
        "﻿#NAME \"selftest-dsl\"\n#INDEX_LANGUAGE \"English\"\n"
        "#CONTENTS_LANGUAGE \"Turkish\"\n\n"
        "apple\n\t[trn]elma[/trn]\n\nbook\n\t[trn]kitap[/trn]\n",
        encoding="utf-16",
    )
    return path


def run() -> int:
    from . import __version__

    lines: list[str] = [f"dictz {__version__} selftest", f"python {sys.version.split()[0]}",
                        f"frozen={bool(getattr(sys, 'frozen', False))}"]
    failures = 0

    def check(name: str, fn) -> None:
        nonlocal failures
        try:
            detail = fn()
            lines.append(f"PASS  {name}" + (f"  ({detail})" if detail else ""))
        except Exception as exc:  # noqa: BLE001 — report every failure
            failures += 1
            lines.append(f"FAIL  {name}: {type(exc).__name__}: {exc}")
            lines.append(traceback.format_exc())

    with tempfile.TemporaryDirectory(prefix="dictz-selftest-") as tmp:
        tmpdir = Path(tmp)

        def stardict() -> str:
            from .core.query_manager import QueryManager
            qm = QueryManager()
            qm.add_dictionary(_write_stardict(tmpdir / "sd"))
            hits = qm.search("apple")
            assert hits and "elma" in hits[0].definition, f"bad lookup: {hits}"
            return hits[0].definition

        def pyglossary_plugins() -> str:
            from .parsers.pyglossary_base import available_formats
            formats = available_formats()
            for needed in ("ABBYYLingvoDSL", "BabylonBgl"):
                assert needed in formats, f"{needed} plugin missing; have {len(formats)}"
            return f"{len(formats)} read formats"

        def dsl() -> str:
            from .core.query_manager import QueryManager
            qm = QueryManager()
            qm.add_dictionary(_write_dsl(tmpdir / "t.dsl"))
            hits = qm.search("book")
            assert hits and "kitap" in hits[0].definition, f"bad lookup: {hits}"
            return "book -> kitap"

        def sqlite_fts5() -> str:
            from .storage.indexer import IndexStore
            store = IndexStore(tmpdir / "i.db")
            store.open()
            store.close()
            return "FTS5 table created"

        check("StarDict parser", stardict)
        check("PyGlossary plugins (DSL + Babylon)", pyglossary_plugins)
        check("DSL parser", dsl)
        check("SQLite FTS5 index", sqlite_fts5)

    lines.append("RESULT: " + ("OK" if failures == 0 else f"{failures} FAILED"))
    report = "\n".join(lines)
    print(report)
    out = os.environ.get("DICTZ_SELFTEST_REPORT")
    if out:
        Path(out).write_text(report + "\n", encoding="utf-8")
    return 0 if failures == 0 else 1
