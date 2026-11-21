"""Dockable side pane like GoldenDict's: the search box with its word list
first, then Sözlükler, Favoriler, Geçmiş (each with a filter box). The tabs
show icons only; their names are in the tooltips."""
from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QDockWidget,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMenu,
    QPushButton,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from ..core.i18n import t
from . import dicticons
from .icons import icon


class SearchPane(QWidget):
    """The search box and the word list under it (GoldenDict's search pane)."""

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)
        self.box = QLineEdit()
        self.box.setObjectName("search_box")
        self.box.setClearButtonEnabled(True)
        layout.addWidget(self.box)
        self.list = QListWidget(self)
        self.list.setObjectName("word_list")
        self.list.setUniformItemSizes(True)
        layout.addWidget(self.list, 1)


class _ListPane(QWidget):
    item_activated = Signal(str)
    action_triggered = Signal()
    remove_requested = Signal(str)

    def __init__(self, title: str, action_label: str = "", removable: bool = False,
                 parent: QWidget | None = None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)
        header = QHBoxLayout()
        self.title = QLabel(title)
        header.addWidget(self.title)
        header.addStretch(1)
        self.action_btn = None
        if action_label:
            self.action_btn = QPushButton(action_label)
            self.action_btn.setFlat(True)
            self.action_btn.clicked.connect(self.action_triggered)
            header.addWidget(self.action_btn)
        layout.addLayout(header)
        self.filter = QLineEdit()
        self.filter.setPlaceholderText(t("Süz…"))
        self.filter.setClearButtonEnabled(True)
        self.filter.textChanged.connect(self._apply_filter)
        layout.addWidget(self.filter)
        self.list = QListWidget(self)
        self.list.itemActivated.connect(self._emit_activated)
        self.list.itemClicked.connect(self._emit_activated)
        if removable:
            self.list.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
            self.list.customContextMenuRequested.connect(self._context_menu)
        layout.addWidget(self.list, 1)
        self.count_label = QLabel("")
        self.count_label.setObjectName("hint")
        layout.addWidget(self.count_label)

    def set_items(self, items: list[str]) -> None:
        self.list.clear()
        for w in items:
            QListWidgetItem(w, self.list)
        self._apply_filter(self.filter.text())

    def _apply_filter(self, text: str) -> None:
        q = text.strip().lower()
        shown = 0
        for i in range(self.list.count()):
            it = self.list.item(i)
            hide = bool(q) and q not in it.text().lower()
            it.setHidden(hide)
            shown += 0 if hide else 1
        self.count_label.setText(f"{shown} / {self.list.count()}" if q else t("{n} öğe", n=self.list.count()))

    def _emit_activated(self, item: QListWidgetItem) -> None:
        self.item_activated.emit(item.data(Qt.ItemDataRole.UserRole) or item.text())

    def _context_menu(self, pos) -> None:
        it = self.list.itemAt(pos)
        if it is None:
            return
        menu = QMenu(self)
        open_act = menu.addAction(t("Aç"))
        rm_act = menu.addAction(icon("remove"), t("Listeden kaldır"))
        chosen = menu.exec(self.list.mapToGlobal(pos))
        if chosen == open_act:
            self.item_activated.emit(it.text())
        elif chosen == rm_act:
            self.remove_requested.emit(it.text())


