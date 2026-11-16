"""Article viewer: QtWebEngine when available, QTextBrowser otherwise."""
from __future__ import annotations

import html
from urllib.parse import quote

from PySide6.QtCore import QUrl, Signal
from PySide6.QtWidgets import QTextBrowser, QVBoxLayout, QWidget

from ..parsers.base import DictionaryEntry
from ..storage.settings import Preferences
from . import dicticons, themes

try:
    from PySide6.QtWebEngineCore import QWebEnginePage
    from PySide6.QtWebEngineWidgets import QWebEngineView
    HAS_WEBENGINE = True
except ImportError:
    QWebEngineView = None  # type: ignore
    QWebEnginePage = None  # type: ignore
    HAS_WEBENGINE = False


if HAS_WEBENGINE:
    class _LinkPage(QWebEnginePage):  # type: ignore[misc, valid-type]
        """Hands clicked links to the window instead of navigating away."""

        def __init__(self, on_link, parent=None):
            super().__init__(parent)
            self._on_link = on_link

        def acceptNavigationRequest(self, url, nav_type, is_main_frame):  # noqa: N802
            if nav_type == QWebEnginePage.NavigationType.NavigationTypeLinkClicked:
                self._on_link(url.toString())
                return False
            return super().acceptNavigationRequest(url, nav_type, is_main_frame)


def group_by_dictionary(entries: list[DictionaryEntry]) -> list[tuple[str, list[DictionaryEntry]]]:
    groups: list[tuple[str, list[DictionaryEntry]]] = []
    for e in entries:
        if not groups or groups[-1][0] != e.dictionary_id:
            groups.append((e.dictionary_id, []))
        groups[-1][1].append(e)
    return groups


