"""Adding, opening and rebuilding dictionaries in the background, several at once.

Same rules as the Rust edition (``dictz-rs/src/loading.rs``).

Reading a dictionary file (DSL and Babylon above all) and writing its article
store is the slow part. That work is done by **worker processes** (a process
pool, so the window's thread never waits on Python's GIL), several
dictionaries at the same time. One coordinating thread (:class:`LoadJob`)
hands the work out, then opens the finished stores, writes the list
(``dictionaries.json``) and the headword index — the only writer of both, so
nothing is written twice at the same moment.

A job keeps an inbox: files, folder scans and rebuilds added while it runs are
taken up by the same job (:meth:`LoadJob.add`); nothing runs in the window's
thread. Progress is a :class:`LoadState` (done / total, the dictionaries being
prepared with their share done), sent at most every 0.1 s.
"""
from __future__ import annotations

import contextlib
import os
import queue
import sqlite3
import threading
import time
from collections.abc import Callable, Iterable
from concurrent.futures import FIRST_COMPLETED, Executor, Future, ThreadPoolExecutor, wait
from dataclasses import dataclass, field
from pathlib import Path

MAX_AUTO_WORKERS = 4

# ----------------------------------------------------------------- workers


def worker_count(setting: int, jobs: int) -> int:
    """Worker processes for ``jobs`` dictionaries: the setting
    (``dictionaries.build_workers``) or, at 0, the processors minus one, at most
    :data:`MAX_AUTO_WORKERS`; never more than there is work for."""
    n = setting if setting > 0 else max(1, min((os.cpu_count() or 2) - 1, MAX_AUTO_WORKERS))
    return max(1, min(n, max(1, jobs)))


_progress: object | None = None  # a queue in each worker (set by _init)


def _init(q) -> None:
    global _progress
    _progress = q


def _report(path: str, phase: str, done: int = 0, total: int = 0) -> None:
    if _progress is not None:
        with contextlib.suppress(Exception):
            _progress.put_nowait((path, phase, done, total))


def prepare(path: str, data_dir: str, rebuild: bool = False) -> str:
    """Runs in a worker: make the article store of ``path`` fresh.

    Returns ``"fresh"`` (nothing to do), ``"built"`` or ``"nostore"`` (the
    file was read but the store could not be written, e.g. a read-only data
    folder — the coordinator then opens the file itself). A file that cannot
    be read raises.
    """
    from ..storage import articles
    from ..storage.registry import path_id
    from .query_manager import QueryManager

    store = articles.store_path(data_dir, path_id(path))
    if not rebuild and articles.is_fresh(store, path):
        return "fresh"
    _report(path, "read")
    parsed = QueryManager.open_file(path)
    try:
        articles.build(parsed, store, progress=lambda d, t: _report(path, "write", d, t))
    except (OSError, sqlite3.Error):
        return "nostore"
    finally:
        parsed.close()
    return "built"


