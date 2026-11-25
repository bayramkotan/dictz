"""Article viewer: QtWebEngine when available, QTextBrowser otherwise."""
from __future__ import annotations

import html
import os
from urllib.parse import quote

from PySide6.QtCore import QUrl, Signal
from PySide6.QtWidgets import QTextBrowser, QVBoxLayout, QWidget

from ..core.i18n import t
from ..parsers.base import DictionaryEntry
from ..storage.settings import Preferences
from . import dicticons, icons, plainview, themes

#: Why the web viewer is not used ("" when it is); shown in Yardım → Hakkında / the log.
WEBENGINE_ERROR = ""
try:
    if os.environ.get("DICTZ_NO_WEBENGINE"):
        raise ImportError("DICTZ_NO_WEBENGINE is set")
    from PySide6.QtWebEngineCore import QWebEnginePage
    from PySide6.QtWebEngineWidgets import QWebEngineView
    HAS_WEBENGINE = True
except ImportError as exc:
    QWebEngineView = None  # type: ignore
    QWebEnginePage = None  # type: ignore
    HAS_WEBENGINE = False
    WEBENGINE_ERROR = str(exc) or exc.__class__.__name__
    import logging

    logging.getLogger("dictz").warning("QtWebEngine unavailable, plain article viewer used: %s", WEBENGINE_ERROR)


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
        self._zoom = 1.0
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
            self._zoom = factor
            self.refresh_style()

    def refresh_style(self) -> None:
        """Re-render with the current preferences."""
        if self._body:
            self.set_html(self._body)

    # ------------------------------------------------------------ content
    def show_welcome(self) -> None:
        self.word, self.entries, self.suggestions = "", [], []
        keys = " · ".join(f"<b>{k}</b> {html.escape(t(v))}" for k, v in (
            ("Ctrl+F", "ara"), ("Ctrl+T", "yeni sekme"), ("Ctrl+D", "favori"), ("Ctrl+R", "rastgele kelime"),
            ("Ctrl+Shift+F", "sayfada bul"), ("Ctrl+,", "tercihler"), ("F11", "tam ekran")))
        body = f"""
        <div style="text-align:center; padding: 80px 24px;">
            <h1 style="font-size: 2.2em; margin: 0;">dictz</h1>
            <p class="muted" style="margin: 6px 0 26px 0;">{html.escape(t("çok formatlı çevrimdışı sözlük"))}</p>
            <p class="muted">{html.escape(t("Başlamak için Dosya → Sözlük Ekle (Ctrl+O) veya Klasör Ekle ile StarDict (.ifo), DSL (.dsl) ya da Babylon (.bgl) sözlüğü ekleyin; dosyaları pencereye sürükleyip bırakabilirsiniz."))}</p>
            <p class="muted" style="font-size: .85em; margin-top: 36px;">{keys}</p>
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
            did_you_mean = f'<h3>{html.escape(t("Bunu mu demek istediniz?"))}</h3><ul class="did-you-mean">{items}</ul>'
        body = f"""
        <div class="page">
            <h2>{html.escape(t("Sonuç yok"))}</h2>
            <p class="muted">{t("“{word}” için hiçbir sözlükte eşleşme bulunamadı.", word=html.escape(query))}</p>
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
        dark = bool(themes.current(p)["dark"])
        if self._html_mode == "text":
            self.set_html(self._plain_entries(entries, dark))
            return
        parts: list[str] = []
        for i, (dict_name, items) in enumerate(group_by_dictionary(entries)):
            lk = dicticons.look(dict_name)
            tint = themes._mix(lk.color, "#1b1f26" if dark else "#ffffff", 0.80 if dark else 0.88)
            ink = themes._mix(lk.color, "#ffffff", 0.40) if dark else lk.color
            count = f'<span class="count">{t("{n} madde", n=len(items))}</span>' if len(items) > 1 else ""
            langs = (f'<span class="chip lang">{html.escape(lk.langs[0])} → {html.escape(lk.langs[1])}</span>'
                     if lk.langs else "")
            fmt = f'<span class="chip fmt">{html.escape(lk.fmt)}</span>' if lk.fmt else ""
            parts.append(
                f'<details class="dict{head_cls}" id="dict-{i}" style="--dc:{lk.color};--dt:{tint};--dk:{ink}"{is_open}>'
                f'<summary title="{html.escape(lk.tooltip())}">'
                f'{dicticons.badge_html(dict_name)}<span class="dname">{html.escape(dict_name)}</span>'
                f'{langs}{fmt}{count}'
                f'<span class="actions"><a href="dictz-action:copy:{i}" title="{t("Makaleyi kopyala")}">⧉</a>'
                f'<a href="dictz-action:only:{i}" title="{t("Yalnızca bu sözlükte ara")}">◎</a></span>'
                f'<span class="chev">▾</span></summary><div class="dbody">'
            )
            for e in items:
                parts.append(self._article(e, dark))
            parts.append("</div></details>")
        self.set_html("\n".join(parts))

    @staticmethod
    def _article(e: DictionaryEntry, dark: bool, plain: bool = False) -> str:
        definition = plainview.readable_colors(e.definition, dark)
        if "<" in definition and ">" in definition:
            body = f'<div class="definition">{definition}</div>'
        elif plain:
            body = f'<div class="definition">{html.escape(definition).replace(chr(10), "<br>")}</div>'
        else:
            body = f'<div class="definition"><pre>{html.escape(definition)}</pre></div>'
        return f'<div class="article"><div class="headword">{html.escape(e.headword)}</div>{body}</div>'

    def _plain_entries(self, entries: list[DictionaryEntry], dark: bool) -> str:
        """The articles as HTML that ``QTextBrowser`` can draw (see ``plainview``)."""
        p = self.prefs
        art = themes.current(p)["article"]
        width = max(200, self._view.viewport().width() - 60)  # type: ignore[attr-defined]
        max_w = min(width, int(p["article.max_width"]) or width)
        max_h = int(p["article.max_image_height"])
        speaker = plainview.png_uri(icons._render("sound", art["muted"], 32) or icons.app_icon().pixmap(32))
        sep = f'<hr style="border: none; height: 1px; background-color: {art["border"]}">'
        parts: list[str] = []
        for i, (dict_name, items) in enumerate(group_by_dictionary(entries)):
            lk = dicticons.look(dict_name)
            if p["article.show_dict_headers"]:
                parts.append(plainview.header(
                    i, dict_name, plainview.png_uri(dicticons.pixmap(dict_name, 16, 2.0)), lk.color,
                    themes._mix(lk.color, art["strip"], 0.86), art["fg"], art["muted"],
                    f"{lk.langs[0]} → {lk.langs[1]}" if lk.langs else "",
                    t("{n} madde", n=len(items)) if len(items) > 1 else "",
                    t("Makaleyi kopyala"), t("Yalnızca bu sözlükte ara")))
            else:
                parts.append(f'<a name="dict-{i}"></a>')
            parts.append('<div style="margin: 2px 12px 8px 14px; line-height: 135%">')
            for j, e in enumerate(items):
                if j:
                    parts.append(sep)
                parts.append(plainview.plain_body(self._article(e, dark, plain=True), speaker, max_w, max_h))
            parts.append("</div>")
        return "\n".join(parts)

    def set_html(self, body_html: str) -> None:
        self._body = body_html
        css = themes.definition_css(self.prefs) if self._html_mode == "web" else self._plain_css()
        full = (f"<!doctype html><html><head><meta charset='utf-8'><style>{css}</style></head>"
                f"<body><main>{body_html}</main></body></html>")
        if self._html_mode == "web":
            self._view.setHtml(full, QUrl("dictz://article/"))  # type: ignore[attr-defined]
        else:
            self._view.setHtml(full)  # type: ignore[attr-defined]

    def _plain_css(self) -> str:
        p = self.prefs
        th = themes.current(p)
        art = th["article"]
        accent = themes.accent_of(p)
        if th["dark"]:
            head_fg, link = themes._mix(accent, "#ffffff", 0.55), themes._mix(accent, "#ffffff", 0.35)
        else:
            head_fg, link = themes._mix(accent, "#000000", 0.35), accent
        fam = p["article.font_family"] or p["appearance.ui_font_family"]
        return plainview.css(art["bg"], art["fg"], art["muted"], link, head_fg,
                             f'"{fam}", ' if fam else "", round(int(p["article.font_size"]) * self._zoom))

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
