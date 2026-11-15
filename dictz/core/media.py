"""Pictures and sounds referenced by dictionary articles.

Lookup order (same as the Rust edition, ``dictz-rs/src/media.rs``):

1. folder ``<file>.files/`` — e.g. ``En-Tr.dsl.files/`` (for ``.dsl.dz`` the
   ``.dz`` is dropped first), then ``<stem>.files/``
2. zip ``<file>.files.zip``, then ``<stem>.files.zip``
3. StarDict ``res/`` folder next to the ``.ifo``
4. the dictionary's own folder

Inside each place the exact name is tried first, then a case-insensitive
match (zips: also by base name, ignoring sub-folders).
"""
from __future__ import annotations

import base64
import html
import re
import zipfile
from pathlib import Path, PurePosixPath

IMAGE_EXTENSIONS = {"jpg", "jpeg", "gif", "tif", "tiff", "png", "bmp"}
AUDIO_EXTENSIONS = {"wav", "mp3", "ogg", "oga", "flac"}

_MIME = {
    "jpg": "image/jpeg", "jpeg": "image/jpeg", "gif": "image/gif", "png": "image/png",
    "bmp": "image/bmp", "tif": "image/tiff", "tiff": "image/tiff",
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


class MediaResolver:
    """Finds media files for one dictionary."""

    def __init__(self, dict_path: Path | str):
        path = Path(dict_path)
        file = path.with_suffix("") if path.suffix.lower() == ".dz" else path
        stem = file.with_suffix("")
        parent = path.parent
        dirs = [Path(f"{file}.files"), Path(f"{stem}.files"), parent / "res", parent]
        self._dirs = list(dict.fromkeys(dirs))
        self._zips = list(dict.fromkeys([Path(f"{file}.files.zip"), Path(f"{stem}.files.zip")]))

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
            data = _read_from_zip(z, str(rel))
            if data is not None:
                return data
        return _read_from_dir(self._dirs[-1], rel)


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


def _read_from_zip(zip_path: Path, rel: str) -> bytes | None:
    if not zip_path.is_file():
        return None
    try:
        with zipfile.ZipFile(zip_path) as z:
            names = z.namelist()
            lower = rel.lower()
            base = lower.rsplit("/", 1)[-1]
            pick = (next((n for n in names if n == rel), None)
                    or next((n for n in names if n.lower() == lower), None)
                    or next((n for n in names if n.lower().rsplit("/", 1)[-1] == base), None))
            return z.read(pick) if pick else None
    except (OSError, zipfile.BadZipFile):
        return None


_IMG_SRC = re.compile(r'(<img\b[^>]*?\bsrc\s*=\s*)(["\'])(.*?)\2', re.IGNORECASE | re.DOTALL)
_OBJECT = re.compile(r'<object\b[^>]*?\bdata\s*=\s*(["\'])(.*?)\1[^>]*>.*?</object>', re.IGNORECASE | re.DOTALL)
_SOUND_LINK = re.compile(r'(<a\b[^>]*?\bhref\s*=\s*)(["\'])sound://(.*?)\2', re.IGNORECASE)

AUDIO_SCHEME = "dictz-audio"


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
        name = html.unescape(m.group(2))
        label = html.escape(name.rsplit("/", 1)[-1])
        return f'<a class="dz-audio" href="{html.escape(audio_url(dictionary, name))}" title="Sesi çal">🔊 {label}</a>'

    def sound(m: re.Match) -> str:
        name = html.unescape(m.group(3))
        return f'{m.group(1)}{m.group(2)}{html.escape(audio_url(dictionary, name))}{m.group(2)}'

    out = _IMG_SRC.sub(img, definition)
    out = _OBJECT.sub(obj, out)
    out = _SOUND_LINK.sub(sound, out)
    return out
