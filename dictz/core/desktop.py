"""Desktop and menu shortcuts (`dictz shortcut`, File → Create desktop shortcut).

Same behaviour as the Rust edition (``dictz-rs/src/desktop.rs``):

* Linux: ``~/.local/share/applications/dictz.desktop`` (menu) and a copy on the
  desktop (``xdg-user-dir DESKTOP``), icon ``dictz.png`` in the data folder
* Windows: ``dictz.lnk`` on the desktop and in the Start menu (created through
  PowerShell / WScript.Shell), icon ``dictz.ico``
* macOS: ``~/Applications/dictz.app`` (a small launcher bundle) and a link to
  it on the desktop

Existing shortcuts are replaced. Nothing here imports Qt.
"""
from __future__ import annotations

import contextlib
import os
import shutil
import struct
import subprocess
import sys
import zlib
from pathlib import Path

from .icon import app_icon_rgba

APP_ID = "dictz"


# ------------------------------------------------------------------ icon files

def png_bytes(n: int = 64) -> bytes:
    """The dictz icon as a PNG file."""
    rgba = app_icon_rgba(n)
    raw = b"".join(b"\x00" + rgba[y * n * 4:(y + 1) * n * 4] for y in range(n))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def ico_bytes(n: int = 64) -> bytes:
    """Windows .ico holding one PNG image."""
    png = png_bytes(n)
    return struct.pack("<HHH", 0, 1, 1) + struct.pack("<BBBBHHII", n, n, 0, 0, 1, 32, len(png), 22) + png


def icns_bytes(n: int = 64) -> bytes:
    """macOS .icns holding one PNG image (``icp6`` = 64×64)."""
    png = png_bytes(n)
    entry = b"icp6" + struct.pack(">I", len(png) + 8) + png
    return b"icns" + struct.pack(">I", len(entry) + 8) + entry


# ------------------------------------------------------------------ what to run

def launch_command() -> tuple[str, list[str]]:
    """(program, arguments) that start the dictz window."""
    if os.environ.get("APPIMAGE"):
        return os.environ["APPIMAGE"], []
    if getattr(sys, "frozen", False):
        return sys.executable, []
    if sys.platform == "win32":
        pyw = Path(sys.executable).with_name("pythonw.exe")
        return (str(pyw) if pyw.exists() else sys.executable), ["-m", "dictz"]
    found = shutil.which("dictz")
    if found:
        return found, []
    return sys.executable, ["-m", "dictz"]


# ------------------------------------------------------------------ helpers

def _desktop_dir() -> Path | None:
    if sys.platform.startswith("linux"):
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            out = subprocess.run(["xdg-user-dir", "DESKTOP"], capture_output=True, text=True, timeout=5)
            p = Path(out.stdout.strip())
            if out.returncode == 0 and str(p) not in ("", str(Path.home())) and p.is_dir():
                return p
    for name in ("Desktop", "Masaüstü"):
        p = Path.home() / name
        if p.is_dir():
            return p
    return None


def _exec_quote(s: str) -> str:
    if any(c in s for c in ' \t"\\$`'):
        return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$").replace("`", "\\`") + '"'
    return s


def desktop_entry(program: str, args: list[str], icon: Path) -> str:
    exec_line = " ".join(_exec_quote(a) for a in [program, *args]) + " %F"
    return "\n".join([
        "[Desktop Entry]",
        "Type=Application",
        "Version=1.0",
        "Name=dictz",
        "GenericName=Dictionary",
        "GenericName[tr]=Sözlük",
        "Comment=Offline dictionary (StarDict, DSL, Babylon)",
        "Comment[tr]=Çevrimdışı sözlük (StarDict, DSL, Babylon)",
        f"Exec={exec_line}",
        f"Icon={icon}",
        "Terminal=false",
        "Categories=Office;Dictionary;Education;",
        "Keywords=dictionary;sözlük;stardict;dsl;babylon;goldendict;",
        "StartupWMClass=dictz",
        "",
    ])


