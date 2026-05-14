"""Abstract base class for all dictionary format parsers.

Every concrete parser (StarDict, DSL, Babylon) implements this interface,
so the rest of the application can treat dictionaries uniformly regardless
of their underlying file format.
"""
from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterator


@dataclass
class DictionaryEntry:
    """A single headword + definition pair from a dictionary."""

    headword: str
    definition: str  # HTML or plain text
    dictionary_id: str = ""  # filled in by the query manager
    score: float = 1.0  # 1.0 = exact match, <1.0 = fuzzy match similarity

    def __repr__(self) -> str:
        preview = self.definition[:50].replace("\n", " ")
        return f"DictionaryEntry({self.headword!r}, {preview!r}...)"


@dataclass
class DictionaryMetadata:
    """Information about a loaded dictionary."""

    name: str
    source_language: str = ""
    target_language: str = ""
    description: str = ""
    author: str = ""
    word_count: int = 0
    file_path: Path = field(default_factory=Path)
    format: str = ""  # "stardict", "dsl", "babylon"

    @property
    def display_name(self) -> str:
        if self.source_language and self.target_language:
            return f"{self.name} ({self.source_language} → {self.target_language})"
        return self.name


class Dictionary(ABC):
    """Abstract base for all dictionary format parsers.

    Lifecycle:
        1. __init__(path)        — store the path, do nothing expensive
        2. load()                — read headers, build internal index
        3. lookup(word) / iter_entries() — used by query manager
        4. close()               — release file handles
    """

    def __init__(self, path: Path | str):
        self.path = Path(path)
        self.metadata = DictionaryMetadata(name=self.path.stem, file_path=self.path)
        self._loaded = False

    @property
    def loaded(self) -> bool:
        return self._loaded

    @abstractmethod
    def load(self) -> None:
        """Load dictionary headers and build an in-memory index.

        Should populate self.metadata. Heavy I/O is expected here.
        """
        ...

    @abstractmethod
    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        """Look up a word. Returns 0 or more matching entries.

        Args:
            word: The headword to search for (case-insensitive by default).
            exact: If True, only exact matches. If False, prefix matches too.
        """
        ...

    @abstractmethod
    def iter_entries(self) -> Iterator[DictionaryEntry]:
        """Yield every entry in the dictionary (used for full indexing)."""
        ...

    def close(self) -> None:
        """Release any open file handles. Default: no-op."""
        self._loaded = False

    def __enter__(self):
        if not self._loaded:
            self.load()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def __repr__(self) -> str:
        return f"<{self.__class__.__name__} {self.metadata.name!r} ({self.metadata.word_count} words)>"
