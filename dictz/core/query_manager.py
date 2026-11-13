"""Query manager: orchestrates lookups across all loaded dictionaries.

The query manager keeps a list of loaded `Dictionary` instances and dispatches
a single user query to all of them, collecting and ranking the results.

For v0.0.2 we run lookups sequentially. Parallelization with QThreadPool comes
in a later version when big glossary loading becomes a bottleneck.
"""
from __future__ import annotations

from pathlib import Path

from ..parsers.babylon import BabylonDictionary
from ..parsers.base import Dictionary, DictionaryEntry
from ..parsers.dsl import DSLDictionary
from ..parsers.stardict import StarDictDictionary
from ..storage.indexer import IndexStore


# File-extension -> parser class. `.dsl.dz` is handled by peeling the .dz
# compression suffix before the lookup.
_PARSER_REGISTRY: dict[str, type[Dictionary]] = {
    ".ifo": StarDictDictionary,
    ".dsl": DSLDictionary,
    ".bgl": BabylonDictionary,
}


def parser_for(path: Path | str) -> type[Dictionary]:
    """Pick a parser class based on file extension.

    Handles compressed forms (.dsl.dz, .gls.dz) by peeling the .dz suffix.
    """
    p = Path(path)
    suffix = p.suffix.lower()

    # Strip dictzip compression layer
    if suffix == ".dz":
        inner_suffix = p.with_suffix("").suffix.lower()
        if inner_suffix in _PARSER_REGISTRY:
            return _PARSER_REGISTRY[inner_suffix]

    if suffix in _PARSER_REGISTRY:
        return _PARSER_REGISTRY[suffix]

    raise ValueError(f"No parser registered for file: {p}")


def supported_extensions() -> list[str]:
    """List of extensions the current build can open."""
    return sorted(_PARSER_REGISTRY.keys())


class QueryManager:
    """Orchestrates dictionary loading and multi-dictionary lookups."""

    def __init__(self, index_store: IndexStore | None = None):
        self.dictionaries: dict[str, Dictionary] = {}
        self.index_store = index_store

    def add_dictionary(self, path: Path | str, reindex: bool = False) -> Dictionary:
        parser_cls = parser_for(path)
        dictionary = parser_cls(path)
        dictionary.load()
        self.dictionaries[dictionary.metadata.name] = dictionary

        if self.index_store is not None:
            already_indexed = self.index_store.is_indexed(dictionary.metadata.name)
            if reindex or not already_indexed:
                self.index_store.index_dictionary(dictionary)

        return dictionary

    def remove_dictionary(self, name: str) -> None:
        if name in self.dictionaries:
            self.dictionaries[name].close()
            del self.dictionaries[name]
        if self.index_store is not None:
            self.index_store.remove_dictionary(name)

    def loaded_names(self) -> list[str]:
        return sorted(self.dictionaries.keys())

    def search(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        if not word.strip():
            return []

        results: list[DictionaryEntry] = []
        for name, dictionary in self.dictionaries.items():
            try:
                entries = dictionary.lookup(word, exact=exact)
            except Exception as e:
                print(f"[dictz] lookup failed in {name}: {e}")
                continue
            results.extend(entries)
        return results

    def suggest(self, prefix: str, limit: int = 30) -> list[str]:
        if not prefix.strip():
            return []

        if self.index_store is not None:
            rows = self.index_store.search_headword(prefix, limit=limit)
            seen: set[str] = set()
            out: list[str] = []
            for headword, _ in rows:
                if headword not in seen:
                    seen.add(headword)
                    out.append(headword)
            return out

        seen = set()
        out = []
        for dictionary in self.dictionaries.values():
            for entry in dictionary.lookup(prefix, exact=False):
                if entry.headword not in seen:
                    seen.add(entry.headword)
                    out.append(entry.headword)
                    if len(out) >= limit:
                        return out
        return out

    def close_all(self) -> None:
        for dictionary in self.dictionaries.values():
            dictionary.close()
        self.dictionaries.clear()
