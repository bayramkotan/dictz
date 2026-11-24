"""Plays a dictionary's sounds (``audio.player``, ``audio.speed``, ``audio.volume``).

The built-in player is Qt Multimedia. A sound used to lose its last
syllable: the player stops the audio device when the data ends, before the
device's buffer has been heard. So the built-in player is given a WAV that
ends with silence (``core.audio.pad_wav``); an MP3 / Ogg / … is first
decoded with ``QAudioDecoder`` into such a WAV (kept in the media cache).
External players (mpv, FFplay, VLC, MPlayer, a command of your own) and the
system's player are started as separate programs. The status bar says when
the chosen player is not installed, and when an external player stops at
once with an error (e.g. it cannot decode the file).
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from PySide6.QtCore import QObject, QTimer, QUrl, Signal
from PySide6.QtGui import QDesktopServices

from ..core import audio
from ..core.i18n import t


def internal_available() -> bool:
    try:
        from PySide6.QtMultimedia import QMediaPlayer  # noqa: F401
    except ImportError:
        return False
    return True


class AudioPlayer(QObject):
    """One player for the window; a new sound stops the one playing."""

    message = Signal(str)  # a line for the status bar

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._player = None
        self._out = None
        self._decoder = None
        self._proc: subprocess.Popen | None = None
        self.last: tuple[Path, str, int, float, str, str] | None = None

    # ------------------------------------------------------------ public
    def play(self, file: Path, player: str, volume: int, speed: float, command: str = "") -> str:
        """Play ``file`` (in the media cache); returns the player used."""
        self.stop()
        self.last = (file, player, volume, speed, command, "")
        used = audio.resolve(player, internal_available(), command)
        if player in audio.EXTERNAL and used != player:
            self.message.emit(t("{player} kurulu değil, {used} kullanılıyor",
                                player=t(audio.label(player)), used=t(audio.label(used))))
        if used == "internal":
            if self._play_internal(file, volume, speed):
                return used
            used = audio.resolve("auto", False, "")
        if used in audio.EXTERNAL or used == "command":
            program = audio.find_program(used) if used != "command" else ""
            cmd = audio.player_command(used, program or "", str(file), volume, speed, command)
            if cmd and self._start(cmd):
                self._watch(self._proc, cmd[0] if used == "command" else t(audio.label(used)))
                return used
            used = "system"
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(file)))
        return "system"

    def replay(self) -> None:
        if self.last is not None:
            f, p, v, s, c, _ = self.last
            self.play(f, p, v, s, c)

    def stop(self) -> None:
        if self._decoder is not None:
            self._decoder.stop()
            self._decoder = None
        if self._player is not None:
            self._player.stop()
        if self._proc is not None and self._proc.poll() is None:
            self._proc.terminate()
        self._proc = None

    # ------------------------------------------------------------ external
    def _start(self, cmd: list[str]) -> bool:
        try:
            flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0  # type: ignore[attr-defined]
            self._proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                          stderr=subprocess.DEVNULL, creationflags=flags)
            return True
        except OSError as e:
            self.message.emit(f"{cmd[0]}: {e}")
            return False

    def _watch(self, proc: subprocess.Popen | None, name: str) -> None:
        """Say so when the player gives up at once (bad codec, wrong option)."""

        def check() -> None:
            if proc is None:
                return
            code = proc.poll()
            if code not in (None, 0) and proc is self._proc:
                self.message.emit(t("{player} sesi çalamadı (çıkış kodu {code})", player=name, code=str(code)))

        QTimer.singleShot(1500, check)

    # ------------------------------------------------------------ internal
    def _ensure_player(self) -> bool:
        if self._player is not None:
            return True
        try:
            from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer
        except ImportError:
            return False
        self._out = QAudioOutput(self)
        self._player = QMediaPlayer(self)
        self._player.setAudioOutput(self._out)
        self._player.errorOccurred.connect(lambda *_a: self._fallback(self.last[0] if self.last else None))
        return True

    def _fallback(self, file: Path | None) -> None:
        """The built-in player cannot play ``file`` (its codec): the first
        external player installed, else the system's player."""
        if file is None:
            return
        _f, _p, volume, speed, _c, _x = self.last or (file, "", 80, 1.0, "", "")
        used = audio.resolve("auto", False, "")
        if used in audio.EXTERNAL:
            cmd = audio.player_command(used, audio.find_program(used) or "", str(file), volume, speed, "")
            if cmd and self._start(cmd):
                self._watch(self._proc, t(audio.label(used)))
                return
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(file)))

    def _play_internal(self, file: Path, volume: int, speed: float) -> bool:
        if not self._ensure_player():
            return False
        padded = file.with_name(file.stem + ".padded.wav")
        if not padded.exists():
            data = audio.pad_wav(file.read_bytes()) if file.suffix.lower() == ".wav" else None
            if data is not None:
                padded.write_bytes(data)
            elif not self._decode(file, padded, volume, speed):
                return self._start_qt(file, volume, speed)  # no decoder: as it is
            else:
                return True  # plays when decoded
        return self._start_qt(padded, volume, speed)

    def _start_qt(self, file: Path, volume: int, speed: float) -> bool:
        self._out.setVolume(max(0, min(100, volume)) / 100.0)
        self._player.setSource(QUrl.fromLocalFile(str(file)))
        self._player.setPlaybackRate(max(0.5, min(1.5, speed)))
        self._player.play()
        return True

    def _decode(self, src: Path, dst: Path, volume: int, speed: float) -> bool:
        """Decode ``src`` (MP3, Ogg …) into a padded WAV, then play it."""
        try:
            from PySide6.QtMultimedia import QAudioDecoder, QAudioFormat
        except ImportError:
            return False
        dec = QAudioDecoder(self)
        fmt = QAudioFormat()
        fmt.setSampleFormat(QAudioFormat.SampleFormat.Int16)
        dec.setAudioFormat(fmt)
        chunks: list[bytes] = []
        info: dict[str, int] = {}

        def ready() -> None:
            buf = dec.read()
            f = buf.format()
            info.setdefault("rate", f.sampleRate())
            info.setdefault("channels", f.channelCount())
            info.setdefault("width", f.bytesPerSample())
            chunks.append(bytes(buf.constData()) if hasattr(buf, "constData") else bytes(buf.data()))

        def finished() -> None:
            self._decoder = None
            if not chunks or not info.get("rate"):
                self._fallback(src)
                return
            dst.write_bytes(audio.pcm_to_wav(b"".join(chunks), info["rate"], info["channels"],
                                             info.get("width") or 2))
            self._start_qt(dst, volume, speed)

        def failed(*_a) -> None:
            self._decoder = None
            self._fallback(src)  # Qt cannot decode it (e.g. Speex .spx)

        dec.bufferReady.connect(ready)
        dec.finished.connect(finished)
        dec.error.connect(failed)
        dec.setSource(QUrl.fromLocalFile(str(src)))
        self._decoder = dec
        dec.start()
        return True
