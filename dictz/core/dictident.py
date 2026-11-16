"""What a dictionary looks like in the window: its icon, colour, monogram and language pair.

Same results as the Rust edition (``dictz-rs/src/dictident.rs``); the parity
golden ``tests/data/parity/dictident_golden.json`` checks both.

* **Icon**, first match wins:
  1. a picture next to the dictionary with the same name
     (``Oxford.ifo`` → ``Oxford.ico`` / ``.png`` / ``.bmp`` / ``.jpg`` / ``.gif``,
     ``Lingvo.dsl.dz`` → ``Lingvo.png``…), as GoldenDict does;
  2. the icon stored inside a Babylon ``.bgl`` file;
  3. otherwise a generated badge: the dictionary's monogram on its own colour.
* **Colour**: picked from the dictionary name (FNV-1a), so a dictionary keeps
  its colour everywhere and between runs.
* **Language pair**: from the dictionary's own metadata (DSL, Babylon), or
  else from its name (``En2Tr``, ``English-Turkish``, ``İngilizce → Türkçe``).

Nothing here imports Qt.
"""
from __future__ import annotations

import struct
import zlib
from pathlib import Path

ICON_EXTENSIONS = (".ico", ".png", ".bmp", ".jpg", ".jpeg", ".gif")

# folded name / code → short code shown on the badge
_LANGS: dict[str, tuple[str, ...]] = {
    "EN": ("english", "en", "eng", "ingilizce", "ing", "englisch", "anglais"),
    "TR": ("turkish", "tr", "tur", "türkçe", "turkce", "türk", "turk", "türkisch", "turc"),
    "DE": ("german", "de", "deu", "ger", "deutsch", "almanca", "alm", "allemand"),
    "FR": ("french", "fr", "fra", "fre", "français", "francais", "fransızca", "fransizca"),
    "ES": ("spanish", "es", "spa", "español", "espanol", "ispanyolca"),
    "IT": ("italian", "it", "ita", "italiano", "italyanca"),
    "PT": ("portuguese", "pt", "por", "português", "portekizce"),
    "NL": ("dutch", "nl", "nld", "nederlands", "flemenkçe", "hollandaca"),
    "RU": ("russian", "ru", "rus", "русский", "rusça", "rusca"),
    "UK": ("ukrainian", "uk", "ukr", "українська", "ukraynaca"),
    "PL": ("polish", "pl", "pol", "polski", "lehçe"),
    "EL": ("greek", "el", "ell", "gre", "yunanca"),
    "LA": ("latin", "la", "lat", "latince"),
    "AR": ("arabic", "ar", "ara", "العربية", "arapça", "arapca"),
    "FA": ("persian", "fa", "fas", "per", "farsi", "farsça", "farsca", "فارسی"),
    "OTA": ("ottoman", "ota", "osmanlıca", "osmanlica"),
    "HE": ("hebrew", "he", "heb", "ibranice"),
    "ZH": ("chinese", "zh", "zho", "chi", "中文", "çince"),
    "JA": ("japanese", "ja", "jpn", "日本語", "japonca"),
    "KO": ("korean", "ko", "kor", "한국어", "korece"),
    "AZ": ("azerbaijani", "az", "aze", "azerice", "azərbaycan"),
    "KK": ("kazakh", "kk", "kaz", "kazakça"),
    "UZ": ("uzbek", "uz", "uzb", "özbekçe"),
    "KU": ("kurdish", "ku", "kur", "kürtçe"),
    "HI": ("hindi", "hi", "hin"),
    "SV": ("swedish", "sv", "swe", "isveççe"),
    "FI": ("finnish", "fi", "fin", "fince"),
    "HU": ("hungarian", "hu", "hun", "macarca"),
    "CS": ("czech", "cs", "ces", "cze", "çekçe"),
    "RO": ("romanian", "ro", "ron", "rum", "romence"),
    "BG": ("bulgarian", "bg", "bul", "bulgarca"),
    "SR": ("serbian", "sr", "srp", "sırpça"),
    "EO": ("esperanto", "eo", "epo"),
}

# words that say nothing about which dictionary it is (left out of the monogram)
_FILLER = frozenset((
    "dictionary", "dict", "dic", "sözlük", "sözlüğü", "sozluk", "sozlugu", "lexicon", "glossary",
    "the", "of", "and", "to", "for", "ve", "v", "big", "full",
))

_SEPARATORS = frozenset(("-", "–", "—", "2", "→", "->", ">", "/", "_", "to", "<>", "↔", "|", "‑"))


def fold(s: str) -> str:
    """Lower case, without the dot that ``İ``.lower() leaves behind."""
    return s.lower().replace("̇", "")


_CODE_BY_WORD = {fold(w): code for code, words in _LANGS.items() for w in words}


def language_code(name: str) -> str:
    """``English`` / ``en`` / ``Türkçe`` → ``EN`` / ``TR``; '' when unknown."""
    return _CODE_BY_WORD.get(fold(name.strip()), "")


def _tokens(name: str) -> list[tuple[str, str]]:
    """(gap before the word, word) for every run of letters in ``name``."""
    out: list[tuple[str, str]] = []
    gap, word = "", ""
    for ch in name:
        if ch.isalpha():
            word += ch
        else:
            if word:
                out.append((gap, word))
                gap, word = "", ""
            gap += ch
    if word:
        out.append((gap, word))
    return out


def languages_from_name(name: str) -> tuple[str, str] | None:
    """``Adil Hindistan En2Tr`` → (``EN``, ``TR``); None when the name has no pair."""
    toks = _tokens(name)
    i = 0
    while i + 1 < len(toks):
        a = toks[i][1]
        j = i + 1
        gap = toks[j][0]
        if fold(toks[j][1]) == "to" and j + 1 < len(toks):
            gap = gap + "to" + toks[j + 1][0]
            j += 1
        sep = "".join(gap.split())
        if sep in _SEPARATORS or fold(sep) in _SEPARATORS:
            src, dst = language_code(a), language_code(toks[j][1])
            if src and dst:
                return src, dst
        i += 1
    return None


