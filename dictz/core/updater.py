"""``dictz update`` — find out how this copy was installed and update it there.

Every copy is updated where it is: a pipx install with pipx, a venv with that
venv's pip, a per-user install with ``--user``, a system install with the
system pip (via sudo when the folder is not writable), the AppImage by
replacing the file, other portable builds by downloading the new archive.
Nothing is moved or removed.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import sysconfig
import tempfile
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

from .. import __version__

PYPI_URL = "https://pypi.org/pypi/dictz/json"
RELEASES_URL = "https://api.github.com/repos/bayramkotan/dictz/releases/latest"
USER_AGENT = f"dictz-updater/{__version__} (+https://github.com/bayramkotan/dictz)"


def parse_version(v: str) -> tuple[int, ...]:
    parts = []
    for p in v.strip().lstrip("v").split("."):
        num = "".join(ch for ch in p if ch.isdigit())
        parts.append(int(num) if num else 0)
    return tuple(parts)


def _get_json(url: str, timeout: float = 15.0) -> dict:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:  # noqa: S310 — fixed https URLs
        return json.loads(r.read().decode("utf-8"))


def latest_version() -> str:
    """Newest dictz on PyPI."""
    return str(_get_json(PYPI_URL)["info"]["version"])


@dataclass
class Install:
    kind: str  # appimage | portable | pipx | venv | editable | user | system
    detail: str
    command: list[str] = field(default_factory=list)
    needs_sudo: bool = False
    location: str = ""


def _dist_location() -> Path | None:
    try:
        from importlib.metadata import distribution

        dist = distribution("dictz")
        return Path(str(dist.locate_file(""))).resolve()
    except Exception:  # noqa: BLE001
        return None


def _is_editable() -> bool:
    try:
        from importlib.metadata import distribution

        text = distribution("dictz").read_text("direct_url.json") or "{}"
        return bool(json.loads(text).get("dir_info", {}).get("editable"))
    except Exception:  # noqa: BLE001
        return False


def detect_install() -> Install:
    if getattr(sys, "frozen", False):
        if os.environ.get("APPIMAGE"):
            return Install("appimage", "Linux AppImage", location=os.environ["APPIMAGE"])
        return Install("portable", "portable build (PyInstaller)", location=str(Path(sys.executable).parent))
    loc = _dist_location()
    location = str(loc) if loc else str(Path(__file__).resolve().parent.parent)
    if _is_editable():
        return Install("editable", "development install (pip -e)", location=location)
    prefix = Path(sys.prefix).resolve()
    pipx_home = os.environ.get("PIPX_HOME", "")
    if "pipx" in prefix.parts or (pipx_home and str(prefix).startswith(pipx_home)):
        return Install("pipx", "pipx", ["pipx", "upgrade", "dictz"], location=str(prefix))
    in_venv = sys.prefix != getattr(sys, "base_prefix", sys.prefix)
    pip = [sys.executable, "-m", "pip", "install", "--upgrade", "--no-cache-dir", "dictz"]
    if in_venv:
        return Install("venv", f"virtual environment {prefix}", pip, location=location)
    import site

    user_site = Path(site.getusersitepackages()).resolve() if site.ENABLE_USER_SITE else None
    managed = (Path(sysconfig.get_path("stdlib")) / "EXTERNALLY-MANAGED").exists()
    extra = ["--break-system-packages"] if managed else []
    if loc is not None and user_site is not None and str(loc).startswith(str(user_site)):
        return Install("user", "per-user pip install", pip + ["--user"] + extra, location=location)
    writable = loc is not None and os.access(loc, os.W_OK)
    needs_sudo = not writable and os.name != "nt"
    cmd = (["sudo"] if needs_sudo else []) + pip + extra
    return Install("system", "system-wide pip install", cmd, needs_sudo=needs_sudo, location=location)


def release_asset_name() -> str | None:
    if sys.platform.startswith("linux"):
        return "dictz-linux-x86_64.AppImage" if os.environ.get("APPIMAGE") else "dictz-linux-x86_64.tar.gz"
    if sys.platform == "win32":
        return "dictz-windows-x86_64.zip"
    if sys.platform == "darwin":
        return "dictz-macos-arm64.zip"
    return None


def _download(url: str, dest: Path) -> None:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=60) as r, open(dest, "wb") as f:  # noqa: S310
        shutil.copyfileobj(r, f)


def _downloads_dir() -> Path:
    d = Path.home() / "Downloads"
    return d if d.is_dir() else Path.home()


def run_update(check_only: bool = False, assume_yes: bool = False, out=print) -> int:
    """Returns a process exit code."""
    inst = detect_install()
    out(f"dictz {__version__} (Python edition) — {inst.detail}")
    if inst.location:
        out(f"  location: {inst.location}")
    try:
        latest = latest_version()
    except Exception as exc:  # noqa: BLE001
        out(f"Could not reach PyPI: {exc}")
        return 2
    out(f"  latest:   {latest}")
    if parse_version(latest) <= parse_version(__version__):
        out("Already up to date.")
        return 0
    if inst.kind == "editable":
        out(f"Development install: update it with `git pull` in {inst.location}.")
        return 0
    if inst.command:
        out("Update command: " + " ".join(inst.command))
    if check_only:
        out(f"Update available: {__version__} -> {latest}")
        return 0
    if not assume_yes:
        if not sys.stdin or not sys.stdin.isatty():
            out("Not a terminal; re-run with --yes to update.")
            return 1
        if input(f"Update dictz {__version__} -> {latest}? [y/N] ").strip().lower() not in ("y", "yes", "e", "evet"):
            out("Cancelled.")
            return 1
    if inst.kind in ("pipx", "venv", "user", "system"):
        if inst.kind == "pipx" and shutil.which("pipx") is None:
            out("pipx is not on PATH.")
            return 1
        if inst.needs_sudo and shutil.which("sudo") is None:
            out("This copy is installed system-wide and the folder is not writable; run as administrator:")
            out("  " + " ".join(inst.command[1:]))
            return 1
        rc = subprocess.call(inst.command)
        out("Updated." if rc == 0 else f"Update failed (exit code {rc}).")
        return rc
    # portable builds: GitHub release assets
    asset = release_asset_name()
    try:
        rel = _get_json(RELEASES_URL)
        url = next(a["browser_download_url"] for a in rel.get("assets", []) if a.get("name") == asset)
    except StopIteration:
        out(f"Release {latest} has no {asset}.")
        return 1
    except Exception as exc:  # noqa: BLE001
        out(f"Could not reach GitHub: {exc}")
        return 2
    if inst.kind == "appimage":
        target = Path(inst.location)
        fd, tmp = tempfile.mkstemp(prefix=".dictz-update-", dir=str(target.parent))
        os.close(fd)
        out(f"Downloading {asset} …")
        _download(url, Path(tmp))
        os.chmod(tmp, 0o755)
        os.replace(tmp, target)
        out(f"Updated {target}. Restart dictz.")
        return 0
    dest = _downloads_dir() / asset
    out(f"Downloading {asset} …")
    _download(url, dest)
    out(f"Saved {dest}. Extract it over {inst.location} (close dictz first).")
    return 0