class Preparer:
    """A pool of workers that run :func:`prepare`. ``mode`` = ``"process"``
    (default) or ``"thread"`` (tests; also the fallback when processes cannot
    be started)."""

    def __init__(self, workers: int, mode: str = "process"):
        self.workers = max(1, workers)
        self.mode = mode
        self._pool: Executor | None = None
        self._q = None

    def _start(self) -> Executor:
        if self._pool is not None:
            return self._pool
        if self.mode == "process":
            try:
                import multiprocessing
                from concurrent.futures import ProcessPoolExecutor

                ctx = multiprocessing.get_context("spawn")  # no fork with Qt threads
                self._q = ctx.Queue()
                self._pool = ProcessPoolExecutor(self.workers, mp_context=ctx, initializer=_init,
                                                 initargs=(self._q,))
                return self._pool
            except (OSError, ValueError, ImportError, NotImplementedError):
                self.mode = "thread"
        self._q = queue.Queue()
        _init(self._q)
        self._pool = ThreadPoolExecutor(self.workers, thread_name_prefix="dictz-prepare")
        return self._pool

    def submit(self, path: str, data_dir: str, rebuild: bool = False) -> Future:
        from concurrent.futures.process import BrokenProcessPool

        try:
            return self._start().submit(prepare, path, data_dir, rebuild)
        except (BrokenProcessPool, RuntimeError, OSError):
            # worker processes cannot run here (a sandbox, an odd frozen build):
            # threads still keep the window free, only slower
            self.kill()
            self.mode = "thread"
            return self._start().submit(prepare, path, data_dir, rebuild)

    def run_here(self, path: str, data_dir: str, rebuild: bool = False) -> str:
        """The fallback after a broken process pool: run in this thread."""
        if self._q is None:
            self._q = queue.Queue()
        _init(self._q)
        return prepare(path, data_dir, rebuild)

    def messages(self) -> list[tuple[str, str, int, int]]:
        out = []
        while self._q is not None:
            try:
                out.append(self._q.get_nowait())
            except (queue.Empty, OSError, ValueError, EOFError):
                break
        return out

    def kill(self) -> None:
        """Quit now: stop the worker processes (half-written stores are temp
        files, removed at a later start by ``articles.remove_stale_temp``)."""
        pool = self._pool
        if pool is None:
            return
        for proc in list(getattr(pool, "_processes", {}).values() or []):
            with contextlib.suppress(Exception):
                proc.terminate()
        with contextlib.suppress(Exception):
            pool.shutdown(wait=False, cancel_futures=True)
        self._pool = None

    def shutdown(self) -> None:
        if self._pool is not None:
            self._pool.shutdown(wait=True, cancel_futures=True)
            self._pool = None
        if self._q is not None and hasattr(self._q, "close"):
            with contextlib.suppress(Exception):
                self._q.close()
        self._q = None


# ------------------------------------------------------------------- state


@dataclass
class LoadState:
    """What the progress bar shows."""

    phase: str = "open"  # "scan" (listing folders), "open", "prepare"
    done: int = 0
    total: int = 0
    running: list[tuple[str, float | None]] = field(default_factory=list)  # (file name, share done or None)
    cancelling: bool = False

    @property
    def fraction(self) -> float:
        """Share of the whole job done, counting the running dictionaries' parts."""
        if self.total <= 0:
            return 0.0
        part = sum(f for _n, f in self.running if f)
        return min(1.0, (self.done + part) / self.total)

    @property
    def names(self) -> str:
        return ", ".join(n for n, _f in self.running)


@dataclass
class _Item:
    kind: str  # "row" (from the list), "file" (new), "rebuild"
    path: str
    row: dict | None = None
    name: str = ""
    enabled: bool = True
    changed: bool = False


# ------------------------------------------------------------------- the job


