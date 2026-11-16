"""Dictionary icons and badges for the window (Qt side of ``core/dictident.py``).

Every place that names a dictionary (article headers, the dictionary bar,
"found in", the sidebar list) shows the same picture: the dictionary's own
icon when it has one, otherwise its monogram on its own colour.
"""
from __future__ import annotations

import base64
import html
from dataclasses import dataclass
from pathlib import Path

from PySide6.QtCore import QBuffer, QByteArray, QIODevice, QRectF, Qt
from PySide6.QtGui import QColor, QFont, QIcon, QImage, QPainter, QPixmap

from ..core import dictident

_qm = None
_cache: dict[tuple[str, str], DictLook] = {}


def set_query_manager(qm) -> None:
    """The QueryManager whose dictionaries are drawn (set once by the main window)."""
    global _qm
    _qm = qm


@dataclass
class DictLook:
    name: str
    color: str
    monogram: str
    langs: tuple[str, str] | None
    fmt: str
    words: int
    image: QImage | None  # the dictionary's real icon

    def tooltip(self) -> str:
        parts = [self.name]
        if self.langs:
            parts.append(f"{self.langs[0]} → {self.langs[1]}")
        parts.append(f"{self.fmt} · {self.words:,} kelime".replace(",", "."))
        return "\n".join(parts)


def _load_image(path: Path) -> QImage | None:
    found = dictident.icon_bytes(path)
    if not found:
        return None
    data, ext = found
    img = QImage()
    if not img.loadFromData(QByteArray(data)):
        img.loadFromData(QByteArray(data), ext.lstrip(".").upper())
    # a 1-pixel "icon" is a placeholder, not a picture
    return None if img.isNull() or img.width() < 8 or img.height() < 8 else img


def look(name: str) -> DictLook:
    """How the dictionary ``name`` is drawn (cached per name and file)."""
    d = _qm.dictionaries.get(name) if _qm is not None else None
    path = str(getattr(getattr(d, "metadata", None), "file_path", "") or "")
    key = (name, path)
    hit = _cache.get(key)
    if hit is not None:
        return hit
    m = d.metadata if d is not None else None
    lk = DictLook(
        name=name,
        color=dictident.color(name),
        monogram=dictident.monogram(name),
        langs=dictident.language_pair(m.source_language if m else "", m.target_language if m else "", name),
        fmt=dictident.format_label(m.format) if m and m.format else "",
        words=m.word_count if m else 0,
        image=_load_image(Path(path)) if path else None,
    )
    _cache[key] = lk
    return lk


def clear_cache() -> None:
    _cache.clear()


def pixmap(name: str, size: int = 20, ratio: float = 2.0) -> QPixmap:
    """The dictionary's badge as a pixmap (``size`` logical pixels)."""
    lk = look(name)
    px = max(1, round(size * ratio))
    pm = QPixmap(px, px)
    pm.fill(Qt.GlobalColor.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform)
    if lk.image is not None:
        img = lk.image.scaled(px, px, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
        p.drawImage((px - img.width()) // 2, (px - img.height()) // 2, img)
    else:
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(lk.color))
        p.drawRoundedRect(QRectF(0, 0, px, px), px * 0.24, px * 0.24)
        f = QFont()
        f.setBold(True)
        f.setPixelSize(max(6, round(px * (0.46 if len(lk.monogram) > 1 else 0.56))))
        p.setFont(f)
        p.setPen(QColor("#ffffff"))
        p.drawText(QRectF(0, 0, px, px), Qt.AlignmentFlag.AlignCenter, lk.monogram)
    p.end()
    pm.setDevicePixelRatio(ratio)
    return pm


def qicon(name: str) -> QIcon:
    return QIcon(pixmap(name, 20))


def badge_html(name: str) -> str:
    """The badge inside an article header (``<span class="dicon">``)."""
    lk = look(name)
    if lk.image is not None:
        img = lk.image.scaled(48, 48, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
        buf = QBuffer()
        buf.open(QIODevice.OpenModeFlag.WriteOnly)
        img.save(buf, "PNG")
        uri = "data:image/png;base64," + base64.b64encode(bytes(buf.data())).decode("ascii")
        return f'<span class="dicon img"><img src="{uri}" alt=""></span>'
    return f'<span class="dicon">{html.escape(lk.monogram)}</span>'
