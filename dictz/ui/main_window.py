"""dictz main window (Python edition).

Same layout, menus and shortcuts as the Rust edition's egui window.
"""
from __future__ import annotations

import contextlib
import os
import sys
import time
from pathlib import Path

from PySide6.QtCore import QSettings, QSize, Qt, QThread, QTimer, QUrl, Signal
from PySide6.QtGui import (
    QAction,
    QActionGroup,
    QDesktopServices,
    QFont,
    QIcon,
    QKeySequence,
    QShortcut,
)
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QDockWidget,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMenu,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSizePolicy,
    QSystemTrayIcon,
    QToolBar,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from .. import __version__
from ..core import i18n
from ..core import instance as _instance
from ..core.hotkeys import HotkeyListener, wayland_session
from ..core.i18n import t
from ..core.media import media_kind, parse_audio_url, rewrite_media_html
from ..core.query_manager import QueryManager, Suggestion, is_dictionary_file, scan_folder
from ..parsers.base import DictionaryEntry
from ..storage.history import HistoryStore
from ..storage.settings import Preferences
from . import dicticons, themes
from .definition_tabs import DefinitionTabs
from .definition_view import group_by_dictionary
from .dictionary_manager import DictionaryManagerDialog, file_filter, info_html
from .icons import app_icon, icon
from .preferences_dialog import PreferencesDialog
from .scan_popup import ScanPopup
from .scanner import ClipboardScanner
from .sidebar import Sidebar


class DictionaryLoader(QThread):
    """Opens dictionaries off the GUI thread; indexes new / changed files with
    its own SQLite connection."""

    loaded = Signal(object, str, bool)  # Dictionary, path, enabled
    failed = Signal(str, str)
    progress = Signal(int, int, str)

    def __init__(self, db_path: Path, rows: list[dict], new_files: list[Path], reindex_changed: bool, parent=None):
        super().__init__(parent)
        self.db_path = db_path
        self.rows = rows
        self.new_files = new_files
        self.reindex_changed = reindex_changed

    def run(self) -> None:
        from ..core.query_manager import _changed
        from ..storage.indexer import IndexStore

        store = IndexStore(self.db_path)
        store.open()
        total = len(self.rows) + len(self.new_files)
        i = 0
        try:
            for row in self.rows:
                i += 1
                self.progress.emit(i, total, Path(row["path"]).name)
                try:
                    d = QueryManager.open_file(row["path"])
                    d.metadata.name = row["name"]  # the name in the list (may be "X (2)")
                    if self.reindex_changed and _changed(row, Path(row["path"])):
                        store.index_dictionary(d)
                    self.loaded.emit(d, row["path"], bool(row["enabled"]))
                except Exception as exc:  # noqa: BLE001
                    self.failed.emit(row["path"], str(exc))
            for f in self.new_files:
                i += 1
                self.progress.emit(i, total, f.name)
                try:
                    d = QueryManager.open_file(f)
                    name = store.register_dictionary(d)  # unique name: "X (2)" for a second "X"
                    if not store.is_indexed(name):
                        store.index_dictionary(d)
                    self.loaded.emit(d, str(f), True)
                except Exception as exc:  # noqa: BLE001
                    self.failed.emit(str(f), str(exc))
        finally:
            store.close()


class UpdateChecker(QThread):
    result = Signal(str, str)  # latest, error

    def run(self) -> None:
        try:
            from ..core.updater import latest_version

            self.result.emit(latest_version(), "")
        except Exception as exc:  # noqa: BLE001
            self.result.emit("", str(exc))


