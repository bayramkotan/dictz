"""Regenerate the parity samples and golden files.

    python tests/data/parity/make_samples.py

* ``sample.dsl`` (+ ``sample.dsl.files.zip``) and ``sample.bgl`` are small,
  hand-made dictionaries that exercise most DSL / Babylon features.
* ``*_golden.json`` hold what the **Python edition** reads from them
  (headwords, definitions, metadata) and RapidFuzz scores. The Rust crate's
  ``tests/parity.rs`` must produce exactly the same, and
  ``tests/test_parity.py`` checks the Python side still does.
"""
from __future__ import annotations

import gzip
import json
import struct
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent.parent
sys.path.insert(0, str(ROOT))

DSL_TEXT = """﻿#NAME "dictz parity sample"
#INDEX_LANGUAGE "English"
#CONTENTS_LANGUAGE "Turkish"

apple
apples
\t[m1][b]elma[/b] [p]i.[/p] {{inline comment}}[/m]
\t[m2][trn]meyve[/trn], [i]kırmızı[/i] [c]yeşil[/c] [c red]kırmızı[/c][/m]
\t[m3][ex][lang id=1033]An ~ a day.[/lang][/ex][/m]
\t[m2]see [ref]pear[/ref] and <<orange juice>>[/m]
\t[s]apple.png[/s] [s]apple.wav[/s]

go (to) {[']}x{[/']}
\t[m1]gitmek[/m]
\t[m1][com]informal[/com] [*]extra[/*][/m]

rock & roll
\t[m0]müzik türü \\[not a tag\\] a\\ b[/m]
\t[url]example.org[/url] [url target="https://dictz.example/"]site[/url]

H[sub]2[/sub]O
\tsu, x[sup]2[/sup] [t]wɔːtə[/t]

broken
\t[b unclosed

pear
\tarmut
"""

PNG_1PX = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02"
    b"\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\xcf\xc0\x00\x00\x03\x01"
    b"\x01\x00\xc9\xfe\x92\xef\x00\x00\x00\x00IEND\xaeB`\x82"
)


def _wav_silence(ms: int = 50, rate: int = 8000) -> bytes:
    n = rate * ms // 1000
    data = b"\x00\x00" * n
    return (
        b"RIFF" + struct.pack("<I", 36 + len(data)) + b"WAVE"
        + b"fmt " + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16)
        + b"data" + struct.pack("<I", len(data)) + data
    )


def write_dsl() -> None:
    (HERE / "sample.dsl").write_bytes(b"\xff\xfe" + DSL_TEXT.encode("utf-16-le"))
    with zipfile.ZipFile(HERE / "sample.dsl.files.zip", "w", zipfile.ZIP_DEFLATED) as z:
        info = zipfile.ZipInfo("apple.png", date_time=(2026, 1, 1, 0, 0, 0))
        z.writestr(info, PNG_1PX)
        info = zipfile.ZipInfo("sounds/Apple.WAV", date_time=(2026, 1, 1, 0, 0, 0))
        z.writestr(info, _wav_silence())


# ------------------------------------------------------------------ BGL

def _block(kind: int, data: bytes) -> bytes:
    n = len(data)
    if n < 12:
        return bytes([((n + 4) << 4) | kind]) + data
    if n < 256:
        return bytes([kind, n]) + data
    return bytes([(1 << 4) | kind]) + struct.pack(">H", n) + data


def _info(code: int, value: bytes) -> bytes:
    return _block(3, struct.pack(">H", code) + value)


def _entry1(word: bytes, defi: bytes, alts: list[bytes] = ()) -> bytes:
    d = bytes([len(word)]) + word + struct.pack(">H", len(defi)) + defi
    for a in alts:
        d += bytes([len(a)]) + a
    return _block(1, d)


def _entry11(word: bytes, alts: list[bytes], defi: bytes) -> bytes:
    d = struct.pack(">BI", 0, len(word)) + word + struct.pack(">I", len(alts))
    for a in alts:
        d += struct.pack(">I", len(a)) + a
    d += struct.pack(">I", len(defi)) + defi
    return _block(11, d)