def _write(path: Path, data: bytes | str, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink() or path.exists():
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink()
    if isinstance(data, str):
        path.write_text(data, encoding="utf-8")
    else:
        path.write_bytes(data)
    if mode is not None:
        os.chmod(path, mode)


# ------------------------------------------------------------------ create / remove

def shortcut_paths(desktop: bool = True, menu: bool = True) -> list[Path]:
    """Where the shortcuts go on this system."""
    out: list[Path] = []
    if sys.platform == "win32":
        return [Path(p) for p in _windows_folders(desktop, menu)]
    if sys.platform == "darwin":
        if menu:
            out.append(Path.home() / "Applications" / "dictz.app")
        d = _desktop_dir()
        if desktop and d:
            out.append(d / "dictz.app")
        return out
    if menu:
        data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
        out.append(data / "applications" / "dictz.desktop")
    d = _desktop_dir()
    if desktop and d:
        out.append(d / "dictz.desktop")
    return out


def _windows_folders(desktop: bool, menu: bool) -> list[str]:
    kinds = (["Desktop"] if desktop else []) + (["Programs"] if menu else [])
    if not kinds:
        return []
    script = "; ".join(f"[Environment]::GetFolderPath('{k}')" for k in kinds)
    try:
        out = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
                             capture_output=True, text=True, timeout=30,
                             creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    except (OSError, subprocess.SubprocessError):
        return []
    return [str(Path(line.strip()) / "dictz.lnk") for line in out.stdout.splitlines() if line.strip()]


def create_shortcuts(data_dir: Path, desktop: bool = True, menu: bool = True) -> list[Path]:
    """Create (replace) the shortcuts; returns the files written."""
    program, args = launch_command()
    data_dir.mkdir(parents=True, exist_ok=True)
    made: list[Path] = []
    if sys.platform == "win32":
        ico = data_dir / "dictz.ico"
        _write(ico, ico_bytes())
        targets = _windows_folders(desktop, menu)
        if not targets:
            raise RuntimeError("Masaüstü / Başlat menüsü klasörü bulunamadı (PowerShell)")
        env = dict(os.environ, DICTZ_SC_TARGET=program, DICTZ_SC_ARGS=subprocess.list2cmdline(args),
                   DICTZ_SC_ICON=str(ico), DICTZ_SC_FILES="|".join(targets), DICTZ_SC_HOME=str(Path.home()))
        script = ("$w = New-Object -ComObject WScript.Shell; "
                  "foreach ($f in $env:DICTZ_SC_FILES.Split('|')) { "
                  "if (Test-Path $f) { Remove-Item -Force $f }; "
                  "$s = $w.CreateShortcut($f); $s.TargetPath = $env:DICTZ_SC_TARGET; "
                  "$s.Arguments = $env:DICTZ_SC_ARGS; $s.IconLocation = $env:DICTZ_SC_ICON; "
                  "$s.WorkingDirectory = $env:DICTZ_SC_HOME; "
                  "$s.Description = 'dictz - offline dictionary'; $s.Save() }")
        r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script], env=env,
                           capture_output=True, text=True, timeout=60,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        if r.returncode != 0:
            raise RuntimeError(r.stderr.strip() or "PowerShell failed")
        return [Path(t) for t in targets if Path(t).exists()]
    if sys.platform == "darwin":
        paths = shortcut_paths(desktop, menu)
        bundle = Path.home() / "Applications" / "dictz.app"
        cmd = " ".join(_exec_quote(a) for a in [program, *args])
        _write(bundle / "Contents" / "MacOS" / "dictz", f'#!/bin/sh\nexec {cmd} "$@"\n', 0o755)
        _write(bundle / "Contents" / "Resources" / "dictz.icns", icns_bytes())
        _write(bundle / "Contents" / "Info.plist", _info_plist())
        made.append(bundle)
        for p in paths:
            if p != bundle:
                _write_link(p, bundle)
                made.append(p)
        return made
    icon = data_dir / "dictz.png"
    _write(icon, png_bytes())
    text = desktop_entry(program, args, icon)
    for p in shortcut_paths(desktop, menu):
        _write(p, text, 0o755)
        made.append(p)
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            subprocess.run(["gio", "set", str(p), "metadata::trusted", "true"], capture_output=True, timeout=5)
    with contextlib.suppress(OSError, subprocess.SubprocessError):
        apps = next((p.parent for p in made if p.parent.name == "applications"), None)
        if apps:
            subprocess.run(["update-desktop-database", str(apps)], capture_output=True, timeout=10)
    return made


def _write_link(link: Path, target: Path) -> None:
    if link.is_symlink() or link.exists():
        if link.is_dir() and not link.is_symlink():
            shutil.rmtree(link)
        else:
            link.unlink()
    link.symlink_to(target)


def _info_plist() -> str:
    from .. import __version__

    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>dictz</string>
  <key>CFBundleDisplayName</key><string>dictz</string>
  <key>CFBundleIdentifier</key><string>com.bayramkotan.dictz</string>
  <key>CFBundleExecutable</key><string>dictz</string>
  <key>CFBundleIconFile</key><string>dictz.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{__version__}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.reference</string>
</dict>
</plist>
"""


def remove_shortcuts(desktop: bool = True, menu: bool = True) -> list[Path]:
    removed: list[Path] = []
    for p in shortcut_paths(desktop, menu):
        if p.is_symlink() or p.exists():
            if p.is_dir() and not p.is_symlink():
                shutil.rmtree(p)
            else:
                p.unlink()
            removed.append(p)
    return removed
