"""dictz main window — functional GoldenDict-style layout."""
from __future__ import annotations

import html
from pathlib import Path

from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QAction, QFont, QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QApplication, QFileDialog, QHBoxLayout, QLabel, QLineEdit, QListWidget,
    QListWidgetItem, QMainWindow, QMessageBox, QSizePolicy, QStatusBar,
    QToolBar, QVBoxLayout, QWidget,
)

from ..core.query_manager import QueryManager
from ..parsers.base import DictionaryEntry
from ..storage.history import HistoryStore
from ..storage.settings import Preferences
from . import themes
from .definition_tabs import DefinitionTabs
from .dictionary_manager import DictionaryManagerDialog
from .icons import icon
from .preferences_dialog import PreferencesDialog
from .sidebar import Sidebar


class MainWindow(QMainWindow):
    def __init__(self, query_manager: QueryManager, history_store: HistoryStore, preferences: Preferences):
        super().__init__()
        self.query_manager = query_manager
        self.history = history_store
        self.prefs = preferences

        self.setWindowTitle("dictz")
        self.resize(1280, 820)

        self._nav_history: list[str] = []
        self._nav_cursor = -1
        self._applying_nav = False

        self._build_center()
        self._build_toolbar()
        self._build_menus()
        self._build_sidebar()
        self._build_statusbar()
        self._build_shortcuts()

        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.timeout.connect(self._run_search)

        self._apply_prefs_to_ui()
        self._refresh_sidebar()
        self._refresh_status()

        if self.prefs.window_geometry:
            try:
                self.restoreGeometry(self.prefs.window_geometry)
            except Exception:
                pass
        if self.prefs.window_state:
            try:
                self.restoreState(self.prefs.window_state)
            except Exception:
                pass

        self.tabs.new_tab()

    def _build_center(self) -> None:
        central = QWidget()
        self.setCentralWidget(central)
        layout = QVBoxLayout(central)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        self.tabs = DefinitionTabs()
        self.tabs.link_activated.connect(self._on_definition_link)
        self.tabs.current_word_changed.connect(self._on_current_tab_word_changed)
        self.suggestions = QListWidget()
        self.suggestions.setMaximumHeight(140)
        self.suggestions.setFrameShape(QListWidget.Shape.NoFrame)
        self.suggestions.itemActivated.connect(self._on_suggestion_activated)
        self.suggestions.itemClicked.connect(self._on_suggestion_activated)
        self.suggestions.hide()
        layout.addWidget(self.suggestions)
        layout.addWidget(self.tabs, 1)

    def _build_toolbar(self) -> None:
        tb = QToolBar("Ana araç çubuğu")
        tb.setMovable(False)
        tb.setObjectName("main_toolbar")
        self.addToolBar(tb)
        self.back_action = QAction(icon("back"), "Geri", self)
        self.back_action.setShortcut(QKeySequence("Alt+Left"))
        self.back_action.triggered.connect(self._nav_back)
        tb.addAction(self.back_action)
        self.fwd_action = QAction(icon("forward"), "İleri", self)
        self.fwd_action.setShortcut(QKeySequence("Alt+Right"))
        self.fwd_action.triggered.connect(self._nav_forward)
        tb.addAction(self.fwd_action)
        tb.addSeparator()
        self.search_box = QLineEdit()
        self.search_box.setPlaceholderText("Bir kelime ara…   (Ctrl+F odaklan)")
        self.search_box.setClearButtonEnabled(True)
        self.search_box.setMinimumWidth(280)
        self.search_box.textChanged.connect(self._on_search_text_changed)
        self.search_box.returnPressed.connect(self._run_search_explicit)
        tb.addWidget(self.search_box)
        spacer = QWidget()
        spacer.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        tb.addWidget(spacer)
        tb.addSeparator()
        self.favorite_action = QAction(icon("star"), "Favori", self)
        self.favorite_action.setCheckable(True)
        self.favorite_action.setShortcut(QKeySequence("Ctrl+D"))
        self.favorite_action.triggered.connect(self._toggle_favorite_current)
        tb.addAction(self.favorite_action)
        tb.addSeparator()
        self.zoom_out_action = QAction(icon("zoom-out"), "Küçült", self)
        self.zoom_out_action.setShortcut(QKeySequence("Ctrl+-"))
        self.zoom_out_action.triggered.connect(self._zoom_out)
        tb.addAction(self.zoom_out_action)
        self.zoom_in_action = QAction(icon("zoom-in"), "Büyüt", self)
        self.zoom_in_action.setShortcut(QKeySequence("Ctrl+="))
        self.zoom_in_action.triggered.connect(self._zoom_in)
        tb.addAction(self.zoom_in_action)
        self.theme_action = QAction(icon("theme"), "Tema", self)
        self.theme_action.triggered.connect(self._cycle_theme)
        tb.addAction(self.theme_action)
        tb.addSeparator()
        self.prefs_action = QAction(icon("settings"), "Tercihler", self)
        self.prefs_action.setShortcut(QKeySequence("Ctrl+,"))
        self.prefs_action.triggered.connect(self._open_preferences)
        tb.addAction(self.prefs_action)

    def _build_menus(self) -> None:
        menubar = self.menuBar()
        file_menu = menubar.addMenu("&Dosya")
        add_dict = QAction(icon("open"), "Sözlük &Ekle…", self)
        add_dict.setShortcut(QKeySequence("Ctrl+O"))
        add_dict.triggered.connect(self._add_dictionary_dialog)
        file_menu.addAction(add_dict)
        manage = QAction(icon("folder"), "Sözlük &Yöneticisi…", self)
        manage.setShortcut(QKeySequence("Ctrl+M"))
        manage.triggered.connect(self._open_dictionary_manager)
        file_menu.addAction(manage)
        file_menu.addSeparator()
        quit_action = QAction("Çı&kış", self)
        quit_action.setShortcut(QKeySequence.StandardKey.Quit)
        quit_action.triggered.connect(self.close)
        file_menu.addAction(quit_action)
        edit_menu = menubar.addMenu("&Düzen")
        copy_word = QAction("Kelimeyi kopyala", self)
        copy_word.setShortcut(QKeySequence("Ctrl+Shift+C"))
        copy_word.triggered.connect(self._copy_current_word)
        edit_menu.addAction(copy_word)
        focus_search = QAction("Arama kutusuna odaklan", self)
        focus_search.setShortcut(QKeySequence("Ctrl+F"))
        focus_search.triggered.connect(lambda: self.search_box.setFocus())
        edit_menu.addAction(focus_search)
        edit_menu.addSeparator()
        prefs = QAction("Tercihler…", self)
        prefs.setShortcut(QKeySequence("Ctrl+,"))
        prefs.triggered.connect(self._open_preferences)
        edit_menu.addAction(prefs)
        view_menu = menubar.addMenu("&Görünüm")
        new_tab = QAction("Yeni sekme", self)
        new_tab.setShortcut(QKeySequence("Ctrl+T"))
        new_tab.triggered.connect(lambda: self.tabs.new_tab())
        view_menu.addAction(new_tab)
        close_tab = QAction("Sekmeyi kapat", self)
        close_tab.setShortcut(QKeySequence("Ctrl+W"))
        close_tab.triggered.connect(self.tabs.close_current_tab)
        view_menu.addAction(close_tab)
        view_menu.addSeparator()
        toggle_sidebar = QAction("Yan paneli göster/gizle", self)
        toggle_sidebar.setShortcut(QKeySequence("Ctrl+B"))
        toggle_sidebar.triggered.connect(self._toggle_sidebar)
        view_menu.addAction(toggle_sidebar)
        view_menu.addSeparator()
        zoom_in = QAction("Büyüt", self)
        zoom_in.setShortcut(QKeySequence("Ctrl+="))
        zoom_in.triggered.connect(self._zoom_in)
        view_menu.addAction(zoom_in)
        zoom_out = QAction("Küçült", self)
        zoom_out.setShortcut(QKeySequence("Ctrl+-"))
        zoom_out.triggered.connect(self._zoom_out)
        view_menu.addAction(zoom_out)
        reset_zoom = QAction("Yakınlaştırmayı sıfırla", self)
        reset_zoom.setShortcut(QKeySequence("Ctrl+0"))
        reset_zoom.triggered.connect(self._zoom_reset)
        view_menu.addAction(reset_zoom)
        help_menu = menubar.addMenu("&Yardım")
        about = QAction("&Hakkında", self)
        about.triggered.connect(self._about)
        help_menu.addAction(about)

    def _build_sidebar(self) -> None:
        self.sidebar = Sidebar(self)
        self.sidebar.setObjectName("sidebar_dock")
        self.sidebar.word_selected.connect(self._open_word)
        self.sidebar.dictionary_toggled.connect(self._on_sidebar_dict_toggled)
        self.sidebar.manage_dictionaries_requested.connect(self._open_dictionary_manager)
        self.sidebar.clear_history_requested.connect(self._clear_history)
        self.addDockWidget(Qt.DockWidgetArea.LeftDockWidgetArea, self.sidebar)
        self.sidebar.setMinimumWidth(240)

    def _build_statusbar(self) -> None:
        self.setStatusBar(QStatusBar())
        self.dict_count_label = QLabel("0 sözlük")
        self.zoom_label = QLabel("100%")
        self.statusBar().addPermanentWidget(self.dict_count_label)
        sep = QLabel("·")
        sep.setContentsMargins(8, 0, 8, 0)
        self.statusBar().addPermanentWidget(sep)
        self.statusBar().addPermanentWidget(self.zoom_label)

    def _build_shortcuts(self) -> None:
        QShortcut(QKeySequence("Ctrl+Tab"), self, lambda: self._cycle_tab(+1))
        QShortcut(QKeySequence("Ctrl+Shift+Tab"), self, lambda: self._cycle_tab(-1))
        open_new = QShortcut(QKeySequence("Ctrl+Return"), self.search_box)
        open_new.activated.connect(self._run_search_new_tab)

    def _apply_prefs_to_ui(self) -> None:
        qss = themes.resolve(self.prefs.theme)
        app = QApplication.instance()
        if app is not None:
            app.setStyleSheet(qss)
        dark = "15181d" in qss
        self.tabs.set_dark(dark)
        if self.prefs.font_family:
            self.setFont(QFont(self.prefs.font_family, self.prefs.font_size))
        self.tabs.set_zoom(self.prefs.zoom_factor)
        self._update_zoom_label()
        self.sidebar.setVisible(self.prefs.show_sidebar)

    def _on_search_text_changed(self, text: str) -> None:
        self._search_timer.start(self.prefs.search_debounce_ms)

    def _run_search(self) -> None:
        query = self.search_box.text().strip()
        if not query:
            self.suggestions.clear()
            self.suggestions.hide()
            return
        suggestions = self.query_manager.suggest(query, limit=100)
        self._populate_suggestions(suggestions)
        entries = self.query_manager.search(query, exact=True)
        if entries:
            self._display_in_current(query, entries)
            self._push_nav(query)

    def _run_search_explicit(self) -> None:
        query = self.search_box.text().strip()
        if not query:
            return
        entries = self.query_manager.search(query, exact=True)
        if not entries:
            entries = self.query_manager.search(query, exact=False)
        self._display_in_current(query, entries)
        self._push_nav(query)
        self.suggestions.hide()

    def _run_search_new_tab(self) -> None:
        query = self.search_box.text().strip()
        if not query:
            return
        entries = self.query_manager.search(query, exact=True)
        if not entries:
            entries = self.query_manager.search(query, exact=False)
        self.tabs.show_in_new_tab(query, entries)
        self._push_nav(query)
        self.suggestions.hide()

    def _populate_suggestions(self, items: list[str]) -> None:
        self.suggestions.clear()
        for s in items:
            QListWidgetItem(s, self.suggestions)
        self.suggestions.setVisible(bool(items))

    def _on_suggestion_activated(self, item: QListWidgetItem) -> None:
        word = item.text()
        self.search_box.setText(word)
        entries = self.query_manager.search(word, exact=True)
        self._display_in_current(word, entries)
        self._push_nav(word)
        self.suggestions.hide()

    def _display_in_current(self, word: str, entries: list[DictionaryEntry]) -> None:
        self.tabs.show_in_current(word, entries)
        self._update_favorite_button(word)
        if word and entries:
            self.history.record(word)
            self._refresh_history_pane()

    def _open_word(self, word: str) -> None:
        if not word:
            return
        self.search_box.setText(word)
        entries = self.query_manager.search(word, exact=True)
        self._display_in_current(word, entries)
        self._push_nav(word)

    def _push_nav(self, word: str) -> None:
        if self._applying_nav or not word:
            return
        self._nav_history = self._nav_history[: self._nav_cursor + 1]
        self._nav_history.append(word)
        self._nav_cursor = len(self._nav_history) - 1
        self._update_nav_buttons()

    def _nav_back(self) -> None:
        if self._nav_cursor <= 0:
            return
        self._nav_cursor -= 1
        self._apply_nav(self._nav_history[self._nav_cursor])

    def _nav_forward(self) -> None:
        if self._nav_cursor + 1 >= len(self._nav_history):
            return
        self._nav_cursor += 1
        self._apply_nav(self._nav_history[self._nav_cursor])

    def _apply_nav(self, word: str) -> None:
        self._applying_nav = True
        try:
            self.search_box.setText(word)
            entries = self.query_manager.search(word, exact=True)
            self._display_in_current(word, entries)
        finally:
            self._applying_nav = False
            self._update_nav_buttons()

    def _update_nav_buttons(self) -> None:
        self.back_action.setEnabled(self._nav_cursor > 0)
        self.fwd_action.setEnabled(self._nav_cursor + 1 < len(self._nav_history))

    def _toggle_favorite_current(self) -> None:
        word = self.tabs.current_word() or self.search_box.text().strip()
        if not word or word == "Yeni sekme":
            return
        now_fav = self.history.toggle_favorite(word)
        self.favorite_action.setChecked(now_fav)
        self._refresh_favorites_pane()

    def _update_favorite_button(self, word: str) -> None:
        self.favorite_action.setChecked(bool(word) and self.history.is_favorite(word))

    def _on_current_tab_word_changed(self, word: str) -> None:
        self._update_favorite_button(word)

    def _cycle_tab(self, delta: int) -> None:
        n = self.tabs.count()
        if n == 0:
            return
        self.tabs.setCurrentIndex((self.tabs.currentIndex() + delta) % n)

    def _zoom_in(self) -> None:
        self.prefs.zoom_factor = min(3.0, self.prefs.zoom_factor + 0.1)
        self.tabs.set_zoom(self.prefs.zoom_factor)
        self.prefs.save()
        self._update_zoom_label()

    def _zoom_out(self) -> None:
        self.prefs.zoom_factor = max(0.5, self.prefs.zoom_factor - 0.1)
        self.tabs.set_zoom(self.prefs.zoom_factor)
        self.prefs.save()
        self._update_zoom_label()

    def _zoom_reset(self) -> None:
        self.prefs.zoom_factor = 1.0
        self.tabs.set_zoom(1.0)
        self.prefs.save()
        self._update_zoom_label()

    def _update_zoom_label(self) -> None:
        self.zoom_label.setText(f"{int(self.prefs.zoom_factor * 100)}%")

    def _cycle_theme(self) -> None:
        order = ["auto", "light", "dark"]
        idx = order.index(self.prefs.theme) if self.prefs.theme in order else 0
        self.prefs.theme = order[(idx + 1) % len(order)]
        self.prefs.save()
        self._apply_prefs_to_ui()
        self.statusBar().showMessage(f"Tema: {self.prefs.theme}", 2500)

    def _add_dictionary_dialog(self) -> None:
        file_filter = ("Tüm sözlükler (*.ifo *.dsl *.dsl.dz *.bgl);;"
                       "StarDict (*.ifo);;DSL / Lingvo (*.dsl *.dsl.dz);;"
                       "Babylon (*.bgl);;Tüm dosyalar (*)")
        paths, _ = QFileDialog.getOpenFileNames(self, "Sözlük dosyası seçin", "", file_filter)
        if not paths:
            return
        for p in paths:
            try:
                d = self.query_manager.add_dictionary(Path(p))
                self.statusBar().showMessage(f"Yüklendi: {d.metadata.name} ({d.metadata.word_count:,} kelime)", 4000)
            except Exception as e:
                QMessageBox.critical(self, "Yüklenemedi", f"<b>{p}</b><br><br>{type(e).__name__}: {e}")
        self._refresh_sidebar()
        self._refresh_status()

    def _open_dictionary_manager(self) -> None:
        dlg = DictionaryManagerDialog(self.query_manager, self)
        dlg.dictionaries_changed.connect(lambda: (self._refresh_sidebar(), self._refresh_status()))
        dlg.exec()

    def _on_sidebar_dict_toggled(self, name: str, enabled: bool) -> None:
        self.query_manager.set_enabled(name, enabled)
        self._refresh_status()

    def _refresh_sidebar(self) -> None:
        items = [(name, self.query_manager.is_enabled(name)) for name in self.query_manager.loaded_names()]
        self.sidebar.set_dictionaries(items)
        self._refresh_favorites_pane()
        self._refresh_history_pane()

    def _refresh_favorites_pane(self) -> None:
        self.sidebar.set_favorites(self.history.favorites(500))

    def _refresh_history_pane(self) -> None:
        self.sidebar.set_history(self.history.recent(200))

    def _refresh_status(self) -> None:
        enabled = [n for n in self.query_manager.loaded_names() if self.query_manager.is_enabled(n)]
        n = len(enabled)
        total = sum(self.query_manager.dictionaries[name].metadata.word_count for name in enabled)
        if n == 0:
            self.dict_count_label.setText("0 sözlük yüklü")
        else:
            self.dict_count_label.setText(f"{n} sözlük · {total:,} kelime")

    def _toggle_sidebar(self) -> None:
        self.sidebar.setVisible(not self.sidebar.isVisible())
        self.prefs.show_sidebar = self.sidebar.isVisible()
        self.prefs.save()

    def _clear_history(self) -> None:
        reply = QMessageBox.question(self, "Geçmişi temizle",
            "Tüm arama geçmişini silmek istediğinizden emin misiniz?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No)
        if reply == QMessageBox.StandardButton.Yes:
            self.history.clear_history()
            self._refresh_history_pane()

    def _copy_current_word(self) -> None:
        word = self.tabs.current_word()
        if word and word != "Yeni sekme":
            QApplication.clipboard().setText(word)
            self.statusBar().showMessage(f"Kopyalandı: {word}", 2000)

    def _on_definition_link(self, url: str) -> None:
        if url.startswith(("dict:", "bword:")):
            word = url.split(":", 1)[1].lstrip("/")
            self._open_word(word)

    def _open_preferences(self) -> None:
        dlg = PreferencesDialog(self.prefs, self)
        dlg.preferences_changed.connect(self._on_prefs_changed)
        dlg.exec()

    def _on_prefs_changed(self, prefs: Preferences) -> None:
        self.prefs = prefs
        self._apply_prefs_to_ui()

    def _about(self) -> None:
        QMessageBox.about(self, "dictz hakkında",
            """
            <h3>dictz 0.0.4</h3>
            <p>Çok formatlı çevrimdışı sözlük uygulaması.</p>
            <p style="color: #888; font-size: 12px;">
                v0.0.4 — portable paketler (AppImage, Windows portable zip, macOS .app)
            </p>
            <p>© 2026 Bayram Kotan · GPL-3.0+</p>
            <p><a href="https://github.com/bayramkotan/dictz">github.com/bayramkotan/dictz</a></p>
            """)

    def closeEvent(self, event):  # noqa: N802
        self.prefs.window_geometry = self.saveGeometry()
        self.prefs.window_state = self.saveState()
        self.prefs.last_search = self.search_box.text()
        self.prefs.save()
        self.query_manager.close_all()
        if self.query_manager.index_store:
            self.query_manager.index_store.close()
        self.history.close()
        super().closeEvent(event)
