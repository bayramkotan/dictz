"""Tests for the packaging artifacts (PyInstaller spec, workflow YAML, scripts).

These don't actually build anything — they just make sure the files are
present, parseable, and point at the right entry points. The real build is
exercised by the CI pipeline.
"""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_pyinstaller_spec_exists() -> None:
    spec = ROOT / "packaging" / "pyinstaller" / "dictz.spec"
    assert spec.is_file()
    text = spec.read_text(encoding="utf-8")
    # The spec must point at dictz.__main__ so `python -m dictz` is the entry
    assert "__main__.py" in text
    # Qt WebEngine must be in hidden_imports or the bundle won't launch
    assert "QtWebEngineWidgets" in text
    # PyGlossary plugin discovery
    assert "pyglossary.plugins" in text


def test_appimage_builder_exists() -> None:
    sh = ROOT / "packaging" / "appimage" / "build_appimage.sh"
    assert sh.is_file()
    assert sh.stat().st_mode & 0o111, "build_appimage.sh must be executable"


def test_build_scripts_exist() -> None:
    assert (ROOT / "scripts" / "build.sh").is_file()
    assert (ROOT / "scripts" / "build.ps1").is_file()


def test_release_workflow_exists() -> None:
    wf = ROOT / ".github" / "workflows" / "release.yml"
    assert wf.is_file()
    text = wf.read_text(encoding="utf-8")
    # Must build on at least these runners
    for runner in ("ubuntu-22.04", "windows-latest", "macos-13", "macos-14"):
        assert runner in text, f"release.yml does not include {runner}"
    # Must run PyInstaller
    assert "pyinstaller" in text.lower()
    # Must create a release on tag
    assert "startsWith(github.ref, 'refs/tags/v')" in text


def test_test_workflow_exists() -> None:
    wf = ROOT / ".github" / "workflows" / "test.yml"
    assert wf.is_file()


def test_packaging_doc_exists() -> None:
    assert (ROOT / "docs" / "PACKAGING.md").is_file()
