"""Article HTML for the plain viewer (``QTextBrowser``) and readable colours.

QtWebEngine is not always there (some PySide6 installs, some frozen builds).
``QTextBrowser`` understands only a small part of HTML 4 and CSS: no flex, no
``<details>``, no inline ``<svg>``, no ``max-width`` / ``max-height``, no CSS
variables. Fed the web viewer's HTML it ran the article headers together
("BaBabylon English-EnglishEN → ENBGL⧉◎▾"), drew badges at 48 px in the text,
lost the sound buttons and left big holes. This module builds what it can
draw: a one-row table per dictionary header, picture sizes written into the
``<img>`` tags, the speaker as a small PNG and inline styles only.

``readable_colors`` (both viewers) lifts dark dictionary colours on a dark
theme and darkens very light ones on a light theme — the same rule as the Rust
edition's ``render.rs::adjust``.
"""
from __future__ import annotations

import base64
import html
import re

from PySide6.QtCore import QBuffer, QByteArray, QIODevice
from PySide6.QtGui import QColor, QImage, QPixmap

from ..core.media import SPEAKER_SVG

_COLOR_ATTR = re.compile(r'(\bcolor\s*=\s*)(["\']?)([#\w]+)\2', re.IGNORECASE)
_STYLE_ATTR = re.compile(r'(\bstyle\s*=\s*)(["\'])(.*?)\2', re.IGNORECASE | re.DOTALL)
_STYLE_COLOR = re.compile(r'((?<![-\w])color\s*:\s*)([^;"\']+)', re.IGNORECASE)
_IMG = re.compile(r"<img\b[^>]*>", re.IGNORECASE | re.DOTALL)
_IMG_SRC = re.compile(r'\bsrc\s*=\s*(["\'])(data:[^"\']*)\1', re.IGNORECASE | re.DOTALL)
_IMG_SIZE = re.compile(r'\s(?:width|height)\s*=\s*(["\']?)[^\s"\'>]*\1', re.IGNORECASE)


def adjust(color: str, dark: bool) -> str | None:
    """The readable form of a dictionary colour, or None to keep it as it is."""
    c = QColor(color.strip().strip("\"'"))
    if not c.isValid():
        return None
    r, g, b = c.red(), c.green(), c.blue()
    lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
    if dark and lum < 0.45:
        r, g, b = (int(v + (255 - v) * 0.55) for v in (r, g, b))
    elif not dark and lum > 0.85:
        r, g, b = (int(v * 0.6) for v in (r, g, b))
    else:
        return None
    return f"#{r:02x}{g:02x}{b:02x}"


def readable_colors(body: str, dark: bool) -> str:
    """Rewrite ``color="…"`` and ``style="color: …"`` so the text stays readable."""
    def attr(m: re.Match) -> str:
        new = adjust(m.group(3), dark)
        return m.group(0) if new is None else f"{m.group(1)}{m.group(2)}{new}{m.group(2)}"

    def style(m: re.Match) -> str:
        def one(s: re.Match) -> str:
            new = adjust(s.group(2), dark)
            return s.group(0) if new is None else f"{s.group(1)}{new}"
        return f"{m.group(1)}{m.group(2)}{_STYLE_COLOR.sub(one, m.group(3))}{m.group(2)}"

    if "color" not in body.lower():
        return body
    return _STYLE_ATTR.sub(style, _COLOR_ATTR.sub(attr, body))


def png_uri(pm: QPixmap | QImage) -> str:
    buf = QBuffer()
    buf.open(QIODevice.OpenModeFlag.WriteOnly)
    pm.save(buf, "PNG")
    return "data:image/png;base64," + base64.b64encode(bytes(buf.data())).decode("ascii")


def image_size(uri: str) -> tuple[int, int] | None:
    """Width and height of a ``data:`` picture, or None."""
    head, _, data = uri.partition(",")
    if ";base64" not in head:
        return None
    try:
        raw = base64.b64decode(data, validate=False)
    except (ValueError, TypeError):
        return None
    img = QImage()
    if not img.loadFromData(QByteArray(raw)):
        return None
    return img.width(), img.height()