def language_pair(source: str, target: str, name: str) -> tuple[str, str] | None:
    """The pair to show: the dictionary's own languages first, then its name."""
    if source.strip() and target.strip():
        src = language_code(source) or source.strip()[:3].upper()
        dst = language_code(target) or target.strip()[:3].upper()
        return src, dst
    return languages_from_name(name)


def monogram(name: str) -> str:
    """One or two letters for the generated badge."""
    words = [w for _, w in _tokens(name)]
    meaningful = [w for w in words if fold(w) not in _FILLER and not language_code(w)]
    pick = meaningful or words
    if not pick:
        return "?"
    if len(pick) >= 2:
        return (pick[0][0] + pick[1][0]).upper()
    w = pick[0]
    return w[0].upper() + (w[1].lower() if len(w) > 1 else "")


def fnv1a64(data: bytes) -> int:
    h = 0xcbf29ce484222325
    for b in data:
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def _hsl_hex(h: float, s: float, l: float) -> str:  # noqa: E741
    c = (1.0 - abs(2.0 * l - 1.0)) * s
    hp = h / 60.0
    x = c * (1.0 - abs(hp % 2.0 - 1.0))
    r, g, b = [(c, x, 0.0), (x, c, 0.0), (0.0, c, x), (0.0, x, c), (x, 0.0, c), (c, 0.0, x)][int(hp) % 6]
    m = l - c / 2.0
    return "#" + "".join(f"{int((v + m) * 255.0 + 0.5):02x}" for v in (r, g, b))


def color(name: str) -> str:
    """The dictionary's own colour (``#rrggbb``), stable for a name."""
    h = fnv1a64(name.encode("utf-8"))
    hue = float(h % 360)
    sat = 0.50 + float((h >> 16) % 20) / 100.0
    light = 0.40 + float((h >> 32) % 10) / 100.0
    return _hsl_hex(hue, sat, light)


# ------------------------------------------------------------------ icon files

def _stem(path: Path) -> str:
    name = path.name
    low = name.lower()
    for ext in (".dsl.dz", ".dsl", ".ifo", ".bgl", ".dz", ".gz"):
        if low.endswith(ext):
            return name[: -len(ext)]
    return path.stem


def icon_file(path: Path | str) -> Path | None:
    """A picture next to the dictionary file with the same name, if there is one."""
    path = Path(path)
    stem = _stem(path)
    try:
        siblings = {p.name.lower(): p for p in path.parent.iterdir() if p.is_file()}
    except OSError:
        return None
    for ext in ICON_EXTENSIONS:
        hit = siblings.get((stem + ext).lower())
        if hit is not None:
            return hit
    return None


def bgl_icon(path: Path | str) -> bytes | None:
    """The icon stored in a Babylon ``.bgl`` file (info block 0x0B), or None."""
    try:
        raw = Path(path).read_bytes()
    except OSError:
        return None
    if len(raw) < 6 or raw[:4] not in (b"\x12\x34\x00\x01", b"\x12\x34\x00\x02"):
        return None
    gz = int.from_bytes(raw[4:6], "big")
    data = raw[gz:]
    if len(data) < 10 or data[:3] != b"\x1f\x8b\x08":
        return None
    flags, pos = data[3], 10
    if flags & 0x04:
        pos += 2 + struct.unpack("<H", data[pos:pos + 2])[0]
    for bit in (0x08, 0x10):
        if flags & bit:
            while pos < len(data) and data[pos] != 0:
                pos += 1
            pos += 1
    if flags & 0x02:
        pos += 2
    dec = zlib.decompressobj(-15)
    src = data[pos:]
    buf = bytearray()

    def need(n: int) -> bool:
        """Inflate until ``buf`` holds ``n`` bytes (the info blocks are at the start)."""
        nonlocal src
        while len(buf) < n:
            if dec.unconsumed_tail:
                chunk = dec.unconsumed_tail
            elif src:
                chunk, src = src[:65536], src[65536:]
            else:
                return False
            try:
                buf.extend(dec.decompress(chunk, 1 << 20))
            except zlib.error:
                return False
        return True

    pos = 0
    while need(pos + 1):
        first = buf[pos]
        pos += 1
        kind, length = first & 0xF, first >> 4
        if length < 4:
            n = length + 1
            if not need(pos + n):
                break
            length = int.from_bytes(buf[pos:pos + n], "big")
            pos += n
        else:
            length -= 4
        if kind in (1, 7, 10, 11):  # entries begin: every info block has been seen
            break
        need(pos + length)
        block = bytes(buf[pos:pos + length])
        pos += length
        if kind == 3 and len(block) > 2 and int.from_bytes(block[:2], "big") == 0x0B:
            return block[2:]
        if len(buf) > (32 << 20):
            break
    return None


def icon_bytes(path: Path | str) -> tuple[bytes, str] | None:
    """(file contents, lower-case extension) of the dictionary's real icon, if any."""
    p = icon_file(path)
    if p is not None:
        try:
            return p.read_bytes(), p.suffix.lower()
        except OSError:
            pass
    if str(path).lower().endswith(".bgl"):
        data = bgl_icon(path)
        if data:
            return data, ".png" if data[:8] == b"\x89PNG\r\n\x1a\n" else ".ico"
    return None


def format_label(fmt: str) -> str:
    return {"stardict": "StarDict", "dsl": "DSL", "babylon": "BGL"}.get(fmt.lower(), fmt.upper())
