"""PyGlossary-backed base parser for DSL / Babylon / other PyGlossary formats."""
from __future__ import annotations

import sys
from pathlib import Path
from typing import Iterator

from .base import Dictionary, DictionaryEntry

_pyglossary_initialized = False


def _is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False))


def _python314_compat() -> None:
    """Python 3.14 moved the private ``_compression`` module to
    ``compression._common._streams``. PyGlossary 4.x's Babylon reader still
    imports ``_compression`` and is silently skipped without it ("Module
    '_compression' not found ... skipping plugin 'babylon_bgl'"), so .bgl
    files could not be opened. Register the new module under the old name.
    """
    if "_compression" in sys.modules:
        return
    try:
        import _compression  # noqa: F401
    except ImportError:
        try:
            from compression._common import _streams
        except ImportError:
            return
        sys.modules["_compression"] = _streams


def _import_glossary_class():
    """Import pyglossary's Glossary class, bundle-aware.

    pyglossary.core computes its root directory once, at import time. When it
    sees ``sys.frozen`` it assumes a PyGlossary-specific layout and uses
    ``dirname(sys.executable)`` — but PyInstaller 6 unpacks data into
    ``sys._MEIPASS`` (``_internal/`` on Windows/Linux, ``Contents/Frameworks``
    on macOS), so plugins, ``langs/langs.json`` etc. would not be found.

    Hiding ``sys.frozen`` for the duration of the first import makes PyGlossary
    derive the root from its own ``__file__`` instead, which PyInstaller sets
    under ``sys._MEIPASS``. Every other PyGlossary module reads paths from
    pyglossary.core, so this one correction covers all of them.
    """
    _python314_compat()
    if _is_frozen() and "pyglossary.core" not in sys.modules:
        frozen_flag = sys.frozen  # type: ignore[attr-defined]
        try:
            del sys.frozen  # type: ignore[attr-defined]
            from pyglossary.glossary_v2 import Glossary
        finally:
            sys.frozen = frozen_flag  # type: ignore[attr-defined]
        return Glossary
    from pyglossary.glossary_v2 import Glossary

    return Glossary


def _quiet_pyglossary() -> None:
    """PyGlossary logs every optional plugin it skips (lxml missing, unknown
    charset …) to the console. Those are not dictz errors; keep them out of the
    terminal unless DICTZ_DEBUG is set."""
    import os

    if os.environ.get("DICTZ_DEBUG"):
        return
    try:
        from pyglossary.core import log

        log.setVerbosity(1)  # errors only
    except Exception:  # noqa: BLE001 — cosmetic only
        pass


def _ensure_init() -> None:
    global _pyglossary_initialized
    if _pyglossary_initialized:
        return
    try:
        Glossary = _import_glossary_class()  # noqa: N806
    except ImportError as exc:
        raise ImportError(
            "pyglossary is required for DSL/Babylon support. "
            "Install with: pip install pyglossary"
        ) from exc
    _quiet_pyglossary()
    if _is_frozen():
        # plugins-meta/index.json is not shipped in the bundle; PyGlossary
        # opens it without an existence check, so skip it and let the
        # directory scan register every plugin instead.
        Glossary.init(usePluginsJson=False)
    else:
        Glossary.init()
    _patch_dsl_media()
    _pyglossary_initialized = True


def _lex_s(tr):  # noqa: ANN001, ANN202 — PyGlossary's Transformer
    """PyGlossary's ``[s]…[/s]`` handler with dictz's rule: the extension in
    any letter case (Lingvo packs have "Apple.WAV") and every sound / picture
    kind dictz plays or shows (PyGlossary knew only wav / mp3). Same as
    ``lex_s`` in ``dictz-rs/src/dsl.rs``."""
    from os.path import splitext

    from pyglossary.plugins.dsl import lex

    from ..core.media import AUDIO_EXTENSIONS, IMAGE_EXTENSIONS

    if tr.end():
        return None, None
    fname = ""
    while not tr.end():
        c = tr.next()
        if c == "[":
            tr.move(-1)
            break
        fname += c
    ext = splitext(fname)[1].lstrip(".").lower()
    if ext in AUDIO_EXTENSIONS:
        if tr.audio:
            tr.output += (
                rf'<object type="audio/x-wav" data="{fname}" '
                'width="40" height="40">'
                '<param name="autoplay" value="false" />'
                "</object>"
            )
    elif ext in IMAGE_EXTENSIONS:
        tr.output += rf'<img align="top" src="{fname}" alt="{fname}" />'
    tr.resFileSet.add(fname)
    tr.resetBuf()
    return lex.lexRoot, None


