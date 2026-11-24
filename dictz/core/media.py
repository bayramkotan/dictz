"""Pictures and sounds referenced by dictionary articles.

Lookup order (same as the Rust edition, ``dictz-rs/src/media.rs``; the same
names GoldenDict tries):

1. folders ``<name>.files/`` for every base name of the file — for
   ``En-Tr.dsl.dz``: ``En-Tr.dsl.dz.files/``, ``En-Tr.dsl.files/``,
   ``En-Tr.files/`` — then StarDict's ``res/`` next to the ``.ifo``
2. zips ``<name>.files.zip`` for the same base names
   (``En-Tr.dsl.dz.files.zip``, ``En-Tr.dsl.files.zip``, ``En-Tr.files.zip``;
   a plain ``.dsl`` tries the same three names)
3. the dictionary's own folder

Folder and zip names match without regard to case (``EN-TR.DSL.FILES.ZIP``).
Inside each place the exact name is tried first, then a case-insensitive
match (zips: also by base name, ignoring sub-folders). A zip's list of names
is read once and kept (big media zips hold 100 000+ files).
"""
from __future__ import annotations

import base64
import contextlib
import html
import re
import zipfile
from pathlib import Path, PurePosixPath

from .i18n import t

# compared in lower case; same lists as dictz-rs/src/media.rs. Lingvo /
# GoldenDict sound packs use .wav (often ADPCM), .ogg, .spx (Speex), .opus …
IMAGE_EXTENSIONS = {"jpg", "jpeg", "gif", "tif", "tiff", "png", "bmp", "webp"}
AUDIO_EXTENSIONS = {"wav", "mp3", "ogg", "oga", "opus", "spx", "flac", "m4a", "aac", "wma"}

_MIME = {
    "jpg": "image/jpeg", "jpeg": "image/jpeg", "gif": "image/gif", "png": "image/png",
    "bmp": "image/bmp", "tif": "image/tiff", "tiff": "image/tiff", "webp": "image/webp",
}


def media_kind(name: str) -> str:
    """"image", "audio" or "other", judged by extension (case-insensitive)."""
    ext = PurePosixPath(name.replace("\\", "/")).suffix.lower().lstrip(".")
    if ext in IMAGE_EXTENSIONS:
        return "image"
    if ext in AUDIO_EXTENSIONS:
        return "audio"
    return "other"


def _safe_relative(name: str) -> PurePosixPath | None:
    name = name.strip().replace("\\", "/")
    while name.startswith("./"):
        name = name[2:]
    if not name:
        return None
    p = PurePosixPath(name)
    if p.is_absolute() or any(part in ("..", ".", "") for part in p.parts) or ":" in p.parts[0]:
        return None
    return p


def _base_names(path: Path) -> list[Path]:
    """``X.dsl.dz`` → ``X.dsl.dz``, ``X.dsl``, ``X``; ``X.dsl`` → the same three
    (GoldenDict tries all of them for any DSL); ``X.ifo`` → ``X.ifo``, ``X``."""
    out = [path]
    p = path
    if p.suffix.lower() == ".dz":
        p = p.with_suffix("")
        out.append(p)
    elif p.suffix.lower() == ".dsl":
        out.insert(0, Path(f"{p}.dz"))
    out.append(p.with_suffix(""))
    return list(dict.fromkeys(out))


def _sibling(path: Path, listing: dict[str, Path] | None) -> Path:
    """``path`` itself, or the entry of its folder with the same name in
    another case (``X.DSL.FILES.ZIP``)."""
    if path.exists() or listing is None:
        return path
    return listing.get(path.name.lower(), path)


class MediaResolver:
    """Finds media files for one dictionary."""

    def __init__(self, dict_path: Path | str):
        path = Path(dict_path)
        parent = path.parent
        try:
            listing = {e.name.lower(): e for e in parent.iterdir()}
        except OSError:
            listing = None
        bases = _base_names(path)
        dirs = [_sibling(Path(f"{b}.files"), listing) for b in bases] + [_sibling(parent / "res", listing)]
        self._dirs = list(dict.fromkeys(dirs)) + [parent]
        self._zips = list(dict.fromkeys(_sibling(Path(f"{b}.files.zip"), listing) for b in bases))
        self._zip_index: dict[Path, tuple[float, zipfile.ZipFile, dict[str, str], dict[str, str]] | None] = {}

    def search_locations(self) -> list[Path]:
        return self._dirs[:-1] + self._zips + self._dirs[-1:]

    def read(self, name: str) -> bytes | None:
        rel = _safe_relative(name)
        if rel is None:
            return None
        for d in self._dirs[:-1]:
            data = _read_from_dir(d, rel)
            if data is not None:
                return data
        for z in self._zips:
            data = self._read_from_zip(z, str(rel))
            if data is not None:
                return data
        return _read_from_dir(self._dirs[-1], rel)

    def _read_from_zip(self, zip_path: Path, rel: str) -> bytes | None:
        try:
            mtime = zip_path.stat().st_mtime
        except OSError:
            return None
        entry = self._zip_index.get(zip_path)
        if entry is None or entry[0] != mtime:
            if entry is not None:
                entry[1].close()
            try:
                z = zipfile.ZipFile(zip_path)
            except (OSError, zipfile.BadZipFile):
                return None
            lower: dict[str, str] = {}
            base: dict[str, str] = {}
            for n in z.namelist():
                lower.setdefault(n.lower(), n)
                base.setdefault(n.lower().rsplit("/", 1)[-1], n)
            entry = (mtime, z, lower, base)
            self._zip_index[zip_path] = entry
        _m, z, lower, base = entry
        want = rel.lower()
        pick = lower.get(want) or base.get(want.rsplit("/", 1)[-1])
        if pick != rel and lower.get(want) is not None:
            with contextlib.suppress(KeyError):
                z.getinfo(rel)  # the exact name wins over another case
                pick = rel
        if not pick:
            return None
        try:
            return z.read(pick)
        except (OSError, KeyError, zipfile.BadZipFile):
            return None

    def close(self) -> None:
        for entry in self._zip_index.values():
            if entry is not None:
                entry[1].close()
        self._zip_index.clear()


