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
from . import fuzzy as _fuzzy
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


def is_dictionary_file(path: Path | str) -> bool:
    name = Path(path).name.lower()
    return name.endswith((".ifo", ".dsl", ".dsl.dz", ".bgl"))


def scan_folder(folder: Path | str, recursive: bool = True) -> list[Path]:
    """Dictionary files (.ifo, .dsl, .dsl.dz, .bgl) under ``folder``, sorted.

    Same rules as the Rust edition's ``query::scan_folder``.
    """
    root = Path(folder).expanduser()
    if root.is_file():
        return [root] if is_dictionary_file(root) else []
    if not root.is_dir():
        return []
    out: list[Path] = []
    stack = [root]
    while stack:
        d = stack.pop()
        try:
            entries = sorted(d.iterdir(), key=lambda p: p.name)
        except OSError:
            continue
        for e in entries:
            if e.is_dir():
                if recursive and not e.name.startswith(".") and not e.name.lower().endswith(".files"):
                    stack.append(e)
            elif is_dictionary_file(e):
                out.append(e)
    return sorted(out, key=lambda p: str(p).lower())


def folder_sources(prefs) -> list[tuple[str, bool]]:
    """Dictionary folders from the settings: ``(folder, recursive)`` in order.

    ``dictionaries.folders`` lists them; a folder that is also in
    ``dictionaries.flat_folders`` is scanned without its subfolders. Same rule
    as the Rust edition's ``query::folder_sources``.
    """
    flat = {str(f) for f in prefs["dictionaries.flat_folders"]}
    return [(str(f), str(f) not in flat) for f in prefs["dictionaries.folders"] if str(f).strip()]


def set_folder_sources(prefs, sources: list[tuple[str, bool]]) -> None:
    """Write ``(folder, recursive)`` pairs back to the two settings (duplicates dropped)."""
    seen: set[str] = set()
    folders, flat = [], []
    for f, rec in sources:
        f = str(f).strip()
        if not f or f in seen:
            continue
        seen.add(f)
        folders.append(f)
        if not rec:
            flat.append(f)
    prefs.set("dictionaries.folders", folders)
    prefs.set("dictionaries.flat_folders", flat)


def add_folder_source(prefs, folder: Path | str, recursive: bool | None = None) -> bool:
    """Add a folder to the dictionary folders (kept for every start).

    Returns ``False`` when it is already there. ``recursive=None`` uses the
    ``dictionaries.recursive`` setting.
    """
    folder = str(Path(folder).expanduser().absolute())
    sources = folder_sources(prefs)
    if any(f == folder for f, _ in sources):
        return False
    rec = bool(prefs["dictionaries.recursive"]) if recursive is None else bool(recursive)
    set_folder_sources(prefs, sources + [(folder, rec)])
    return True


def scan_sources(prefs) -> list[Path]:
    """Dictionary files in every dictionary folder (each with its own recursion)."""
    out: list[Path] = []
    for folder, rec in folder_sources(prefs):
        out += scan_folder(folder, rec)
    return out


