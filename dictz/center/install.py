"""Downloading, checking, unpacking and installing catalog entries.

``install`` does one entry from start to end (the CLI calls it directly; the
window runs it through :class:`DownloadQueue`):

1. the files to fetch — the entry's own, or an archive.org item's, looked up now
2. download each to ``center/downloads/<hash>.part`` (resumes a partial file)
3. check size and SHA-256 / SHA-512 / MD5 when the catalog gives them
4. unpack zip / tar / tar.gz / tar.xz / tar.bz2 (never outside the folder)
5. move into ``DATA/dictionaries/<source>/<entry>/`` (or ``center.install_folder``)
6. record it in ``installed.json`` and ``downloads.json``

The dictionary files found are returned; adding them to the list is the
caller's (the window's background loader, or the CLI).
"""
from __future__ import annotations

import contextlib
import hashlib
import re
import shutil
import tarfile
import threading
import time
import zipfile
from collections.abc import Callable
from pathlib import Path, PurePosixPath

from . import adapters
from .catalog import now_iso
from .model import archive_of
from .net import Cancelled, Http, auth_headers, local_path
from .sources import Sources, center_dir, slug
from .store import JsonFile

HISTORY_LIMIT = 500


class InstallError(Exception):
    pass


def _fnv(text: str) -> str:
    from ..storage.registry import fnv1a64

    return f"{fnv1a64(text.encode('utf-8')):016x}"


def _part_name(url: str) -> str:
    return _fnv(url) + ".part"


def file_hashes(path: Path) -> dict[str, str]:
    h = {"sha256": hashlib.sha256(), "sha512": hashlib.sha512(), "md5": hashlib.md5()}  # noqa: S324 — a checksum
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            for x in h.values():
                x.update(chunk)
    return {k: v.hexdigest() for k, v in h.items()}


def verify(path: Path, f: dict) -> None:
    """Size and checksums given by the catalog; raises on a mismatch."""
    if f.get("size") and path.stat().st_size != int(f["size"]):
        raise InstallError(f"{f['name']}: size {path.stat().st_size} ≠ {f['size']}")
    wanted = {k: f.get(k, "") for k in ("sha256", "sha512", "md5") if f.get(k)}
    if not wanted:
        return
    got = file_hashes(path)
    for k, v in wanted.items():
        if got[k].lower() != v.lower():
            raise InstallError(f"{f['name']}: {k.upper()} does not match")


def _safe_target(root: Path, name: str) -> Path | None:
    name = name.replace("\\", "/")
    p = PurePosixPath(name)
    if p.is_absolute() or ".." in p.parts or (p.parts and ":" in p.parts[0]):
        return None
    target = (root / Path(*p.parts)).resolve()
    if root.resolve() not in target.parents and target != root.resolve():
        return None
    return target


def unpack(archive: Path, kind: str, dest: Path) -> list[Path]:
    """Extract ``archive`` into ``dest``; returns the files written. Entries
    that would land outside ``dest`` (``../``, absolute) and links are skipped."""
    dest.mkdir(parents=True, exist_ok=True)
    out = []
    if kind == "zip":
        try:
            with zipfile.ZipFile(archive) as z:
                for info in z.infolist():
                    if info.is_dir():
                        continue
                    t = _safe_target(dest, info.filename)
                    if t is None:
                        continue
                    t.parent.mkdir(parents=True, exist_ok=True)
                    with z.open(info) as src, open(t, "wb") as dst:
                        shutil.copyfileobj(src, dst, 1 << 20)
                    out.append(t)
        except zipfile.BadZipFile as e:
            raise InstallError(f"{archive.name}: broken zip ({e})") from e
        return out
    if kind.startswith("tar"):
        try:
            with tarfile.open(archive, "r:*") as t:
                for m in t.getmembers():
                    if not m.isfile():
                        continue  # folders are made as needed; links and devices are skipped
                    target = _safe_target(dest, m.name)
                    if target is None:
                        continue
                    target.parent.mkdir(parents=True, exist_ok=True)
                    src = t.extractfile(m)
                    if src is None:
                        continue
                    with src, open(target, "wb") as dst:
                        shutil.copyfileobj(src, dst, 1 << 20)
                    out.append(target)
        except (tarfile.TarError, EOFError, OSError) as e:
            raise InstallError(f"{archive.name}: cannot unpack ({e})") from e
        return out
    raise InstallError(f"{archive.name}: {kind} archives are not supported — unpack it yourself and add the folder")