def _read_from_dir(directory: Path, rel: PurePosixPath) -> bytes | None:
    if not directory.is_dir():
        return None
    exact = directory.joinpath(*rel.parts)
    if exact.is_file():
        return exact.read_bytes()
    parent = exact.parent
    if not parent.is_dir():
        return None
    wanted = rel.name.lower()
    for entry in parent.iterdir():
        if entry.name.lower() == wanted and entry.is_file():
            return entry.read_bytes()
    return None


_IMG_SRC = re.compile(r'(<img\b[^>]*?\bsrc\s*=\s*)(["\'])(.*?)\2', re.IGNORECASE | re.DOTALL)
_OBJECT = re.compile(r'<object\b[^>]*?\bdata\s*=\s*(["\'])(.*?)\1[^>]*>.*?</object>', re.IGNORECASE | re.DOTALL)
_SOUND_LINK = re.compile(r'(<a\b[^>]*?\bhref\s*=\s*)(["\'])sound://(.*?)\2', re.IGNORECASE)

AUDIO_SCHEME = "dictz-audio"

# the "sound" icon of dictz's own set (dictz/resources/icons/sound.svg), in the text colour
SPEAKER_SVG = ('<svg class="dz-speaker" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
               'stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">'
               '<path d="M4.5 9.5h3l4.5-4v13l-4.5-4h-3z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/></svg>')


def audio_url(dictionary: str, name: str) -> str:
    from urllib.parse import quote

    return f"{AUDIO_SCHEME}:{quote(dictionary, safe='')}/{quote(name, safe='')}"


def parse_audio_url(url: str) -> tuple[str, str] | None:
    from urllib.parse import unquote

    if not url.startswith(f"{AUDIO_SCHEME}:"):
        return None
    rest = url[len(AUDIO_SCHEME) + 1:]
    if "/" not in rest:
        return None
    d, n = rest.split("/", 1)
    return unquote(d), unquote(n)


def rewrite_media_html(definition: str, dictionary: str, resolver: MediaResolver | None) -> str:
    """Make article media usable in the viewer.

    * ``<img src="pic.png">`` → inline ``data:`` URI when the file is found
    * ``<object data="snd.wav">…</object>`` and ``<a href="sound://snd.wav">``
      → a ▶ button link (``dictz-audio:<dict>/<file>``) handled by the window
    """
    if "<" not in definition:
        return definition

    def img(m: re.Match) -> str:
        src = html.unescape(m.group(3))
        if src.startswith(("data:", "http://", "https://")) or resolver is None:
            return m.group(0)
        data = resolver.read(src)
        if data is None:
            return m.group(0)
        ext = PurePosixPath(src).suffix.lower().lstrip(".")
        mime = _MIME.get(ext, "application/octet-stream")
        return f'{m.group(1)}{m.group(2)}data:{mime};base64,{base64.b64encode(data).decode()}{m.group(2)}'

    def obj(m: re.Match) -> str:
        # only the speaker; the file name is in the tooltip
        name = html.unescape(m.group(2))
        tip = html.escape(f"{t('Sesi çal')} — {name.rsplit('/', 1)[-1]}")
        return f'<a class="dz-audio" href="{html.escape(audio_url(dictionary, name))}" title="{tip}">{SPEAKER_SVG}</a>'

    def sound(m: re.Match) -> str:
        name = html.unescape(m.group(3))
        return f'{m.group(1)}{m.group(2)}{html.escape(audio_url(dictionary, name))}{m.group(2)}'

    out = _IMG_SRC.sub(img, definition)
    out = _OBJECT.sub(obj, out)
    out = _SOUND_LINK.sub(sound, out)
    return out
