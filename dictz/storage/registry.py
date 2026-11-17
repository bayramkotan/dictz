"""The dictionary list: ``dictionaries.json`` in the data folder.

Same file and rules as the Rust edition (``dictz-rs/src/registry.rs``):

* the list order is the search order;
* a dictionary is identified by its **file path**; its ``name`` is unique —
  a second dictionary with the same name gets ``"Name (2)"``;
* every change re-reads the file first (always, not only when it looks
  changed) and is written at once (temp file + rename), so a change made by
  the other edition, the CLI or another window is never overwritten;
* a change holds ``dictionaries.json.lock`` (threads, windows, the CLI and
  both editions take turns), so two changes at once never lose one;
* the previous file is kept as ``dictionaries.json.bak``; when the file
  cannot be read, the backup is used and the list is never written empty
  over an unreadable file;
* the first start migrates the old SQLite ``dictionaries`` table.

```json
{"version": 1, "dictionaries": [
  {"id": "…", "name": "…", "path": "/…/x.ifo", "format": "stardict", "enabled": true,
   "word_count": 0, "source_lang": "", "target_lang": "", "description": "",
   "mtime": 0.0, "indexed": true}
]}
```
"""
from __future__ import annotations

import contextlib
import json
import os
import shutil
import tempfile
import threading
import time
from pathlib import Path
from typing import Any, Iterator

REGISTRY_FILE = "dictionaries.json"
_THREAD_LOCK = threading.RLock()
FIELDS = ("id", "name", "path", "format", "enabled", "word_count", "source_lang", "target_lang",
          "description", "mtime", "indexed", "stamp", "author", "icon")


def fnv1a64(data: bytes) -> int:
    h = 0xcbf29ce484222325
    for b in data:
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def norm_path(path: Path | str) -> str:
    """Absolute path as text (the identity of a dictionary)."""
    return str(Path(path).expanduser().absolute())


def path_id(path: Path | str) -> str:
    """Stable id of a dictionary file: FNV-1a 64 of its absolute path."""
    return f"{fnv1a64(norm_path(path).encode('utf-8')):016x}"


def unique_name(base: str, taken: set[str]) -> str:
    """``base``, or ``base (2)``, ``base (3)``… if it is taken."""
    base = base.strip() or "?"
    if base not in taken:
        return base
    n = 2
    while f"{base} ({n})" in taken:
        n += 1
    return f"{base} ({n})"