class DefinitionView(QWidget):
    link_activated = Signal(str)
    find_result = Signal(int, int)  # active match, total

    def __init__(self, prefs: Preferences, parent: QWidget | None = None):
        super().__init__(parent)
        self.prefs = prefs
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        self.word = ""
        self.entries: list[DictionaryEntry] = []
        self.suggestions: list[tuple[str, float]] = []
        self._body = ""
        if HAS_WEBENGINE:
            self._view = QWebEngineView(self)
            self._view.setPage(_LinkPage(self.link_activated.emit, self._view))
            layout.addWidget(self._view)
            self._html_mode = "web"
        else:
            tb = QTextBrowser(self)
            tb.setOpenLinks(False)
            tb.anchorClicked.connect(lambda url: self.link_activated.emit(url.toString()))
            self._view = tb
            layout.addWidget(tb)
            self._html_mode = "text"
        self.set_zoom(float(prefs["article.zoom"]))

    # ------------------------------------------------------------ settings
    def set_zoom(self, factor: float) -> None:
        factor = max(0.5, min(3.0, factor))
        if self._html_mode == "web":
            self._view.setZoomFactor(factor)  # type: ignore[attr-defined]
        else:
            f = self._view.font()
            f.setPointSizeF(max(6.0, 11.0 * factor))
            self._view.setFont(f)

    def refresh_style(self) -> None:
        """Re-render with the current preferences."""
        if self._body:
            self.set_html(self._body)

    # ------------------------------------------------------------ content
    def show_welcome(self) -> None:
        self.word, self.entries, self.suggestions = "", [], []
        body = """
        <div style="text-align:center; padding: 80px 24px;">
            <h1 style="font-size: 2.2em; margin: 0;">dictz</h1>
            <p class="muted" style="margin: 6px 0 26px 0;">çok formatlı çevrimdışı sözlük</p>
            <p class="muted">Başlamak için <b>Dosya → Sözlük Ekle</b> (Ctrl+O) veya <b>Klasör Ekle</b> ile
            StarDict (.ifo), DSL (.dsl) ya da Babylon (.bgl) sözlüğü ekleyin;<br>dosyaları pencereye
            sürükleyip bırakabilirsiniz.</p>
            <p class="muted" style="font-size: .85em; margin-top: 36px;">
            <b>Ctrl+F</b> ara · <b>Ctrl+T</b> yeni sekme · <b>Ctrl+D</b> favori · <b>Ctrl+R</b> rastgele kelime ·
            <b>Ctrl+Shift+F</b> sayfada bul · <b>Ctrl+,</b> tercihler · <b>F11</b> tam ekran</p>
        </div>"""
        self.set_html(body)

    def show_not_found(self, query: str, suggestions: list[tuple[str, float]] | None = None) -> None:
        self.word, self.entries, self.suggestions = query, [], list(suggestions or [])
        did_you_mean = ""
        if suggestions:
            items = "".join(
                f'<li><a href="bword://{quote(w)}">{html.escape(w)}</a>'
                f' <span class="score">%{score:.0f}</span></li>'
                for w, score in suggestions
            )
            did_you_mean = f'<h3>Bunu mu demek istediniz?</h3><ul class="did-you-mean">{items}</ul>'
        body = f"""
        <div style="padding: 36px 8px;">
            <h2>Sonuç yok</h2>
            <p class="muted"><b>{html.escape(query)}</b> için hiçbir sözlükte eşleşme bulunamadı.</p>
            {did_you_mean}
        </div>"""
        self.set_html(body)

    def show_entries(self, word: str, entries: list[DictionaryEntry]) -> None:
        if not entries:
            self.show_not_found(word)
            return
        self.word, self.entries, self.suggestions = word, list(entries), []
        p = self.prefs
        is_open = "" if p["article.collapsed"] else " open"
        head_cls = "" if p["article.show_dict_headers"] else " nohead"
        if not p["article.show_dict_headers"]:
            is_open = " open"
        parts: list[str] = []
        dark = themes.is_dark(p["appearance.theme"])
        for i, (dict_name, items) in enumerate(group_by_dictionary(entries)):
            lk = dicticons.look(dict_name)
            tint = themes._mix(lk.color, "#1b1f26" if dark else "#ffffff", 0.80 if dark else 0.88)
            ink = themes._mix(lk.color, "#ffffff", 0.40) if dark else lk.color
            count = f'<span class="count">{len(items)} madde</span>' if len(items) > 1 else ""
            langs = (f'<span class="chip lang">{html.escape(lk.langs[0])} → {html.escape(lk.langs[1])}</span>'
                     if lk.langs else "")
            fmt = f'<span class="chip fmt">{html.escape(lk.fmt)}</span>' if lk.fmt else ""
            parts.append(
                f'<details class="dict{head_cls}" id="dict-{i}" style="--dc:{lk.color};--dt:{tint};--dk:{ink}"{is_open}>'
                f'<summary title="{html.escape(lk.tooltip())}">'
                f'{dicticons.badge_html(dict_name)}<span class="dname">{html.escape(dict_name)}</span>'
                f'{langs}{fmt}{count}'
                f'<span class="actions"><a href="dictz-action:copy:{i}" title="Makaleyi kopyala">⧉ kopyala</a>'
                f'<a href="dictz-action:only:{i}" title="Yalnızca bu sözlükte ara">◎ yalnızca bu</a></span>'
                f'<span class="chev">▾</span></summary><div class="dbody">'
            )
            for e in items:
                definition = e.definition
                if "<" in definition and ">" in definition:
                    body = f'<div class="definition">{definition}</div>'
                else:
                    body = f'<div class="definition"><pre>{html.escape(definition)}</pre></div>'
                parts.append(f'<div class="article"><div class="headword">{html.escape(e.headword)}</div>{body}</div>')
            parts.append("</div></details>")
        self.set_html("\n".join(parts))

    def set_html(self, body_html: str) -> None:
        self._body = body_html
        css = themes.definition_css(self.prefs)
        full = (f"<!doctype html><html><head><meta charset='utf-8'><style>{css}</style></head>"
                f"<body><main>{body_html}</main></body></html>")
        if self._html_mode == "web":
            self._view.setHtml(full, QUrl("dictz://article/"))  # type: ignore[attr-defined]
        else:
            self._view.setHtml(full)  # type: ignore[attr-defined]

    # ------------------------------------------------------------ actions
    def scroll_to_dictionary(self, index: int) -> None:
        if self._html_mode == "web":
            self._view.page().runJavaScript(  # type: ignore[attr-defined]
                f"var d=document.getElementById('dict-{index}'); if(d){{d.open=true; d.scrollIntoView({{behavior:'smooth'}});}}")
        else:
            self._view.scrollToAnchor(f"dict-{index}")  # type: ignore[attr-defined]

    def set_all_open(self, opened: bool) -> None:
        if self._html_mode == "web":
            self._view.page().runJavaScript(  # type: ignore[attr-defined]
                f"document.querySelectorAll('details.dict').forEach(d=>d.open={'true' if opened else 'false'});")

    def find(self, text: str, backward: bool = False, case: bool = False) -> None:
        if self._html_mode == "web":
            flags = QWebEnginePage.FindFlag(0)
            if backward:
                flags |= QWebEnginePage.FindFlag.FindBackward
            if case:
                flags |= QWebEnginePage.FindFlag.FindCaseSensitively

            def done(result) -> None:
                try:
                    self.find_result.emit(result.activeMatch(), result.numberOfMatches())
                except AttributeError:
                    self.find_result.emit(0, 0)

            self._view.findText(text, flags, done)  # type: ignore[attr-defined]
        else:
            from PySide6.QtGui import QTextDocument

            flags = QTextDocument.FindFlag(0)
            if backward:
                flags |= QTextDocument.FindFlag.FindBackward
            if case:
                flags |= QTextDocument.FindFlag.FindCaseSensitively
            ok = self._view.find(text, flags)  # type: ignore[attr-defined]
            self.find_result.emit(1 if ok else 0, 1 if ok else 0)

    def article_text(self, group_index: int | None = None) -> str:
        """Plain text of all articles, or of one dictionary group."""
        from ..core.htmltext import to_text

        groups = group_by_dictionary(self.entries)
        if group_index is not None:
            groups = groups[group_index:group_index + 1]
        out = []
        for name, items in groups:
            out.append(f"== {name} ==")
            for e in items:
                out.append(e.headword)
                out.extend(f"  {ln}" if ln else "" for ln in to_text(e.definition).split("\n"))
                out.append("")
        return "\n".join(out).rstrip() + "\n"

    def article_html(self) -> str:
        css = themes.definition_css(self.prefs)
        return (f"<!doctype html><html><head><meta charset='utf-8'><title>{html.escape(self.word)} — dictz</title>"
                f"<style>{css}</style></head><body><main>{self._body}</main></body></html>")
