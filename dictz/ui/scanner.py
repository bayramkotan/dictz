"""Watches the clipboard (and the X11 / Wayland selection) for the scan popup."""
from __future__ import annotations

import time

from PySide6.QtCore import QObject, QTimer, Signal
from PySide6.QtGui import QClipboard, QGuiApplication
from PySide6.QtWidgets import QApplication

from ..core.scantext import DoubleCopy, clean_scan_text
from ..storage.settings import Preferences


class ClipboardScanner(QObject):
    """Emits ``word_found`` according to the ``scan.*`` settings."""

    word_found = Signal(str)

    def __init__(self, prefs: Preferences, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self.prefs = prefs
        self.enabled = False
        self._double = DoubleCopy(int(prefs["scan.double_copy_ms"]))
        self._ignore_until = 0.0
        self._selection_timer = QTimer(self)
        self._selection_timer.setSingleShot(True)
        self._selection_timer.timeout.connect(self._selection_settled)
        cb = QGuiApplication.clipboard()
        cb.dataChanged.connect(self._clipboard_changed)
        if cb.supportsSelection():
            cb.selectionChanged.connect(lambda: self._selection_timer.start(350))

    @staticmethod
    def selection_supported() -> bool:
        return QGuiApplication.clipboard().supportsSelection()

    def set_enabled(self, on: bool) -> None:
        self.enabled = on
        self._double = DoubleCopy(int(self.prefs["scan.double_copy_ms"]))

    def ignore_for(self, seconds: float) -> None:
        """Ignore clipboard changes we cause ourselves (copy word / article)."""
        self._ignore_until = time.monotonic() + seconds

    def _own_window_active(self) -> bool:
        return QApplication.activeWindow() is not None

    def _clipboard_changed(self) -> None:
        if not self.enabled or time.monotonic() < self._ignore_until or self._own_window_active():
            return
        text = QGuiApplication.clipboard().text(QClipboard.Mode.Clipboard)
        word = clean_scan_text(text, int(self.prefs["scan.max_length"]))
        if word is None:
            return
        self._double.interval_ms = int(self.prefs["scan.double_copy_ms"])
        double = self._double.feed(word, time.monotonic() * 1000)
        if self.prefs["scan.clipboard"] or (self.prefs["scan.ctrl_c_c"] and double):
            self.word_found.emit(word)

    def _selection_settled(self) -> None:
        if not self.enabled or not self.prefs["scan.selection"] or self._own_window_active():
            return
        text = QGuiApplication.clipboard().text(QClipboard.Mode.Selection)
        word = clean_scan_text(text, int(self.prefs["scan.max_length"]))
        if word is not None:
            self.word_found.emit(word)

    def clipboard_word(self) -> str | None:
        text = QGuiApplication.clipboard().text(QClipboard.Mode.Clipboard)
        return clean_scan_text(text, max(int(self.prefs["scan.max_length"]), 200))
