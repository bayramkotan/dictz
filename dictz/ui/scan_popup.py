"""Scan popup: a small always-on-top window that shows the articles of the
word copied / selected in another application (GoldenDict's scan popup)."""
from __future__ import annotations

import time

from PySide6.QtCore import QPoint, QSize, Qt, QTimer, Signal
from PySide6.QtGui import QCursor, QGuiApplication, QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from ..storage.settings import Preferences
from .definition_view import DefinitionView


class ScanPopup(QWidget):
    """Shows a word; the main window does the lookups (``lookup_requested``)."""

    lookup_requested = Signal(str)
    open_in_main = Signal(str)
    link_activated = Signal(str)

    def __init__(self, prefs: Preferences, parent: QWidget | None = None) -> None:
        super().__init__(parent, Qt.WindowType.Tool)
        self.prefs = prefs
        self.setWindowTitle("dictz — tarama")
        self.setObjectName("scan_popup")
        self.word = ""

        bar = QHBoxLayout()
        bar.setContentsMargins(6, 6, 6, 2)
        self.word_box = QLineEdit(self)
        self.word_box.setPlaceholderText("Kelime…")
        self.word_box.returnPressed.connect(lambda: self.lookup_requested.emit(self.word_box.text().strip()))
        self.pin = QToolButton(self)
        self.pin.setText("📌")
        self.pin.setCheckable(True)
        self.pin.setToolTip("Sabitle (fare çıkınca kapanmaz)")
        self.open_main = QToolButton(self)
        self.open_main.setText("⤢")
        self.open_main.setToolTip("Ana pencerede aç")
        self.open_main.clicked.connect(lambda: self.open_in_main.emit(self.word))
        self.close_btn = QToolButton(self)
        self.close_btn.setText("✕")
        self.close_btn.setToolTip("Kapat (Esc)")
        self.close_btn.clicked.connect(self.hide)
        bar.addWidget(self.word_box, 1)
        bar.addWidget(self.pin)
        bar.addWidget(self.open_main)
        bar.addWidget(self.close_btn)

        self.info = QLabel(self)
        self.info.setObjectName("scan_info")
        self.info.setContentsMargins(8, 0, 8, 0)

        self.view = DefinitionView(prefs, self)
        self.view.link_activated.connect(self.link_activated)

        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(2)
        lay.addLayout(bar)
        lay.addWidget(self.info)
        lay.addWidget(self.view, 1)

        self._hide_timer = QTimer(self)
        self._hide_timer.setSingleShot(True)
        self._hide_timer.timeout.connect(self._auto_hide)
        self._shown_at = 0.0
        QShortcut(QKeySequence(Qt.Key.Key_Escape), self, activated=self.hide)
        self.apply_prefs()

    def apply_prefs(self) -> None:
        flags = Qt.WindowType.Tool
        if self.prefs["scan.always_on_top"]:
            flags |= Qt.WindowType.WindowStaysOnTopHint
        visible = self.isVisible()
        self.setWindowFlags(flags)
        if visible:
            self.show()
        self.resize(QSize(int(self.prefs["scan.width"]), int(self.prefs["scan.height"])))
        self.view.set_zoom(float(self.prefs["article.zoom"]))
        self.view.refresh_style()

    # ------------------------------------------------------------ showing
    def show_result(self, word: str, entries, fuzzy, near_cursor: bool) -> None:
        self.word = word
        self.word_box.setText(word)
        n = len({e.dictionary_id for e in entries})
        self.info.setText(f"{len(entries)} madde · {n} sözlük" if entries else "")
        if entries:
            self.view.show_entries(word, entries)
        else:
            self.view.show_not_found(word, fuzzy)
        if not self.isVisible():
            if near_cursor:
                self._move_near_cursor()
            self.show()
        self.raise_()
        self._shown_at = time.monotonic()
        self._hide_timer.stop()

    def _move_near_cursor(self) -> None:
        pos = QCursor.pos()
        screen = QGuiApplication.screenAt(pos) or QGuiApplication.primaryScreen()
        geo = screen.availableGeometry()
        w, h = self.width(), self.height()
        x = pos.x() + 16
        y = pos.y() + 20
        if x + w > geo.right():
            x = max(geo.left(), pos.x() - w - 16)
        if y + h > geo.bottom():
            y = max(geo.top(), pos.y() - h - 20)
        self.move(QPoint(x, y))

    # ------------------------------------------------------------ auto-hide
    def enterEvent(self, event):  # noqa: N802
        self._hide_timer.stop()
        super().enterEvent(event)

    def leaveEvent(self, event):  # noqa: N802
        if self.prefs["scan.auto_hide"] and not self.pin.isChecked() and not self.word_box.hasFocus():
            self._hide_timer.start(int(self.prefs["scan.hide_delay_ms"]))
        super().leaveEvent(event)

    def _auto_hide(self) -> None:
        if self.pin.isChecked() or self.underMouse():
            return
        self.hide()

    def hideEvent(self, event):  # noqa: N802
        size = self.size()
        if (size.width(), size.height()) != (int(self.prefs["scan.width"]), int(self.prefs["scan.height"])):
            self.prefs.set("scan.width", size.width())
            self.prefs.set("scan.height", size.height())
        super().hideEvent(event)
