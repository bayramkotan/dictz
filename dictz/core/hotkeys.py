"""System-wide hotkeys (work while dictz is not focused).

* Windows: ``RegisterHotKey`` (ctypes, no extra package)
* Linux X11 (and X11 apps under XWayland): ``XGrabKey`` through python-xlib
* macOS: pynput when it is installed

Wayland does not let applications grab keys; there, bind a desktop shortcut
to ``dictz show`` / ``dictz popup`` / ``dictz scan toggle`` instead.
"""
from __future__ import annotations

import contextlib
import os
import sys
import threading
from collections.abc import Callable
from dataclasses import dataclass

MODIFIERS = {"ctrl": "ctrl", "control": "ctrl", "alt": "alt", "shift": "shift",
             "super": "super", "win": "super", "meta": "super", "cmd": "super"}

_NAMED_KEYS = {"space": "space", "tab": "tab", "enter": "return", "return": "return", "esc": "escape",
               "escape": "escape", "insert": "insert", "ins": "insert", "delete": "delete", "del": "delete",
               "home": "home", "end": "end", "pageup": "prior", "pgup": "prior", "pagedown": "next",
               "pgdown": "next", "up": "up", "down": "down", "left": "left", "right": "right",
               "pause": "pause", "print": "print", "backspace": "backspace"}


@dataclass(frozen=True)
class Hotkey:
    mods: frozenset[str]  # ctrl, alt, shift, super
    key: str  # "a".."z", "0".."9", "f1".."f24" or a name from _NAMED_KEYS values

    def __str__(self) -> str:
        order = ["ctrl", "alt", "shift", "super"]
        names = {"ctrl": "Ctrl", "alt": "Alt", "shift": "Shift", "super": "Super"}
        display = {"prior": "PgUp", "next": "PgDown", "return": "Enter", "escape": "Esc"}
        if len(self.key) == 1 or self.key.startswith("f"):
            k = self.key.upper()
        else:
            k = display.get(self.key, self.key.capitalize())
        return "+".join([names[m] for m in order if m in self.mods] + [k])


def parse_hotkey(text: str) -> Hotkey:
    """``"Ctrl+Alt+F11"`` → Hotkey. Raises ValueError."""
    parts = [p.strip().lower() for p in text.replace(" ", "").split("+")]
    if not parts or not parts[-1]:
        raise ValueError(f"empty hotkey: {text!r}")
    mods: set[str] = set()
    for p in parts[:-1]:
        if p not in MODIFIERS:
            raise ValueError(f"unknown modifier {p!r} in {text!r}")
        mods.add(MODIFIERS[p])
    key = parts[-1]
    single = len(key) == 1 and key.isascii() and key.isalnum()
    function = key.startswith("f") and key[1:].isdigit() and 1 <= int(key[1:]) <= 24
    if single or function:
        pass
    elif key in _NAMED_KEYS:
        key = _NAMED_KEYS[key]
    else:
        raise ValueError(f"unknown key {parts[-1]!r} in {text!r}")
    if not mods and not key.startswith("f"):
        raise ValueError(f"a hotkey needs a modifier: {text!r}")
    return Hotkey(frozenset(mods), key)


def wayland_session() -> bool:
    return sys.platform.startswith("linux") and (
        os.environ.get("XDG_SESSION_TYPE") == "wayland" or bool(os.environ.get("WAYLAND_DISPLAY"))
    ) and not os.environ.get("DISPLAY")


