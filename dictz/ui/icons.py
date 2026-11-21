"""dictz's own line icons (``dictz/resources/icons/*.svg``), drawn in the theme's
text colour. The Rust edition uses the same pictures (``dictz-rs/src/uiicons``,
made from these files by ``tests/data/parity/make_samples.py``)."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QByteArray, QRectF, Qt
from PySide6.QtGui import QIcon, QImage, QPainter, QPixmap
from PySide6.QtWidgets import QApplication

ICON_DIR = Path(__file__).resolve().parent.parent / "resources" / "icons"
_color = "#374151"
_cache: dict[tuple[str, str], QIcon] = {}


def set_color(color: str) -> None:
    """Icons drawn from now on use ``color`` (call when the theme changes)."""
    global _color
    if color != _color:
        _color = color
        _cache.clear()


def names() -> list[str]:
    return sorted(p.stem for p in ICON_DIR.glob("*.svg"))


def _render(name: str, color: str, size: int, faded: bool = False) -> QPixmap | None:
    from PySide6.QtSvg import QSvgRenderer

    f = ICON_DIR / f"{name}.svg"
    try:
        svg = f.read_text(encoding="utf-8").replace("currentColor", color)
        if faded:
            svg = svg.replace("<svg ", '<svg opacity="0.38" ', 1)
    except OSError:
        return None
    r = QSvgRenderer(QByteArray(svg.encode("utf-8")))
    img = QImage(size, size, QImage.Format.Format_ARGB32_Premultiplied)
    img.fill(Qt.GlobalColor.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    r.render(p, QRectF(0, 0, size, size))
    p.end()
    return QPixmap.fromImage(img)


def icon(name: str, color: str | None = None) -> QIcon:
    if QApplication.instance() is None:
        return QIcon()
    c = color or _color
    key = (name, c)
    if key not in _cache:
        ic = QIcon()
        for size in (16, 20, 24, 32, 48):
            pm = _render(name, c, size)
            if pm is not None:
                ic.addPixmap(pm)
        # a disabled action draws the same picture, lighter
        for size in (16, 20, 24, 32, 48):
            pm = _render(name, c, size, faded=True)
            if pm is not None:
                ic.addPixmap(pm, QIcon.Mode.Disabled)
        _cache[key] = ic
    return _cache[key]


def app_icon() -> QIcon:
    from ..core.icon import app_icon_rgba

    data = app_icon_rgba(64)
    img = QImage(data, 64, 64, 64 * 4, QImage.Format.Format_RGBA8888).copy()
    return QIcon(QPixmap.fromImage(img))
