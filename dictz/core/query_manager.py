"""Query manager: orchestrates lookups across all loaded dictionaries.

Mirrors ``dictz-rs/src/query.rs`` (Rust edition).
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from ..parsers.babylon import BabylonDictionary
from ..parsers.base import Dictionary, DictionaryEntry
from ..parsers.dsl import DSLDictionary
from ..parsers.stardict import StarDictDictionary
from ..storage.indexer import IndexStore
from . import fuzzy
from .media import MediaResolver

_PARSER_REGISTRY: dict[str, type[Dictionary]] = {
    ".ifo": StarDictDictionary,
    ".dsl": DSLDictionary,
    ".bgl": BabylonDictionary,
}

# Suggestions are filled up with fuzzy matches when fewer prefix hits exist.
FUZZY_FILL_BELOW = 10


def parser_for(path: Path | str) -> type[Dictionary]:
    p = Path(path)
    suffix = p.suffix.lower()
    if suffix == ".dz":
        inner_suffix = p.with_suffix("").suffix.lower()
        if inner_suffix in _PARSER_REGISTRY:
            return _PARSER_REGISTRY[inner_suffix]
    if suffix in _PARSER_REGISTRY:
        return _PARSER_REGISTRY[suffix]
    raise ValueError(f"No parser registered for file: {p}")


def supported_extensions() -> list[str]:
    return sorted(_PARSER_REGISTRY.keys())


@dataclass(frozen=True)
class Suggestion:
    word: str
    fuzzy: bool = False


class QueryManager:
    """Coordinates lookups across dictionaries, in the user's order."""

    def __init__(self, index_store: IndexStore | None = None):
        self.dictionaries: dict[str, Dictionary] = {}
        self.enabled_names: set[str] = set()
        self.index_store = index_store
        self._order: list[str] = []
        self._media: dict[str, MediaResolver] = {}
        self._headword_cache: list[str] | None = None

    # ------------------------------------------------------------ loading

    def add_dictionary(self, path: Path | str, reindex: bool = False) -> Dictionary:
        parser_cls = parser_for(path)
        dictionary = parser_cls(path)
        dictionary.load()
        name = dictionary.metadata.name
        if name in self.dictionaries:
            self._order.remove(name)
        self.dictionaries[name] = dictionary
        self._order.append(name)
        self._media[name] = MediaResolver(path)
        self.enabled_names.add(name)
        if self.index_store is not None:
            already_indexed = self.index_store.is_indexed(name)
            if reindex or not already_indexed:
                self.index_store.index_dictionary(dictionary)
        self._persist_order()
        self._headword_cache = None
        return dictionary

    def restore_from_index(self) -> tuple[list[str], list[str]]:
        """Reload every dictionary recorded in the index (start-up).

        Returns (loaded names, error messages).
        """
        if self.index_store is None:
            return [], []
        loaded: list[str] = []
        errors: list[str] = []
        for row in self.index_store.list_dictionaries(enabled_only=False):
            name = row["name"]
            if name in self.dictionaries:
                continue
            path = Path(row["path"])
            try:
                dictionary = parser_for(path)(path)
                dictionary.load()
            except Exception as exc:  # noqa: BLE001 — report and keep going
                errors.append(f"{path}: {exc}")
                continue
            real = dictionary.metadata.name
            self.dictionaries[real] = dictionary
            self._order.append(real)
            self._media[real] = MediaResolver(path)
            if row["enabled"]:
                self.enabled_names.add(real)
            loaded.append(real)
        self._headword_cache = None
        return loaded, errors

    def remove_dictionary(self, name: str) -> None:
        if name in self.dictionaries:
            self.dictionaries[name].close()
            del self.dictionaries[name]
        if name in self._order:
            self._order.remove(name)
        self._media.pop(name, None)
        self.enabled_names.discard(name)
        if self.index_store is not None:
            self.index_store.remove_dictionary(name)
        self._persist_order()
        self._headword_cache = None

    def set_enabled(self, name: str, enabled: bool) -> None:
        if enabled:
            self.enabled_names.add(name)
        else:
            self.enabled_names.discard(name)
        if self.index_store is not None:
            self.index_store.set_enabled(name, enabled)
        self._headword_cache = None

    def is_enabled(self, name: str) -> bool:
        return name in self.enabled_names

    def move_dictionary(self, name: str, delta: int) -> None:
        """Move a dictionary up (``delta < 0``) or down in the list."""
        if name not in self._order:
            return
        i = self._order.index(name)
        j = max(0, min(len(self._order) - 1, i + delta))
        if i != j:
            self._order.insert(j, self._order.pop(i))
            self._persist_order()
            self._headword_cache = None

    def _persist_order(self) -> None:
        if self.index_store is None:
            return
        for i, name in enumerate(self._order):
            self.index_store.set_sort_order(name, i)
            self.index_store.set_enabled(name, name in self.enabled_names)

    def loaded_names(self) -> list[str]:
        """Names in list order."""
        return list(self._order)

    def media(self, name: str) -> MediaResolver | None:
        return self._media.get(name)

    # ------------------------------------------------------------ lookups

    def search(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        if not word.strip():
            return []
        results: list[DictionaryEntry] = []
        for name in self._order:
            if name not in self.enabled_names:
                continue
            try:
                entries = self.dictionaries[name].lookup(word, exact=exact)
            except Exception as e:  # noqa: BLE001
                print(f"[dictz] lookup failed in {name}: {e}")
                continue
            results.extend(entries)
        return results

    def all_headwords(self) -> list[str]:
        """Headwords of all enabled dictionaries (list order, each dictionary
        sorted, duplicates removed) — the fuzzy candidate list."""
        if self._headword_cache is None:
            seen: set[str] = set()
            out: list[str] = []
            for name in self._order:
                if name not in self.enabled_names:
                    continue
                for w in self.dictionaries[name].headwords():
                    if w not in seen:
                        seen.add(w)
                        out.append(w)
            self._headword_cache = out
        return self._headword_cache

    def fuzzy(self, word: str, limit: int = fuzzy.DEFAULT_LIMIT,
              cutoff: float = fuzzy.DEFAULT_CUTOFF) -> list[tuple[str, float]]:
        """"Did you mean" candidates for ``word``, best first."""
        word = word.strip()
        if not word:
            return []
        return fuzzy.extract(word, self.all_headwords(), limit=limit, cutoff=cutoff)

    def suggest(self, prefix: str, limit: int = 30) -> list[Suggestion]:
        """Prefix hits ranked closest-first, filled up with fuzzy matches when
        there are fewer than ``FUZZY_FILL_BELOW``."""
        prefix = prefix.strip()
        if not prefix:
            return []
        enabled = [n for n in self._order if n in self.enabled_names]
        words: list[str] = []
        seen: set[str] = set()
        if self.index_store is not None:
            for headword, _ in self.index_store.search_headword(prefix, limit=limit, dictionaries=enabled):
                if headword not in seen:
                    seen.add(headword)
                    words.append(headword)
        else:
            for name in enabled:
                for entry in self.dictionaries[name].lookup(prefix, exact=False):
                    if entry.headword not in seen:
                        seen.add(entry.headword)
                        words.append(entry.headword)
                if len(words) >= limit:
                    words = words[:limit]
                    break
        out = [Suggestion(w) for w in fuzzy.rank(prefix, words)]
        if len(out) < FUZZY_FILL_BELOW:
            for w, _score in self.fuzzy(prefix):
                if len(out) >= limit:
                    break
                if w not in seen:
                    seen.add(w)
                    out.append(Suggestion(w, fuzzy=True))
        return out

    def close_all(self) -> None:
        for dictionary in self.dictionaries.values():
            dictionary.close()
        self.dictionaries.clear()
        self.enabled_names.clear()
        self._order.clear()
        self._media.clear()
        self._headword_cache = None
