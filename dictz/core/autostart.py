"""Start dictz when the user logs in (setting ``general.autostart``).

Same places and rules as the Rust edition (``dictz-rs/src/autostart.rs``):

* Linux / BSD: ``$XDG_CONFIG_HOME/autostart/dictz.desktop`` (default ``~/.config``)
* Windows: ``HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`` value ``dictz``
* macOS: ``~/Library/LaunchAgents/com.bayramkotan.dictz.plist`` (``RunAtLoad``)

The command is the program that starts the window plus ``--autostart``: dictz
then starts hidden in the system tray (when a tray icon can be shown) and,
when dictz is already running, does nothing.
"""
from __future__ import annotations

import contextlib
import os
import subprocess
import sys
from pathlib import Path

from .desktop import _exec_quote, desktop_entry, launch_command, png_bytes

FLAG = "--autostart"
RUN_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
VALUE = "dictz"
PLIST_LABEL = "com.bayramkotan.dictz"


def command() -> list[str]:
    program, args = launch_command()
    return [program, *args, FLAG]


def entry_path() -> Path | None:
    """The autostart file (None on Windows: the registry is used)."""
    if sys.platform == "win32":
        return None
    if sys.platform == "darwin":
        return Path.home() / "Library" / "LaunchAgents" / f"{PLIST_LABEL}.plist"
    base = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return base / "autostart" / "dictz.desktop"


def _plist(cmd: list[str]) -> str:
    from xml.sax.saxutils import escape

    args = "".join(f"\n    <string>{escape(a)}</string>" for a in cmd)
    return ('<?xml version="1.0" encoding="UTF-8"?>\n'
            '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
            '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
            '<plist version="1.0">\n<dict>\n'
            f'  <key>Label</key>\n  <string>{PLIST_LABEL}</string>\n'
            f'  <key>ProgramArguments</key>\n  <array>{args}\n  </array>\n'
            '  <key>RunAtLoad</key>\n  <true/>\n</dict>\n</plist>\n')


def _win_cmdline(cmd: list[str]) -> str:
    return subprocess.list2cmdline(cmd)


def is_enabled() -> bool:
    if sys.platform == "win32":
        try:
            import winreg

            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY) as k:
                winreg.QueryValueEx(k, VALUE)
            return True
        except OSError:
            return False
    p = entry_path()
    return p is not None and p.is_file()


def enable(data_dir: Path | None = None) -> None:
    """Write the autostart entry for the program running now (replaced every time,
    so it always points at the current copy)."""
    cmd = command()
    if sys.platform == "win32":
        import winreg

        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, RUN_KEY) as k:
            winreg.SetValueEx(k, VALUE, 0, winreg.REG_SZ, _win_cmdline(cmd))
        return
    p = entry_path()
    assert p is not None
    p.parent.mkdir(parents=True, exist_ok=True)
    if sys.platform == "darwin":
        p.write_text(_plist(cmd), encoding="utf-8")
        return
    icon = (data_dir or p.parent) / "dictz.png"
    with contextlib.suppress(OSError):
        if not icon.is_file():
            icon.parent.mkdir(parents=True, exist_ok=True)
            icon.write_bytes(png_bytes())
    text = desktop_entry(cmd[0], cmd[1:], icon).replace(" %F\n", "\n")
    text += "X-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\n"
    p.write_text(text, encoding="utf-8")


def disable() -> None:
    if sys.platform == "win32":
        try:
            import winreg

            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY, 0, winreg.KEY_SET_VALUE) as k:
                winreg.DeleteValue(k, VALUE)
        except OSError:
            pass
        return
    p = entry_path()
    if p is not None:
        with contextlib.suppress(OSError):
            p.unlink()


def sync(on: bool, data_dir: Path | None = None) -> str:
    """Make the system match the setting; returns "" or the error text."""
    try:
        if on:
            enable(data_dir)
        elif is_enabled():
            disable()
        return ""
    except OSError as exc:
        return str(exc)


__all__ = ["FLAG", "command", "disable", "enable", "entry_path", "is_enabled", "sync", "_exec_quote"]