def _patch_dsl_media() -> None:
    """Use :func:`_lex_s` for ``[s]`` tags (PyGlossary calls ``lexS`` through
    its module, so replacing the module's function is enough)."""
    try:
        from pyglossary.plugins.dsl import lex
    except ImportError:
        return
    if hasattr(lex, "lexS"):
        lex.lexS = _lex_s


def _format_kwarg(glos, fmt: str) -> dict[str, str]:
    """Keyword that names the input format for Glossary.directRead.

    PyGlossary 4.x calls it ``format``; 5.x renamed it to ``formatName``.
    Passing the wrong one is silently treated as a reader option and the
    format falls back to extension sniffing.
    """
    import inspect

    params = inspect.signature(glos.directRead).parameters
    return {"formatName": fmt} if "formatName" in params else {"format": fmt}


def _xml_unescape(text: str) -> str:
    """Inverse of ``xml.sax.saxutils.escape`` (``&``, ``<``, ``>``)."""
    return text.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")


def available_formats() -> list[str]:
    """PyGlossary plugin names that can read files (for diagnostics)."""
    _ensure_init()
    from pyglossary.glossary_v2 import Glossary

    return sorted(Glossary.readFormats)


class PyGlossaryError(Exception):
    pass


class PyGlossaryDictionary(Dictionary):
    _format: str = ""
    format_label: str = ""
    # PyGlossary XML-escapes some formats' headwords ("rock &amp; roll");
    # those formats set this so the index holds the real text.
    _unescape_terms: bool = False

    def __init__(self, path: Path | str):
        super().__init__(path)
        self._entries: dict[str, str] = {}
        self._sorted_words: list[str] = []
        self.metadata.format = self.format_label

    def load(self) -> None:
        if self._loaded:
            return
        if not self._format:
            raise PyGlossaryError(f"{self.__class__.__name__} did not set _format")
        if not self.path.exists():
            raise PyGlossaryError(f"File not found: {self.path}")

        _ensure_init()
        from pyglossary.glossary_v2 import Glossary

        glos = Glossary()
        try:
            opened = glos.directRead(str(self.path), **_format_kwarg(glos, self._format))
        except Exception as exc:
            raise PyGlossaryError(f"PyGlossary could not read {self.path}: {exc}") from exc

        if not opened:
            raise PyGlossaryError(f"PyGlossary refused to open {self.path} as {self._format}")

        self.metadata.name = glos.getInfo("name") or glos.getInfo("title") or self.path.stem
        self.metadata.author = glos.getInfo("author") or ""
        self.metadata.description = glos.getInfo("description") or ""
        self.metadata.source_language = glos.getInfo("sourceLang") or ""
        self.metadata.target_language = glos.getInfo("targetLang") or ""

        for entry in glos:
            if entry is None:
                continue
            if getattr(entry, "isData", lambda: False)():
                continue
            word = getattr(entry, "s_term", None) or getattr(entry, "s_word", "") or ""
            defi = entry.defi or ""
            if not word:
                continue
            variants = [w.strip() for w in word.split("|") if w.strip()]
            if not variants:
                variants = [word]
            if self._unescape_terms:
                variants = [_xml_unescape(v) for v in variants]
            for variant in variants:
                if variant not in self._entries:
                    self._entries[variant] = defi

        self.metadata.word_count = len(self._entries)
        self._sorted_words = sorted(self._entries.keys(), key=str.lower)
        self._loaded = True

    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        if not self._loaded:
            self.load()
        results: list[DictionaryEntry] = []
        if word in self._entries:
            results.append(self._entry_for(word))
        if not results:
            word_lower = word.lower()
            for indexed_word in self._sorted_words:
                if indexed_word.lower() == word_lower:
                    results.append(self._entry_for(indexed_word))
                    break
        if not exact and not results:
            word_lower = word.lower()
            for indexed_word in self._sorted_words:
                if indexed_word.lower().startswith(word_lower):
                    results.append(self._entry_for(indexed_word))
                    if len(results) >= 50:
                        break
        return results

    def _entry_for(self, word: str) -> DictionaryEntry:
        return DictionaryEntry(
            headword=word,
            definition=self._entries[word],
            dictionary_id=self.metadata.name,
            score=1.0,
        )

    def iter_entries(self) -> Iterator[DictionaryEntry]:
        if not self._loaded:
            self.load()
        for word in self._sorted_words:
            yield self._entry_for(word)

    def close(self) -> None:
        self._entries.clear()
        self._sorted_words.clear()
        super().close()
