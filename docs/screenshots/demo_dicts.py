"""Build the small demo dictionaries used for the README screenshots.

    python docs/screenshots/demo_dicts.py OUT_DIR

Everything here is invented sample text (no third-party dictionary content).
Needs PySide6 (for the two pictures) and the repository's parity helpers.
"""
from __future__ import annotations

import gzip
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests" / "data" / "parity"))


def stardict(out: Path, stem: str, bookname: str, entries: list[tuple[str, str]], kind: str = "h") -> None:
    idx, dat = b"", b""
    for word, text in sorted(entries, key=lambda e: e[0].lower()):
        body = text.encode("utf-8")
        idx += word.encode("utf-8") + b"\0" + struct.pack(">II", len(dat), len(body))
        dat += body
    (out / f"{stem}.dict").write_bytes(dat)
    (out / f"{stem}.idx").write_bytes(idx)
    (out / f"{stem}.ifo").write_text(
        "StarDict's dict ifo file\nversion=2.4.2\n"
        f"wordcount={len(entries)}\nidxfilesize={len(idx)}\nbookname={bookname}\nsametypesequence={kind}\n",
        encoding="utf-8")


DSL = """#NAME "Universal English–Russian"
#INDEX_LANGUAGE "English"
#CONTENTS_LANGUAGE "Russian"

apple
\t[m0][b]apple[/b] [c gray][t]ˈæp(ə)l[/t][/c] [p]n.[/p][/m]
\t[m1]1) [trn]яблоко[/trn][/m]
\t[m2][ex][lang id=1033]an apple a day keeps the doctor away[/lang] — [i]кто ест яблоко в день, у того доктора нет[/i][/ex][/m]
\t[m1]2) [trn]яблоня[/trn] [com](дерево)[/com][/m]
\t[m1]→ [ref]pear[/ref], [ref]orange[/ref][/m]
\t[s]apple.png[/s] [s]apple.wav[/s]

pear
\t[m0][b]pear[/b] [c gray][t]peə[/t][/c] [p]n.[/p][/m]
\t[m1][trn]груша[/trn][/m]

orange
\t[m0][b]orange[/b] [c gray][t]ˈɒrɪndʒ[/t][/c] [p]n., adj.[/p][/m]
\t[m1]1) [trn]апельсин[/trn][/m]
\t[m1]2) [trn]оранжевый[/trn][/m]

light
\t[m0][b]light[/b] [c gray][t]laɪt[/t][/c][/m]
\t[m1][p]n.[/p] [trn]свет[/trn]; [p]adj.[/p] [trn]лёгкий[/trn], [trn]светлый[/trn][/m]
"""


def wav_tone(ms: int = 300, rate: int = 8000) -> bytes:
    import math

    n = rate * ms // 1000
    data = b"".join(struct.pack("<h", int(6000 * math.sin(2 * math.pi * 440 * i / rate))) for i in range(n))
    return (b"RIFF" + struct.pack("<I", 36 + len(data)) + b"WAVE" + b"fmt "
            + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16) + b"data" + struct.pack("<I", len(data)) + data)


