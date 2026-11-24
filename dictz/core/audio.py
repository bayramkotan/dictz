"""Sound playback choices, shared by both editions (``dictz-rs/src/audioplay.rs``).

The setting ``audio.player`` picks who plays a dictionary's sounds:

* ``auto`` — ``audio.command`` when one is typed (like GoldenDict's
  "external program" field); on Linux the first external player installed
  (mpv, ffplay, VLC, MPlayer — they talk to PipeWire / PulseAudio
  themselves; the built-in players sounded bad there), else the built-in
  player; on Windows / macOS the built-in player, else the first external
  one; else the system's
* ``internal`` — Qt Multimedia (Python) / rodio (Rust) in the program
* ``ffplay``, ``mpv``, ``vlc``, ``mplayer`` — that program, if installed
* ``system`` — the system's default program for the file
* ``command`` — ``audio.command``, ``{file}`` is the sound file

The end of a word used to be cut off: players stop the device when the
data ends, before its buffer has been heard. Every sound the built-in
players get therefore ends with :data:`TAIL_MS` of silence
(:func:`pad_wav`), and the Rust player waits for the buffer to drain.
``audio.speed`` (0.5–1.5) slows a sound down to hear it better.
"""
from __future__ import annotations

import io
import os
import shutil
import sys
import wave
from pathlib import Path

PLAYERS: tuple[tuple[str, str], ...] = (
    ("auto", "Otomatik"),
    ("internal", "Dahili oynatıcı"),
    ("mpv", "mpv"),
    ("ffplay", "FFplay (FFmpeg)"),
    ("vlc", "VLC"),
    ("mplayer", "MPlayer"),
    ("system", "Sistemin oynatıcısı"),
    ("command", "Özel komut"),
)
EXTERNAL = ("mpv", "ffplay", "vlc", "mplayer")
TAIL_MS = 350

