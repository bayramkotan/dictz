"""Tabbed definition area."""
from __future__ import annotations

from PySide6.QtCore import Signal
from PySide6.QtWidgets import QTabWidget, QWidget

from ..parsers.base import DictionaryEntry
from .definition_view import DefinitionView


class DefinitionTabs(QTabWidget):
    current_word_changed = Signal(str)
    link_activated = Signal(str)

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setMovable(True)
        self.setTabsClosable(True)
        self.setDocumentMode(True)
        self.tabCloseRequested.connect(self._on_tab_close_requested)
        self.currentChanged.connect(self._on_current_changed)
        self._dark = False
        self._zoom = 1.0

    def set_dark(self, dark: bool) -> None:
        self._dark = dark
        for i in range(self.count()):
            view = self.widget(i)
            if isinstance(view, DefinitionView):
                view.set_dark(dark)

    def set_zoom(self, factor: float) -> None:
        self._zoom = factor
        for i in range(self.count()):
            view = self.widget(i)
            if isinstance(view, DefinitionView):
                view.set_zoom(factor)

    def current_view(self) -> DefinitionView | None:
        w = self.currentWidget()
        return w if isinstance(w, DefinitionView) else None

    def current_word(self) -> str:
        idx = self.currentIndex()
        if idx < 0:
            return ""
        return self.tabText(idx)

    def new_tab(self, word: str = "") -> DefinitionView:
        view = DefinitionView(self)
        view.set_dark(self._dark)
        view.set_zoom(self._zoom)
        view.link_activated.connect(self.link_activated)
        label = word or "Yeni sekme"
        self.addTab(view, label)
        self.setCurrentWidget(view)
        if not word:
            view.show_welcome()
        return view

    def show_in_current(self, word: str, entries: list[DictionaryEntry]) -> None:
        view = self.current_view()
        if view is None:
            view = self.new_tab(word)
        view.show_entries(entries)
        idx = self.currentIndex()
        if idx >= 0:
            self.setTabText(idx, word or "Sonuç yok")

    def show_in_new_tab(self, word: str, entries: list[DictionaryEntry]) -> None:
        view = self.new_tab(word)
        view.show_entries(entries)

    def close_current_tab(self) -> None:
        idx = self.currentIndex()
        if idx >= 0:
            self._on_tab_close_requested(idx)

    def _on_tab_close_requested(self, index: int) -> None:
        if self.count() <= 1:
            view = self.widget(index)
            if isinstance(view, DefinitionView):
                view.show_welcome()
            self.setTabText(index, "Yeni sekme")
            self.current_word_changed.emit("")
            return
        widget = self.widget(index)
        self.removeTab(index)
        if widget is not None:
            widget.deleteLater()

    def _on_current_changed(self, index: int) -> None:
        if index < 0:
            self.current_word_changed.emit("")
            return
        self.current_word_changed.emit(self.tabText(index))