class Sidebar(QDockWidget):
    word_selected = Signal(str)
    dictionary_toggled = Signal(str, bool)
    dictionary_activated = Signal(str)
    manage_dictionaries_requested = Signal()
    clear_history_requested = Signal()
    history_remove_requested = Signal(str)
    favorite_remove_requested = Signal(str)
    export_favorites_requested = Signal()

    def __init__(self, parent: QWidget | None = None):
        super().__init__(t("Kenar çubuğu"), parent)
        self.setFeatures(
            QDockWidget.DockWidgetFeature.DockWidgetClosable
            | QDockWidget.DockWidgetFeature.DockWidgetMovable
            | QDockWidget.DockWidgetFeature.DockWidgetFloatable
        )
        self.setAllowedAreas(Qt.DockWidgetArea.LeftDockWidgetArea | Qt.DockWidgetArea.RightDockWidgetArea)
        self.tabs = QTabWidget(self)
        self.tabs.setObjectName("side_tabs")
        self.tabs.setDocumentMode(True)
        self.tabs.tabBar().setExpanding(True)
        self.tabs.tabBar().setUsesScrollButtons(False)
        self.search_pane = SearchPane()

        self.dicts_pane = _ListPane(t("Sözlükler"), action_label=t("Yönet…"))
        self.dicts_pane.action_triggered.connect(self.manage_dictionaries_requested)
        self.dicts_pane.list.itemChanged.connect(self._on_dict_item_changed)
        self.dicts_pane.list.itemActivated.disconnect()
        self.dicts_pane.list.itemClicked.disconnect()
        self.dicts_pane.list.itemDoubleClicked.connect(
            lambda it: self.dictionary_activated.emit(it.data(Qt.ItemDataRole.UserRole)))

        self.favorites_pane = _ListPane(t("Favoriler"), action_label=t("Dışa aktar…"), removable=True)
        self.favorites_pane.item_activated.connect(self.word_selected)
        self.favorites_pane.remove_requested.connect(self.favorite_remove_requested)
        self.favorites_pane.action_triggered.connect(self.export_favorites_requested)

        self.history_pane = _ListPane(t("Geçmiş"), action_label=t("Temizle"), removable=True)
        self.history_pane.action_triggered.connect(self.clear_history_requested)
        self.history_pane.item_activated.connect(self.word_selected)
        self.history_pane.remove_requested.connect(self.history_remove_requested)

        self._tab_icons = ["search", "dict", "star", "history"]
        for pane, name, tip in ((self.search_pane, "search", t("Ara")), (self.dicts_pane, "dict", t("Sözlükler")),
                                (self.favorites_pane, "star", t("Favoriler")),
                                (self.history_pane, "history", t("Geçmiş"))):
            i = self.tabs.addTab(pane, icon(name), "")
            self.tabs.setTabToolTip(i, tip)
        self.setWidget(self.tabs)
        self.setTitleBarWidget(QWidget())  # no title row: the icon tabs are the header

    def show_search(self) -> None:
        self.show()
        self.tabs.setCurrentWidget(self.search_pane)

    def restyle_icons(self) -> None:
        for i, name in enumerate(self._tab_icons):
            self.tabs.setTabIcon(i, icon(name))

    def set_dictionaries(self, items: list[tuple]) -> None:
        """``(name, enabled, words[, error])``; a dictionary with an error is
        one of the list that could not be opened: shown with ⚠, never hidden."""
        from PySide6.QtGui import QBrush, QColor

        lst = self.dicts_pane.list
        lst.blockSignals(True)
        lst.clear()
        for item in items:
            name, enabled = item[0], item[1]
            error = item[3] if len(item) > 3 else ""
            if error:
                it = QListWidgetItem(f"⚠ {name}")
                it.setForeground(QBrush(QColor("#dc2626")))
                it.setToolTip(t("açılamadı") + f": {error}")
            else:
                it = QListWidgetItem(dicticons.qicon(name), name)
                it.setToolTip(dicticons.look(name).tooltip() + "\n\n" + t("Çift tık: bilgiler"))
            it.setData(Qt.ItemDataRole.UserRole, name)
            it.setFlags(it.flags() | Qt.ItemFlag.ItemIsUserCheckable)
            it.setCheckState(Qt.CheckState.Checked if enabled else Qt.CheckState.Unchecked)
            lst.addItem(it)
        lst.blockSignals(False)
        self.dicts_pane._apply_filter(self.dicts_pane.filter.text())

    def _on_dict_item_changed(self, item: QListWidgetItem) -> None:
        self.dictionary_toggled.emit(item.data(Qt.ItemDataRole.UserRole),
                                     item.checkState() == Qt.CheckState.Checked)

    def set_favorites(self, words: list[str]) -> None:
        self.favorites_pane.set_items(words)

    def set_history(self, words: list[str]) -> None:
        self.history_pane.set_items(words)
