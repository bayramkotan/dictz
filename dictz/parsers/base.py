"""Abstract base class for all dictionary format parsers."""
from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterator


@dataclass
class DictionaryEntry:
    headword: str
    definition: str
    dictionary_id: str = ""
    score: float = 1.0

    def __repr__(self) -> str:
        preview = self.definition[:50].replace("\n", " ")
        return f"DictionaryEntry({self.headword!r}, {preview!r}...)"


@dataclass
class DictionaryMetadata:
    name: str
    source_language: str = ""
    target_language: str = ""
    description: str = ""
    author: str = ""
    word_count: int = 0
    file_path: Path = field(default_factory=Path)
    format: str = ""

    @property
    def display_name(self) -> str:
        if self.source_language and self.target_language:
            return f"{self.name} ({self.source_language} → {self.target_language})"
        return self.name


def sort_synonyms(pairs) -> list[tuple[str, str]]:
    """Synonym pairs without duplicates, ordered by (lower-cased synonym,
    synonym, headword) — the order kept in the article store."""
    return sorted(set(pairs), key=lambda p: (p[0].lower(), p[0], p[1]))


def synonym_targets(pairs: list[tuple[str, str]], word: str) -> list[str]:
    """The headwords ``word`` is a synonym of: the pairs whose synonym is
    exactly ``word``, otherwise those of the first synonym equal to it
    ignoring case. Each headword once, in pair order."""
    hits = [t for s, t in pairs if s == word]
    if not hits:
        low = word.lower()
        first = next((s for s, _ in pairs if s.lower() == low), None)
        if first is not None:
            hits = [t for s, t in pairs if s == first]
    return list(dict.fromkeys(hits))


def merge_search_words(headwords: list[str], pairs: list[tuple[str, str]]) -> list[str]:
    """Headwords plus the synonyms that are not headwords, sorted case-insensitively
    (headwords first among equals)."""
    if not pairs:
        return list(headwords)
    have = set(headwords)
    extra = list(dict.fromkeys(s for s, _ in pairs if s not in have))
    return sorted(list(headwords) + extra, key=str.lower)


class Dictionary(ABC):
    def __init__(self, path: Path | str):
        self.path = Path(path)
        self.metadata = DictionaryMetadata(name=self.path.stem, file_path=self.path)
        self._loaded = False

    @property
    def loaded(self) -> bool:
        return self._loaded

    @abstractmethod
    def load(self) -> None: ...

    @abstractmethod
    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]: ...

    @abstractmethod
    def iter_entries(self) -> Iterator[DictionaryEntry]: ...

    def headwords(self) -> list[str]:
        """Headwords sorted case-insensitively (the articles' own words)."""
        if not self._loaded:
            self.load()
        return list(getattr(self, "_sorted_words", []))

    def synonyms(self) -> list[tuple[str, str]]:
        """``(synonym, headword)`` pairs in :func:`sort_synonyms` order
        (StarDict's ``.syn``: "went" → "go"); empty for other formats."""
        return []

    def search_words(self) -> list[str]:
        """Headwords and synonyms: what the index, the word list and "did you
        mean" offer. Same rule as ``Dictionary::search_words`` in Rust."""
        return merge_search_words(self.headwords(), self.synonyms())

    def close(self) -> None:
        self._loaded = False

    def __enter__(self):
        if not self._loaded:
            self.load()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def __repr__(self) -> str:
        return f"<{self.__class__.__name__} {self.metadata.name!r} ({self.metadata.word_count} words)>"
