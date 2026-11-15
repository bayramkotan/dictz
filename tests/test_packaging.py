"""Tests for the packaging artifacts (PyInstaller spec, workflow YAML, scripts).

Platform-aware: Unix executable-bit checks are skipped on Windows because NTFS
does not carry that bit. YAML + spec parsing is done in Python so a broken
workflow file fails the test instead of silently waiting for CI.
"""
from __future__ import annotations

import ast
import os
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


# --------------------------------------------------------------- spec file

def test_pyinstaller_spec_exists() -> None:
    spec = ROOT / "packaging" / "pyinstaller" / "dictz.spec"
    assert spec.is_file()


def test_pyinstaller_spec_parses_as_python() -> None:
    """The .spec file is Python; make sure it is syntactically valid."""
    spec = ROOT / "packaging" / "pyinstaller" / "dictz.spec"
    source = spec.read_text(encoding="utf-8")
    ast.parse(source)


def test_pyinstaller_spec_has_required_pieces() -> None:
    spec = (ROOT / "packaging" / "pyinstaller" / "dictz.spec").read_text(encoding="utf-8")
    # Entry point must be the absolute-import launcher, not dictz/__main__.py
    # (its relative import crashes a frozen binary on startup).
    assert 'SPEC_DIR / "launch_dictz.py"' in spec
    assert (ROOT / "packaging" / "pyinstaller" / "launch_dictz.py").is_file()
    # PyGlossary scans plugin .py files on disk, so they must be shipped as data
    assert "include_py_files=True" in spec
    # macOS bundle version is read from dictz/__init__.py, never hard-coded
    assert '"CFBundleShortVersionString": VERSION' in spec
    # Qt WebEngine — bundle fails to launch without this hidden import
    assert "QtWebEngineWidgets" in spec
    assert "QtWebEngineCore" in spec
    # PyGlossary plugin discovery via collect_submodules
    assert "collect_submodules" in spec
    assert "pyglossary.plugins" in spec
    # macOS bundle definition
    assert "BUNDLE" in spec
    assert "com.bayramkotan.dictz" in spec


# ----------------------------------------------------------- build scripts

def test_appimage_builder_exists() -> None:
    sh = ROOT / "packaging" / "appimage" / "build_appimage.sh"
    assert sh.is_file()


@pytest.mark.skipif(os.name == "nt", reason="NTFS does not track Unix executable bits")
def test_appimage_builder_executable_on_unix() -> None:
    sh = ROOT / "packaging" / "appimage" / "build_appimage.sh"
    assert sh.stat().st_mode & 0o111, "build_appimage.sh must be executable on Unix"


def test_appimage_builder_has_shebang() -> None:
    sh = ROOT / "packaging" / "appimage" / "build_appimage.sh"
    first = sh.read_text(encoding="utf-8").splitlines()[0]
    assert first.startswith("#!"), f"missing shebang: {first!r}"
    assert "bash" in first, f"expected bash shebang, got {first!r}"


def test_build_scripts_exist() -> None:
    assert (ROOT / "scripts" / "build.sh").is_file()
    assert (ROOT / "scripts" / "build.ps1").is_file()


@pytest.mark.skipif(os.name == "nt", reason="NTFS does not track Unix executable bits")
def test_build_sh_executable_on_unix() -> None:
    sh = ROOT / "scripts" / "build.sh"
    assert sh.stat().st_mode & 0o111, "scripts/build.sh must be executable on Unix"


# -------------------------------------------------------- GitHub workflows

def _load_yaml(path: Path) -> dict:
    # yaml is a transitive dep via pyglossary — in case it isn't, skip gracefully
    try:
        import yaml  # type: ignore
    except ImportError:
        pytest.skip("PyYAML not installed; skip YAML structural checks")
    return yaml.safe_load(path.read_text(encoding="utf-8"))


CI_YML = ROOT / ".github" / "workflows" / "ci.yml"


def test_single_workflow_file() -> None:
    """Only one workflow file — ci.yml — carries lint/test/build/release."""
    wf_dir = ROOT / ".github" / "workflows"
    yml_files = sorted(p.name for p in wf_dir.glob("*.yml"))
    assert yml_files == ["ci.yml"], (
        f"expected a single ci.yml workflow, found: {yml_files}"
    )


def test_ci_workflow_is_valid_yaml() -> None:
    _load_yaml(CI_YML)


def test_ci_workflow_builds_three_platforms_on_tag() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    for runner in ("ubuntu-22.04", "windows-latest", "macos-latest"):
        assert runner in text, f"ci.yml does not include {runner}"


def test_ci_workflow_never_uses_starved_intel_mac_runner() -> None:
    """macos-13 jobs never get a runner on public repos and block the release."""
    text = CI_YML.read_text(encoding="utf-8")
    assert "macos-13" not in text


