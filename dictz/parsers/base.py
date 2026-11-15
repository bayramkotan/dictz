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
