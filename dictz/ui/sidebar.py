"""Dockable left sidebar with three tabs: Sözlükler, Favoriler, Geçmiş."""
from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QDockWidget,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from .icons import icon


class _ListPane(QWidget):
    item_activated = Signal(str)
    action_triggered = Signal()

    def __init__(self, title: str, action_label: str = "", parent: QWidget | None = None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.setSpacing(6)
        if action_label:
            header = QHBoxLayout()
            header.addWidget(QLabel(title))
            header.addStretch(1)
            btn = QPushButton(action_label)
            btn.setFlat(True)
            btn.clicked.connect(self.action_triggered)
            header.addWidget(btn)
            layout.addLayout(header)
        self.list = QListWidget(self)
        self.list.itemActivated.connect(self._emit_activated)
        self.list.itemClicked.connect(self._emit_activated)
        layout.addWidget(self.list, 1)

    def set_items(self, items: list[str]) -> None:
        self.list.clear()
        for w in items:
            QListWidgetItem(w, self.list)

    def _emit_activated(self, item: QListWidgetItem) -> None:
        self.item_activated.emit(item.text())


class Sidebar(QDockWidget):
    word_selected = Signal(str)
    dictionary_toggled = Signal(str, bool)
    manage_dictionaries_requested = Signal()
    clear_history_requested = Signal()

    def __init__(self, parent: QWidget | None = None):
        super().__init__("", parent)
        self.setFeatures(
            QDockWidget.DockWidgetFeature.DockWidgetClosable
            | QDockWidget.DockWidgetFeature.DockWidgetMovable
            | QDockWidget.DockWidgetFeature.DockWidgetFloatable
        )
        self.setAllowedAreas(
            Qt.DockWidgetArea.LeftDockWidgetArea | Qt.DockWidgetArea.RightDockWidgetArea
        )
        self.tabs = QTabWidget(self)
        self.tabs.setDocumentMode(True)

        self.dicts_pane = _ListPane("Sözlükler", action_label="Yönet…")
        self.dicts_pane.action_triggered.connect(self.manage_dictionaries_requested)
        self.dicts_pane.list.itemChanged.connect(self._on_dict_item_changed)
        self.dicts_pane.list.itemActivated.disconnect()
        self.dicts_pane.list.itemClicked.disconnect()

        self.favorites_pane = _ListPane("Favoriler")
        self.favorites_pane.item_activated.connect(self.word_selected)

        self.history_pane = _ListPane("Geçmiş", action_label="Temizle")
        self.history_pane.action_triggered.connect(self.clear_history_requested)
        self.history_pane.item_activated.connect(self.word_selected)

        self.tabs.addTab(self.dicts_pane, icon("dict"), "Sözlükler")
        self.tabs.addTab(self.favorites_pane, icon("star"), "Favoriler")
        self.tabs.addTab(self.history_pane, icon("history"), "Geçmiş")
        self.setWidget(self.tabs)

    def set_dictionaries(self, items: list[tuple[str, bool]]) -> None:
        self.dicts_pane.list.blockSignals(True)
        self.dicts_pane.list.clear()
        for name, enabled in items:
            it = QListWidgetItem(name)
            it.setFlags(it.flags() | Qt.ItemFlag.ItemIsUserCheckable)
            it.setCheckState(Qt.CheckState.Checked if enabled else Qt.CheckState.Unchecked)
            self.dicts_pane.list.addItem(it)
        self.dicts_pane.list.blockSignals(False)

    def _on_dict_item_changed(self, item: QListWidgetItem) -> None:
        self.dictionary_toggled.emit(item.text(), item.checkState() == Qt.CheckState.Checked)

    def set_favorites(self, words: list[str]) -> None:
        self.favorites_pane.set_items(words)

    def set_history(self, words: list[str]) -> None:
        self.history_pane.set_items(words)