def test_ci_workflow_selftests_every_binary() -> None:
    """Every frozen binary is executed with --selftest before it is shipped."""
    text = CI_YML.read_text(encoding="utf-8")
    assert "./dist/dictz/dictz --selftest" in text                       # Linux
    assert "dist/dictz.app/Contents/MacOS/dictz --selftest" in text      # macOS
    assert '"dist\\dictz\\dictz.exe" -ArgumentList "--selftest"' in text  # Windows
    assert ".AppImage --selftest" in text                                # AppImage
    assert "dictz --selftest" in text                                    # pip install


def test_ci_workflow_uses_token_secrets_not_oidc_environments() -> None:
    """Publishing uses API-token secrets; no GitHub environments / PyPI OIDC
    publisher setup is required (that setup caused invalid-publisher errors)."""
    text = CI_YML.read_text(encoding="utf-8")
    assert "secrets.PYPI_API_TOKEN" in text
    assert "secrets.CARGO_REGISTRY_TOKEN" in text
    assert "environment:" not in text
    assert "id-token: write" not in text


def test_ci_workflow_preflight_fails_fast_on_missing_secrets() -> None:
    data = _load_yaml(CI_YML)
    assert "preflight" in data["jobs"]
    assert "preflight" in data["jobs"]["publish-pypi"]["needs"]
    assert "preflight" in data["jobs"]["publish-crates"]["needs"]


def test_ci_workflow_runs_pyinstaller() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "pyinstaller" in text.lower()


def test_ci_workflow_tag_and_push_triggered() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "tags: ['v*']" in text or 'tags: ["v*"]' in text
    assert "branches: [main]" in text


def test_ci_workflow_publishes_to_pypi() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "pypa/gh-action-pypi-publish" in text
    assert "python -m build" in text


def test_ci_workflow_publishes_to_crates_io() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "crates.io" in text.lower()
    assert "cargo publish" in text


def test_ci_workflow_builds_rust() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "cargo check" in text
    assert "cargo test" in text


def test_ci_workflow_release_jobs_tag_gated() -> None:
    """Build + publish jobs are gated behind the tag ref."""
    text = CI_YML.read_text(encoding="utf-8")
    assert "if: startsWith(github.ref, 'refs/tags/v')" in text


# ------------------------------------------------------- Rust crate

def test_rust_cargo_toml_exists() -> None:
    assert (ROOT / "dictz-rs" / "Cargo.toml").is_file()


def test_rust_src_lib_exists() -> None:
    assert (ROOT / "dictz-rs" / "src" / "lib.rs").is_file()


def test_rust_version_matches_python() -> None:
    """Python __version__ and Cargo.toml version must stay in sync."""
    from dictz import __version__
    cargo = (ROOT / "dictz-rs" / "Cargo.toml").read_text(encoding="utf-8")
    import re
    m = re.search(r'^version\s*=\s*"([^"]+)"', cargo, flags=re.MULTILINE)
    assert m, "Cargo.toml has no version line"
    assert m.group(1) == __version__, (
        f"Cargo.toml version {m.group(1)} != Python {__version__}"
    )


def test_rust_license_matches_python() -> None:
    cargo = (ROOT / "dictz-rs" / "Cargo.toml").read_text(encoding="utf-8")
    assert 'license = "GPL-3.0-or-later"' in cargo


def test_ci_workflow_covers_multiple_python_versions() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    present = sum(1 for v in ("3.10", "3.11", "3.12", "3.13") if f"'{v}'" in text)
    assert present >= 3, f"ci.yml should exercise 3+ Python versions, found {present}"


def test_ci_workflow_covers_all_three_oses() -> None:
    text = CI_YML.read_text(encoding="utf-8")
    assert "ubuntu" in text and "windows" in text and "macos" in text


def test_ci_workflow_sets_offscreen_qt() -> None:
    """Qt must start headlessly on CI — otherwise QWidget tests crash."""
    text = CI_YML.read_text(encoding="utf-8")
    assert "QT_QPA_PLATFORM" in text
    assert "offscreen" in text


# ------------------------------------------------------------- docs

def test_packaging_doc_exists() -> None:
    assert (ROOT / "docs" / "PACKAGING.md").is_file()


def test_changelog_mentions_current_version() -> None:
    from dictz import __version__
    text = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    assert f"[{__version__}]" in text, (
        f"CHANGELOG.md has no [{__version__}] section"
    )


def test_todo_exists() -> None:
    assert (ROOT / "TODO.md").is_file()


def test_license_is_gpl3() -> None:
    text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    assert "GNU GENERAL PUBLIC LICENSE" in text
    assert "Version 3" in text


# ---------------------------------------------------------- environment

def test_python_version_floor_matches_pyproject() -> None:
    if sys.version_info < (3, 11):
        pytest.skip("tomllib requires Python 3.11+")
    import tomllib
    data = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    requires = data["project"]["requires-python"]
    assert requires.startswith(">="), f"unexpected requires-python: {requires}"