class MainWindow(QMainWindow):
    hotkey_pressed = Signal(str)
    remote_request = Signal(dict)

    def __init__(self, query_manager: QueryManager, history_store: HistoryStore, preferences: Preferences,
                 startup_files: list[Path] | None = None):
        super().__init__()
        self.query_manager = query_manager
        dicticons.set_query_manager(query_manager)
        self.history = history_store
        self.prefs = preferences
        self.startup_files = list(startup_files or [])
        self.setWindowTitle("dictz")
        self.resize(1280, 820)
        self.setAcceptDrops(True)

        self._nav_history: list[str] = []
        self._nav_cursor = -1
        self._applying_nav = False
        self._loader: DictionaryLoader | None = None
        self._update_checker: UpdateChecker | None = None
        self._load_errors: list[str] = []
        self._loaded_count = 0
        self._loading_done = False
        self._pending_popup: str | None = None
        self._really_quit = False
        self._tray_hint_shown = False
        self.scan_popup: ScanPopup | None = None
        self.tray: QSystemTrayIcon | None = None
        self._hotkeys: HotkeyListener | None = None
        self._hotkey_conf: tuple | None = None
        self._server: _instance.Server | None = None
        self._restart_requested = False
        self._scan_on = bool(self.prefs["scan.enabled"])
        self.setWindowIcon(app_icon())

        self._build_center()
        self._build_toolbar()
        self._build_dictionary_bar()
        self._build_sidebar()
        self._build_found_in()
        self._build_statusbar()
        self._build_menus()
        self._build_shortcuts()
        self.scanner = ClipboardScanner(self.prefs, self)
        self.scanner.word_found.connect(self.popup_lookup)
        self.hotkey_pressed.connect(self._on_hotkey)
        self.remote_request.connect(self._on_remote)

        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.timeout.connect(self._run_search)

        qs = QSettings()
        if self.prefs["general.remember_window"]:
            with contextlib.suppress(Exception):
                if qs.value("window/geometry") is not None:
                    self.restoreGeometry(qs.value("window/geometry"))
                if qs.value("window/state") is not None:
                    self.restoreState(qs.value("window/state"))

        self.apply_prefs()
        self.tabs.new_tab()
        if self.prefs["general.restore_last_search"] and self.prefs["state.last_search"]:
            self.search_box.setText(self.prefs["state.last_search"])
            self._search_timer.stop()
        self.search_box.setFocus()
        if int(self.prefs["history.max_entries"]):
            self.history.purge(int(self.prefs["history.max_entries"]))
        if self.prefs["general.check_updates_on_start"]:
            QTimer.singleShot(3000, lambda: self._check_updates(silent=True))

    # ================================================================ build
    def _build_center(self) -> None:
        central = QWidget()
        self.setCentralWidget(central)
        layout = QVBoxLayout(central)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        self.tabs = DefinitionTabs(self.prefs)
        self.tabs.link_activated.connect(self._on_definition_link)
        self.tabs.current_word_changed.connect(self._on_current_tab_word_changed)
        self.tabs.find_result.connect(self._on_find_result)
        self.suggestions = QListWidget()
        self.suggestions.setMaximumHeight(160)
        self.suggestions.setFrameShape(QListWidget.Shape.NoFrame)
        self.suggestions.itemActivated.connect(self._on_suggestion_activated)
        self.suggestions.itemClicked.connect(self._on_suggestion_activated)
        self.suggestions.hide()
        layout.addWidget(self.suggestions)
        layout.addWidget(self.tabs, 1)
        # find bar
        self.find_bar = QWidget()
        self.find_bar.setObjectName("find_bar")
        fl = QHBoxLayout(self.find_bar)
        fl.setContentsMargins(8, 4, 8, 4)
        fl.addWidget(QLabel(t("Sayfada bul:")))
        self.find_box = QLineEdit()
        self.find_box.setPlaceholderText(t("metin…"))
        self.find_box.textChanged.connect(lambda t: self._find(False))
        self.find_box.returnPressed.connect(lambda: self._find(False))
        fl.addWidget(self.find_box, 1)
        prev_b = QPushButton("◀")
        next_b = QPushButton("▶")
        prev_b.clicked.connect(lambda: self._find(True))
        next_b.clicked.connect(lambda: self._find(False))
        fl.addWidget(prev_b)
        fl.addWidget(next_b)
        self.find_case = QCheckBox(t("Büyük/küçük harf"))
        self.find_case.toggled.connect(lambda _: self._find(False))
        fl.addWidget(self.find_case)
        self.find_count = QLabel("")
        fl.addWidget(self.find_count)
        close_b = QToolButton()
        close_b.setText("✕")
        close_b.clicked.connect(self._close_find)
        fl.addWidget(close_b)
        self.find_bar.hide()
        layout.addWidget(self.find_bar)

    def _act(self, text: str, slot, shortcut: str | None = None, icon_name: str | None = None,
             checkable: bool = False, tip: str | None = None) -> QAction:
        a = QAction(icon(icon_name), text, self) if icon_name else QAction(text, self)
        if shortcut:
            a.setShortcut(QKeySequence(shortcut))
        a.setCheckable(checkable)
        if tip:
            a.setToolTip(tip)
        a.triggered.connect(slot)
        self.addAction(a)
        return a

    def _build_toolbar(self) -> None:
        tb = QToolBar(t("Araç çubuğu"))
        tb.setMovable(False)
        tb.setObjectName("main_toolbar")
        self.addToolBar(tb)
        self.toolbar = tb
        self.back_action = self._act(t("Geri"), self._nav_back, "Alt+Left", "back")
        self.fwd_action = self._act(t("İleri"), self._nav_forward, "Alt+Right", "forward")
        tb.addAction(self.back_action)
        tb.addAction(self.fwd_action)
        tb.addSeparator()
        self.search_box = QLineEdit()
        self.search_box.setObjectName("search_box")
        self.search_box.setPlaceholderText(t("Bir kelime ara…   (Ctrl+F)"))
        self.search_box.setClearButtonEnabled(True)
        self.search_box.setMinimumWidth(320)
        self.search_box.textChanged.connect(self._on_search_text_changed)
        self.search_box.returnPressed.connect(self._run_search_explicit)
        self.search_box.installEventFilter(self)
        tb.addWidget(self.search_box)
        spacer = QWidget()
        spacer.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        tb.addWidget(spacer)
        self.random_action = self._act(t("Rastgele"), self._random_word, "Ctrl+R", "refresh", tip=t("Rastgele kelime (Ctrl+R)"))
        tb.addAction(self.random_action)
        self.favorite_action = self._act(t("Favori"), self._toggle_favorite_current, "Ctrl+D", "star", checkable=True)
        tb.addAction(self.favorite_action)
        tb.addSeparator()
        self.zoom_out_action = self._act(t("Küçült"), self._zoom_out, "Ctrl+-", "zoom-out")
        self.zoom_in_action = self._act(t("Büyüt"), self._zoom_in, "Ctrl+=", "zoom-in")
        tb.addAction(self.zoom_out_action)
        tb.addAction(self.zoom_in_action)
        self.theme_action = self._act(t("Tema"), self._cycle_theme, None, "theme", tip=t("Tema: Otomatik → Açık → Koyu"))
        tb.addAction(self.theme_action)
        tb.addSeparator()
        self.scan_action = self._act(t("Tarama"), self.set_scan, None, "scan", checkable=True,
                                     tip=t("Tarama penceresi: başka uygulamada kopyalanan / seçilen kelimeyi çevirir"))
        tb.addAction(self.scan_action)
        self.prefs_action = self._act(t("Tercihler"), self._open_preferences, "Ctrl+,", "settings")
        tb.addAction(self.prefs_action)
        tb.addSeparator()
        self._build_language_button(tb)

    def _build_dictionary_bar(self) -> None:
        bar = QToolBar(t("Sözlük çubuğu"))
        bar.setObjectName("dictionary_bar")
        bar.setIconSize(QSize(18, 18))
        bar.setMovable(True)
        self.addToolBarBreak()
        self.addToolBar(bar)
        self.dictionary_bar = bar

    def _refresh_dictionary_bar(self) -> None:
        bar = self.dictionary_bar
        bar.clear()
        qm = self.query_manager
        names = qm.loaded_names()
        if not names:
            lab = QLabel("  " + t("Sözlük yok — Ctrl+O ile ekleyin"))
            lab.setObjectName("hint")
            bar.addWidget(lab)
            return
        for name in names:
            b = QToolButton()
            short = name if len(name) <= 22 else name[:21] + "…"
            b.setText(short)
            b.setIcon(dicticons.qicon(name))
            b.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextBesideIcon)
            b.setCheckable(True)
            b.setChecked(qm.is_enabled(name))
            b.setToolTip(dicticons.look(name).tooltip() + "\n\n" + t("Tıkla: aç/kapat · Sağ tık: yalnızca bu"))
            b.toggled.connect(lambda on, n=name: self._set_dictionary_enabled(n, on))
            b.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
            b.customContextMenuRequested.connect(lambda _p, n=name: self._solo_dictionary(n))
            bar.addWidget(b)
        bar.addSeparator()
        all_b = QToolButton()
        all_b.setText(t("Tümü"))
        all_b.setToolTip(t("Tüm sözlükleri etkinleştir"))
        all_b.clicked.connect(lambda: self._enable_all(True))
        bar.addWidget(all_b)

    def _build_sidebar(self) -> None:
        self.sidebar = Sidebar(self)
        self.sidebar.setObjectName("sidebar_dock")
        self.sidebar.word_selected.connect(self._open_word)
        self.sidebar.dictionary_toggled.connect(self._set_dictionary_enabled)
        self.sidebar.dictionary_activated.connect(self._show_dictionary_info)
        self.sidebar.manage_dictionaries_requested.connect(self._open_dictionary_manager)
        self.sidebar.clear_history_requested.connect(self._clear_history)
        self.sidebar.history_remove_requested.connect(self._remove_history_word)
        self.sidebar.favorite_remove_requested.connect(self._remove_favorite)
        self.sidebar.export_favorites_requested.connect(self._export_favorites)
        self.addDockWidget(Qt.DockWidgetArea.LeftDockWidgetArea, self.sidebar)
        self.sidebar.setMinimumWidth(220)

    def _build_found_in(self) -> None:
        dock = QDockWidget(t("Bulunduğu sözlükler"), self)
        dock.setObjectName("found_in_dock")
        self.found_list = QListWidget()
        self.found_list.itemClicked.connect(self._on_found_in_clicked)
        dock.setWidget(self.found_list)
        self.addDockWidget(Qt.DockWidgetArea.RightDockWidgetArea, dock)
        dock.setMinimumWidth(180)
        self.found_dock = dock

    def _build_statusbar(self) -> None:
        sb = self.statusBar()
        self.progress = QProgressBar()
        self.progress.setMaximumWidth(180)
        self.progress.setMaximumHeight(14)
        self.progress.setTextVisible(False)
        self.progress.hide()
        self.timing_label = QLabel("")
        self.dict_count_label = QLabel(t("0 sözlük"))
        self.zoom_label = QLabel("100%")
        sb.addPermanentWidget(self.progress)
        sb.addPermanentWidget(self.timing_label)
        sb.addPermanentWidget(self.dict_count_label)
        sb.addPermanentWidget(QLabel("·"))
        sb.addPermanentWidget(self.zoom_label)

    def _build_menus(self) -> None:
        mb = self.menuBar()
        m = mb.addMenu(t("&Dosya"))
        m.addAction(self._act(t("Sözlük &Ekle…"), self._add_dictionary_dialog, "Ctrl+O", "open"))
        m.addAction(self._act(t("&Klasör Ekle…"), self._add_folder_dialog, "Ctrl+Shift+O", "folder"))
        m.addAction(self._act(t("Sözlük &Yöneticisi…"), self._open_dictionary_manager, "Ctrl+M", "dict"))
        m.addSeparator()
        m.addAction(self._act(t("Makaleyi HTML olarak &kaydet…"), self._save_article, "Ctrl+S", "save"))
        m.addAction(self._act(t("Favorileri dışa aktar…"), self._export_favorites))
        m.addSeparator()
        m.addAction(self._act(t("Masaüstü kısayolu oluştur"), self._create_shortcuts))
        m.addAction(self._act(t("Çı&kış"), self.quit_app, "Ctrl+Q"))

        e = mb.addMenu(t("&Düzen"))
        e.addAction(self._act(t("Kelimeyi kopyala"), self._copy_current_word, "Ctrl+Shift+C", "copy"))
        e.addAction(self._act(t("Makaleyi metin olarak kopyala"), self._copy_article, "Ctrl+Shift+A"))
        e.addSeparator()
        e.addAction(self._act(t("Arama kutusuna odaklan"), self._focus_search, "Ctrl+F"))
        e.addAction(self._act(t("Sayfada bul…"), self._open_find, "Ctrl+Shift+F"))
        e.addAction(self._act(t("Sonraki eşleşme"), lambda: self._find(False), "F3"))
        e.addAction(self._act(t("Önceki eşleşme"), lambda: self._find(True), "Shift+F3"))
        e.addSeparator()
        e.addAction(self._act(t("Tercihler…"), self._open_preferences))

        v = mb.addMenu(t("&Görünüm"))
        v.addAction(self._act(t("Yeni sekme"), lambda: self.tabs.new_tab(), "Ctrl+T", "new"))
        v.addAction(self._act(t("Sekmeyi kapat"), self.tabs.close_current_tab, "Ctrl+W", "close"))
        v.addAction(self._act(t("Tüm makaleleri aç"), lambda: self._all_open(True)))
        v.addAction(self._act(t("Tüm makaleleri kapat"), lambda: self._all_open(False)))
        v.addSeparator()
        self.view_toggles: dict[str, QAction] = {}
        for key, label, sc in [
            ("appearance.show_toolbar", t("Araç çubuğu"), None),
            ("appearance.show_dictionary_bar", t("Sözlük çubuğu"), None),
            ("appearance.show_sidebar", t("Yan panel"), "Ctrl+B"),
            ("appearance.show_found_in", t("Bulunduğu sözlükler"), None),
            ("appearance.show_statusbar", t("Durum çubuğu"), None),
        ]:
            a = self._act(label, lambda on, k=key: self._toggle_pref(k, on), sc, checkable=True)
            self.view_toggles[key] = a
            v.addAction(a)
        v.addSeparator()
        self.fullscreen_action = self._act(t("Tam ekran"), self._toggle_fullscreen, "F11", checkable=True)
        v.addAction(self.fullscreen_action)
        self.ontop_action = self._act(t("Her zaman üstte"), lambda on: self._toggle_pref("general.always_on_top", on),
                                      checkable=True)
        v.addAction(self.ontop_action)
        v.addAction(self.scan_action)
        v.addAction(self._act(t("Panodaki kelimeyi tarama penceresinde ara"), lambda: self.popup_lookup("")))
        v.addSeparator()
        v.addAction(self.zoom_in_action)
        v.addAction(self.zoom_out_action)
        v.addAction(self._act(t("Yakınlaştırmayı sıfırla"), self._zoom_reset, "Ctrl+0"))
        tm = v.addMenu(t("Tema"))
        self.theme_group = QActionGroup(self)
        self.theme_actions = {}
        for val, label in [("auto", t("Otomatik")), ("light", t("Açık")), ("dark", t("Koyu"))]:
            a = QAction(label, self, checkable=True)
            a.triggered.connect(lambda _=False, t=val: self._set_theme(t))
            self.theme_group.addAction(a)
            tm.addAction(a)
            self.theme_actions[val] = a

        g = mb.addMenu(t("&Git"))
        g.addAction(self.back_action)
        g.addAction(self.fwd_action)
        g.addAction(self.random_action)
        g.addAction(self._act(t("Sonraki sekme"), lambda: self._cycle_tab(1), "Ctrl+Tab"))
        g.addAction(self._act(t("Önceki sekme"), lambda: self._cycle_tab(-1), "Ctrl+Shift+Tab"))

        self.dict_menu = mb.addMenu(t("&Sözlükler"))
        self.dict_menu.aboutToShow.connect(self._fill_dictionary_menu)

        h = mb.addMenu(t("&Yardım"))
        h.addAction(self._act(t("Kısayollar"), self._show_shortcuts, "F1", "help"))
        h.addAction(self._act(t("Komut satırı (CLI)"), self._show_cli_help))
        h.addAction(self._act(t("Güncellemeleri denetle…"), lambda: self._check_updates(silent=False)))
        h.addAction(self._act(t("Veri klasörünü aç"), self._open_data_dir, None, "folder"))
        h.addSeparator()
        h.addAction(self._act(t("&Hakkında"), self._about))

    def _build_shortcuts(self) -> None:
        QShortcut(QKeySequence("Ctrl+Return"), self.search_box).activated.connect(self._run_search_new_tab)
        QShortcut(QKeySequence("Escape"), self.find_bar).activated.connect(self._close_find)

    def eventFilter(self, obj, event):  # noqa: N802
        from PySide6.QtCore import QEvent

        if obj is self.search_box and event.type() == QEvent.Type.FocusIn and self.prefs["search.select_all_on_focus"]:
            QTimer.singleShot(0, self.search_box.selectAll)
        if obj is self.search_box and event.type() == QEvent.Type.KeyPress and self.suggestions.isVisible():
            if event.key() == Qt.Key.Key_Down:
                self.suggestions.setFocus()
                self.suggestions.setCurrentRow(0)
                return True
            if event.key() == Qt.Key.Key_Escape:
                self.suggestions.hide()
                return True
        return super().eventFilter(obj, event)

    # ================================================================ loading
    def start_loading(self) -> None:
        """Open dictionaries from the index, scan dictionary folders and add
        files given on the command line — in the background."""
        qm = self.query_manager
        store = qm.index_store
        rows = [dict(r) for r in store.list_dictionaries(enabled_only=False)] if store else []
        from ..storage.registry import norm_path

        known = {norm_path(r["path"]) for r in rows}
        new_files: list[Path] = []
        if self.prefs["dictionaries.scan_on_startup"]:
            for folder in self.prefs["dictionaries.folders"]:
                new_files += scan_folder(folder, bool(self.prefs["dictionaries.recursive"]))
        new_files += [f for f in self.startup_files if is_dictionary_file(f)]
        seen: set[str] = set()
        new_files = [f for f in new_files if (r := norm_path(f)) not in known and not (r in seen or seen.add(r))]
        if not rows and not new_files:
            self._loading_finished()
            return
        db = Path(store.db_path) if store else Path()
        self._loader = DictionaryLoader(db, rows, new_files, bool(self.prefs["dictionaries.reindex_on_change"]), self)
        self._loader.loaded.connect(self._on_dictionary_loaded)
        self._loader.failed.connect(lambda p, e: self._load_errors.append(f"{p}: {e}"))
        self._loader.progress.connect(self._on_load_progress)
        self._loader.finished.connect(self._loading_finished)
        self.progress.show()
        if self.prefs["general.load_in_background"]:
            self._loader.start()
        else:
            self._loader.run()

    def _on_load_progress(self, i: int, total: int, name: str) -> None:
        self.progress.setMaximum(total)
        self.progress.setValue(i)
        self.statusBar().showMessage(t("Sözlükler yükleniyor ({i}/{n}): {name}", i=i, n=total, name=name))

    def _on_dictionary_loaded(self, dictionary, path: str, enabled: bool) -> None:
        self.query_manager.attach(dictionary, path, enabled)
        self._loaded_count += 1
        self._refresh_dictionaries()

    def _loading_finished(self) -> None:
        # the order comes from dictionaries.json; nothing to write back here
        self.progress.hide()
        self._loading_done = True
        self._refresh_dictionaries()
        if self._pending_popup is not None:
            word, self._pending_popup = self._pending_popup, None
            QTimer.singleShot(0, lambda: self.popup_lookup(word))
        if self._load_errors:
            self.statusBar().showMessage(t("{n} sözlük açılamadı:", n=len(self._load_errors)) + " " + "; ".join(self._load_errors),
                                         12000)
        elif self._loaded_count:
            self.statusBar().showMessage(t("{n} sözlük yüklendi", n=self._loaded_count), 4000)
        else:
            self.statusBar().clearMessage()
        if self.prefs["general.restore_tabs"] and self.prefs["state.open_tabs"]:
            words = [w for w in self.prefs["state.open_tabs"] if isinstance(w, str) and w]
            if words:
                first = True
                for w in words:
                    entries = self._search_with_fallback(w)[0]
                    if first:
                        self.tabs.show_in_current(w, self._with_media(entries))
                        first = False
                    else:
                        self.tabs.show_in_new_tab(w, self._with_media(entries))
                self.tabs.setCurrentIndex(min(int(self.prefs["state.current_tab"]), self.tabs.count() - 1))
                return
        if self.search_box.text().strip():
            self._run_search()

    # ================================================================ prefs
    def apply_prefs(self) -> None:
        p = self.prefs
        app = QApplication.instance()
        if app is not None:
            app.setStyleSheet(themes.qss(p))
            if p["appearance.ui_font_family"]:
                app.setFont(QFont(p["appearance.ui_font_family"], int(p["appearance.ui_font_size"])))
        self.tabs.apply_prefs()
        self.toolbar.setVisible(p["appearance.show_toolbar"])
        self.dictionary_bar.setVisible(p["appearance.show_dictionary_bar"])
        self.sidebar.setVisible(p["appearance.show_sidebar"])
        self.found_dock.setVisible(p["appearance.show_found_in"])
        self.statusBar().setVisible(p["appearance.show_statusbar"])
        area = Qt.DockWidgetArea.RightDockWidgetArea if p["appearance.sidebar_side"] == "right" \
            else Qt.DockWidgetArea.LeftDockWidgetArea
        if self.dockWidgetArea(self.sidebar) != area:
            self.addDockWidget(area, self.sidebar)
        for key, a in self.view_toggles.items():
            a.blockSignals(True)
            a.setChecked(bool(p[key]))
            a.blockSignals(False)
        self.ontop_action.blockSignals(True)
        self.ontop_action.setChecked(bool(p["general.always_on_top"]))
        self.ontop_action.blockSignals(False)
        on_top = bool(self.windowFlags() & Qt.WindowType.WindowStaysOnTopHint)
        if on_top != bool(p["general.always_on_top"]):
            visible = self.isVisible()
            self.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint, bool(p["general.always_on_top"]))
            if visible:
                self.show()
        self.theme_actions.get(p["appearance.theme"], self.theme_actions["auto"]).setChecked(True)
        self._update_zoom_label()
        self._refresh_dictionaries()
        self._apply_scan_prefs()

    def _toggle_pref(self, key: str, on: bool) -> None:
        self.prefs.set(key, on, save=True)
        self.apply_prefs()

    def _open_preferences(self) -> None:
        dlg = PreferencesDialog(self.prefs, self)
        dlg.preferences_changed.connect(lambda _p: self.apply_prefs())
        dlg.action_requested.connect(self._on_prefs_action)
        dlg.exec()

    def _on_prefs_action(self, action: str) -> None:
        if action == "open_data":
            self._open_data_dir()
        elif action == "reindex":
            qm = self.query_manager
            if qm.index_store is not None:
                for n in qm.loaded_names():
                    qm.index_store.index_dictionary(qm.dictionaries[n])
                QMessageBox.information(self, t("İndeks"), t("{n} sözlük yeniden indekslendi.", n=len(qm.loaded_names())))
        elif action == "check_updates":
            self._check_updates(silent=False)
        elif action == "clear_history":
            self.history.clear_history()
            self._refresh_history_pane()
        elif action == "clear_favorites":
            self.history.clear_favorites()
            self._refresh_favorites_pane()

    # ================================================================ search
    def _on_search_text_changed(self, text: str) -> None:
        self._search_timer.start(int(self.prefs["search.debounce_ms"]))

    def _search_with_fallback(self, query: str, dictionaries: list[str] | None = None):
        p = self.prefs
        limit = int(p["search.max_articles"])
        entries = self._case_filter(
            query, self.query_manager.search(query, exact=True, dictionaries=dictionaries, max_articles=limit))
        if not entries and p["search.prefix_fallback"]:
            entries = self.query_manager.search(query, exact=False, dictionaries=dictionaries, max_articles=limit)
        fuzzy = self._fuzzy(query) if not entries else []
        return entries, fuzzy

    def _case_filter(self, query: str, entries: list[DictionaryEntry]) -> list[DictionaryEntry]:
        """With search.ignore_case off, exact matches must match case too."""
        if self.prefs["search.ignore_case"]:
            return entries
        return [e for e in entries if e.headword == query]

    def _fuzzy(self, query: str) -> list[tuple[str, float]]:
        p = self.prefs
        if not p["search.fuzzy"]:
            return []
        return self.query_manager.fuzzy(query, int(p["search.fuzzy_limit"]), float(p["search.fuzzy_cutoff"]))

    def _run_search(self) -> None:
        query = self.search_box.text().strip()
        p = self.prefs
        if len(query) < int(p["search.min_chars"]):
            self.suggestions.clear()
            self.suggestions.hide()
            return
        fill = int(p["search.fuzzy_fill_below"]) if p["search.fuzzy"] else 0
        suggestions = self.query_manager.suggest(
            query, limit=int(p["search.suggestion_limit"]), fuzzy_fill_below=fill,
            fuzzy_limit=int(p["search.fuzzy_limit"]), fuzzy_cutoff=float(p["search.fuzzy_cutoff"]))
        self._populate_suggestions(suggestions)
        if not p["search.live"]:
            return
        started = time.perf_counter()
        entries = self._case_filter(
            query, self.query_manager.search(query, exact=True, max_articles=int(p["search.max_articles"])))
        if entries:
            self._display_in_current(query, entries, None, started)
            self._push_nav(query)

    def _run_search_explicit(self) -> None:
        if self.prefs["search.enter_new_tab"]:
            self._run_search_new_tab()
            return
        query = self.search_box.text().strip()
        if not query:
            return
        started = time.perf_counter()
        entries, fuzzy = self._search_with_fallback(query)
        self._display_in_current(query, entries, fuzzy, started)
        self._push_nav(query)
        self.suggestions.hide()

    def _run_search_new_tab(self) -> None:
        query = self.search_box.text().strip()
        if not query:
            return
        started = time.perf_counter()
        entries, fuzzy = self._search_with_fallback(query)
        self.tabs.show_in_new_tab(query, self._with_media(entries), fuzzy)
        self._after_display(query, entries, started)
        self._push_nav(query)
        self.suggestions.hide()

    def _populate_suggestions(self, items: list[Suggestion]) -> None:
        self.suggestions.clear()
        for s in items:
            it = QListWidgetItem(f"≈ {s.word}" if s.fuzzy else s.word, self.suggestions)
            it.setData(Qt.ItemDataRole.UserRole, s.word)
            if s.fuzzy:
                font = it.font()
                font.setItalic(True)
                it.setFont(font)
                it.setToolTip(t("Benzer kelime (bulanık eşleşme)"))
        self.suggestions.setVisible(bool(items))

    def _on_suggestion_activated(self, item: QListWidgetItem) -> None:
        word = item.data(Qt.ItemDataRole.UserRole) or item.text()
        self.search_box.blockSignals(True)
        self.search_box.setText(word)
        self.search_box.blockSignals(False)
        self._open_word(word)
        self.suggestions.hide()

    def _with_media(self, entries: list[DictionaryEntry]) -> list[DictionaryEntry]:
        out = []
        show_images = self.prefs["article.show_images"]
        for e in entries:
            resolver = self.query_manager.media(e.dictionary_id) if show_images else None
            definition = rewrite_media_html(e.definition, e.dictionary_id, resolver)
            out.append(DictionaryEntry(e.headword, definition, e.dictionary_id, e.score))
        return out

    def _display_in_current(self, word: str, entries: list[DictionaryEntry],
                            fuzzy: list[tuple[str, float]] | None = None, started: float | None = None) -> None:
        if not entries and fuzzy is None:
            fuzzy = self._fuzzy(word) if word else []
        self.tabs.show_in_current(word, self._with_media(entries), fuzzy)
        self._after_display(word, entries, started)

    def _after_display(self, word: str, entries: list[DictionaryEntry], started: float | None) -> None:
        self._update_favorite_button(word)
        self._refresh_found_in(entries)
        if started is not None:
            ms = (time.perf_counter() - started) * 1000
            n = len({e.dictionary_id for e in entries})
            self.timing_label.setText(t("{n} madde · {m} sözlük", n=len(entries), m=n) + f" · {ms:.0f} ms")
        if word and entries:
            if self.prefs["history.enabled"]:
                self.history.record(word)
                self._refresh_history_pane()
            if self.prefs["audio.auto_play"]:
                self._auto_play(entries)

    def _auto_play(self, entries: list[DictionaryEntry]) -> None:
        import re

        for e in entries:
            m = re.search(r'<object\b[^>]*\bdata="([^"]+)"', e.definition) or re.search(
                r'href="sound://([^"]+)"', e.definition)
            if m:
                self._play_audio(e.dictionary_id, m.group(1))
                return

    def _open_word(self, word: str, dictionaries: list[str] | None = None, new_tab: bool = False) -> None:
        if not word:
            return
        self.search_box.blockSignals(True)
        self.search_box.setText(word)
        self.search_box.blockSignals(False)
        started = time.perf_counter()
        entries, fuzzy = self._search_with_fallback(word, dictionaries)
        if new_tab:
            self.tabs.show_in_new_tab(word, self._with_media(entries), fuzzy)
            self._after_display(word, entries, started)
        else:
            self._display_in_current(word, entries, fuzzy, started)
        self._push_nav(word)

    def _random_word(self) -> None:
        e = self.query_manager.random_entry()
        if e is None:
            self.statusBar().showMessage(t("Önce bir sözlük ekleyin."), 3000)
            return
        self._open_word(e.headword)

    # ================================================================ found-in
    def _refresh_found_in(self, entries: list[DictionaryEntry]) -> None:
        self.found_list.clear()
        for i, (name, items) in enumerate(group_by_dictionary(entries)):
            it = QListWidgetItem(dicticons.qicon(name), f"{name}  ({len(items)})" if len(items) > 1 else name)
            it.setData(Qt.ItemDataRole.UserRole, i)
            it.setToolTip(dicticons.look(name).tooltip() + "\n\n" + t("Tıkla: makaleye git"))
            self.found_list.addItem(it)

    def _on_found_in_clicked(self, item: QListWidgetItem) -> None:
        v = self.tabs.current_view()
        if v is not None:
            v.scroll_to_dictionary(int(item.data(Qt.ItemDataRole.UserRole)))

    # ================================================================ nav
    def _push_nav(self, word: str) -> None:
        if self._applying_nav or not word:
            return
        if self._nav_cursor >= 0 and self._nav_history[self._nav_cursor] == word:
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
            self._open_word(word)
        finally:
            self._applying_nav = False
            self._update_nav_buttons()

    def _update_nav_buttons(self) -> None:
        self.back_action.setEnabled(self._nav_cursor > 0)
        self.fwd_action.setEnabled(self._nav_cursor + 1 < len(self._nav_history))

    def _cycle_tab(self, delta: int) -> None:
        n = self.tabs.count()
        if n:
            self.tabs.setCurrentIndex((self.tabs.currentIndex() + delta) % n)

    def _on_current_tab_word_changed(self, word: str) -> None:
        self._update_favorite_button(word)
        v = self.tabs.current_view()
        self._refresh_found_in(v.entries if v else [])

    # ================================================================ favorites / history
    def _toggle_favorite_current(self) -> None:
        word = self.tabs.current_word() or self.search_box.text().strip()
        if not word:
            self.favorite_action.setChecked(False)
            return
        now_fav = self.history.toggle_favorite(word)
        self.favorite_action.setChecked(now_fav)
        self.statusBar().showMessage((t("Favorilere eklendi") if now_fav else t("Favorilerden çıkarıldı")) + f": {word}", 2500)
        self._refresh_favorites_pane()

    def _update_favorite_button(self, word: str) -> None:
        self.favorite_action.setChecked(bool(word) and self.history.is_favorite(word))

    def _refresh_favorites_pane(self) -> None:
        self.sidebar.set_favorites(self.history.favorites(int(self.prefs["history.favorites_limit"])))

    def _refresh_history_pane(self) -> None:
        self.sidebar.set_history(self.history.recent(int(self.prefs["history.limit"])))

    def _remove_history_word(self, word: str) -> None:
        self.history.remove_history(word)
        self._refresh_history_pane()

    def _remove_favorite(self, word: str) -> None:
        self.history.remove_favorite(word)
        self._refresh_favorites_pane()
        self._update_favorite_button(self.tabs.current_word())

    def _export_favorites(self) -> None:
        path, _ = QFileDialog.getSaveFileName(self, t("Favorileri dışa aktar"), "dictz-favoriler.txt",
                                              t("Metin (*.txt);;CSV (*.csv)"))
        if path:
            words = self.history.favorites(1_000_000)
            Path(path).write_text("\n".join(words) + "\n", encoding="utf-8")
            self.statusBar().showMessage(t("{n} favori kaydedildi: {path}", n=len(words), path=path), 4000)

    def _clear_history(self) -> None:
        if QMessageBox.question(self, t("Geçmişi temizle"), t("Tüm arama geçmişini silmek istediğinizden emin misiniz?")) \
                == QMessageBox.StandardButton.Yes:
            self.history.clear_history()
            self._refresh_history_pane()

    # ================================================================ dictionaries
    def _refresh_dictionaries(self) -> None:
        qm = self.query_manager
        self.sidebar.set_dictionaries([(n, qm.is_enabled(n), qm.dictionaries[n].metadata.word_count)
                                       for n in qm.loaded_names()])
        self._refresh_dictionary_bar()
        self._refresh_favorites_pane()
        self._refresh_history_pane()
        self._refresh_status()

    def _refresh_status(self) -> None:
        qm = self.query_manager
        enabled = [n for n in qm.loaded_names() if qm.is_enabled(n)]
        total = sum(qm.dictionaries[n].metadata.word_count for n in enabled)
        self.dict_count_label.setText(t("{n} sözlük · {w} kelime", n=len(enabled), w=f"{total:,}") if enabled else t("0 sözlük yüklü"))

    def _set_dictionary_enabled(self, name: str, enabled: bool) -> None:
        if self.query_manager.is_enabled(name) == enabled:
            return
        self.query_manager.set_enabled(name, enabled)
        self._refresh_dictionaries()

    def _enable_all(self, on: bool) -> None:
        for n in self.query_manager.loaded_names():
            self.query_manager.set_enabled(n, on)
        self._refresh_dictionaries()

    def _solo_dictionary(self, name: str) -> None:
        for n in self.query_manager.loaded_names():
            self.query_manager.set_enabled(n, n == name)
        self._refresh_dictionaries()
        self.statusBar().showMessage(t("Yalnızca “{name}” etkin — Sözlükler → Tümünü etkinleştir ile geri alın", name=name), 5000)

    def _fill_dictionary_menu(self) -> None:
        m = self.dict_menu
        m.clear()
        qm = self.query_manager
        for n in qm.loaded_names():
            a = QAction(n, m, checkable=True)
            a.setChecked(qm.is_enabled(n))
            a.toggled.connect(lambda on, name=n: self._set_dictionary_enabled(name, on))
            m.addAction(a)
        if qm.loaded_names():
            m.addSeparator()
        m.addAction(t("Tümünü etkinleştir"), lambda: self._enable_all(True))
        m.addAction(t("Tümünü devre dışı bırak"), lambda: self._enable_all(False))
        m.addSeparator()
        m.addAction(icon("dict"), t("Sözlük yöneticisi…"), self._open_dictionary_manager)
        m.addAction(icon("open"), t("Sözlük ekle…"), self._add_dictionary_dialog)
        m.addAction(icon("folder"), t("Klasör ekle…"), self._add_folder_dialog)

    def _show_dictionary_info(self, name: str) -> None:
        if name in self.query_manager.dictionaries:
            QMessageBox.information(self, name, info_html(self.query_manager, name))

    def add_paths(self, paths: list[Path]) -> None:
        added = 0
        for p in paths:
            try:
                d = self.query_manager.add_dictionary(Path(p))
                added += 1
                self.statusBar().showMessage(t("Yüklendi: {name} ({n} kelime)", name=d.metadata.name, n=f"{d.metadata.word_count:,}"), 4000)
            except Exception as e:  # noqa: BLE001
                QMessageBox.critical(self, t("Yüklenemedi"), f"<b>{p}</b><br><br>{type(e).__name__}: {e}")
        if added > 1:
            self.statusBar().showMessage(t("{n} sözlük eklendi", n=added), 4000)
        self._refresh_dictionaries()

    def _add_dictionary_dialog(self) -> None:
        paths, _ = QFileDialog.getOpenFileNames(self, t("Sözlük dosyası seçin"), "", file_filter())
        if paths:
            self.add_paths([Path(p) for p in paths])

    def _add_folder_dialog(self) -> None:
        d = QFileDialog.getExistingDirectory(self, t("Sözlük klasörü seçin"))
        if not d:
            return
        files = scan_folder(d, bool(self.prefs["dictionaries.recursive"]))
        if not files:
            QMessageBox.information(self, t("Klasör ekle"), t("Bu klasörde sözlük dosyası bulunamadı."))
            return
        known = {str(Path(self.query_manager.dictionaries[n].metadata.file_path).resolve())
                 for n in self.query_manager.loaded_names()}
        new = [f for f in files if str(f.resolve()) not in known]
        if QMessageBox.question(
                self, t("Klasör ekle"),
                t("{n} sözlük bulundu, {m} tanesi yeni.\nYeni olanlar eklensin mi?").format(n=len(files), m=len(new))
                + "\n\n" + t("İpucu: Tercihler → Sözlükler'e klasör ekleyince her açılışta otomatik taranır.")) \
                == QMessageBox.StandardButton.Yes:
            self.add_paths(new)

    def _open_dictionary_manager(self) -> None:
        dlg = DictionaryManagerDialog(self.query_manager, self)
        dlg.dictionaries_changed.connect(self._refresh_dictionaries)
        dlg.add_paths_requested.connect(lambda paths: (self.add_paths(paths), dlg.refresh()))
        dlg.exec()

    def dragEnterEvent(self, event):  # noqa: N802
        if event.mimeData().hasUrls():
            event.acceptProposedAction()

    def dropEvent(self, event):  # noqa: N802
        paths: list[Path] = []
        for url in event.mimeData().urls():
            p = Path(url.toLocalFile())
            paths += scan_folder(p, True) if p.is_dir() else ([p] if is_dictionary_file(p) else [])
        if paths:
            self.add_paths(paths)

    # ================================================================ view
    def _zoom(self, value: float) -> None:
        self.prefs.set("article.zoom", round(value, 2), save=True)
        self.tabs.set_zoom(float(self.prefs["article.zoom"]))
        self._update_zoom_label()

    def _zoom_in(self) -> None:
        self._zoom(float(self.prefs["article.zoom"]) + 0.1)

    def _zoom_out(self) -> None:
        self._zoom(float(self.prefs["article.zoom"]) - 0.1)

    def _zoom_reset(self) -> None:
        self._zoom(1.0)

    def _update_zoom_label(self) -> None:
        self.zoom_label.setText(f"{round(float(self.prefs['article.zoom']) * 100)}%")

    def _set_theme(self, theme: str) -> None:
        self.prefs.set("appearance.theme", theme, save=True)
        self.apply_prefs()
        label = {"auto": t("Otomatik"), "light": t("Açık"), "dark": t("Koyu")}[theme]
        self.statusBar().showMessage(f"Tema: {label}", 2500)

    def _cycle_theme(self) -> None:
        order = ["auto", "light", "dark"]
        cur = self.prefs["appearance.theme"]
        self._set_theme(order[(order.index(cur) + 1) % 3 if cur in order else 0])

    def _toggle_fullscreen(self, on: bool) -> None:
        self.showFullScreen() if on else self.showNormal()

    def _all_open(self, opened: bool) -> None:
        v = self.tabs.current_view()
        if v:
            v.set_all_open(opened)

    def _focus_search(self) -> None:
        self.search_box.setFocus()
        self.search_box.selectAll()

    def _open_find(self) -> None:
        self.find_bar.show()
        self.find_box.setFocus()
        self.find_box.selectAll()

    def _close_find(self) -> None:
        self.find_box.clear()
        v = self.tabs.current_view()
        if v:
            v.find("")
        self.find_bar.hide()

    def _find(self, backward: bool) -> None:
        if not self.find_bar.isVisible():
            self._open_find()
        v = self.tabs.current_view()
        if v:
            v.find(self.find_box.text(), backward, self.find_case.isChecked())

    def _on_find_result(self, active: int, total: int) -> None:
        if not self.find_box.text():
            self.find_count.setText("")
        else:
            self.find_count.setText(f"{active}/{total}" if total else t("bulunamadı"))

    # ================================================================ article actions
    def _copy_current_word(self) -> None:
        word = self.tabs.current_word()
        if word:
            QApplication.clipboard().setText(word)
            self.statusBar().showMessage(t("Kopyalandı: {word}", word=word), 2000)

    def _copy_article(self, group: int | None = None) -> None:
        v = self.tabs.current_view()
        if v and v.entries:
            QApplication.clipboard().setText(v.article_text(group if isinstance(group, int) else None))
            self.statusBar().showMessage(t("Makale kopyalandı"), 2000)

    def _save_article(self) -> None:
        v = self.tabs.current_view()
        if not v or not v.entries:
            return
        path, _ = QFileDialog.getSaveFileName(self, t("Makaleyi kaydet"), f"{v.word}.html",
                                              t("HTML (*.html);;Metin (*.txt)"))
        if not path:
            return
        text = v.article_text() if path.lower().endswith(".txt") else v.article_html()
        Path(path).write_text(text, encoding="utf-8")
        self.statusBar().showMessage(f"Kaydedildi: {path}", 4000)

    def _on_definition_link(self, url: str) -> None:
        from urllib.parse import unquote

        if url.startswith(("dict:", "bword:")):
            self._open_word(unquote(url.split(":", 1)[1].lstrip("/")))
            return
        if url.startswith("dictz-action:"):
            _, action, idx = url.split(":", 2)
            v = self.tabs.current_view()
            if v is None:
                return
            groups = group_by_dictionary(v.entries)
            i = int(idx)
            if action == "copy":
                self._copy_article(i)
            elif action == "only" and i < len(groups):
                self._open_word(v.word, [groups[i][0]], new_tab=True)
            return
        audio = parse_audio_url(url)
        if audio is not None:
            self._play_audio(*audio)
            return
        if url.startswith(("http://", "https://", "mailto:")):
            QDesktopServices.openUrl(QUrl(url))
            return
        if url.startswith("sound://") or media_kind(url) == "audio":
            v = self.tabs.current_view()
            if v and v.entries:
                self._play_audio(v.entries[0].dictionary_id, url.removeprefix("sound://"))

    def _play_audio(self, dictionary: str, name: str) -> None:
        import hashlib

        resolver = self.query_manager.media(dictionary)
        data = resolver.read(name) if resolver else None
        if data is None:
            self.statusBar().showMessage(t("Ses dosyası bulunamadı: {name}", name=name), 4000)
            return
        from ..app import user_data_dir

        cache = user_data_dir() / "media-cache"
        cache.mkdir(parents=True, exist_ok=True)
        suffix = Path(name).suffix.lower() or ".wav"
        target = cache / (hashlib.sha1(f"{dictionary}/{name}".encode()).hexdigest()[:16] + suffix)
        if not target.exists():
            target.write_bytes(data)
        self.statusBar().showMessage(t("Çalınıyor: {name}", name=name), 3000)
        if self.prefs["audio.system_player"]:
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(target)))
            return
        try:
            from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer

            if not hasattr(self, "_player"):
                self._audio_out = QAudioOutput(self)
                self._player = QMediaPlayer(self)
                self._player.setAudioOutput(self._audio_out)
            self._audio_out.setVolume(int(self.prefs["audio.volume"]) / 100.0)
            self._player.stop()
            self._player.setSource(QUrl.fromLocalFile(str(target)))
            self._player.play()
        except Exception:  # noqa: BLE001 — no multimedia backend: hand it to the OS
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(target)))

    # ================================================================ help
    def _show_shortcuts(self) -> None:
        from ..core.shortcuts import SHORTCUTS

        rows = "".join(f"<tr><td><b>{k}</b></td><td>{t(v)}</td></tr>" for k, v in SHORTCUTS)
        QMessageBox.information(self, t("Kısayollar"), f"<table cellpadding=3>{rows}</table>")

    def _show_cli_help(self) -> None:
        from ..cli import COMMANDS

        rows = "".join(f"<tr><td><code>dictz {c}</code></td><td>{d}</td></tr>" for c, d in COMMANDS)
        QMessageBox.information(
            self, t("Komut satırı"),
            "<p>" + t("dictz komut satırından da tam olarak kullanılabilir (Rust sürümünde aynı komutlar):") + "</p>"
            + f"<table cellpadding=2>{rows}</table>"
            + "<p>" + t("Ayrıntı:") + " <code>dictz help</code>, <code>dictz &lt;komut&gt; --help</code></p>")

    def _open_data_dir(self) -> None:
        from ..app import user_data_dir

        QDesktopServices.openUrl(QUrl.fromLocalFile(str(user_data_dir())))

    def _create_shortcuts(self) -> None:
        from ..app import user_data_dir
        from ..core.desktop import create_shortcuts

        try:
            paths = create_shortcuts(user_data_dir())
        except (OSError, RuntimeError) as exc:
            QMessageBox.warning(self, t("Kısayol"), t("Kısayol oluşturulamadı:") + f"\n{exc}")
            return
        if paths:
            QMessageBox.information(self, t("Kısayol"), t("Oluşturuldu:") + "\n" + "\n".join(str(p) for p in paths))
        else:
            QMessageBox.warning(self, t("Kısayol"), t("Masaüstü veya menü klasörü bulunamadı."))

    def _check_updates(self, silent: bool) -> None:
        if self._update_checker is not None and self._update_checker.isRunning():
            return
        self._update_checker = UpdateChecker(self)
        self._update_checker.result.connect(lambda latest, err: self._on_update_result(latest, err, silent))
        self._update_checker.start()
        if not silent:
            self.statusBar().showMessage(t("Güncellemeler denetleniyor…"), 3000)

    def _on_update_result(self, latest: str, error: str, silent: bool) -> None:
        from ..core.updater import detect_install, parse_version

        if error:
            if not silent:
                QMessageBox.warning(self, t("Güncelleme"), t("Denetlenemedi:") + f"\n{error}")
            return
        if parse_version(latest) <= parse_version(__version__):
            if not silent:
                QMessageBox.information(self, t("Güncelleme"), t("dictz {v} güncel.", v=__version__))
            return
        inst = detect_install()
        cmd = "dictz update"
        msg = (t("Yeni sürüm var: {latest} (kurulu: {v}).", latest=f"<b>{latest}</b>", v=__version__) + "<br><br>"
               + t("Kurulum:") + f" {inst.detail}<br>" + t("Güncellemek için terminalde:") + f"<br><code>{cmd}</code>")
        if silent:
            self.statusBar().showMessage(t("Yeni sürüm: dictz {latest} — terminalde `dictz update`", latest=latest), 15000)
        else:
            QMessageBox.information(self, t("Güncelleme"), msg)

    def _about(self) -> None:
        QMessageBox.about(
            self, t("dictz hakkında"),
            f"<h3>dictz {__version__}</h3><p>" + t("Çok formatlı çevrimdışı sözlük — Python sürümü.") + "</p>"
            + "<p>" + t("Rust sürümü aynı özelliklere sahiptir:") + " <code>cargo install dictz</code></p>"
            "<p>© 2026 Bayram Kotan · GPL-3.0+</p>"
            '<p><a href="https://github.com/bayramkotan/dictz">github.com/bayramkotan/dictz</a></p>')

    # ================================================================ close
    def closeEvent(self, event):  # noqa: N802
        if (not self._really_quit and self.prefs["general.close_to_tray"]
                and self.tray is not None and self.tray.isVisible()):
            self.hide()
            event.ignore()
            if not self._tray_hint_shown:
                self._tray_hint_shown = True
                self.tray.showMessage("dictz", t("dictz tepside çalışmaya devam ediyor."),
                                      QSystemTrayIcon.MessageIcon.Information, 3000)
            return
        if self.prefs["general.confirm_quit"] and QMessageBox.question(
                self, t("Çıkış"), t("dictz kapatılsın mı?")) != QMessageBox.StandardButton.Yes:
            event.ignore()
            return
        if self._loader is not None and self._loader.isRunning():
            self._loader.requestInterruption()
            self._loader.wait(3000)
        qs = QSettings()
        qs.setValue("window/geometry", self.saveGeometry())
        qs.setValue("window/state", self.saveState())
        self.prefs.set("state.last_search", self.search_box.text())
        self.prefs.set("state.open_tabs", self.tabs.open_words())
        self.prefs.set("state.current_tab", max(0, self.tabs.currentIndex()))
        if self.prefs["history.clear_on_exit"]:
            self.history.clear_history()
        with contextlib.suppress(OSError):
            self.prefs.save()
        self.stop_services()
        super().closeEvent(event)
        if self._restart_requested:
            _restart_detached()
        app = QApplication.instance()
        if app is not None:
            QTimer.singleShot(0, app.quit)

    # ================================================================ scan popup, tray, hotkeys
    def starts_hidden(self) -> bool:
        """``general.start_in_tray`` (only when a tray icon can be shown)."""
        return bool(self.prefs["general.start_in_tray"]) and self.tray is not None and self.tray.isVisible()

    def start_services(self, data_dir: Path) -> None:
        """Instance server (for `dictz show/popup/watch` and single instance)."""
        self._server = _instance.Server(data_dir, self._remote_handler, "python", __version__)
        if not self._server.start():
            self._server = None

    def stop_services(self) -> None:
        if self._hotkeys is not None:
            self._hotkeys.stop()
            self._hotkeys = None
        if self._server is not None:
            self._server.stop()
            self._server = None
        if self.tray is not None:
            self.tray.hide()
        if self.scan_popup is not None:
            self.scan_popup.close()

    def set_ui_language(self, code: str) -> None:
        """Save the interface language and restart dictz in it (tabs are restored)."""
        if code == self.prefs["general.language"]:
            return
        self.prefs.set("general.language", code, save=True)
        if i18n.resolve(code) == i18n.current():
            return
        self._restart_requested = True
        self.quit_app()

    def _build_language_button(self, tb: QToolBar) -> None:
        btn = QToolButton()
        btn.setObjectName("language_button")
        btn.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        btn.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextBesideIcon)
        cur = i18n.current()
        btn.setIcon(QIcon(str(i18n.flag_path(cur))))
        btn.setIconSize(QSize(24, 18))
        btn.setText(cur.upper())
        btn.setToolTip(t("Arayüz dili") + f": {i18n.native_name(cur)}")
        menu = QMenu(btn)
        group = QActionGroup(menu)
        setting = self.prefs["general.language"]
        auto = menu.addAction(t("Otomatik (sistem)") + f" — {i18n.native_name(i18n.system_language())}")
        auto.setCheckable(True)
        auto.setChecked(setting == "auto")
        auto.triggered.connect(lambda: self.set_ui_language("auto"))
        group.addAction(auto)
        menu.addSeparator()
        for code, native, english, _rtl in i18n.LANGUAGES:
            a = menu.addAction(QIcon(str(i18n.flag_path(code))), f"{native}   ({english})")
            a.setCheckable(True)
            a.setChecked(setting == code)
            a.triggered.connect(lambda _c=False, c=code: self.set_ui_language(c))
            group.addAction(a)
        btn.setMenu(menu)
        tb.addWidget(btn)
        self.language_button = btn

    def quit_app(self) -> None:
        self._really_quit = True
        if not self.isVisible():
            self.show()
        self.close()

    def _apply_scan_prefs(self) -> None:
        p = self.prefs
        self._scan_on = bool(p["scan.enabled"])
        self.scanner.set_enabled(self._scan_on)
        self.scan_action.blockSignals(True)
        self.scan_action.setChecked(self._scan_on)
        self.scan_action.blockSignals(False)
        # tray
        if p["general.tray_icon"] and QSystemTrayIcon.isSystemTrayAvailable():
            if self.tray is None:
                self._build_tray()
            self.tray.show()
        elif self.tray is not None:
            self.tray.hide()
        if self.tray is not None:
            self._tray_scan.blockSignals(True)
            self._tray_scan.setChecked(self._scan_on)
            self._tray_scan.blockSignals(False)
        app = QApplication.instance()
        if app is not None:
            app.setQuitOnLastWindowClosed(not (self.tray is not None and self.tray.isVisible()))
        # hotkeys
        conf = (bool(p["hotkeys.enabled"]), p["hotkeys.show_main"], p["hotkeys.toggle_scan"],
                p["hotkeys.lookup_clipboard"])
        if conf != self._hotkey_conf:
            self._hotkey_conf = conf
            if self._hotkeys is not None:
                self._hotkeys.stop()
                self._hotkeys = None
            if conf[0]:
                self._hotkeys = HotkeyListener(
                    {"show_main": conf[1], "toggle_scan": conf[2], "lookup_clipboard": conf[3]},
                    self.hotkey_pressed.emit)
                self._hotkeys.start()
                if self._hotkeys.errors:
                    msg = t("Genel kısayollar:") + " " + "; ".join(self._hotkeys.errors)
                    if wayland_session():
                        msg += " — " + t("masaüstü ayarlarından `dictz show` / `dictz popup` komutuna kısayol atayın.")
                    self.statusBar().showMessage(msg, 10000)
        if self.scan_popup is not None:
            self.scan_popup.apply_prefs()

    def _build_tray(self) -> None:
        self.tray = QSystemTrayIcon(app_icon(), self)
        self.tray.setToolTip(f"dictz {__version__}")
        menu = QMenu(self)
        menu.addAction(t("Göster / gizle"), self.toggle_main)
        self._tray_scan = menu.addAction(t("Tarama"))
        self._tray_scan.setCheckable(True)
        self._tray_scan.toggled.connect(self.set_scan)
        menu.addAction(t("Panodaki kelimeyi ara"), lambda: self.popup_lookup(""))
        menu.addSeparator()
        menu.addAction(t("Tercihler…"), lambda: (self.show_main(), self._open_preferences()))
        menu.addAction(t("Çıkış"), self.quit_app)
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(self._on_tray_activated)

    def _on_tray_activated(self, reason) -> None:
        if reason in (QSystemTrayIcon.ActivationReason.Trigger, QSystemTrayIcon.ActivationReason.DoubleClick):
            self.toggle_main()

    def set_scan(self, on: bool) -> None:
        on = bool(on)
        self._scan_on = on
        if bool(self.prefs["scan.enabled"]) != on:
            self.prefs.set("scan.enabled", on, save=True)
        self._apply_scan_prefs()
        self.statusBar().showMessage(t("Tarama açık") if on else t("Tarama kapalı"), 3000)

    def show_main(self) -> None:
        if self.isMinimized():
            self.showNormal()
        else:
            self.show()
        self.raise_()
        self.activateWindow()

    def toggle_main(self) -> None:
        if self.isVisible() and not self.isMinimized():
            if self.tray is not None and self.tray.isVisible():
                self.hide()
            else:
                self.showMinimized()
        else:
            self.show_main()

    def _ensure_popup(self) -> ScanPopup:
        if self.scan_popup is None:
            self.scan_popup = ScanPopup(self.prefs)
            self.scan_popup.lookup_requested.connect(self.popup_lookup)
            self.scan_popup.open_in_main.connect(self._open_in_main)
            self.scan_popup.link_activated.connect(self._on_popup_link)
        return self.scan_popup

    def popup_lookup(self, word: str | None = "") -> None:
        """Show ``word`` (empty = the clipboard text) in the scan popup."""
        word = (word or "").strip()
        if not word:
            word = self.scanner.clipboard_word() or ""
            if not word:
                self.statusBar().showMessage(t("Panoda aranacak kelime yok."), 3000)
                return
        if not self._loading_done:
            self._pending_popup = word
            return
        entries, fuzzy = self._search_with_fallback(word)
        popup = self._ensure_popup()
        popup.show_result(word, self._with_media(entries), fuzzy, bool(self.prefs["scan.near_cursor"]))
        if entries:
            if self.prefs["history.enabled"] and self.prefs["scan.record_history"]:
                self.history.record(word)
                self._refresh_history_pane()
            if self.prefs["audio.auto_play"]:
                self._auto_play(entries)

    def _open_in_main(self, word: str) -> None:
        if self.scan_popup is not None:
            self.scan_popup.hide()
        self.show_main()
        if word:
            self._open_word(word)

    def _on_popup_link(self, url: str) -> None:
        from urllib.parse import unquote

        if url.startswith(("dict:", "bword:")):
            self.popup_lookup(unquote(url.split(":", 1)[1].lstrip("/")))
            return
        if url.startswith("dictz-action:"):
            _, action, idx = url.split(":", 2)
            view = self.scan_popup.view if self.scan_popup else None
            if view is None:
                return
            groups = group_by_dictionary(view.entries)
            i = int(idx)
            if i >= len(groups):
                return
            if action == "copy":
                from ..core.htmltext import to_text

                self.scanner.ignore_for(1.0)
                QApplication.clipboard().setText(
                    "\n\n".join(f"{e.headword}\n{to_text(e.definition)}" for e in groups[i][1]))
            elif action == "only":
                self._open_in_main("")
                self._open_word(view.word, [groups[i][0]], new_tab=True)
            return
        audio = parse_audio_url(url)
        if audio is not None:
            self._play_audio(*audio)
            return
        if url.startswith(("http://", "https://", "mailto:")):
            QDesktopServices.openUrl(QUrl(url))
            return
        view = self.scan_popup.view if self.scan_popup else None
        if (url.startswith("sound://") or media_kind(url) == "audio") and view and view.entries:
            self._play_audio(view.entries[0].dictionary_id, url.removeprefix("sound://"))

    def _on_hotkey(self, name: str) -> None:
        if name == "show_main":
            self.toggle_main()
        elif name == "toggle_scan":
            self.set_scan(not self._scan_on)
        elif name == "lookup_clipboard":
            self.popup_lookup("")

    # ---------------------------------------------------- `dictz show / popup / watch`
    def _remote_handler(self, req: dict) -> dict:
        """Runs in the instance-server thread; the work is done on the GUI thread."""
        cmd = req.get("cmd")
        if cmd == "ping":
            return {"ok": True, "edition": "python", "version": __version__}
        if cmd == "scan":
            state = req.get("state", "status")
            if state not in ("on", "off", "toggle", "status"):
                return {"ok": False, "error": f"unknown state: {state}"}
            new = {"on": True, "off": False, "toggle": not self._scan_on}.get(state, self._scan_on)
            if state != "status":
                self._scan_on = new
                self.remote_request.emit({"cmd": "scan", "on": new})
            return {"ok": True, "scan": new}
        if cmd in ("show", "hide", "toggle", "lookup", "popup", "open", "quit"):
            self.remote_request.emit(dict(req))
            return {"ok": True}
        return {"ok": False, "error": f"unknown command: {cmd}"}

    def _on_remote(self, req: dict) -> None:
        cmd = req.get("cmd")
        if cmd == "show":
            self.show_main()
        elif cmd == "hide":
            self.hide()
        elif cmd == "toggle":
            self.toggle_main()
        elif cmd == "lookup":
            self.show_main()
            self._open_word(str(req.get("word", "")).strip())
        elif cmd == "popup":
            self.popup_lookup(str(req.get("word", "")))
        elif cmd == "scan":
            self.set_scan(bool(req.get("on")))
        elif cmd == "open":
            files = [Path(f) for f in req.get("files", []) if isinstance(f, str)]
            self.show_main()
            if files:
                self.add_paths(files)
        elif cmd == "quit":
            self.quit_app()


def _restart_detached() -> None:
    """Start dictz again with the same arguments (after a language change)."""
    from PySide6.QtCore import QProcess

    if getattr(sys, "frozen", False):
        program, args = sys.executable, sys.argv[1:]
    else:
        program = sys.executable
        if sys.platform == "win32":
            pyw = Path(sys.executable).with_name("pythonw.exe")
            program = str(pyw) if pyw.exists() else program
        args = ["-m", "dictz", *sys.argv[1:]]
    if os.environ.get("APPIMAGE"):
        program, args = os.environ["APPIMAGE"], sys.argv[1:]
    QProcess.startDetached(program, args)