def write_bgl() -> None:
    t = "cp1254"
    body = b""
    body += _block(0, bytes([8, 0x4B]))                       # default charset cp1254
    body += _info(0x01, "dictz parity BGL — Türkçe".encode(t))
    body += _info(0x02, "Bayram Kotan".encode(t))
    body += _info(0x09, "Örnek sözlük\x00".encode(t))
    body += _info(0x07, struct.pack(">I", 0))                 # English
    body += _info(0x08, struct.pack(">I", 13))                # Turkish
    body += _info(0x1A, bytes([0x41]))                        # source cp1252
    body += _info(0x1B, bytes([0x4B]))                        # target cp1254
    body += _info(0x11, struct.pack(">I", 0))                 # not utf-8
    body += _block(2, b"\x05a.png" + PNG_1PX)                 # resource (ignored)
    # 1: part of speech + alternates (one with a leading slash) + $index$
    body += _entry1(
        b"apple$1$",
        "<b>elma</b>, meyve\x14\x02\x30".encode(t)[:-3] + b"\x14\x02\x30",
        [b"apples", b"/apple", b"apple"],
    )
    # 2: entities in key and definition, Babylon char references, newlines
    body += _entry1(
        b"A &amp; B",
        b"caf&eacute; &#65;&#x42; &lt;tag&gt; <charset c=T>011F;0131;</charset> &csdot;\r\n\r\nline2\x01",
    )
    # 3: title field + transcription field
    title = b"<b>Book</b>"
    trans = b"buk"
    body += _entry1(
        b"book",
        "kitap; defter".encode(t) + b"\x14\x18" + bytes([len(title)]) + title
        + b"\x50\x1b" + bytes([len(trans)]) + trans,
    )
    # 4: <charset c=U> section in a cp1254 definition
    body += _entry1(b"cherry", "kiraz ".encode(t) + b"<charset c=U>\xc3\xa7ilek</charset> \xfc")
    # 5: type 11 block
    body += _entry11(b"door", [b"doors", b"Door"], "kapı<br>".encode(t))
    # 6: empty definition (dropped), duplicate headword (first wins)
    body += _entry1(b"empty", b"")
    body += _entry1(b"book", b"second book")
    # 7: html tags in key are stripped
    body += _entry1(b"<font face='x'>fig</font>", "incir".encode(t))
    body += _block(4, b"")
    gz = gzip.compress(body, mtime=0)
    (HERE / "sample.bgl").write_bytes(b"\x12\x34\x00\x01\x00\x06" + gz)


# --------------------------------------------------------------- golden

FUZZY_PAIRS = [
    ("apple", "apple"), ("aple", "apple"), ("appel", "apple"), ("kitap", "kitab"),
    ("Apple", "APPLE"), ("ışık", "isik"), ("İstanbul", "istanbul"), ("çiçek", "cicek"),
    ("", ""), ("abc", ""), ("colour", "color"), ("teh", "the"), ("dictionary", "dictionnaire"),
    ("Straße", "strasse"), ("москва", "Москва"), ("εὐχαριστῶ", "ευχαριστω"),
]


def golden_for(path: Path) -> dict:
    from dictz.core.query_manager import parser_for

    d = parser_for(path)(path)
    d.load()
    m = d.metadata
    return {
        "meta": {
            "name": m.name, "author": m.author, "description": m.description,
            "source": m.source_language, "target": m.target_language, "count": m.word_count,
        },
        "entries": [[e.headword, e.definition] for e in d.iter_entries()],
    }


def write_golden() -> None:
    from rapidfuzz import fuzz

    for name in ("sample.dsl", "sample.bgl"):
        g = golden_for(HERE / name)
        stem = name.split(".")[1]
        (HERE / f"{stem}_golden.json").write_text(
            json.dumps(g, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
        )
    fz = [[a, b, fuzz.ratio(a.lower(), b.lower())] for a, b in FUZZY_PAIRS]
    (HERE / "fuzzy_golden.json").write_text(json.dumps(fz, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    write_dsl()
    write_bgl()
    write_golden()
    print("written:", sorted(p.name for p in HERE.iterdir()))