class LoadJob:
    """One background job; :meth:`run` in a thread (or right away).

    Callbacks (called from the job's thread):

    - ``on_loaded(dictionary, path, enabled, replaced)`` — ``replaced`` is the
      list name of a rebuilt dictionary (keep its place), else ``""``
    - ``on_failed(path, error)`` — the dictionary stays in the list (⚠)
    - ``on_progress(LoadState)``
    """

    def __init__(self, db_path: Path, *, rows: Iterable[dict] = (), files: Iterable[Path] = (),
                 scans: Iterable[Callable[[], list[Path]]] = (), reindex_changed: bool = True,
                 workers: int = 0, mode: str = "process",
                 on_loaded: Callable | None = None, on_failed: Callable | None = None,
                 on_progress: Callable | None = None):
        self.db_path = Path(db_path)
        self.data_dir = self.db_path.parent
        self.reindex_changed = reindex_changed
        self.workers_setting = workers
        self.mode = mode
        self.on_loaded = on_loaded or (lambda *a: None)
        self.on_failed = on_failed or (lambda *a: None)
        self.on_progress = on_progress or (lambda s: None)
        self._lock = threading.Lock()
        self._closed = False
        self._cancel = threading.Event()
        self._stop = threading.Event()
        self._preparer: Preparer | None = None
        self._inbox: list[tuple[str, object]] = []
        self._seen: set[str] = set()
        self.loaded = 0
        self.errors: list[str] = []
        self.state = LoadState()
        self._last_sent = 0.0
        for r in rows:
            self._inbox.append(("row", r))
        for s in scans:
            self._inbox.append(("scan", s))
        for f in files:
            self._inbox.append(("file", Path(f)))

    # ---------------------------------------------------------- from outside

    def add(self, files: Iterable[Path] = (), scans: Iterable[Callable[[], list[Path]]] = (),
            rebuild: Iterable[tuple[str, str, bool]] = ()) -> bool:
        """Give the running job more work. ``rebuild`` = (list name, path,
        enabled). ``False`` when the job has already finished (start a new
        one)."""
        with self._lock:
            if self._closed:
                return False
            self._inbox += [("scan", s) for s in scans]
            self._inbox += [("file", Path(f)) for f in files]
            self._inbox += [("rebuild", r) for r in rebuild]
            return True

    def cancel(self) -> None:
        """Add no more new dictionaries (the ones being prepared now finish);
        the list's own dictionaries and rebuilds still complete."""
        self._cancel.set()
        self.state.cancelling = True
        self._send(force=True)

    def stop(self) -> None:
        """The program quits: stop at once, worker processes included."""
        self._stop.set()
        self._cancel.set()
        if self._preparer is not None:
            self._preparer.kill()

    @property
    def cancelled(self) -> bool:
        return self._cancel.is_set()

    # ---------------------------------------------------------- the thread

    def _send(self, force: bool = False) -> None:
        now = time.monotonic()
        if force or now - self._last_sent >= 0.1:
            self._last_sent = now
            s = self.state
            self.on_progress(LoadState(s.phase, s.done, s.total, list(s.running), s.cancelling))

    def _take(self) -> list[tuple[str, object]]:
        with self._lock:
            items, self._inbox = self._inbox, []
            if not items and self._finishing:
                self._closed = True
            return items

    def run(self) -> None:
        from ..storage.articles import remove_stale_temp
        from ..storage.indexer import IndexStore
        from ..storage.registry import norm_path

        remove_stale_temp(self.data_dir)
        store = IndexStore(self.db_path)
        store.open()
        known = {norm_path(r["path"]) for r in store.list_dictionaries(enabled_only=False)}
        preparer: Preparer | None = None
        running: dict[Future, _Item] = {}
        waiting: list[_Item] = []  # to prepare, not yet handed out
        shares: dict[str, float | None] = {}
        self._finishing = False
        try:
            while True:
                self._finishing = not running and not waiting
                items = self._take()
                if self._stop.is_set() or (not items and self._closed):
                    break
                for kind, obj in items:
                    if self.cancelled and kind in ("scan", "file"):
                        continue  # cancelled: no new dictionaries; the list's own still open
                    if kind == "scan":
                        self.state.phase = "scan"
                        self._send(force=True)
                        try:
                            found = obj()
                        except OSError as exc:
                            self.errors.append(str(exc))
                            found = []
                        items += [("file", Path(f)) for f in found]
                    elif kind == "row":
                        self._open_row(obj, store, waiting)
                    elif kind == "file":
                        key = norm_path(obj)
                        if key in known or key in self._seen:
                            continue
                        self._seen.add(key)
                        waiting.append(_Item("file", str(obj)))
                        self.state.total += 1
                    elif kind == "rebuild":
                        name, path, enabled = obj
                        waiting.append(_Item("rebuild", str(path), name=name, enabled=enabled))
                        self.state.total += 1
                if self.cancelled:
                    dropped = [it for it in waiting if it.kind == "file"]
                    waiting[:] = [it for it in waiting if it.kind != "file"]
                    self.state.total -= len(dropped)
                    for f, it in running.items():
                        if it.kind == "file":
                            f.cancel()  # only one not started yet; a running worker finishes
                if waiting:
                    if preparer is None:
                        preparer = Preparer(worker_count(self.workers_setting, len(waiting) + len(running)),
                                            self.mode)
                        self._preparer = preparer
                    while waiting and len(running) < preparer.workers:
                        it = waiting.pop(0)
                        running[preparer.submit(it.path, str(self.data_dir), it.kind == "rebuild")] = it
                        shares[it.path] = None
                self.state.phase = "prepare" if (running or waiting) else self.state.phase
                if not running:
                    self.state.running = []
                    self._send()
                    if not waiting:
                        self._finishing = True
                    continue
                finished, _ = wait(list(running), timeout=0.1, return_when=FIRST_COMPLETED)
                if self._stop.is_set():
                    break
                for path, phase, d, total in (preparer.messages() if preparer else []):
                    if path in shares:
                        shares[path] = (d / total) if (phase == "write" and total) else 0.0
                for fut in finished:
                    it = running.pop(fut)
                    shares.pop(it.path, None)
                    self._finish(fut, it, store, preparer)
                    self.state.done += 1
                self.state.running = [(Path(it.path).name, shares.get(it.path)) for it in running.values()]
                self._send(force=bool(finished))
        finally:
            with self._lock:
                self._closed = True
            if self._stop.is_set():
                if preparer is not None:
                    preparer.kill()  # its temp files go at a later start (remove_stale_temp)
            elif preparer is not None:
                preparer.shutdown()
            store.close()
            self.state.running = []
            self._send(force=True)

    # ---------------------------------------------------------- steps

    def _open_row(self, row: dict, store, waiting: list[_Item]) -> None:
        """A dictionary of the list: from ``dictionaries.json`` alone when its
        files are unchanged; otherwise its store is prepared by a worker."""
        from ..storage import articles
        from ..storage.registry import norm_path, path_id
        from .query_manager import QueryManager, _changed

        path = Path(row["path"])
        self._seen.add(norm_path(path))
        self.state.total += 1
        changed = _changed(row, path)
        st = articles.store_path(self.data_dir, path_id(path))
        fast = bool(row.get("stamp")) and st.is_file() and row["stamp"] == articles.source_stamp(path)
        if fast or (path.exists() and articles.is_fresh(st, path)) or not path.exists():
            try:
                d, built = QueryManager.open_row(row, self.data_dir, store)
                if changed and (built or self.reindex_changed):
                    store.index_dictionary(d)
                self.loaded += 1
                self.on_loaded(d, row["path"], bool(row["enabled"]), "")
            except Exception as exc:  # noqa: BLE001 — stays in the list, marked ⚠
                self.errors.append(f"{path}: {exc}")
                self.on_failed(row["path"], str(exc))
            self.state.done += 1
            self._send()
            return
        waiting.append(_Item("row", row["path"], row=row, enabled=bool(row["enabled"]), changed=changed))

    def _finish(self, fut: Future, it: _Item, store, preparer: Preparer | None) -> None:
        from concurrent.futures.process import BrokenProcessPool

        from .query_manager import QueryManager

        try:
            try:
                result = fut.result()
            except BrokenProcessPool:
                result = preparer.run_here(it.path, str(self.data_dir), it.kind == "rebuild") if preparer else "nostore"
            # the store is fresh now (or could not be written: open_cached reads the file)
            if it.kind == "row":
                d, _ = QueryManager.open_row(it.row, self.data_dir, store)
                if it.changed and (result == "built" or self.reindex_changed):
                    store.index_dictionary(d)
                self.on_loaded(d, it.path, it.enabled, "")
            elif it.kind == "rebuild":
                d, _ = QueryManager.open_cached(it.path, self.data_dir)
                d.metadata.name = it.name
                store.index_dictionary(d)
                self.on_loaded(d, it.path, it.enabled, it.name)
            else:
                d, _ = QueryManager.open_cached(it.path, self.data_dir)
                name = store.register_dictionary(d)  # unique name: "X (2)" for a second "X"
                if not store.is_indexed(name):
                    store.index_dictionary(d)
                self.on_loaded(d, it.path, True, "")
            self.loaded += 1
        except Exception as exc:  # noqa: BLE001 — reported, the job goes on
            if fut.cancelled():
                return
            self.errors.append(f"{it.path}: {exc}")
            self.on_failed(it.path, str(exc))
