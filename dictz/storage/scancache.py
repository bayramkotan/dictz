"""``scan_cache.json``: what the dictionary folders held at the last scan.

Same file and rules as the Rust edition (``dictz-rs/src/scancache.rs``).
A folder's listing is kept with the folder's modification time; a folder
whose time has not changed is not listed again (adding, removing or renaming
a file changes it). So a start only looks at the folders' times — nothing is
re-scanned unless something was really added or removed.

```json
{"version": 1, "dirs": {"/abs/dir": [mtime_ns, ["a.ifo", "b.dsl"], ["sub"]]}}
```
"""
from __future__ import annotations

import contextlib
import json
import os
import tempfile
from pathlib import Path

SCAN_CACHE_FILE = "scan_cache.json"


def _is_dictionary_name(name: str) -> bool:
    return name.lower().endswith((".ifo", ".dsl", ".dsl.dz", ".bgl"))


class ScanCache:
    def __init__(self, path: Path | str | None):
        self.path = Path(path) if path is not None else None
        self.dirs: dict[str, list] = {}
        self.changed = False
        if self.path is not None:
            try:
                data = json.loads(self.path.read_text(encoding="utf-8"))
                if isinstance(data, dict) and isinstance(data.get("dirs"), dict):
                    self.dirs = data["dirs"]
            except (OSError, ValueError):
                pass

    def _listing(self, d: Path) -> tuple[list[str], list[str]] | None:
        try:
            m = d.stat().st_mtime_ns
        except OSError:
            return None
        key = str(d)
        hit = self.dirs.get(key)
        if isinstance(hit, list) and len(hit) == 3 and hit[0] == m:
            return list(hit[1]), list(hit[2])
        files: list[str] = []
        subs: list[str] = []
        try:
            entries = sorted(d.iterdir(), key=lambda p: p.name)
        except OSError:
            return None
        for e in entries:
            try:
                if e.is_dir():
                    if not e.name.startswith(".") and not e.name.lower().endswith(".files"):
                        subs.append(e.name)
                elif _is_dictionary_name(e.name):
                    files.append(e.name)
            except OSError:
                continue
        self.dirs[key] = [m, files, subs]
        self.changed = True
        return files, subs

    def scan(self, folder: Path | str, recursive: bool = True) -> list[Path]:
        """Same result as ``query_manager.scan_folder`` (sorted dictionary files)."""
        root = Path(folder).expanduser()
        if root.is_file():
            return [root] if _is_dictionary_name(root.name) else []
        out: list[Path] = []
        stack = [root]
        while stack:
            d = stack.pop()
            got = self._listing(d)
            if got is None:
                continue
            files, subs = got
            out += [d / f for f in files]
            if recursive:
                stack += [d / s for s in subs]
        return sorted(out, key=lambda p: str(p).lower())

    def save(self) -> None:
        if self.path is None or not self.changed:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        text = json.dumps({"version": 1, "dirs": self.dirs}, ensure_ascii=False, indent=1, sort_keys=True) + "\n"
        fd, tmp = tempfile.mkstemp(prefix=".scan_cache-", suffix=".json", dir=str(self.path.parent))
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(text)
            os.replace(tmp, self.path)
        except OSError:
            with contextlib.suppress(OSError):
                os.unlink(tmp)
        self.changed = False