def pictures(out: Path) -> None:
    from PySide6.QtCore import QPointF, QRectF, Qt
    from PySide6.QtGui import (
        QColor,
        QFont,
        QGuiApplication,
        QImage,
        QLinearGradient,
        QPainter,
        QPainterPath,
        QRadialGradient,
    )

    _app = QGuiApplication.instance() or QGuiApplication([])
    # an apple for the DSL article
    img = QImage(240, 240, QImage.Format.Format_ARGB32)
    img.fill(Qt.GlobalColor.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    g = QRadialGradient(QPointF(95, 105), 120)
    g.setColorAt(0, QColor("#ff6b6b"))
    g.setColorAt(1, QColor("#b91c1c"))
    path = QPainterPath()
    path.moveTo(120, 70)
    path.cubicTo(60, 30, 10, 90, 40, 160)
    path.cubicTo(60, 215, 100, 225, 120, 205)
    path.cubicTo(140, 225, 180, 215, 200, 160)
    path.cubicTo(230, 90, 180, 30, 120, 70)
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(g)
    p.drawPath(path)
    p.setBrush(QColor("#6b4423"))
    p.drawRoundedRect(QRectF(114, 30, 10, 45), 4, 4)
    leaf = QPainterPath()
    leaf.moveTo(124, 50)
    leaf.cubicTo(150, 20, 185, 30, 190, 40)
    leaf.cubicTo(170, 65, 140, 65, 124, 50)
    p.setBrush(QColor("#16a34a"))
    p.drawPath(leaf)
    p.setBrush(QColor(255, 255, 255, 90))
    p.drawEllipse(QRectF(70, 95, 28, 45))
    p.end()
    img.save(str(out / "apple.png"))
    # a custom icon for one dictionary (Name.png next to Name.ifo)
    ico = QImage(64, 64, QImage.Format.Format_ARGB32)
    ico.fill(Qt.GlobalColor.transparent)
    p = QPainter(ico)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    lg = QLinearGradient(0, 0, 64, 64)
    lg.setColorAt(0, QColor("#f59e0b"))
    lg.setColorAt(1, QColor("#b45309"))
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(lg)
    p.drawRoundedRect(QRectF(4, 6, 56, 52), 10, 10)
    p.setBrush(QColor("#fff7ed"))
    p.drawRoundedRect(QRectF(12, 14, 19, 36), 3, 3)
    p.drawRoundedRect(QRectF(33, 14, 19, 36), 3, 3)
    f = QFont()
    f.setBold(True)
    f.setPixelSize(15)
    p.setFont(f)
    p.setPen(QColor("#92400e"))
    p.drawText(QRectF(12, 14, 19, 36), Qt.AlignmentFlag.AlignCenter, "A")
    p.drawText(QRectF(33, 14, 19, 36), Qt.AlignmentFlag.AlignCenter, "Ç")
    p.end()
    ico.save(str(out / "pocket.png"))


def bgl(out: Path) -> None:
    from make_samples import _block, _entry1, _info

    t = "cp1252"
    body = _block(0, bytes([8, 0x41]))
    body += _info(0x01, "English–German Glossary".encode(t, "replace").replace(b"?", b"-"))
    body += _info(0x02, b"dictz demo")
    body += _info(0x07, struct.pack(">I", 0))  # English
    body += _info(0x08, struct.pack(">I", 6))  # German
    body += _info(0x1A, bytes([0x41]))
    body += _info(0x1B, bytes([0x41]))
    for word, defi in [
        ("apple", "<b>der Apfel</b> (<i>pl.</i> Äpfel); <i>apple tree</i> — der Apfelbaum"),
        ("pear", "<b>die Birne</b>"),
        ("light", "<b>das Licht</b>; <i>adj.</i> hell, leicht"),
        ("book", "<b>das Buch</b>; <i>v.</i> buchen, reservieren"),
        ("yield", "<b>der Ertrag</b>; <i>v.</i> ergeben, nachgeben"),
    ]:
        body += _entry1(word.encode(t), defi.encode(t))
    body += _block(4, b"")
    (out / "English-German Glossary.bgl").write_bytes(b"\x12\x34\x00\x01\x00\x06" + gzip.compress(body, mtime=0))


def main(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    stardict(out, "en-tr-general", "English–Turkish General Dictionary", [
        ("apple", "<b>elma</b> <i>(i.)</i><br>1. elma; 2. elma ağacı<br>"
                  "<span style='color:#6b7280'>apple pie</span> — elmalı turta · "
                  "<span style='color:#6b7280'>the apple of sb's eye</span> — birinin gözbebeği"),
        ("pear", "<b>armut</b> <i>(i.)</i>"),
        ("orange", "<b>portakal</b> <i>(i.)</i>; turuncu <i>(s.)</i>"),
        ("light", "<b>ışık</b> <i>(i.)</i>; hafif, açık <i>(s.)</i>; yakmak <i>(f.)</i>"),
        ("book", "<b>kitap</b> <i>(i.)</i>; yer ayırtmak <i>(f.)</i>"),
        ("yield", "<b>vermek, getirmek</b> <i>(f.)</i>: <i>The tree yields a lot of fruit.</i> — Ağaç çok meyve verir."
                  "<br>2. teslim olmak, boyun eğmek<br>3. <i>(i.)</i> ürün, verim, hasılat"),
    ])
    stardict(out, "en-thesaurus", "English Thesaurus", [
        ("apple", "<b>apple</b> <i>noun</i><br>the round fruit of a tree of the rose family, with red, yellow or "
                  "green skin and crisp flesh.<br><span style='color:#2563eb'>Related:</span> "
                  "<a href='bword://pear'>pear</a>, <a href='bword://orange'>orange</a>, quince"),
        ("pear", "<b>pear</b> <i>noun</i><br>a sweet fruit, narrow at the top and wide at the bottom."),
        ("light", "<b>light</b> <i>noun, adjective</i><br>1. the natural agent that makes things visible.<br>"
                  "2. of little weight."),
        ("yield", "<b>yield</b> <i>verb</i><br>1. produce or provide (a natural, agricultural or industrial "
                  "product).<br>2. give way to arguments, demands or pressure."),
    ])
    stardict(out, "pocket", "İngilizce–Türkçe Cep Sözlüğü", [
        ("apple", "elma"), ("pear", "armut"), ("orange", "portakal"), ("light", "ışık; hafif"),
        ("book", "kitap"), ("yield", "ürün; vermek; teslim olmak"),
    ])
    (out / "universal-en-ru.dsl").write_bytes(b"\xff\xfe" + DSL.encode("utf-16-le"))
    files = out / "universal-en-ru.dsl.files"
    files.mkdir(exist_ok=True)
    (files / "apple.wav").write_bytes(wav_tone())
    pictures(out)
    (out / "apple.png").replace(files / "apple.png")
    bgl(out)
    print("demo dictionaries in", out)


if __name__ == "__main__":
    main(Path(sys.argv[1] if len(sys.argv) > 1 else "demo-dicts"))
