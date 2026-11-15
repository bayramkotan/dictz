"""PyGlossary-backed base parser for DSL / Babylon / other PyGlossary formats."""
from __future__ import annotations

from pathlib import Path
from typing import Iterator

from .base import Dictionary, DictionaryEntry

_pyglossary_initialized = False


def _ensure_init() -> None:
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
    pass


class PyGlossaryDictionary(Dictionary):
    _format: str = ""
    format_label: str = ""

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
            opened = glos.directRead(str(self.path), formatName=self._format)
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