def dictionary_files(folder: Path) -> list[Path]:
    """Dictionary files dictz opens, inside ``folder`` (sorted)."""
    from ..core.query_manager import scan_folder

    return sorted(scan_folder(folder, True))


class Installer:
    def __init__(self, data_dir: Path | str, sources: Sources | None = None, http: Http | None = None,
                 install_root: Path | str | None = None, verify: bool = True, keep_archives: bool = False):
        self.data_dir = Path(data_dir)
        self.sources = sources or Sources(data_dir)
        self.http = http or Http()
        self.install_root = Path(install_root) if install_root else self.data_dir / "dictionaries"
        self.verify = verify
        self.keep_archives = keep_archives
        cdir = center_dir(self.data_dir)
        self.installed_file = JsonFile(cdir / "installed.json", lambda: {"version": 1, "installed": {}})
        self.history_file = JsonFile(cdir / "downloads.json", lambda: {"version": 1, "downloads": []})
        self.part_dir = cdir / "downloads"

    # ------------------------------------------------------------ records
    def installed(self) -> dict[str, dict]:
        d = self.installed_file.read()
        return dict(d.get("installed") or {}) if isinstance(d, dict) else {}

    def history(self) -> list[dict]:
        d = self.history_file.read()
        return list(d.get("downloads") or []) if isinstance(d, dict) else []

    def log(self, item: dict) -> None:
        def fn(d):
            if not isinstance(d, dict):
                d = {"version": 1, "downloads": []}
            rows = d.setdefault("downloads", [])
            rows.insert(0, item)
            del rows[HISTORY_LIMIT:]
            return d

        with contextlib.suppress(OSError):
            self.history_file.update(fn)

    def clear_history(self) -> None:
        self.history_file.write({"version": 1, "downloads": []})

    def folder_for(self, entry: dict) -> Path:
        tail = entry["id"].split(":", 1)[-1]
        return self.install_root / slug(entry["source"] or "link") / (slug(tail) + "-" + _fnv(entry["id"])[:6])

    # ------------------------------------------------------------ install
    def files_of(self, entry: dict) -> list[dict]:
        if entry["files"]:
            return entry["files"]
        r = entry.get("resolve") or ""
        if r.startswith("archive-org:"):
            from .model import norm_file

            return [norm_file(f) for f in adapters.resolve_archive_org(r.split(":", 1)[1], self.http)]
        return []

    def _headers(self, entry: dict) -> dict:
        src = self.sources.get(entry["source"]) if entry["source"] else None
        if not src:
            return {}
        pw, tok = self.sources.secret(src["id"])
        if src["type"] == "nextcloud-share":
            import base64

            _root, token = adapters.nextcloud_endpoint(src["url"])
            return {"Authorization": "Basic " + base64.b64encode(f"{token}:{pw}".encode()).decode()}
        return auth_headers(src, pw, tok)

    def fetch(self, f: dict, headers: dict, progress: Callable[[int, int], None] | None,
              cancelled: Callable[[], bool] | None) -> Path:
        url = f["url"]
        p = local_path(url)
        if p is not None:
            if not p.is_file():
                raise InstallError(f"no such file: {p}")
            return p
        part = self.part_dir / _part_name(url)
        self.http.download(url, part, headers, progress, cancelled)
        return part

    def install(self, entry: dict, progress: Callable[[str, int, int], None] | None = None,
                cancelled: Callable[[], bool] | None = None) -> dict:
        """Download and install ``entry``; returns its ``installed.json`` record
        (``files`` = the dictionary files to add to the list).
        ``progress(stage, done, total)``: stage ``download`` (bytes), ``check``, ``unpack``."""
        started = now_iso()
        files = self.files_of(entry)
        if not files:
            raise InstallError(f"{entry['name']}: nothing to download here — open its page")
        headers = self._headers(entry)
        folder = self.folder_for(entry)
        staging = folder.with_name(folder.name + ".new")
        shutil.rmtree(staging, ignore_errors=True)
        staging.mkdir(parents=True, exist_ok=True)
        total = sum(int(f.get("size") or 0) for f in files)
        done_before = 0
        hashes: dict[str, str] = {}
        try:
            for f in files:
                def prog(d: int, t: int, base=done_before) -> None:
                    if progress is not None:
                        progress("download", base + d, max(total, base + t))
                path = self.fetch(f, headers, prog, cancelled)
                if progress is not None:
                    progress("check", 0, 0)
                if self.verify:
                    try:
                        verify(path, f)
                    except InstallError:
                        if path.parent == self.part_dir:
                            path.unlink(missing_ok=True)  # a broken download is not resumed
                        raise
                hashes[f["name"]] = file_hashes(path)["sha256"]
                done_before += path.stat().st_size
                kind = f.get("archive") or archive_of(f["name"])
                if progress is not None:
                    progress("unpack", 0, 0)
                name = re.sub(r'[\\/:*?"<>|]+', "_", f["name"]) or "download"
                if kind:
                    unpack(path, kind, staging)
                    if self.keep_archives:
                        shutil.copyfile(path, staging / name)
                elif path.parent == self.part_dir:
                    shutil.move(str(path), str(staging / name))
                else:
                    shutil.copyfile(path, staging / name)
                if path.parent == self.part_dir:
                    path.unlink(missing_ok=True)
            found = dictionary_files(staging)
            if not found and entry.get("supported") is not False:
                raise InstallError(f"{entry['name']}: no dictionary dictz can open was found in the download")
            if folder.exists():
                shutil.rmtree(folder)
            staging.rename(folder)
        except (Cancelled, Exception) as exc:
            shutil.rmtree(staging, ignore_errors=True)
            if not isinstance(exc, Cancelled):
                self.log({"id": entry["id"], "name": entry["name"], "url": files[0]["url"] if files else "",
                          "status": "failed", "error": str(exc), "started": started, "finished": now_iso(),
                          "license": entry["license"]})
            raise
        record = {
            "id": entry["id"], "name": entry["name"], "source": entry["source"], "version": entry["version"],
            "date": entry["date"], "installed": now_iso(), "folder": str(folder),
            "files": [str(folder / p.relative_to(staging)) for p in found],
            "license": entry["license"], "license_url": entry["license_url"], "homepage": entry["homepage"],
            "page": entry["page"], "urls": [f["url"] for f in files], "sha256": hashes,
            "size": done_before, "format": entry["format"], "source_lang": entry["source_lang"],
            "target_lang": entry["target_lang"], "supported": bool(found),
        }

        def fn(d):
            if not isinstance(d, dict):
                d = {"version": 1, "installed": {}}
            d.setdefault("installed", {})[entry["id"]] = record
            return d

        self.installed_file.update(fn)
        self.log({"id": entry["id"], "name": entry["name"], "url": record["urls"][0], "status": "done",
                  "error": "", "started": started, "finished": record["installed"], "license": entry["license"],
                  "size": done_before})
        return record

    def uninstall(self, entry_id: str, delete_files: bool = True) -> dict | None:
        """Forget an installed entry (and delete its folder); returns its record."""
        rec = self.installed().get(entry_id)
        if rec is None:
            return None

        def fn(d):
            (d.get("installed") or {}).pop(entry_id, None)
            return d

        self.installed_file.update(fn)
        if delete_files and rec.get("folder"):
            folder = Path(rec["folder"])
            if self.install_root in folder.parents:  # only what dictz put there
                shutil.rmtree(folder, ignore_errors=True)
        return rec

    def record_for_path(self, path: Path | str) -> dict | None:
        """The installed entry a dictionary file belongs to."""
        from ..storage.registry import norm_path

        p = norm_path(path)
        for rec in self.installed().values():
            if any(norm_path(f) == p for f in rec.get("files") or []):
                return rec
        return None

    def updates(self, entries: list[dict]) -> list[tuple[dict, dict]]:
        """(installed record, catalog entry) where the catalog has something newer."""
        by_id = {e["id"]: e for e in entries}
        out = []
        for rid, rec in sorted(self.installed().items()):
            e = by_id.get(rid)
            if e and (e["version"], e["date"]) != (rec.get("version", ""), rec.get("date", "")) \
                    and (e["version"] or e["date"]):
                out.append((rec, e))
        return out