class Registry:
    """``dictionaries.json`` (``path=None`` keeps it in memory only)."""

    def __init__(self, path: Path | str | None):
        self.path = Path(path) if path is not None else None
        self.items: list[dict[str, Any]] = []
        self._stamp: tuple[float, int] | None = None
        self.reload()

    # ------------------------------------------------------------------ file
    def exists(self) -> bool:
        return self.path is not None and self.path.is_file()

    def _file_stamp(self) -> tuple[float, int] | None:
        try:
            st = self.path.stat()  # type: ignore[union-attr]
            return st.st_mtime, st.st_size
        except (OSError, AttributeError):
            return None

    @property
    def backup_path(self) -> Path | None:
        return self.path.with_name(self.path.name + ".bak") if self.path is not None else None

    @staticmethod
    def _read(path: Path) -> list[dict[str, Any]] | None:
        """Entries of a list file, ``None`` when it is missing or unreadable."""
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None
        if not isinstance(data, dict) or not isinstance(data.get("dictionaries"), list):
            return None
        return [Registry._clean(d) for d in data["dictionaries"] if isinstance(d, dict) and d.get("path")]

    def reload(self, force: bool = False) -> None:
        """Re-read the file (only when it changed on disk, unless ``force``)."""
        if self.path is None:
            return
        stamp = self._file_stamp()
        if stamp is None or (stamp == self._stamp and not force):
            return
        items = self._read(self.path)
        if items is None and self.backup_path is not None:
            items = self._read(self.backup_path)  # half-written or broken file: use the backup
        if items is None:
            return
        self.items = items
        self._stamp = stamp

    def _fresh(self) -> None:
        """Before a change: always read what is on disk now."""
        self.reload(force=True)

    @staticmethod
    def _clean(d: dict[str, Any]) -> dict[str, Any]:
        return {
            "id": str(d.get("id") or path_id(d["path"])),
            "name": str(d.get("name") or Path(d["path"]).stem),
            "path": str(d["path"]),
            "format": str(d.get("format") or ""),
            "enabled": bool(d.get("enabled", True)),
            "word_count": int(d.get("word_count") or 0),
            "source_lang": str(d.get("source_lang") or ""),
            "target_lang": str(d.get("target_lang") or ""),
            "description": str(d.get("description") or ""),
            "mtime": float(d.get("mtime") or 0.0),
            "indexed": bool(d.get("indexed", False)),
            # files' "name:size:mtime_ns" when the article store was built: a start
            # that finds the same stamp opens the dictionary from this list alone
            "stamp": str(d.get("stamp") or ""),
            "author": str(d.get("author") or ""),
            # cached icon in DATA/icons ("" = not looked for yet, "-" = none)
            "icon": str(d.get("icon") or ""),
        }

    def save(self) -> None:
        if self.path is None:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        text = json.dumps({"version": 1, "dictionaries": self.items}, ensure_ascii=False, indent=1) + "\n"
        if self.path.is_file() and self._read(self.path) is not None:
            with contextlib.suppress(OSError):
                shutil.copyfile(self.path, self.backup_path)  # type: ignore[arg-type]
        fd, tmp = tempfile.mkstemp(prefix=".dictionaries-", suffix=".json", dir=str(self.path.parent))
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(text)
            for attempt in range(10):  # Windows: the file may be open in the other edition for a moment
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
        self._stamp = self._file_stamp()

    # ------------------------------------------------------------------ queries
    def rows(self, enabled_only: bool = False) -> list[dict[str, Any]]:
        self.reload()
        out = []
        for i, d in enumerate(self.items):
            if enabled_only and not d["enabled"]:
                continue
            row = dict(d)
            row["sort_order"] = i
            out.append(row)
        return out

    def get(self, name: str) -> dict[str, Any] | None:
        return next((r for r in self.rows() if r["name"] == name), None)

    def by_path(self, path: Path | str) -> dict[str, Any] | None:
        p = norm_path(path)
        return next((d for d in self.items if norm_path(d["path"]) == p), None)

    # ------------------------------------------------------------------ changes
    @contextlib.contextmanager
    def _locked(self) -> Iterator[None]:
        """Hold the list for one read-change-write: no other thread, window,
        CLI or edition changes it in between (``dictionaries.json.lock``,
        the same lock file as the Rust edition; a lock older than 10 s is
        stale). Every change re-reads the file inside the lock."""
        if self.path is None:
            yield
            return
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
                    break  # read-only folder: no lock
            try:
                self._fresh()
                yield
            finally:
                if fd is not None:
                    os.close(fd)
                    with contextlib.suppress(OSError):
                        lock.unlink()

    def register(self, name: str, path: Path | str, **fields: Any) -> str:
        """Add or update the dictionary at ``path``; returns its (unique) name.

        An existing entry keeps its name, place and enabled flag.
        """
        with self._locked():
            hit = self.by_path(path)
            if hit is None:
                taken = {d["name"] for d in self.items}
                hit = self._clean({"path": norm_path(path), "name": unique_name(name, taken), "enabled": True})
                self.items.append(hit)
            for k, v in fields.items():
                if k in FIELDS and k not in ("id", "name", "path", "enabled"):
                    hit[k] = v
            hit["id"] = path_id(hit["path"])
            self.save()
            return hit["name"]

    def _edit(self, name: str, **fields: Any) -> None:
        with self._locked():
            for d in self.items:
                if d["name"] == name:
                    d.update(fields)
                    self.save()
                    return

    def set_enabled(self, name: str, enabled: bool) -> None:
        self._edit(name, enabled=bool(enabled))

    def set_icon(self, name: str, icon: str) -> None:
        self._edit(name, icon=str(icon))

    def set_indexed(self, name: str, indexed: bool = True) -> None:
        self._edit(name, indexed=bool(indexed))

    def set_order(self, names: list[str]) -> None:
        """Put ``names`` first in this order; the rest keep their order after them."""
        with self._locked():
            pos = {n: i for i, n in enumerate(names)}
            first = sorted((d for d in self.items if d["name"] in pos), key=lambda d: pos[d["name"]])
            self.items = first + [d for d in self.items if d["name"] not in pos]
            self.save()

    def move_to(self, name: str, index: int) -> None:
        with self._locked():
            d = next((x for x in self.items if x["name"] == name), None)
            if d is None:
                return
            self.items.remove(d)
            self.items.insert(max(0, min(index, len(self.items))), d)
            self.save()

    def remove(self, name: str) -> None:
        with self._locked():
            self.items = [d for d in self.items if d["name"] != name]
            self.save()
