"""Rich-HTML definition viewer."""
from __future__ import annotations

import html

from PySide6.QtCore import QUrl, Signal
from PySide6.QtWidgets import QTextBrowser, QWidget

from ..parsers.base import DictionaryEntry
from . import themes


try:
    from PySide6.QtWebEngineWidgets import QWebEngineView
    HAS_WEBENGINE = True
except ImportError:
    QWebEngineView = None  # type: ignore
    HAS_WEBENGINE = False


class DefinitionView(QWidget):
    link_activated = Signal(str)

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        from PySide6.QtWidgets import QVBoxLayout
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        self._dark = False
        self._zoom = 1.0

        if HAS_WEBENGINE:
            self._view = QWebEngineView(self)
            layout.addWidget(self._view)
            self._html_mode = "web"
        else:
            tb = QTextBrowser(self)
            tb.setOpenExternalLinks(False)
            tb.anchorClicked.connect(self._on_anchor_clicked)
            self._view = tb
            layout.addWidget(tb)
            self._html_mode = "text"

    def set_dark(self, dark: bool) -> None:
        self._dark = dark

    def set_zoom(self, factor: float) -> None:
        self._zoom = max(0.5, min(3.0, factor))
        if self._html_mode == "web":
            self._view.setZoomFactor(self._zoom)  # type: ignore[attr-defined]

    def show_welcome(self) -> None:
        body = """
        <div style="text-align: center; padding: 100px 24px; opacity: 0.6;">
            <h1 style="font-size: 32px; margin: 0;">dictz</h1>
            <p style="margin: 8px 0 24px 0;">çok formatlı çevrimdışı sözlük</p>
            <p style="font-size: 13px;">Başlamak için <b>Dosya → Sözlük Ekle</b> menüsünden<br>
            bir StarDict (.ifo), DSL (.dsl) veya Babylon (.bgl) dosyası yükleyin.</p>
            <p style="font-size: 12px; margin-top: 40px; opacity: 0.7;">
                <b>Ctrl+F</b> ara · <b>Ctrl+T</b> yeni sekme ·
                <b>Ctrl+D</b> favoriye ekle · <b>Ctrl++</b>/<b>Ctrl+-</b> yakınlaştır
            </p>
        </div>
        """
        self.set_html(body)

    def show_not_found(self, query: str) -> None:
        body = f"""
        <div style="padding: 48px 24px; opacity: 0.7;">
            <h2>Sonuç yok</h2>
            <p><b>{html.escape(query)}</b> için hiçbir sözlükte eşleşme bulunamadı.</p>
        </div>
        """
        self.set_html(body)

    def show_entries(self, entries: list[DictionaryEntry]) -> None:
        if not entries:
            self.show_not_found("")
            return
        parts: list[str] = []
        current_dict = None
        for entry in entries:
            if entry.dictionary_id != current_dict:
                current_dict = entry.dictionary_id
                parts.append(f'<div class="dict-header">{html.escape(current_dict)}</div>')
            parts.append(f'<div class="headword">{html.escape(entry.headword)}</div>')
            definition = entry.definition
            if "<" in definition and ">" in definition:
                parts.append(f'<div class="definition">{definition}</div>')
            else:
                parts.append(f'<div class="definition"><pre>{html.escape(definition)}</pre></div>')
        self.set_html("\n".join(parts))

    def set_html(self, body_html: str) -> None:
        css = themes.definition_css(self._dark)
        full = f"<!doctype html><html><head><meta charset='utf-8'><style>{css}</style></head><body>{body_html}</body></html>"
        if self._html_mode == "web":
            self._view.setHtml(full, QUrl("dictz://"))  # type: ignore[attr-defined]
        else:
            self._view.setHtml(full)  # type: ignore[attr-defined]

    def _on_anchor_clicked(self, url: QUrl) -> None:
        self.link_activated.emit(url.toString())