class HotkeyListener:
    """Registers ``bindings`` (name → hotkey text) and calls ``callback(name)``
    from a background thread when one is pressed."""

    def __init__(self, bindings: dict[str, str], callback: Callable[[str], None]) -> None:
        self.callback = callback
        self.bindings: dict[str, Hotkey] = {}
        self.errors: list[str] = []
        for name, text in bindings.items():
            if not text.strip():
                continue
            try:
                self.bindings[name] = parse_hotkey(text)
            except ValueError as exc:
                self.errors.append(str(exc))
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()
        self._impl: object | None = None

    def start(self) -> bool:
        if not self.bindings:
            return True
        if sys.platform == "win32":
            target = self._run_windows
        elif sys.platform == "darwin":
            target = self._run_pynput
        else:
            if not os.environ.get("DISPLAY"):
                self.errors.append("genel kısayollar Wayland'da desteklenmiyor (DISPLAY yok)")
                return False
            target = self._run_x11
        ready = threading.Event()
        self._thread = threading.Thread(target=target, args=(ready,), name="dictz-hotkeys", daemon=True)
        self._thread.start()
        ready.wait(3.0)
        return not self.errors

    def stop(self) -> None:
        self._stop.set()
        stopper = getattr(self, "_stopper", None)
        if stopper is not None:
            with contextlib.suppress(Exception):
                stopper()
        if self._thread is not None:
            self._thread.join(2.0)

    # ------------------------------------------------------------ Windows
    def _run_windows(self, ready: threading.Event) -> None:
        import ctypes
        from ctypes import wintypes

        user32 = ctypes.windll.user32
        kernel32 = ctypes.windll.kernel32
        mod_bits = {"alt": 0x1, "ctrl": 0x2, "shift": 0x4, "super": 0x8}
        named_vk = {"space": 0x20, "tab": 0x09, "return": 0x0D, "escape": 0x1B, "insert": 0x2D,
                    "delete": 0x2E, "home": 0x24, "end": 0x23, "prior": 0x21, "next": 0x22, "up": 0x26,
                    "down": 0x28, "left": 0x25, "right": 0x27, "pause": 0x13, "print": 0x2C, "backspace": 0x08}
        ids: dict[int, str] = {}
        for i, (name, hk) in enumerate(self.bindings.items(), start=1):
            if len(hk.key) == 1:
                vk = ord(hk.key.upper())
            elif hk.key.startswith("f") and hk.key[1:].isdigit():
                vk = 0x70 + int(hk.key[1:]) - 1
            else:
                vk = named_vk[hk.key]
            mods = sum(mod_bits[m] for m in hk.mods) | 0x4000  # MOD_NOREPEAT
            if user32.RegisterHotKey(None, i, mods, vk):
                ids[i] = name
            else:
                self.errors.append(f"{hk} başka bir uygulama tarafından kullanılıyor")
        thread_id = kernel32.GetCurrentThreadId()
        self._stopper = lambda: user32.PostThreadMessageW(thread_id, 0x0012, 0, 0)  # WM_QUIT
        ready.set()
        msg = wintypes.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            if msg.message == 0x0312 and msg.wParam in ids:  # WM_HOTKEY
                self.callback(ids[msg.wParam])
        for i in ids:
            user32.UnregisterHotKey(None, i)

    # ------------------------------------------------------------ X11
    def _run_x11(self, ready: threading.Event) -> None:
        import select

        try:
            from Xlib import XK, X, display
            from Xlib.error import CatchError
        except ImportError:
            self.errors.append("python-xlib kurulu değil (pip install python-xlib)")
            ready.set()
            return
        try:
            d = display.Display()
        except Exception as exc:  # noqa: BLE001
            self.errors.append(f"X sunucusuna bağlanılamadı: {exc}")
            ready.set()
            return
        root = d.screen().root
        mod_bits = {"shift": X.ShiftMask, "ctrl": X.ControlMask, "alt": X.Mod1Mask, "super": X.Mod4Mask}
        ignored = [0, X.LockMask, X.Mod2Mask, X.LockMask | X.Mod2Mask]  # CapsLock / NumLock
        x11_names = {"space": "space", "prior": "Prior", "next": "Next", "return": "Return", "escape": "Escape",
                     "backspace": "BackSpace", "print": "Print"}
        grabs: dict[tuple[int, int], str] = {}
        catch = CatchError()
        for name, hk in self.bindings.items():
            keyname = hk.key.upper() if hk.key.startswith("f") else x11_names.get(hk.key, hk.key.capitalize())
            if len(hk.key) == 1:
                keyname = hk.key
            keycode = d.keysym_to_keycode(XK.string_to_keysym(keyname))
            if not keycode:
                self.errors.append(f"{hk}: tuş bulunamadı")
                continue
            mask = 0
            for m in hk.mods:
                mask |= mod_bits[m]
            for extra in ignored:
                root.grab_key(keycode, mask | extra, True, X.GrabModeAsync, X.GrabModeAsync, onerror=catch)
            grabs[(keycode, mask)] = name
        d.sync()
        if catch.get_error():
            self.errors.append("bir kısayol başka bir uygulama tarafından kullanılıyor")
        self._stopper = None
        ready.set()
        relevant = X.ShiftMask | X.ControlMask | X.Mod1Mask | X.Mod4Mask
        fd = d.fileno()
        while not self._stop.is_set():
            r, _, _ = select.select([fd], [], [], 0.25)
            if not r:
                continue
            for _ in range(d.pending_events()):
                ev = d.next_event()
                if ev.type == X.KeyPress:
                    name = grabs.get((ev.detail, ev.state & relevant))
                    if name:
                        self.callback(name)
        with contextlib.suppress(Exception):
            for keycode, mask in grabs:
                for extra in ignored:
                    root.ungrab_key(keycode, mask | extra)
            d.close()

    # ------------------------------------------------------------ macOS
    def _run_pynput(self, ready: threading.Event) -> None:
        try:
            from pynput import keyboard
        except Exception:  # noqa: BLE001
            self.errors.append("macOS'ta genel kısayollar için: pip install pynput")
            ready.set()
            return
        names = {"ctrl": "<ctrl>", "alt": "<alt>", "shift": "<shift>", "super": "<cmd>"}
        mapping = {}
        for name, hk in self.bindings.items():
            key = hk.key if len(hk.key) == 1 else f"<{hk.key}>"
            combo = "+".join([names[m] for m in sorted(hk.mods)] + [key])
            mapping[combo] = (lambda n=name: self.callback(n))
        listener = keyboard.GlobalHotKeys(mapping)
        listener.start()
        self._stopper = listener.stop
        ready.set()
        listener.join()
