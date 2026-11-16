"""Single instance / `dictz show|popup|watch` protocol (shared with the Rust edition)."""
from __future__ import annotations

import json

from dictz.core import instance
from dictz.core.hotkeys import parse_hotkey


def test_roundtrip(tmp_path) -> None:
    assert instance.send(tmp_path, {"cmd": "ping"}) is None
    seen = []

    def handler(req):
        seen.append(req)
        return {"ok": True, "echo": req["cmd"]}

    s = instance.Server(tmp_path, handler, "python", "0")
    assert s.start()
    info = json.loads((tmp_path / instance.INSTANCE_FILE).read_text())
    assert info["edition"] == "python" and len(info["token"]) == 32
    assert instance.send(tmp_path, {"cmd": "show"}) == {"ok": True, "echo": "show"}
    assert "token" not in seen[0]
    assert instance.is_running(tmp_path)
    s.stop()
    assert not (tmp_path / instance.INSTANCE_FILE).exists()
    assert instance.send(tmp_path, {"cmd": "show"}) is None


def test_bad_token_is_refused(tmp_path) -> None:
    s = instance.Server(tmp_path, lambda r: {"ok": True}, "python", "0")
    assert s.start()
    info = json.loads((tmp_path / instance.INSTANCE_FILE).read_text())
    info["token"] = "0" * 32
    (tmp_path / instance.INSTANCE_FILE).write_text(json.dumps(info))
    assert instance.send(tmp_path, {"cmd": "show"}) == {"ok": False, "error": "bad token"}
    s.stop()


def test_cli_watch_without_running_dictz(tmp_path, monkeypatch, capsys) -> None:
    from dictz.cli import main

    monkeypatch.setenv("DICTZ_DATA_DIR", str(tmp_path))
    assert main(["watch", "status"]) == 1
    assert "not running" in capsys.readouterr().err


def test_hotkey_text() -> None:
    assert str(parse_hotkey("ctrl+shift+pgdown")) == "Ctrl+Shift+PgDown"
