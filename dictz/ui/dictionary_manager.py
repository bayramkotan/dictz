"""Helpers for dictionary files: the open-file filter and the information
of one dictionary. (The list itself is the Dictionaries window's
"Sözlüklerim" tab, ``dictionary_center.py``.)"""
from __future__ import annotations

from pathlib import Path

from ..core.i18n import t
from ..core.query_manager import QueryManager


def file_filter() -> str:
    return (t("Tüm sözlükler") + " (*.ifo *.dsl *.dz *.bgl);;StarDict (*.ifo);;DSL / Lingvo (*.dsl *.dz);;"
            "Babylon (*.bgl);;" + t("Tüm dosyalar") + " (*)")


def info_html(qm: QueryManager, name: str) -> str:
    import html
    import time

    d = qm.dictionaries[name]
    m = d.metadata
    p = Path(m.file_path)
    size = p.stat().st_size if p.exists() else 0
    mtime = time.strftime("%Y-%m-%d %H:%M", time.localtime(p.stat().st_mtime)) if p.exists() else "—"
    media = qm.media(name)
    rows = [
        (t("Ad"), m.name), (t("Biçim"), m.format), (t("Kelime"), f"{m.word_count:,}"),
        (t("Kaynak dil"), m.source_language or "—"), (t("Hedef dil"), m.target_language or "—"),
        (t("Yazar"), m.author or "—"), (t("Dosya"), str(p)), (t("Boyut"), f"{size / 1024:,.0f} KB"),
        (t("Değiştirilme"), mtime), (t("Etkin"), t("evet") if qm.is_enabled(name) else t("hayır")),
        (t("Medya klasörleri"), "<br>".join(html.escape(str(x)) for x in (media.search_locations() if media else []))),
    ]
    body = "".join(f"<tr><td><b>{k}</b></td><td>{v if k == t('Medya klasörleri') else html.escape(v)}</td></tr>"
                   for k, v in rows)
    desc = f"<p>{html.escape(m.description)}</p>" if m.description else ""
    return f"<table cellpadding=4>{body}</table>{desc}"
