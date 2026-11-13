"""PyGlossary-backed base parser.

PyGlossary is a mature multi-format converter library that can read 60+
dictionary formats. We wrap it behind our own Dictionary interface so the
rest of dictz doesn't need to know about PyGlossary's quirks.

For each supported format (DSL, Babylon, etc.) we subclass PyGlossaryDictionary
and set `_format` to the PyGlossary plugin name. Everything else is shared:
headers, iteration, exact and prefix lookup.

The PyGlossary Glossary.init() call is global and expensive (it loads every
plugin module). We call it once per process via _ensure_init().
"""
from __future__ import annotations

from pathlib import Path
from typing import Iterator

from .base import Dictionary, DictionaryEntry


_pyglossary_initialized = False


def _ensure_init() -> None:
    """Call Glossary.init() exactly once per process."""
    global _pyglossary_initialized
    if _pyglossary_initialized:
        return
    try:
        from pyglossary.glossary_v2 import Glossary
    except ImportError as exc:
        raise ImportError(
            "pyglossary is required for DSL/Babylon support. "
            "Install with: pip install pyglossary"
        ) from exc
    Glossary.init()
    _pyglossary_initialized = True


class PyGlossaryError(Exception):
    """Raised when PyGlossary fails to read a dictionary file."""


class PyGlossaryDictionary(Dictionary):
    """Generic PyGlossary-backed dictionary.

    Subclasses set:
        _format: the PyGlossary plugin name (e.g. "ABBYYLingvoDSL", "Babylon")
        format_label: the human-readable format label stored in metadata
    """

    _format: str = ""  # overridden by subclass
    format_label: str = ""  # overridden by subclass

    def __init__(self, path: Path | str):
        super().__init__(path)
        # word -> definition (loaded eagerly on load() — PyGlossary doesn't
        # offer cheap random access for most formats, so we materialize the
        # whole dictionary in memory on first load, then serve lookups from
        # the dict. For very large glossaries this trades memory for speed.
        self._entries: dict[str, str] = {}
        self._sorted_words: list[str] = []
        self.metadata.format = self.format_label

    # ------------------------------------------------------------------ load

    def load(self) -> None:
        if self._loaded:
            return
        if not self._format:
            raise PyGlossaryError(
                f"{self.__class__.__name__} did not set _format"
            )
        if not self.path.exists():
            raise PyGlossaryError(f"File not found: {self.path}")

        _ensure_init()
        from pyglossary.glossary_v2 import Glossary

        glos = Glossary()
        try:
            opened = glos.directRead(str(self.path), formatName=self._format)
        except Exception as exc:
            raise PyGlossaryError(
                f"PyGlossary could not read {self.path}: {exc}"
            ) from exc

        if not opened:
            raise PyGlossaryError(
                f"PyGlossary refused to open {self.path} as {self._format}"
            )

        # Pull header fields
        self.metadata.name = glos.getInfo("name") or glos.getInfo("title") or self.path.stem
        self.metadata.author = glos.getInfo("author") or ""
        self.metadata.description = glos.getInfo("description") or ""
        self.metadata.source_language = glos.getInfo("sourceLang") or ""
        self.metadata.target_language = glos.getInfo("targetLang") or ""

        # Materialize all entries. PyGlossary iterates once in direct mode.
        for entry in glos:
            if entry is None:
                continue
            # Skip resource entries (images, sounds, binary blobs)
            if getattr(entry, "isData", lambda: False)():
                continue
            # PyGlossary 5.0+ renamed s_word -> s_term; keep a fallback.
            word = getattr(entry, "s_term", None) or getattr(entry, "s_word", "") or ""
            defi = entry.defi or ""
            if not word:
                continue
            # A headword can have multiple variants; PyGlossary gives them as
            # a single `\n`-separated string in s_word. Register them all.
            variants = [w.strip() for w in word.split("|") if w.strip()]
            if not variants:
                variants = [word]
            for variant in variants:
                if variant not in self._entries:
                    self._entries[variant] = defi

        self.metadata.word_count = len(self._entries)
        self._sorted_words = sorted(self._entries.keys(), key=str.lower)
        self._loaded = True

    # ---------------------------------------------------------------- lookup

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
