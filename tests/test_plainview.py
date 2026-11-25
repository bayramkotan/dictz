"""The plain article viewer (QTextBrowser, used when QtWebEngine is missing)."""
from __future__ import annotations

import base64
import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
QtWidgets = pytest.importorskip("PySide6.QtWidgets")

from PySide6.QtGui import QColor, QImage  # noqa: E402

from dictz.core.media import SPEAKER_SVG  # noqa: E402
from dictz.ui import plainview  # noqa: E402


@pytest.fixture(scope="module", autouse=True)
def _app():
    return QtWidgets.QApplication.instance() or QtWidgets.QApplication([])


def _png(w: int, h: int) -> str:
    img = QImage(w, h, QImage.Format.Format_ARGB32)
    img.fill(QColor("#ff0000"))
    return plainview.png_uri(img)


def test_dark_dictionary_colours_are_lifted_like_the_rust_edition() -> None:
    assert plainview.adjust("blue", True) == "#8c8cff"       # (0,0,255) → +55 % towards white
    assert plainview.adjust("#ffff00", True) is None          # light enough already
    assert plainview.adjust("yellow", False) == "#999900"     # too light on a light page
    assert plainview.adjust("not-a-colour", True) is None
    html = '<font color="blue">a</font><span style="background-color: navy; color: navy">b</span>'
    out = plainview.readable_colors(html, True)
    assert 'color="#8c8cff"' in out
    assert "background-color: navy" in out and "; color: #8c8cc5" in out


def test_pictures_get_a_size_that_fits() -> None:
    assert plainview.fit(800, 400, 400, 300) == (400, 200)
    assert plainview.fit(100, 900, 400, 300) == (33, 300)
    assert plainview.fit(50, 40, 400, 300) == (50, 40)        # never grown
    uri = _png(800, 400)
    assert plainview.image_size(uri) == (800, 400)
    out = plainview.sized_images(f'<img src="{uri}" width="9999">', 400, 300)
    assert 'width="400" height="200"' in out and "9999" not in out
    assert plainview.sized_images('<img src="x.png">', 400, 300) == '<img src="x.png">'


def test_speaker_svg_becomes_a_picture() -> None:
    icon = "data:image/png;base64," + base64.b64encode(b"x").decode()
    out = plainview.plain_body(f'<a class="dz-audio" href="dictz-audio://d/a.wav">{SPEAKER_SVG}</a>', icon, 400, 300)
    assert "<svg" not in out and f'src="{icon}"' in out


def test_header_is_one_table_row() -> None:
    h = plainview.header(2, "Babylon <EN>", "data:image/png;base64,AA==", "#123456", "#eeeeee", "#111111",
                         "#777777", "EN → EN", "", "Kopyala", "Yalnızca")
    assert h.count("<tr>") == 1 and 'name="dict-2"' in h
    assert "Babylon &lt;EN&gt;" in h and "dictz-action:copy:2" in h and "dictz-action:only:2" in h
    assert "<details" not in h and "<svg" not in h
