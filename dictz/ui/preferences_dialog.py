"""Preferences dialog."""
from __future__ import annotations

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QComboBox, QDialog, QDialogButtonBox, QFontComboBox,
    QFormLayout, QSpinBox, QVBoxLayout, QWidget,
)

from ..storage.settings import Preferences


class PreferencesDialog(QDialog):
    preferences_changed = Signal(object)

    def __init__(self, prefs: Preferences, parent: QWidget | None = None):
        super().__init__(parent)
        self.setWindowTitle("Tercihler")
        self.resize(420, 300)
        self._prefs = prefs
        root = QVBoxLayout(self)
        form = QFormLayout()
        form.setSpacing(10)

        self.theme_combo = QComboBox()
        self.theme_combo.addItem("Otomatik", "auto")
        self.theme_combo.addItem("Açık", "light")
        self.theme_combo.addItem("Koyu", "dark")
        idx = self.theme_combo.findData(prefs.theme)
        self.theme_combo.setCurrentIndex(max(0, idx))
        form.addRow("Tema", self.theme_combo)

        self.font_combo = QFontComboBox()
        if prefs.font_family:
            self.font_combo.setCurrentText(prefs.font_family)
        form.addRow("Yazı tipi", self.font_combo)

        self.font_size = QSpinBox()
        self.font_size.setRange(8, 32)
        self.font_size.setValue(prefs.font_size)
        self.font_size.setSuffix(" px")
        form.addRow("Yazı boyutu", self.font_size)

        self.debounce = QSpinBox()
        self.debounce.setRange(0, 1000)
        self.debounce.setValue(prefs.search_debounce_ms)
        self.debounce.setSingleStep(25)
        self.debounce.setSuffix(" ms")
        form.addRow("Arama gecikmesi", self.debounce)

        root.addLayout(form)
        root.addStretch(1)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(self._apply_and_accept)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)

    def _apply_and_accept(self) -> None:
        self._prefs.theme = self.theme_combo.currentData()
        self._prefs.font_family = self.font_combo.currentText()
        self._prefs.font_size = self.font_size.value()
        self._prefs.search_debounce_ms = self.debounce.value()
        self._prefs.save()
        self.preferences_changed.emit(self._prefs)
        self.accept()