class DownloadQueue:
    """Downloads for the window: ``parallel`` at a time, pause / resume /
    cancel / retry. ``on_change(task)`` is called from worker threads."""

    def __init__(self, installer: Installer, parallel: int = 2,
                 on_change: Callable[[dict], None] | None = None,
                 on_installed: Callable[[dict, dict], None] | None = None):
        self.installer = installer
        self.parallel = max(1, parallel)
        self.on_change = on_change or (lambda t: None)
        self.on_installed = on_installed or (lambda rec, entry: None)
        self.tasks: list[dict] = []
        self._lock = threading.RLock()
        self._running = 0

    def add(self, entry: dict) -> dict:
        with self._lock:
            for t in self.tasks:
                if t["entry"]["id"] == entry["id"] and t["state"] in ("queued", "running", "paused"):
                    return t
            t = {"id": f"{len(self.tasks) + 1}", "entry": entry, "state": "queued", "stage": "", "done": 0,
                 "total": int(entry.get("size") or 0), "speed": 0.0, "error": "", "_stop": ""}
            self.tasks.append(t)
        self._pump()
        self.on_change(t)
        return t

    def _pump(self) -> None:
        with self._lock:
            while self._running < self.parallel:
                t = next((x for x in self.tasks if x["state"] == "queued"), None)
                if t is None:
                    return
                t["state"], t["_stop"] = "running", ""
                self._running += 1
                threading.Thread(target=self._run, args=(t,), daemon=True, name="dictz-download").start()

    def _run(self, t: dict) -> None:
        start = time.monotonic()
        last = [0.0]

        def prog(stage: str, done: int, total: int) -> None:
            t["stage"] = stage
            if stage == "download":
                t["done"], t["total"] = done, max(total, t["total"])
                el = time.monotonic() - start
                t["speed"] = done / el if el > 0 else 0.0
            if time.monotonic() - last[0] > 0.2 or stage != "download":
                last[0] = time.monotonic()
                self.on_change(t)

        try:
            rec = self.installer.install(t["entry"], prog, lambda: bool(t["_stop"]))
            t["state"], t["stage"] = "done", ""
            self.on_installed(rec, t["entry"])
        except Cancelled:
            if t["_stop"] == "cancel":
                t["state"] = "cancelled"
                for f in t["entry"]["files"]:
                    (self.installer.part_dir / _part_name(f["url"])).unlink(missing_ok=True)
            else:
                t["state"] = "paused"
        except Exception as exc:  # noqa: BLE001 — shown in the Downloads page
            t["state"], t["error"] = "failed", str(exc) or type(exc).__name__
        finally:
            with self._lock:
                self._running -= 1
            self.on_change(t)
            self._pump()

    def _find(self, tid: str) -> dict | None:
        return next((t for t in self.tasks if t["id"] == tid), None)

    def pause(self, tid: str) -> None:
        t = self._find(tid)
        if t and t["state"] == "running":
            t["_stop"] = "pause"
        elif t and t["state"] == "queued":
            t["state"] = "paused"
            self.on_change(t)

    def resume(self, tid: str) -> None:
        t = self._find(tid)
        if t and t["state"] in ("paused", "failed", "cancelled"):
            t["state"], t["error"] = "queued", ""
            self.on_change(t)
            self._pump()

    def cancel(self, tid: str) -> None:
        t = self._find(tid)
        if t and t["state"] == "running":
            t["_stop"] = "cancel"
        elif t and t["state"] in ("queued", "paused"):
            t["state"] = "cancelled"
            self.on_change(t)

    def clear_finished(self) -> None:
        with self._lock:
            self.tasks = [t for t in self.tasks if t["state"] in ("queued", "running", "paused")]

    def active(self) -> int:
        return sum(1 for t in self.tasks if t["state"] in ("queued", "running"))

    def stop_all(self) -> None:
        for t in self.tasks:
            if t["state"] == "running":
                t["_stop"] = "pause"  # partial files stay: resumed next time
