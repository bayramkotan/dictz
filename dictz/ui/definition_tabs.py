"""Tabbed article area."""
from __future__ import annotations

from PySide6.QtCore import Signal
from PySide6.QtWidgets import QTabWidget, QWidget

from ..core.i18n import t
from ..parsers.base import DictionaryEntry
from ..storage.settings import Preferences
from .definition_view import DefinitionView

NEW_TAB = "Yeni sekme"


class DefinitionTabs(QTabWidget):
    current_word_changed = Signal(str)
    link_activated = Signal(str)
    find_result = Signal(int, int)

    def __init__(self, prefs: Preferences, parent: QWidget | None = None):
        super().__init__(parent)
        self.prefs = prefs
        self.setMovable(True)
        self.setTabsClosable(True)
        self.setDocumentMode(True)
        self.tabCloseRequested.connect(self._on_tab_close_requested)
        self.currentChanged.connect(self._on_current_changed)

    def views(self) -> list[DefinitionView]:
        return [w for i in range(self.count()) if isinstance(w := self.widget(i), DefinitionView)]

    def apply_prefs(self) -> None:
        for v in self.views():
            v.set_zoom(float(self.prefs["article.zoom"]))
            v.refresh_style()
        for i in range(self.count()):
            v = self.widget(i)
            if isinstance(v, DefinitionView):
                self.setTabText(i, self._title(v.word, bool(v.entries)))

    def set_zoom(self, factor: float) -> None:
        for v in self.views():
            v.set_zoom(factor)

    def current_view(self) -> DefinitionView | None:
        w = self.currentWidget()
        return w if isinstance(w, DefinitionView) else None

    def current_word(self) -> str:
        v = self.current_view()
        return v.word if v else ""

    def _title(self, word: str, found: bool) -> str:
        if not word:
            return t(NEW_TAB)
        n = int(self.prefs["appearance.tab_title_length"])
        title = word if len(word) <= n else word[:n] + "…"
        return title if found else f"{title} ✗"

    def new_tab(self, word: str = "") -> DefinitionView:
        view = DefinitionView(self.prefs, self)
        view.link_activated.connect(self.link_activated)
        view.find_result.connect(self.find_result)
        self.addTab(view, self._title(word, True))
        self.setCurrentWidget(view)
        if not word:
            view.show_welcome()
        return view

    def show_in_current(self, word: str, entries: list[DictionaryEntry],
                        suggestions: list[tuple[str, float]] | None = None) -> None:
        view = self.current_view() or self.new_tab(word)
        self._show(view, word, entries, suggestions)

    def show_in_new_tab(self, word: str, entries: list[DictionaryEntry],
                        suggestions: list[tuple[str, float]] | None = None) -> None:
        self._show(self.new_tab(word), word, entries, suggestions)

    def _show(self, view: DefinitionView, word: str, entries, suggestions) -> None:
        if entries:
            view.show_entries(word, entries)
        else:
            view.show_not_found(word, suggestions)
        idx = self.indexOf(view)
        if idx >= 0:
            self.setTabText(idx, self._title(word, bool(entries)))
            self.setTabToolTip(idx, word)
        self.current_word_changed.emit(self.current_word())

    def open_words(self) -> list[str]:
        return [v.word for v in self.views() if v.word]

    def close_current_tab(self) -> None:
        idx = self.currentIndex()
        if idx >= 0:
            self._on_tab_close_requested(idx)

    def _on_tab_close_requested(self, index: int) -> None:
        if self.count() <= 1:
            view = self.widget(index)
            if isinstance(view, DefinitionView):
                view.show_welcome()
            self.setTabText(index, t(NEW_TAB))
            self.current_word_changed.emit("")
            return
        widget = self.widget(index)
        self.removeTab(index)
        if widget is not None:
            widget.deleteLater()

    def _on_current_changed(self, index: int) -> None:
        self.current_word_changed.emit(self.current_word() if index >= 0 else "")
