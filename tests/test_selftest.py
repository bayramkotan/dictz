"""Tests for the headless entry points: `dictz --version` and `dictz --selftest`."""
from __future__ import annotations

from pathlib import Path

import pytest

pytest.importorskip("pyglossary")

from dictz import __version__  # noqa: E402
from dictz.app import main  # noqa: E402


def test_version_flag(capsys: pytest.CaptureFixture[str]) -> None:
    assert main(["--version"]) == 0
    assert capsys.readouterr().out.strip() == f"dictz {__version__} (Python edition)"


def test_selftest_passes(capsys: pytest.CaptureFixture[str]) -> None:
    assert main(["--selftest"]) == 0
    out = capsys.readouterr().out
    assert "RESULT: OK" in out
    assert "FAIL" not in out


def test_selftest_writes_report_file(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    report = tmp_path / "report.txt"
    monkeypatch.setenv("DICTZ_SELFTEST_REPORT", str(report))
    assert main(["--selftest"]) == 0
    assert "RESULT: OK" in report.read_text(encoding="utf-8")


def test_app_has_no_hardcoded_version() -> None:
    """dictz/app.py must take its version from dictz/__init__.py."""
    src = (Path(__file__).resolve().parent.parent / "dictz" / "app.py").read_text(encoding="utf-8")
    assert "__version__ = " not in src
