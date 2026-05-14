"""Query manager: orchestrates lookups across all loaded dictionaries.

The query manager keeps a list of loaded `Dictionary` instances and dispatches
a single user query to all of them, collecting and ranking the results.

For v0.0.1 we run lookups sequentially (StarDict lookups are dict[] hits and
essentially instant once loaded). Parallelization with QThreadPool comes in
v0.3 when fuzzy matching makes things slower.
"""
from __future__ import annotations

from pathlib import Path

from ..parsers.base import Dictionary, DictionaryEntry
from ..parsers.stardict import StarDictDictionary
from ..storage.indexer import IndexStore


# Map of file extension/format -> parser class. Future parsers register here.
_PARSER_REGISTRY: dict[str, type[Dictionary]] = {
    ".ifo": StarDictDictionary,
    "stardict": StarDictDictionary,
}


def parser_for(path: Path | str) -> type[Dictionary]:
    """Pick a parser class based on file extension."""
    p = Path(path)
    suffix = p.suffix.lower()
    if suffix in _PARSER_REGISTRY:
        return _PARSER_REGISTRY[suffix]
    raise ValueError(f"No parser registered for file: {p}")


class QueryManager:
    """Orchestrates dictionary loading and multi-dictionary lookups."""

    def __init__(self, index_store: IndexStore | None = None):
        self.dictionaries: dict[str, Dictionary] = {}  # name -> dict
        self.index_store = index_store

    # ------------------------------------------------------- dictionary mgmt

    def add_dictionary(self, path: Path | str, reindex: bool = False) -> Dictionary:
        """Load a dictionary file and add it to the active set.

        If an index_store is configured and the dictionary is not yet
        indexed (or reindex=True), index its headwords.
        """
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

    # ------------------------------------------------------------- search

    def search(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        """Search every loaded dictionary for `word`.

        Returns a flat list of entries, grouped by dictionary in load order.
        """
        if not word.strip():
            return []

        results: list[DictionaryEntry] = []
        for name, dictionary in self.dictionaries.items():
            try:
                entries = dictionary.lookup(word, exact=exact)
            except Exception as e:
                # Don't let one broken dictionary kill the whole query
                print(f"[dictz] lookup failed in {name}: {e}")
                continue
            results.extend(entries)
        return results

    def suggest(self, prefix: str, limit: int = 30) -> list[str]:
        """Suggest headwords for autocomplete, using the FTS5 index.

        Falls back to per-dictionary prefix scan if no index_store is set.
        """
        if not prefix.strip():
            return []

        if self.index_store is not None:
            rows = self.index_store.search_headword(prefix, limit=limit)
            # Deduplicate while preserving order
            seen: set[str] = set()
            out: list[str] = []
            for headword, _ in rows:
                if headword not in seen:
                    seen.add(headword)
                    out.append(headword)
            return out

        # Fallback: query each dictionary directly (slow for many dicts)
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