# where Windows / macOS install them when they are not on PATH
_KNOWN_PATHS = {
    "vlc": [r"C:\Program Files\VideoLAN\VLC\vlc.exe", r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe",
            "/Applications/VLC.app/Contents/MacOS/VLC"],
    "mpv": [r"C:\Program Files\mpv\mpv.exe", "/Applications/mpv.app/Contents/MacOS/mpv",
            "/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
    "ffplay": ["/opt/homebrew/bin/ffplay", "/usr/local/bin/ffplay"],
    "mplayer": ["/opt/homebrew/bin/mplayer", "/usr/local/bin/mplayer"],
}


def find_program(name: str) -> str | None:
    """The path of an external player, or ``None`` when it is not installed."""
    exe = "cvlc" if name == "vlc" and sys.platform.startswith("linux") and shutil.which("cvlc") else name
    found = shutil.which(exe)
    if found:
        return found
    for p in _KNOWN_PATHS.get(name, []):
        if os.path.isfile(p):
            return p
    return None


def split_command(s: str) -> list[str]:
    """A command line split at spaces; "…" and '…' group (quotes removed).
    No backslash escapes, so Windows paths stay as they are. Same in Rust."""
    out: list[str] = []
    cur, have, quote = [], False, ""
    for c in s:
        if quote:
            if c == quote:
                quote = ""
            else:
                cur.append(c)
        elif c.isspace():
            if have:
                out.append("".join(cur))
                cur, have = [], False
        elif c in "\"'":
            quote, have = c, True
        else:
            cur.append(c)
            have = True
    if have:
        out.append("".join(cur))
    return out


def player_command(player: str, program: str, file: str, volume: int, speed: float,
                   custom: str = "") -> list[str]:
    """The command line that plays ``file`` with ``player`` (found at
    ``program``) at ``volume`` (0–100) and ``speed`` (0.5–1.5). Same in Rust."""
    v = max(0, min(100, int(volume)))
    s = f"{max(0.5, min(1.5, float(speed))):g}"
    if player == "mpv":
        return [program, "--no-video", "--really-quiet", f"--volume={v}", f"--speed={s}", file]
    if player == "ffplay":
        return [program, "-nodisp", "-autoexit", "-loglevel", "quiet", "-volume", str(v),
                "-af", f"atempo={s},apad=pad_dur={TAIL_MS / 1000:g}", file]
    if player == "vlc":
        return [program, "-I", "dummy", "--play-and-exit", "--no-video", f"--rate={s}",
                f"--gain={v / 100:.2f}", file]
    if player == "mplayer":
        return [program, "-really-quiet", "-novideo", "-volume", str(v), "-af", "scaletempo",
                "-speed", s, file]
    if player == "command":
        parts = split_command(custom)
        if not parts:
            return []
        if not any("{file}" in p for p in parts):
            parts.append("{file}")
        return [p.replace("{file}", file).replace("{volume}", str(v)).replace("{speed}", s) for p in parts]
    return []


def pad_wav(data: bytes, ms: int = TAIL_MS) -> bytes | None:
    """A PCM WAV with ``ms`` of silence added at the end (``None`` when
    ``data`` is not a WAV the ``wave`` module reads)."""
    try:
        with wave.open(io.BytesIO(data), "rb") as r:
            params = r.getparams()
            frames = r.readframes(r.getnframes())
    except (wave.Error, EOFError, ValueError):
        return None
    silence_byte = b"\x80" if params.sampwidth == 1 else b"\x00"  # 8-bit WAV is unsigned
    n = int(params.framerate * ms / 1000)
    out = io.BytesIO()
    with wave.open(out, "wb") as w:
        w.setnchannels(params.nchannels)
        w.setsampwidth(params.sampwidth)
        w.setframerate(params.framerate)
        w.writeframes(frames + silence_byte * (n * params.nchannels * params.sampwidth))
    return out.getvalue()


def pcm_to_wav(pcm: bytes, rate: int, channels: int, sampwidth: int = 2, ms: int = TAIL_MS) -> bytes:
    """Decoded samples as a WAV, with ``ms`` of silence at the end."""
    n = int(rate * ms / 1000) * channels * sampwidth
    out = io.BytesIO()
    with wave.open(out, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(sampwidth)
        w.setframerate(rate)
        w.writeframes(pcm + (b"\x80" if sampwidth == 1 else b"\x00") * n)
    return out.getvalue()


def prefers_external() -> bool:
    """On Linux ``auto`` takes an installed external player before the
    built-in one."""
    return sys.platform.startswith("linux")


def resolve(player: str, internal_ok: bool, command: str = "", prefer_external: bool | None = None) -> str:
    """The player that really plays (see the module text): ``auto`` →
    the typed command, else (Linux) an external player before the built-in
    one, else the system's; an external player that is not installed →
    ``system``; ``command`` without a command → ``auto``."""
    if prefer_external is None:
        prefer_external = prefers_external()
    if player == "auto":
        if command.strip():
            return "command"
        ext = next((p for p in EXTERNAL if find_program(p)), None)
        if prefer_external and ext:
            return ext
        if internal_ok:
            return "internal"
        return ext or "system"
    if player == "internal" and not internal_ok:
        return resolve("auto", False, "", prefer_external)
    if player == "command" and not command.strip():
        return resolve("auto", internal_ok, "", prefer_external)
    if player in EXTERNAL and not find_program(player):
        return "system"
    return player


def label(player: str) -> str:
    """The (Turkish) name of a player choice; translate with ``t()``."""
    return dict(PLAYERS).get(player, player)


def available() -> dict[str, bool]:
    """Which external players are installed (shown next to the choice)."""
    return {p: find_program(p) is not None for p in EXTERNAL}


def cache_file(cache: Path, key: str, suffix: str) -> Path:
    import hashlib

    return cache / (hashlib.sha1(key.encode()).hexdigest()[:16] + suffix)
