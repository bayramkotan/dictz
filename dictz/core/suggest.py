"""Search-box suggestions and "did you mean" without blocking the window.

``Candidates`` keeps the fuzzy candidate list (every headword of the enabled
dictionaries, in list order, duplicates removed) together with its
lower-cased copy, built once per set of dictionaries; each dictionary's
own headword list is kept too, so switching one dictionary on or off does
not read all the others again.

``SuggestWorker`` computes suggestions and fuzzy matches on its own thread
with its own read-only connection to the index; only the newest request of
each kind is answered, older ones are dropped. The window shows what comes
back if the search box (or the page) still asks for it.

Mirrors ``dictz-rs/src/suggest.rs``.
"""
from __future__ import annotations

import sqlite3
import threading
from collections.abc import Callable
from pathlib import Path
from typing import Any

from . import fuzzy as _fuzzy

Snapshot = list  # [(name, Dictionary)] of the enabled dictionaries, in order


class Candidates:
    """The fuzzy candidate list, cached per set of enabled dictionaries."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._per: dict[tuple, list[str]] = {}
        # (key, words, lowered): replaced in one assignment, read without the lock
        self._state: tuple[tuple, list[str], list[str]] = ((), [], [])

    @staticmethod
    def key(dicts: Snapshot) -> tuple:
        # the dictionary objects themselves: a reopened dictionary is a new key
        return tuple((n, d) for n, d in dicts)

    def ready(self, dicts: Snapshot) -> bool:
        return self._state[0] == self.key(dicts)

    def get(self, dicts: Snapshot, wait: bool = True) -> tuple[list[str], list[str]] | None:
        """``(words, lowered)``; ``None`` when ``wait`` is false and the list
        is not built yet (or another thread is building it)."""
        key = self.key(dicts)
        state = self._state
        if state[0] == key:
            return state[1], state[2]
        if not self._lock.acquire(blocking=wait):
            return None
        try:
            state = self._state
            if state[0] == key:
                return state[1], state[2]
            per = {k: v for k, v in self._per.items() if k in key}
            seen: set[str] = set()
            words: list[str] = []
            for item in key:
                ws = per.get(item)
                if ws is None:
                    ws = per[item] = list(item[1].headwords())
                for w in ws:
                    if w not in seen:
                        seen.add(w)
                        words.append(w)
            lowered = [w.lower() for w in words]
            self._per = per
            self._state = (key, words, lowered)
            return words, lowered
        finally:
            self._lock.release()


def merge_suggestions(prefix: str, prefix_words: list[str], fuzzy: Callable[[], list[tuple[str, float]]],
                      limit: int, fuzzy_fill_below: int) -> list:
    """Prefix hits ranked closest-first, filled up with fuzzy matches when
    there are fewer than ``fuzzy_fill_below`` (``QueryManager.suggest``)."""
    from .query_manager import Suggestion

    seen: set[str] = set()
    words: list[str] = []
    for w in prefix_words:
        if w not in seen:
            seen.add(w)
            words.append(w)
    out = [Suggestion(w) for w in _fuzzy.rank(prefix, words)]
    if len(out) < fuzzy_fill_below:
        for w, _score in fuzzy():
            if len(out) >= limit:
                break
            if w not in seen:
                seen.add(w)
                out.append(Suggestion(w, fuzzy=True))
    return out


class SuggestWorker(threading.Thread):
    """One background thread for "suggest", "fuzzy" and "warm" requests.

    ``on_result(kind, query, result)`` is called on the worker thread; the
    window turns it into a Qt signal.
    """

    def __init__(self, db_path: Path | str | None, on_result: Callable[[str, str, Any], None],
                 candidates: Candidates | None = None):
        super().__init__(name="dictz-suggest", daemon=True)
        self.db_path = Path(db_path) if db_path else None
        self.on_result = on_result
        self.candidates = candidates if candidates is not None else Candidates()
        self._cond = threading.Condition()
        self._jobs: dict[str, tuple] = {}
        self._stop = False
        self._con: sqlite3.Connection | None = None

    # ------------------------------------------------------------ requests (window thread)
    def submit(self, kind: str, query: str, dicts: Snapshot, **params) -> None:
        with self._cond:
            self._jobs[kind] = (query, list(dicts), params)
            self._cond.notify()

    def cancel(self, kind: str) -> None:
        with self._cond:
            self._jobs.pop(kind, None)

    def stop(self) -> None:
        with self._cond:
            self._stop = True
            self._jobs.clear()
            self._cond.notify()

    # ------------------------------------------------------------ worker thread
    def run(self) -> None:
        while True:
            with self._cond:
                while not self._jobs and not self._stop:
                    self._cond.wait()
                if self._stop:
                    break
                # suggestions first: they are what the user is waiting for
                kind = next(k for k in ("suggest", "fuzzy", "warm") if k in self._jobs)
                job = self._jobs.pop(kind)
            try:
                result = self._do(kind, *job)
            except Exception as e:  # noqa: BLE001 — a failed suggestion must not kill the thread
                print(f"[dictz] suggestions failed: {e}")
                continue
            with self._cond:
                stale = kind in self._jobs  # a newer request of the same kind is waiting
            if not stale and kind != "warm":
                self.on_result(kind, job[0], result)
        if self._con is not None:
            self._con.close()

    def _prefix_words(self, prefix: str, dicts: Snapshot, limit: int) -> list[str]:
        names = [n for n, _ in dicts]
        if self.db_path is not None and self.db_path.is_file():
            if self._con is None:
                self._con = sqlite3.connect(f"file:{self.db_path.as_posix()}?mode=ro", uri=True)
            from ..storage.indexer import search_headword_rows

            return [w for w, _ in search_headword_rows(self._con, prefix, limit, names)]
        words: list[str] = []
        for _n, d in dicts:
            for e in d.lookup(prefix, exact=False):
                words.append(e.headword)
            if len(set(words)) >= limit:
                break
        return words

    def _fuzzy(self, word: str, dicts: Snapshot, limit: int, cutoff: float) -> list[tuple[str, float]]:
        got = self.candidates.get(dicts)
        if not got or not word.strip():
            return []
        words, lowered = got
        return _fuzzy.extract_prepared(word.strip(), words, lowered, limit=limit, cutoff=cutoff)

    def _do(self, kind: str, query: str, dicts: Snapshot, params: dict):
        if kind == "warm":
            self.candidates.get(dicts)
            return None
        if kind == "fuzzy":
            return self._fuzzy(query, dicts, params["limit"], params["cutoff"])
        prefix = query.strip()
        if not prefix:
            return []
        limit = params["limit"]
        words = self._prefix_words(prefix, dicts, limit)
        return merge_suggestions(
            prefix, words, lambda: self._fuzzy(prefix, dicts, params["fuzzy_limit"], params["fuzzy_cutoff"]),
            limit, params["fuzzy_fill_below"])
