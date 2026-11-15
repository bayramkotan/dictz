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


def _write_bgl(path: Path) -> Path:
    """Minimal UTF-8 Babylon file: title, English → Turkish, two entries."""
    import gzip

    def block(kind: int, data: bytes) -> bytes:
        n = len(data)
        if n < 12:
            return bytes([((n + 4) << 4) | kind]) + data
        if n < 256:
            return bytes([kind, n]) + data
        return bytes([(1 << 4) | kind]) + struct.pack(">H", n) + data

    def info(code: int, value: bytes) -> bytes:
        return block(3, struct.pack(">H", code) + value)

    def entry(word: bytes, defi: bytes, alts: tuple[bytes, ...] = ()) -> bytes:
        d = bytes([len(word)]) + word + struct.pack(">H", len(defi)) + defi
        for a in alts:
            d += bytes([len(a)]) + a
        return block(1, d)

    body = (info(0x11, struct.pack(">I", 0x8000)) + info(0x01, b"selftest-bgl")
            + info(0x07, struct.pack(">I", 0)) + info(0x08, struct.pack(">I", 13))
            + entry(b"apple", b"elma", (b"apples",)) + entry(b"book", b"kitap") + block(4, b""))
    path.write_bytes(b"\x12\x34\x00\x01\x00\x06" + gzip.compress(body, mtime=0))
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

        def babylon() -> str:
            from .core.query_manager import QueryManager
            qm = QueryManager()
            qm.add_dictionary(_write_bgl(tmpdir / "t.bgl"))
            hits = qm.search("apples")
            assert hits and hits[0].definition == "elma", f"bad lookup: {hits}"
            return "apples -> elma"

        def sqlite_fts5() -> str:
            from .core.query_manager import QueryManager
            from .storage.indexer import IndexStore
            sd = tmpdir / "fts"
            body, idx = b"", b""
            for word, definition in (("café", "x"), ("cafeteria", "y")):
                data = definition.encode()
                idx += word.encode() + b"\x00" + struct.pack(">II", len(body), len(data))
                body += data
            sd.with_suffix(".dict").write_bytes(body)
            sd.with_suffix(".idx").write_bytes(idx)
            sd.with_suffix(".ifo").write_text(
                "StarDict's dict ifo file\nversion=2.4.2\nbookname=selftest-fts\nwordcount=2\n", encoding="utf-8")
            store = IndexStore(tmpdir / "i.db")
            store.open()
            qm = QueryManager(store)
            qm.add_dictionary(sd.with_suffix(".ifo"))
            got = [s for s in qm.suggest("cafe", 10) if not s.fuzzy]
            store.close()
            assert len(got) == 2, f"unexpected suggestions: {got}"
            return "FTS5 prefix search"

        def fuzzy_matching() -> str:
            from .core.fuzzy import ratio
            r = ratio("aple", "apple")
            assert r == 88.88888888888889, f"unexpected ratio {r}"
            return f"ratio(aple, apple) = {r:.2f}"

        def media() -> str:
            import zipfile

            from .core.media import MediaResolver
            with zipfile.ZipFile(tmpdir / "m.dsl.files.zip", "w") as z:
                z.writestr("pic.png", b"\x89PNG")
            data = MediaResolver(tmpdir / "m.dsl").read("PIC.png")
            assert data == b"\x89PNG", f"media not found: {data!r}"
            return "zip lookup"

        check("StarDict parser", stardict)
        check("PyGlossary plugins (DSL + Babylon)", pyglossary_plugins)
        check("DSL parser", dsl)
        check("Babylon parser", babylon)
        check("SQLite FTS5 index", sqlite_fts5)
        check("Fuzzy matching", fuzzy_matching)
        check("Media resolver", media)

    lines.append("RESULT: " + ("OK" if failures == 0 else f"{failures} FAILED"))
    report = "\n".join(lines)
    print(report)
    out = os.environ.get("DICTZ_SELFTEST_REPORT")
    if out:
        Path(out).write_text(report + "\n", encoding="utf-8")
    return 0 if failures == 0 else 1
