"""Sound playback: the player choices, command lines, silence at the end."""
from __future__ import annotations

import io
import json
import wave
from pathlib import Path

from dictz.core import audio
from dictz.storage.settings import SCHEMA_BY_KEY, Preferences

PARITY = Path(__file__).resolve().parent / "data" / "parity"


def _wav(n: int, rate: int = 16000) -> bytes:
    out = io.BytesIO()
    with wave.open(out, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(b"\x10\x00" * n)
    return out.getvalue()


def test_padded_wav_ends_with_silence() -> None:
    padded = audio.pad_wav(_wav(1000))
    with wave.open(io.BytesIO(padded), "rb") as r:
        assert r.getnframes() == 1000 + 16000 * audio.TAIL_MS // 1000
        frames = r.readframes(r.getnframes())
    assert frames[:2000] == b"\x10\x00" * 1000 and set(frames[2000:]) == {0}
    assert audio.pad_wav(b"ID3 not a wav") is None


def test_commands_match_golden_and_rust() -> None:
    g = json.loads((PARITY / "audio_golden.json").read_text(encoding="utf-8"))
    for *args, want in g["commands"]:
        assert audio.player_command(*args) == want
    assert audio.split_command('"C:\\Program Files\\x.exe" -a \'b c\'') == ["C:\\Program Files\\x.exe", "-a", "b c"]


def test_choices_and_resolve(monkeypatch) -> None:
    assert SCHEMA_BY_KEY["audio.player"].choices == audio.PLAYERS
    monkeypatch.setattr(audio, "find_program", lambda name: "/usr/bin/mpv" if name == "mpv" else None)
    # Windows / macOS: the built-in player first
    assert audio.resolve("auto", True, prefer_external=False) == "internal"
    assert audio.resolve("auto", False, prefer_external=False) == "mpv"
    # Linux: an installed external player first (the built-in ones sounded bad)
    assert audio.resolve("auto", True, prefer_external=True) == "mpv"
    assert audio.resolve("vlc", True) == "system"  # not installed
    assert audio.resolve("internal", False, prefer_external=False) == "mpv"
    # a typed command is used like GoldenDict's "external program"
    assert audio.resolve("auto", True, "mplayer -really-quiet", False) == "command"
    assert audio.resolve("command", True, "mpv") == "command"
    assert audio.resolve("command", True, "  ", prefer_external=False) == "internal"
    assert audio.label("mplayer") == "MPlayer"


def test_old_system_player_setting_moves(tmp_path: Path) -> None:
    f = tmp_path / "settings.json"
    f.write_text(json.dumps({"audio": {"system_player": True}}), encoding="utf-8")
    assert Preferences.load(f)["audio.player"] == "system"
    f.write_text(json.dumps({"audio": {"system_player": True, "player": "mpv"}}), encoding="utf-8")
    assert Preferences.load(f)["audio.player"] == "mpv"
    assert Preferences.load(tmp_path / "none.json")["audio.player"] == "auto"