def fit(w: int, h: int, max_w: int, max_h: int) -> tuple[int, int]:
    """Shrink (never grow) to fit ``max_w`` × ``max_h``, keeping the aspect."""
    scale = 1.0
    if max_w > 0 and w > max_w:
        scale = min(scale, max_w / w)
    if max_h > 0 and h > max_h:
        scale = min(scale, max_h / h)
    return max(1, round(w * scale)), max(1, round(h * scale))


def sized_images(body: str, max_w: int, max_h: int) -> str:
    """Write each ``data:`` picture's display size into its ``<img>`` tag."""
    def one(m: re.Match) -> str:
        tag = m.group(0)
        src = _IMG_SRC.search(tag)
        if src is None:
            return tag
        size = image_size(src.group(2))
        if size is None:
            return tag
        w, h = fit(size[0], size[1], max_w, max_h)
        tag = _IMG_SIZE.sub("", tag)
        end = "/>" if tag.endswith("/>") else ">"
        return f'{tag[:-len(end)].rstrip()} width="{w}" height="{h}" style="vertical-align: middle"{end}'

    return _IMG.sub(one, body) if "<img" in body.lower() else body


def plain_body(body: str, speaker_uri: str, max_w: int, max_h: int) -> str:
    """A definition made drawable for ``QTextBrowser``."""
    if SPEAKER_SVG in body:
        body = body.replace(
            SPEAKER_SVG, f'<img src="{speaker_uri}" width="16" height="16" style="vertical-align: middle">')
    return sized_images(body, max_w, max_h)


def header(index: int, name: str, badge_uri: str, color: str, strip: str, fg: str, muted: str,
           pair: str, count: str, copy_title: str, only_title: str) -> str:
    """One dictionary's header strip as a one-row table."""
    pair_html = (f'&nbsp;&nbsp;<span style="color:{muted}; font-weight:normal">{html.escape(pair)}</span>'
                 if pair else "")
    count_html = (f'&nbsp;&nbsp;<span style="color:{muted}; font-weight:normal">{html.escape(count)}</span>'
                  if count else "")
    return (
        f'<a name="dict-{index}"></a>'
        f'<table width="100%" cellspacing="0" cellpadding="3" bgcolor="{strip}" '
        f'style="margin-top: 8px; margin-bottom: 4px"><tr>'
        f'<td width="3" bgcolor="{color}"></td>'
        f'<td width="22" style="vertical-align: middle">'
        f'<img src="{badge_uri}" width="16" height="16" style="vertical-align: middle"></td>'
        f'<td style="vertical-align: middle; color:{fg}; font-weight:600">{html.escape(name)}{pair_html}{count_html}</td>'
        f'<td align="right" style="vertical-align: middle">'
        f'<a href="dictz-action:copy:{index}" title="{html.escape(copy_title)}" '
        f'style="color:{muted}; text-decoration:none">⧉</a>&nbsp;&nbsp;'
        f'<a href="dictz-action:only:{index}" title="{html.escape(only_title)}" '
        f'style="color:{muted}; text-decoration:none">◎</a>&nbsp;</td>'
        f'</tr></table>'
    )


def css(bg: str, fg: str, muted: str, link: str, head_fg: str, family: str, size: int) -> str:
    """The few rules ``QTextBrowser`` follows (px sizes, no variables, no em line heights)."""
    return f"""
body {{ background-color: {bg}; color: {fg}; font-family: {family}"Segoe UI", "Noto Sans", sans-serif;
        font-size: {size}px; }}
a {{ color: {link}; text-decoration: none; }}
h1, h2, h3 {{ color: {head_fg}; }}
.muted {{ color: {muted}; }}
.score {{ color: {muted}; }}
.headword {{ color: {head_fg}; font-weight: bold; font-size: {round(size * 1.15)}px; }}
pre {{ font-family: {family}"Segoe UI", "Noto Sans", sans-serif; white-space: pre-wrap; }}
p {{ margin-top: 2px; margin-bottom: 2px; }}
"""
