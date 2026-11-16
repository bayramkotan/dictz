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


TEXT_SNIPPETS = [
    "plain text\nsecond line\n\n  indented",
    "<b>apple</b> <i>n.</i><br/>elma<p style=\"padding-left:2em;margin:0\">meyve</p>",
    "<ul><li>one</li><li>two <a href=\"bword://x\">x</a></li></ul><hr/>end",
    "a &amp; b &lt;c&gt; &#65;&#x42; &nbsp;x &foo; &#xZZ; &#; & z",
    "<div style=\"margin-left: 24px\">px indent</div><blockquote>quoted</blockquote>",
    "<pre>keep   spaces\n  and lines</pre>after",
    "<p>open<p>second <u>under</u> <a href=\"sound://s.wav\">play</a></p>",
    "<style>b{}</style><script>x</script><!-- c --><img src=\"p.png\"/> text <object data=\"a.mp3\"><param/></object>",
    "<font color=red>c</font>   many    spaces\t\ttabs",
    "x < y and y > z",
]


def _icon_rgba() -> bytes:
    from dictz.core.icon import app_icon_rgba

    return app_icon_rgba(64)


def _fnv1a64(data: bytes) -> str:
    h = 0xcbf29ce484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


def _rs(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def _num(x) -> str:
    return "None" if x is None else f"Some({float(x)!r})"


def write_rust_schema() -> None:
    """Regenerate the schema block of dictz-rs/src/settings.rs from the Python schema."""
    from dictz.storage.settings import SCHEMA, SECTIONS

    kinds = {"bool": "Bool", "int": "Int", "float": "Float", "str": "Str", "choice": "Choice",
             "color": "Color", "list": "List"}
    out = ["// BEGIN GENERATED SCHEMA — written by tests/data/parity/make_samples.py from dictz/storage/settings.py",
           "/// Preference sections: (id, title).",
           "pub const SECTIONS: &[(&str, &str)] = &["]
    out += [f"    ({_rs(a)}, {_rs(b)})," for a, b in SECTIONS]
    out += ["];", "", "/// Every setting (same order as the Python edition).", "pub static SCHEMA: &[Setting] = &["]
    for st in SCHEMA:
        if st.kind == "bool":
            d = f"Def::Bool({'true' if st.default else 'false'})"
        elif st.kind == "int":
            d = f"Def::Int({int(st.default)})"
        elif st.kind == "float":
            d = f"Def::Float({float(st.default)!r})"
        elif st.kind == "list":
            d = "Def::List"
        else:
            d = f"Def::Str({_rs(st.default)})"
        choices = "&[" + ", ".join(f"({_rs(a)}, {_rs(b)})" for a, b in st.choices) + "]"
        out += ["    Setting {", f"        key: {_rs(st.key)},", f"        kind: Kind::{kinds[st.kind]},",
                f"        default: {d},", f"        label: {_rs(st.label)},", f"        help: {_rs(st.help)},",
                f"        min: {_num(st.min)},", f"        max: {_num(st.max)},", f"        choices: {choices},",
                "    },"]
    out += ["];", "// END GENERATED SCHEMA"]
    rs = ROOT / "dictz-rs" / "src" / "settings.rs"
    text = rs.read_text(encoding="utf-8")
    a = text.index("// BEGIN GENERATED SCHEMA")
    b = text.index("// END GENERATED SCHEMA") + len("// END GENERATED SCHEMA")
    rs.write_text(text[:a] + "\n".join(out) + text[b:], encoding="utf-8")


def write_golden() -> None:
    from rapidfuzz import fuzz

    for name in ("sample.dsl", "sample.bgl"):
        g = golden_for(HERE / name)
        stem = name.split(".")[1]
        (HERE / f"{stem}_golden.json").write_text(
            json.dumps(g, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
        )
    from dictz.core.htmltext import to_text

    texts = list(TEXT_SNIPPETS)
    for name in ("dsl_golden.json", "bgl_golden.json"):
        texts += [e[1] for e in json.loads((HERE / name).read_text(encoding="utf-8"))["entries"]]
    seen: set[str] = set()
    pairs = [[t, to_text(t)] for t in texts if not (t in seen or seen.add(t))]
    (HERE / "text_golden.json").write_text(json.dumps(pairs, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    from dictz.storage.settings import schema_as_json

    (HERE / "settings_schema.json").write_text(
        json.dumps(schema_as_json(), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    from dictz.cli import COMMANDS, completion_script

    rs_dir = ROOT / "dictz-rs" / "src" / "completion"
    rs_dir.mkdir(exist_ok=True)
    for shell in ("bash", "zsh", "fish", "powershell"):
        text = completion_script(shell)
        (HERE / f"completion_{shell}.txt").write_text(text, encoding="utf-8", newline="\n")
        # the Rust edition embeds the same scripts (include_str!)
        (rs_dir / f"{shell}.txt").write_text(text, encoding="utf-8", newline="\n")
    (HERE / "cli_commands.json").write_text(json.dumps(COMMANDS, ensure_ascii=False, indent=1) + "\n",
                                            encoding="utf-8")
    from dictz.core.shortcuts import SHORTCUTS

    (HERE / "shortcuts.json").write_text(json.dumps(SHORTCUTS, ensure_ascii=False, indent=1) + "\n",
                                         encoding="utf-8")
    from dictz.core.hotkeys import parse_hotkey
    from dictz.core.scantext import DoubleCopy, clean_scan_text

    scan_cases = ["  Hello, ", '"apple."', "two  words", "a\nb", "123", "", "(kitap)…", "x" * 90,
                  "İstanbul’da", "«merhaba»", "  — dash —  ", "e-mail", "C++", "naïve café", "tab\tsep",
                  "x" * 80, "x" * 81, "¿qué?", "日本語", "...", "'quoted'"]
    double_seq = [["a", 0], ["a", 300], ["a", 400], ["a", 1000], ["a", 1600], ["b", 1700], ["a", 1800],
                  ["a", 2400], ["a", 2401]]
    d = DoubleCopy(500)
    hotkey_cases = ["Ctrl+F11", "ctrl+alt+d", "Shift+Super+Space", "F5", "Ctrl+PgUp", "Alt+Enter",
                    "a", "Ctrl+", "Hyper+A", "Ctrl+F25", "Win+1", "Cmd+Shift+K"]

    def hk(text):
        try:
            h = parse_hotkey(text)
            return {"ok": True, "mods": sorted(h.mods), "key": h.key, "text": str(h)}
        except ValueError as exc:
            return {"ok": False, "error": str(exc)}

    scan = {
        "clean": [[t, clean_scan_text(t, 80)] for t in scan_cases],
        "double_copy_500": [[t, ms, d.feed(t, ms)] for t, ms in double_seq],
        "hotkeys": [[t, hk(t)] for t in hotkey_cases],
        "icon_fnv1a64": _fnv1a64(_icon_rgba()),
    }
    (HERE / "scan_golden.json").write_text(json.dumps(scan, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    write_rust_schema()
    fz = [[a, b, fuzz.ratio(a.lower(), b.lower())] for a, b in FUZZY_PAIRS]
    (HERE / "fuzzy_golden.json").write_text(json.dumps(fz, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    write_dsl()
    write_bgl()
    write_golden()
    print("written:", sorted(p.name for p in HERE.iterdir()))
