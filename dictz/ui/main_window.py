"""dictz main window — three-pane layout:

    +-------------------+----------------------------+
    | Search box        |                            |
    +-------------------+                            |
    | Suggestion list   |   Definition view          |
    | (headwords)       |   (HTML, all dictionaries) |
    |                   |                            |
    |                   |                            |
    +-------------------+----------------------------+
    | Status bar (loaded dicts, word counts)         |
    +-------------------+----------------------------+
"""
from __future__ import annotations

import html
from pathlib import Path

from PySide6.QtCore import Qt, QTimer, Signal
from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QSplitter,
    QStatusBar,
    QTextBrowser,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

from ..core.query_manager import QueryManager
from ..parsers.base import DictionaryEntry
from ..storage.indexer import IndexStore

# Time to wait after typing stops before firing a search (ms)
SEARCH_DEBOUNCE_MS = 150


class MainWindow(QMainWindow):
    dictionary_loaded = Signal(str)

    def __init__(self, query_manager: QueryManager, parent: QWidget | None = None):
        super().__init__(parent)
        self.query_manager = query_manager

        self.setWindowTitle("dictz")
        self.resize(1100, 720)

        self._build_ui()
        self._build_menus()
        self._build_toolbar()

        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.timeout.connect(self._run_search)

        self._refresh_status()

    # ----------------------------------------------------------- UI build

    def _build_ui(self) -> None:
        central = QWidget()
        self.setCentralWidget(central)

        # Left pane: search box + suggestion list
        left = QWidget()
        left_layout = QVBoxLayout(left)
        left_layout.setContentsMargins(8, 8, 8, 8)

        self.search_box = QLineEdit()
        self.search_box.setPlaceholderText("Bir kelime arayın…")
        self.search_box.setClearButtonEnabled(True)
        self.search_box.textChanged.connect(self._on_search_text_changed)
        self.search_box.returnPressed.connect(self._run_search)
        left_layout.addWidget(self.search_box)

        self.suggestion_list = QListWidget()
        self.suggestion_list.itemActivated.connect(self._on_suggestion_activated)
        self.suggestion_list.currentItemChanged.connect(self._on_suggestion_current_changed)
        left_layout.addWidget(self.suggestion_list, 1)

        # Right pane: definition view
        self.result_view = QTextBrowser()
        self.result_view.setOpenExternalLinks(True)
        self.result_view.setStyleSheet(
            "QTextBrowser { font-size: 14px; padding: 8px; }"
        )
        self._set_welcome_text()

        splitter = QSplitter(Qt.Orientation.Horizontal)
        splitter.addWidget(left)
        splitter.addWidget(self.result_view)
        splitter.setStretchFactor(0, 1)
        splitter.setStretchFactor(1, 3)
        splitter.setSizes([300, 800])

        layout = QHBoxLayout(central)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(splitter)

        # Status bar
        self.setStatusBar(QStatusBar())
        self.dict_count_label = QLabel("0 sözlük")
        self.statusBar().addPermanentWidget(self.dict_count_label)

    def _build_menus(self) -> None:
        menu = self.menuBar()

        file_menu = menu.addMenu("&Dosya")

        add_dict_action = QAction("Sözlük &Ekle…", self)
        add_dict_action.setShortcut(QKeySequence("Ctrl+O"))
        add_dict_action.triggered.connect(self.add_dictionary_dialog)
        file_menu.addAction(add_dict_action)

        file_menu.addSeparator()

        quit_action = QAction("Çı&kış", self)
        quit_action.setShortcut(QKeySequence.StandardKey.Quit)
        quit_action.triggered.connect(self.close)
        file_menu.addAction(quit_action)

        help_menu = menu.addMenu("&Yardım")
        about_action = QAction("&Hakkında", self)
        about_action.triggered.connect(self._show_about)
        help_menu.addAction(about_action)

    def _build_toolbar(self) -> None:
        tb = QToolBar("Main")
        tb.setMovable(False)
        self.addToolBar(tb)

        add_dict = QAction("Sözlük Ekle", self)
        add_dict.triggered.connect(self.add_dictionary_dialog)
        tb.addAction(add_dict)

    def _set_welcome_text(self) -> None:
        self.result_view.setHtml(
            """
            <div style="text-align: center; padding: 60px; color: #888;">
                <h2 style="margin-bottom: 4px;">dictz</h2>
                <p>Çok formatlı sözlük uygulaması</p>
                <p style="margin-top: 32px; font-size: 13px;">
                    Başlamak için <b>Dosya → Sözlük Ekle</b> menüsünden<br>
                    bir StarDict (.ifo) dosyası yükleyin.
                </p>
            </div>
            """
        )

    # -------------------------------------------------- search interaction

    def _on_search_text_changed(self, text: str) -> None:
        # Debounce: wait 150 ms after the last keystroke
        self._search_timer.start(SEARCH_DEBOUNCE_MS)

    def _run_search(self) -> None:
        query = self.search_box.text().strip()
        if not query:
            self.suggestion_list.clear()
            self._set_welcome_text()
            return

        # Update the suggestion list (prefix matches)
        self.suggestion_list.blockSignals(True)
        self.suggestion_list.clear()
        suggestions = self.query_manager.suggest(query, limit=100)
        for s in suggestions:
            QListWidgetItem(s, self.suggestion_list)
        self.suggestion_list.blockSignals(False)

        # Run an exact-match search and display results
        entries = self.query_manager.search(query, exact=True)
        if not entries and not suggestions:
            # Try fuzzy fallback (just shows prefix suggestions for now;
            # real fuzzy comes in v0.3 with rapidfuzz)
            entries = self.query_manager.search(query, exact=False)

        self._display_entries(query, entries)

    def _on_suggestion_activated(self, item: QListWidgetItem) -> None:
        # Double-click or Enter on suggestion -> jump to it
        word = item.text()
        self.search_box.setText(word)
        entries = self.query_manager.search(word, exact=True)
        self._display_entries(word, entries)

    def _on_suggestion_current_changed(self, current, _previous) -> None:
        # Single-click / arrow navigation: live preview
        if current is None:
            return
        word = current.text()
        entries = self.query_manager.search(word, exact=True)
        self._display_entries(word, entries)

    # --------------------------------------------------------- display

    def _display_entries(self, query: str, entries: list[DictionaryEntry]) -> None:
        if not entries:
            self.result_view.setHtml(
                f"""
                <div style="padding: 20px; color: #888;">
                    <p><b>{html.escape(query)}</b> için sonuç bulunamadı.</p>
                </div>
                """
            )
            return

        # Group by dictionary for readability
        parts: list[str] = []
        parts.append(
            f"""
            <style>
                body {{ font-family: 'Segoe UI', sans-serif; line-height: 1.6; }}
                .dict-header {{
                    background: #f3f3f3;
                    border-left: 3px solid #5d7eb0;
                    padding: 6px 10px;
                    margin: 16px 0 8px 0;
                    font-size: 12px;
                    color: #555;
                    font-weight: bold;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                }}
                .headword {{
                    font-size: 18px;
                    font-weight: bold;
                    color: #2a4d7a;
                    margin-bottom: 6px;
                }}
                .definition {{ padding-left: 8px; }}
                .definition pre {{ white-space: pre-wrap; font-family: inherit; margin: 0; }}
            </style>
            """
        )

        current_dict = None
        for entry in entries:
            if entry.dictionary_id != current_dict:
                current_dict = entry.dictionary_id
                parts.append(f'<div class="dict-header">{html.escape(current_dict)}</div>')

            parts.append(f'<div class="headword">{html.escape(entry.headword)}</div>')
            # StarDict definitions may already be HTML; for v0.0.1 we keep it
            # simple: if it looks like HTML, render as-is; otherwise wrap in <pre>
            definition = entry.definition
            if "<" in definition and ">" in definition:
                # Already HTML-ish — let QTextBrowser render it
                parts.append(f'<div class="definition">{definition}</div>')
            else:
                parts.append(
                    f'<div class="definition"><pre>{html.escape(definition)}</pre></div>'
                )

        self.result_view.setHtml("\n".join(parts))

    # ---------------------------------------------------- dictionary mgmt

    def add_dictionary_dialog(self) -> None:
        path_str, _ = QFileDialog.getOpenFileName(
            self,
            "Sözlük dosyası seçin",
            "",
            "StarDict (*.ifo);;Tüm dosyalar (*)",
        )
        if not path_str:
            return

        try:
            dictionary = self.query_manager.add_dictionary(Path(path_str))
        except Exception as e:
            QMessageBox.critical(
                self,
                "Sözlük yüklenemedi",
                f"<b>{path_str}</b><br><br>{type(e).__name__}: {e}",
            )
            return

        self._refresh_status()
        self.statusBar().showMessage(
            f"Yüklendi: {dictionary.metadata.name} "
            f"({dictionary.metadata.word_count:,} kelime)",
            5000,
        )
        self.dictionary_loaded.emit(dictionary.metadata.name)

    def _refresh_status(self) -> None:
        n = len(self.query_manager.dictionaries)
        total_words = sum(
            d.metadata.word_count for d in self.query_manager.dictionaries.values()
        )
        if n == 0:
            self.dict_count_label.setText("0 sözlük")
        else:
            self.dict_count_label.setText(f"{n} sözlük · {total_words:,} kelime")

    def _show_about(self) -> None:
        QMessageBox.about(
            self,
            "dictz hakkında",
            """
            <h3>dictz 0.0.1</h3>
            <p>Çok formatlı sözlük uygulaması (GoldenDict tarzı).</p>
            <p style="color: #777; font-size: 12px;">
                v0.0.1 — StarDict desteği, temel arama
            </p>
            <p>© 2026 Bayram Kotan · GPL-3.0+</p>
            """,
        )

    def closeEvent(self, event):  # noqa: N802 (Qt naming)
        self.query_manager.close_all()
        if self.query_manager.index_store:
            self.query_manager.index_store.close()
        super().closeEvent(event)
