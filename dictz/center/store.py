"""A JSON file changed by several writers (window, CLI, the other edition).

Same rules as ``dictionaries.json`` (``dictz/storage/registry.py``): every
change holds ``<file>.lock`` (created with O_EXCL; stale after 10 s), reads
the file fresh inside the lock, writes a temp file and renames it, and keeps
the previous file as ``<file>.bak`` (read when the file itself is broken).
Written with sorted keys, one-space indent, UTF-8 — byte for byte what the
Rust edition writes (``dictz-rs/src/center/store.rs``).
"""
from __future__ import annotations

import contextlib
import json
import os
import shutil
import tempfile
import threading
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import Any

_THREAD_LOCK = threading.RLock()


def dumps(data: Any) -> str:
    return json.dumps(data, ensure_ascii=False, indent=1, sort_keys=True) + "\n"


class JsonFile:
    def __init__(self, path: Path | str, default: Callable[[], Any]):
        self.path = Path(path)
        self.default = default

    @property
    def backup(self) -> Path:
        return self.path.with_name(self.path.name + ".bak")

    @staticmethod
    def _load(path: Path) -> Any:
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None

    def read(self) -> Any:
        data = self._load(self.path)
        if data is None and self.path.exists():
            data = self._load(self.backup)  # half-written or broken: the backup
        return self.default() if data is None else data

    def write(self, data: Any) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        if self.path.is_file() and self._load(self.path) is not None:
            with contextlib.suppress(OSError):
                shutil.copyfile(self.path, self.backup)
        fd, tmp = tempfile.mkstemp(prefix=f".{self.path.stem}-", suffix=".json", dir=str(self.path.parent))
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
                f.write(dumps(data))
            for attempt in range(10):  # Windows: the other edition may hold it for a moment
                try:
                    os.replace(tmp, self.path)
                    break
                except PermissionError:
                    if attempt == 9:
                        raise
                    time.sleep(0.05)
        except OSError:
            with contextlib.suppress(OSError):
                os.unlink(tmp)
            raise

    @contextlib.contextmanager
    def locked(self) -> Iterator[None]:
        with _THREAD_LOCK:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            lock = self.path.with_name(self.path.name + ".lock")
            fd = None
            deadline = time.monotonic() + 5
            while True:
                try:
                    fd = os.open(str(lock), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                    break
                except FileExistsError:
                    with contextlib.suppress(OSError):
                        if time.time() - lock.stat().st_mtime > 10:
                            lock.unlink()
                            continue
                    if time.monotonic() > deadline:
                        break  # do the change anyway rather than lose it
                    time.sleep(0.01)
                except OSError:
                    break
            try:
                yield
            finally:
                if fd is not None:
                    os.close(fd)
                    with contextlib.suppress(OSError):
                        lock.unlink()

    def update(self, change: Callable[[Any], Any]) -> Any:
        """Read fresh, change, write — under the lock. ``change`` returns the
        new data (or changes it in place and returns ``None``)."""
        with self.locked():
            data = self.read()
            new = change(data)
            data = data if new is None else new
            self.write(data)
            return data
