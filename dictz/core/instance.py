"""Talking to the running dictz (single instance, ``dictz popup`` / ``show`` / ``scan``).

The running window listens on 127.0.0.1 and writes its port and a random token
to ``instance.json`` in the data folder. A request is one JSON line with the
token; the answer is one JSON line. The Rust edition speaks the same protocol
(``dictz-rs/src/instance.rs``), so either edition's CLI can drive either window.

Requests: ``{"cmd": "ping" | "show" | "hide" | "toggle" | "lookup" | "popup" |
"scan" | "open" | "quit", ...}``
  * ``lookup`` / ``popup``: ``"word"`` (empty popup word = clipboard text)
  * ``scan``: ``"state": "on" | "off" | "toggle" | "status"``
  * ``open``: ``"files": [paths]``
Answer: ``{"ok": true, ...}`` (``scan`` adds ``"scan": bool``) or
``{"ok": false, "error": "..."}``.
"""
from __future__ import annotations

import contextlib
import json
import os
import secrets
import socket
import threading
from collections.abc import Callable
from pathlib import Path
from typing import Any

INSTANCE_FILE = "instance.json"
MAX_REQUEST = 1 << 20


def info_path(data_dir: Path) -> Path:
    return Path(data_dir) / INSTANCE_FILE


def read_info(data_dir: Path) -> dict | None:
    try:
        info = json.loads(info_path(data_dir).read_text(encoding="utf-8"))
        if isinstance(info, dict) and isinstance(info.get("port"), int) and isinstance(info.get("token"), str):
            return info
    except (OSError, ValueError):
        pass
    return None


def _read_line(sock: socket.socket) -> bytes:
    buf = b""
    while b"\n" not in buf and len(buf) < MAX_REQUEST:
        chunk = sock.recv(65536)
        if not chunk:
            break
        buf += chunk
    return buf.split(b"\n", 1)[0]


def send(data_dir: Path, message: dict, timeout: float = 2.0) -> dict | None:
    """Send ``message`` to the running dictz; ``None`` when none is running."""
    info = read_info(data_dir)
    if info is None:
        return None
    payload = dict(message, token=info["token"])
    try:
        with socket.create_connection(("127.0.0.1", info["port"]), timeout=timeout) as s:
            s.sendall(json.dumps(payload, ensure_ascii=False).encode("utf-8") + b"\n")
            line = _read_line(s)
        answer = json.loads(line.decode("utf-8"))
        return answer if isinstance(answer, dict) else None
    except (OSError, ValueError):
        return None


def is_running(data_dir: Path) -> bool:
    answer = send(data_dir, {"cmd": "ping"}, timeout=1.0)
    return bool(answer and answer.get("ok"))


class Server:
    """Accepts requests in a background thread and passes them to ``handler``
    (called in that thread; it must be thread-safe and return the answer)."""

    def __init__(self, data_dir: Path, handler: Callable[[dict], dict], edition: str = "python",
                 version: str = "") -> None:
        self.data_dir = Path(data_dir)
        self.handler = handler
        self.token = secrets.token_hex(16)
        self.edition = edition
        self.version = version
        self._sock: socket.socket | None = None
        self._thread: threading.Thread | None = None

    def start(self) -> bool:
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.bind(("127.0.0.1", 0))
            sock.listen(8)
        except OSError:
            return False
        self._sock = sock
        info = {"port": sock.getsockname()[1], "token": self.token, "pid": os.getpid(),
                "edition": self.edition, "version": self.version}
        path = info_path(self.data_dir)
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".json.tmp")
            tmp.write_text(json.dumps(info), encoding="utf-8")
            with contextlib.suppress(OSError):
                os.chmod(tmp, 0o600)
            os.replace(tmp, path)
        except OSError:
            sock.close()
            self._sock = None
            return False
        self._thread = threading.Thread(target=self._serve, name="dictz-instance", daemon=True)
        self._thread.start()
        return True

    def _serve(self) -> None:
        assert self._sock is not None
        while True:
            try:
                conn, _ = self._sock.accept()
            except OSError:
                return
            with conn:
                conn.settimeout(3.0)
                try:
                    request = json.loads(_read_line(conn).decode("utf-8"))
                    if not isinstance(request, dict) or not secrets.compare_digest(
                            str(request.get("token", "")), self.token):
                        answer: dict[str, Any] = {"ok": False, "error": "bad token"}
                    else:
                        request.pop("token", None)
                        answer = self.handler(request)
                except Exception as exc:  # noqa: BLE001
                    answer = {"ok": False, "error": str(exc)}
                with contextlib.suppress(OSError):
                    conn.sendall(json.dumps(answer, ensure_ascii=False).encode("utf-8") + b"\n")

    def stop(self) -> None:
        if self._sock is not None:
            with contextlib.suppress(OSError):
                self._sock.close()
            self._sock = None
        info = read_info(self.data_dir)
        if info and info.get("token") == self.token:
            with contextlib.suppress(OSError):
                info_path(self.data_dir).unlink()