def _changed(row, path: Path) -> bool:
    from ..storage.indexer import file_mtime

    try:
        stored = float(row["mtime"] or 0)
    except (IndexError, KeyError, TypeError, ValueError):
        stored = 0.0
    return stored > 0 and abs(file_mtime(path) - stored) > 1e-6


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
        # dictionaries in the list that could not be opened: name -> reason.
        # They stay in the list (and in the dictionary manager), never dropped.
        self.failed: dict[str, str] = {}

    # ------------------------------------------------------------ loading

    @property
    def data_dir(self) -> Path | None:
        """The data folder (where the article stores live), when there is an index."""
        return self.index_store.db_path.parent if self.index_store is not None else None

    def add_dictionary(self, path: Path | str, reindex: bool = False) -> Dictionary:
        if self.data_dir is not None:
            dictionary, _built = self.open_cached(path, self.data_dir, rebuild=reindex)
        else:
            dictionary = self.open_file(path)
        if self.index_store is not None:
            # a unique name per file: a second "X" becomes "X (2)"
            self.index_store.register_dictionary(dictionary)
        else:
            from ..storage.registry import unique_name

            taken = {n for n, d in self.dictionaries.items()
                     if str(d.metadata.file_path) != str(dictionary.metadata.file_path)}
            dictionary.metadata.name = unique_name(dictionary.metadata.name, taken)
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

    def restore_from_index(self, names: list[str] | None = None, enabled_only: bool = False,
                           reindex_changed: bool = False, progress=None) -> tuple[list[str], list[str]]:
        """Reload dictionaries recorded in the index (start-up, CLI).

        ``names`` limits which ones, ``enabled_only`` skips disabled ones,
        ``reindex_changed`` rebuilds the index of files modified since they
        were indexed. ``progress(i, total, name)`` is called before each.
        Returns (loaded names, error messages).
        """
        if self.index_store is None:
            return [], []
        rows = [r for r in self.index_store.list_dictionaries(enabled_only=False)
                if (names is None or r["name"] in names) and (not enabled_only or r["enabled"])
                and r["name"] not in self.dictionaries]
        loaded: list[str] = []
        errors: list[str] = []
        for i, row in enumerate(rows):
            if progress is not None:
                progress(i, len(rows), row["name"])
            path = Path(row["path"])
            try:
                dictionary, built = self.open_cached(path, self.data_dir)
            except Exception as exc:  # noqa: BLE001 — report and keep going
                errors.append(f"{path}: {exc}")
                self.failed[row["name"]] = str(exc)
                continue
            dictionary.metadata.name = row["name"]  # the name in the list (may be "X (2)")
            self.attach(dictionary, path, bool(row["enabled"]))
            if _changed(row, path) and (built or reindex_changed):
                self.index_store.index_dictionary(dictionary)
            loaded.append(dictionary.metadata.name)
        return loaded, errors

    @staticmethod
    def open_file(path: Path | str) -> Dictionary:
        """Parse a dictionary file (safe to call from a worker thread)."""
        dictionary = parser_for(path)(path)
        dictionary.load()
        return dictionary

    @staticmethod
    def open_cached(path: Path | str, data_dir: Path | str, rebuild: bool = False) -> tuple[Dictionary, bool]:
        """Open a dictionary from its article store; parse it and build the
        store first when the store is missing, old or ``rebuild`` is set.

        Returns ``(dictionary, built)``. Safe to call from a worker thread.
        When the store cannot be written (read-only data folder) the parsed
        dictionary is returned instead.
        """
        import sqlite3

        from ..storage import articles
        from ..storage.registry import path_id

        store = articles.store_path(data_dir, path_id(path))
        if not rebuild and articles.is_fresh(store, path):
            try:
                d = articles.StoredDictionary(store, path)
                d.load()
                return d, False
            except sqlite3.Error:
                pass
        parsed = QueryManager.open_file(path)
        try:
            articles.build(parsed, store)
        except (OSError, sqlite3.Error):
            return parsed, True
        parsed.close()
        d = articles.StoredDictionary(store, path)
        d.load()
        return d, True

    def rebuild(self, name: str) -> int:
        """Read a dictionary's file again: new article store and headword index."""
        old = self.dictionaries[name]
        path = old.metadata.file_path
        old.close()  # Windows: an open store cannot be replaced
        if self.data_dir is not None:
            d, _ = self.open_cached(path, self.data_dir, rebuild=True)
        else:
            d = self.open_file(path)
        d.metadata.name = name
        self.dictionaries[name] = d
        self._headword_cache = None
        return self.index_store.index_dictionary(d) if self.index_store is not None else d.metadata.word_count

    def attach(self, dictionary: Dictionary, path: Path | str, enabled: bool = True) -> None:
        """Add an already-loaded dictionary at the end of the list (no indexing)."""
        name = dictionary.metadata.name
        self.failed.pop(name, None)
        if name in self.dictionaries:
            self._order.remove(name)
        self.dictionaries[name] = dictionary
        self._order.append(name)
        self._media[name] = MediaResolver(path)
        if enabled:
            self.enabled_names.add(name)
        else:
            self.enabled_names.discard(name)
        self._headword_cache = None

    def remove_dictionary(self, name: str) -> None:
        if name in self.dictionaries:
            self.dictionaries[name].close()
            del self.dictionaries[name]
        if name in self._order:
            self._order.remove(name)
        self._media.pop(name, None)
        self.enabled_names.discard(name)
        self.failed.pop(name, None)
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
        if name in self.failed and name not in self.dictionaries and self.index_store is not None:
            row = self.index_store.get_dictionary(name)
            return bool(row and row["enabled"])
        return name in self.enabled_names

    def note_failed(self, name: str, reason: str) -> None:
        """Remember that a dictionary of the list could not be opened."""
        if name not in self.dictionaries:
            self.failed[name] = reason

    def list_rows(self) -> list[dict]:
        """Every dictionary of the list in list order — also the ones that
        could not be opened (``error`` set) — so none ever disappears.

        Keys: name, path, format, word_count, enabled, loaded, error.
        """
        out = []
        rows = self.index_store.list_dictionaries(enabled_only=False) if self.index_store is not None else []
        listed = set()
        for r in rows:
            name = r["name"]
            listed.add(name)
            d = self.dictionaries.get(name)
            loaded = d is not None
            if not loaded and name not in self.failed:
                continue  # still loading
            out.append({
                "name": name, "path": r["path"], "format": d.metadata.format if loaded else r["format"],
                "word_count": d.metadata.word_count if loaded else int(r["word_count"] or 0),
                "enabled": self.is_enabled(name), "loaded": loaded,
                "error": "" if loaded else self.failed.get(name, ""),
            })
        for name in self._order:
            if name not in listed:
                m = self.dictionaries[name].metadata
                out.append({"name": name, "path": str(m.file_path), "format": m.format,
                            "word_count": m.word_count, "enabled": self.is_enabled(name),
                            "loaded": True, "error": ""})
        return out

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
        """Write the order of the loaded dictionaries to the list.

        Dictionaries that are in the list but not loaded (file missing, failed
        to open) keep their places: they are never dropped or moved.
        """
        if self.index_store is None:
            return
        all_names = [r["name"] for r in self.index_store.list_dictionaries(enabled_only=False)]
        loaded = set(self._order)
        it = iter([n for n in self._order if n in set(all_names)])
        merged = [next(it) if n in loaded else n for n in all_names]
        merged += [n for n in self._order if n not in set(all_names)]
        if merged != all_names:
            self.index_store.set_order(merged)

    def loaded_names(self) -> list[str]:
        """Names in list order."""
        return list(self._order)

    def media(self, name: str) -> MediaResolver | None:
        return self._media.get(name)

    # ------------------------------------------------------------ lookups

    def search(self, word: str, exact: bool = True, dictionaries: list[str] | None = None,
               max_articles: int | None = None) -> list[DictionaryEntry]:
        if not word.strip():
            return []
        results: list[DictionaryEntry] = []
        for name in self._order:
            if name not in self.enabled_names:
                continue
            if dictionaries is not None and name not in dictionaries:
                continue
            if max_articles is not None and len(results) >= max_articles:
                break
            try:
                entries = self.dictionaries[name].lookup(word, exact=exact)
            except Exception as e:  # noqa: BLE001
                print(f"[dictz] lookup failed in {name}: {e}")
                continue
            results.extend(entries)
        if max_articles is not None:
            results = results[:max_articles]
        return results

    def found_in(self, word: str) -> list[str]:
        """Enabled dictionaries that have ``word`` (exact or case-insensitive)."""
        return [name for name in self._order
                if name in self.enabled_names and self.dictionaries[name].lookup(word, exact=True)]

    def random_entry(self, dictionary: str | None = None, rng=None) -> DictionaryEntry | None:
        """A random article from one (or any enabled) dictionary."""
        import random

        rng = rng or random
        names = [dictionary] if dictionary else [n for n in self._order if n in self.enabled_names]
        names = [n for n in names if n in self.dictionaries and self.dictionaries[n].headwords()]
        if not names:
            return None
        d = self.dictionaries[rng.choice(names)]
        word = rng.choice(d.headwords())
        hits = d.lookup(word, exact=True)
        return hits[0] if hits else None

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

    def fuzzy(self, word: str, limit: int = _fuzzy.DEFAULT_LIMIT,
              cutoff: float = _fuzzy.DEFAULT_CUTOFF) -> list[tuple[str, float]]:
        """"Did you mean" candidates for ``word``, best first."""
        word = word.strip()
        if not word:
            return []
        return _fuzzy.extract(word, self.all_headwords(), limit=limit, cutoff=cutoff)

    def suggest(self, prefix: str, limit: int = 30, fuzzy_fill_below: int = FUZZY_FILL_BELOW,
                fuzzy_limit: int = _fuzzy.DEFAULT_LIMIT,
                fuzzy_cutoff: float = _fuzzy.DEFAULT_CUTOFF) -> list[Suggestion]:
        """Prefix hits ranked closest-first, filled up with fuzzy matches when
        there are fewer than ``fuzzy_fill_below``."""
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
        out = [Suggestion(w) for w in _fuzzy.rank(prefix, words)]
        if len(out) < fuzzy_fill_below:
            for w, _score in self.fuzzy(prefix, fuzzy_limit, fuzzy_cutoff):
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
