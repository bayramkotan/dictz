"""Command line (the Rust edition has the same commands and output)."""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from dictz import __version__
from dictz.cli import completion_script, is_cli_invocation, main

PARITY = Path(__file__).resolve().parent / "data" / "parity"


@pytest.fixture()
def data_dir(tmp_path, monkeypatch):
    monkeypatch.setenv("DICTZ_DATA_DIR", str(tmp_path))
    monkeypatch.setenv("NO_COLOR", "1")
    return tmp_path


def run(capsys, *args) -> tuple[int, str, str]:
    try:
        rc = main(list(args))
    except SystemExit as e:  # argparse / resolve errors
        rc = e.code if isinstance(e.code, int) else 1
        if isinstance(e.code, str):
            print(e.code, file=__import__("sys").stderr)
    out = capsys.readouterr()
    return rc, out.out, out.err


def test_dispatch_rules() -> None:
    assert not is_cli_invocation([])
    assert is_cli_invocation(["lookup", "x"])
    assert is_cli_invocation(["ls"])
    assert is_cli_invocation(["--version"])
    assert not is_cli_invocation(["En-Tr.dsl"])


def test_version(data_dir, capsys) -> None:
    rc, out, _ = run(capsys, "version")
    assert rc == 0 and out.strip() == f"dictz {__version__} (Python edition)"
    rc, out, _ = run(capsys, "version", "--json")
    assert json.loads(out)["edition"] == "python"


def test_add_lookup_and_manage(data_dir, capsys) -> None:
    rc, out, _ = run(capsys, "add", str(PARITY / "sample.dsl"), str(PARITY / "sample.bgl"))
    assert rc == 0 and out.count("added:") == 2
    rc, out, _ = run(capsys, "dicts", "--json")
    rows = json.loads(out)
    assert [r["position"] for r in rows] == [1, 2]
    assert all(Path(r["path"]).is_absolute() for r in rows)

    rc, out, _ = run(capsys, "lookup", "apple")
    assert rc == 0 and "== dictz parity sample ==" in out and "  elma" in out
    rc, out, err = run(capsys, "lookup", "aple")
    assert rc == 1 and "Did you mean: apple" in err
    rc, out, _ = run(capsys, "lookup", "-f", "aple")
    assert rc == 0 and "apple" in out

    rc, out, _ = run(capsys, "disable", "bgl")
    assert out.startswith("disabled: dictz parity BGL")
    rc, out, _ = run(capsys, "add", "-r", str(PARITY), "--disabled")
    rows = {r["name"]: r["enabled"] for r in json.loads(run(capsys, "dicts", "--json")[1])}
    # -r also finds stardict/sample.ifo (the synonym sample)
    assert rows == {"dictz parity BGL — Türkçe": False, "dictz parity sample": False, "Synonym Test": False}
    run(capsys, "enable", "sample")
    rc, out, _ = run(capsys, "move", "sample", "top")
    assert out.strip() == "dictz parity sample: position 1"
    rc, _, err = run(capsys, "lookup", "-d", "nope", "x")
    assert rc == 1 and "no dictionary named 'nope'" in err

    rc, out, _ = run(capsys, "history", "--json")
    assert "apple" in json.loads(out)
    run(capsys, "favorites", "add", "apple")
    assert json.loads(run(capsys, "favorites", "--json")[1]) == ["apple"]

    rc, out, _ = run(capsys, "dump", "sample", "-F", "json")
    assert any(e["headword"] == "apple" for e in json.loads(out))
    rc, out, _ = run(capsys, "remove", "bgl")
    assert out.startswith("removed:")


def test_config(data_dir, capsys) -> None:
    rc, out, _ = run(capsys, "config", "set", "article.font_size", "99")
    assert out.strip() == "article.font_size = 48"
    rc, out, _ = run(capsys, "config", "get", "article.font_size")
    assert out.strip() == "48"
    rc, _, err = run(capsys, "config", "set", "search.fuzzy", "x")
    assert rc == 1 and "got 'x'" in err
    rc, out, _ = run(capsys, "config", "list")
    assert "[search] Arama" in out and "(100–1000000)" in out
    data = json.loads((data_dir / "settings.json").read_text(encoding="utf-8"))
    assert data["article"]["font_size"] == 48
    rc, out, _ = run(capsys, "config", "reset")
    assert out.strip() == "reset: all settings"


def test_completion_scripts_mention_every_command() -> None:
    from dictz.cli import COMMANDS

    for shell in ("bash", "zsh", "fish", "powershell"):
        text = completion_script(shell)
        for c, _ in COMMANDS:
            assert c in text, (shell, c)


def test_updater_version_compare() -> None:
    from dictz.core.updater import parse_version

    assert parse_version("0.0.10") > parse_version("0.0.9")
    assert parse_version("v0.1.0") > parse_version("0.0.99")
    assert parse_version("0.0.7") == parse_version("0.0.7")


@pytest.mark.parametrize("flag", ["-V", "-v", "--version", "--Version", "--VERSION", "-version", "-Version"])
def test_every_version_spelling(data_dir, capsys, flag) -> None:
    assert is_cli_invocation([flag])
    rc, out, _ = run(capsys, flag)
    assert rc == 0 and out.strip() == f"dictz {__version__} (Python edition)"


def test_short_name_dct(monkeypatch) -> None:
    from dictz.cli import program_name

    monkeypatch.setattr("sys.argv", ["/usr/bin/dct"])
    assert program_name() == "dct"
    monkeypatch.setattr("sys.argv", [r"C:\x\Scripts\dct.exe"])
    assert program_name() == "dct"
    monkeypatch.setattr("sys.argv", ["/usr/bin/dictz"])
    assert program_name() == "dictz"
    for shell in ("bash", "zsh", "fish", "powershell"):
        assert "dct" in completion_script(shell)


@pytest.mark.skipif(not __import__("sys").platform.startswith("linux"), reason="Linux .desktop files")
def test_shortcut_create_and_remove(data_dir, capsys, monkeypatch, tmp_path) -> None:
    monkeypatch.setenv("HOME", str(tmp_path / "home"))
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "share"))
    rc, out, _ = run(capsys, "shortcut", "--menu-only")
    entry = tmp_path / "share" / "applications" / "dictz.desktop"
    assert rc == 0 and entry.is_file()
    text = entry.read_text(encoding="utf-8")
    assert "Exec=" in text and "Name=dictz" in text and "Icon=" in text
    rc, out, _ = run(capsys, "shortcut", "--remove", "--menu-only")
    assert rc == 0 and not entry.exists()


def test_icon_files_are_valid() -> None:
    import struct
    import zlib

    from dictz.core.desktop import icns_bytes, ico_bytes, png_bytes

    png = png_bytes(64)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    w, h = struct.unpack(">II", png[16:24])
    assert (w, h) == (64, 64)
    ln = struct.unpack(">I", png[33:37])[0]
    assert png[37:41] == b"IDAT"
    assert len(zlib.decompress(png[41:41 + ln])) == 64 * (64 * 4 + 1)
    ico = ico_bytes(64)
    assert ico[:6] == b"\x00\x00\x01\x00\x01\x00" and ico[22:] == png
    assert icns_bytes(64)[:4] == b"icns"
