# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.5] — 2026-11-15

### Added
- **Single unified workflow** (`.github/workflows/ci.yml`) — replaces separate `test.yml` + `release.yml`. One file, 9 jobs, every event (push / PR / tag) handled through conditional `if:` gates.
- **PyPI auto-publish** via Trusted Publishing (OIDC, no API token in secrets) — `pypa/gh-action-pypi-publish@release/v1`
- **crates.io auto-publish** — `cargo publish` dry-run then live, gated on `CARGO_REGISTRY_TOKEN` secret
- **`dictz-rs/` Rust crate scaffold** — `Dictionary` trait, `DictionaryEntry`, `DictionaryMetadata`, `DictzError`, `VERSION`, 4 unit tests, dependency-free (`#![forbid(unsafe_code)]`), GPL-3.0-or-later, version pinned to the Python package's version
- **Rust CI** — `cargo check` + `cargo test` on every push; `cargo build --release` on tag
- `dictz-rs/README.md` — crate-level docs with badges, status table, install snippet
- GitHub Release body auto-generated with per-registry install instructions
- `tests/test_packaging.py` — new `test_single_workflow_file` + 8 renamed `test_ci_workflow_*` assertions

### Changed
- Workflow jobs: `lint` → `test` + `rust` → `install-smoke` run on every push/PR. `build-python-binaries` + `build-python-dist` + `publish-pypi` + `publish-crates` + `github-release` are **tag-gated** via `if: startsWith(github.ref, 'refs/tags/v')`.
- `test.yml` and `release.yml` deleted — merged into `ci.yml`

## [0.0.4] — 2026-11-15

### Added
- **Cross-platform portable builds** — Windows portable zip, Linux tarball + AppImage, macOS `.app` bundle
- **`packaging/pyinstaller/dictz.spec`** — single PyInstaller spec that builds on all three OSes; pulls in every PyGlossary plugin, keeps QtWebEngine, trims the unused Qt modules for a ~80 MB size cut
- **`packaging/appimage/build_appimage.sh`** — wraps a PyInstaller `--onedir` build into a standalone AppImage via `linuxdeploy`
- **`scripts/build.sh`** + **`scripts/build.ps1`** — one-command local builds that produce the same artifacts CI does
- **`.github/workflows/release.yml`** — tag-triggered matrix build (Ubuntu 22.04, Windows, macOS Intel + Apple Silicon), uploads each archive as a workflow artifact, and attaches them all to a new GitHub Release with auto-generated notes
- **`.github/workflows/test.yml`** — runs the test suite on every push/PR across Linux/macOS/Windows × Python 3.10/3.11/3.12
- **`docs/PACKAGING.md`** — build process, artifact table, CI flow, known caveats (Windows SmartScreen, macOS Gatekeeper, glibc floor)
- **`tests/test_packaging.py`** — asserts the spec, scripts and workflows exist, have the expected entry points, and are executable

### Changed
- About-box, welcome footer, and version strings bumped to 0.0.4
- README gains badges + links to GitHub Releases and a "Download" section
- TODO.md marks packaging done, promotes fuzzy matching + rich media to the top

## [0.0.3] — 2026-11-13

### Added
- **Tabbed definition view** — each lookup opens in its own tab, Ctrl+T / Ctrl+W / Ctrl+Tab
- **QWebEngineView-based renderer** — rich HTML rendering of definitions, with QTextBrowser fallback
- **Dockable sidebar** with Sözlükler / Favoriler / Geçmiş panes (Ctrl+B)
- **Back / Forward navigation** — Alt+← / Alt+→
- **Favorites + history** — persistent in SQLite
- **Dictionary manager dialog** — add / remove / reorder / enable / disable
- **Light + dark themes** with `auto` selection
- **Preferences dialog** — theme, font, debounce
- **Zoom** — Ctrl+= / Ctrl+- / Ctrl+0
- **Window state persistence**
- `dictz/storage/history.py`, `dictz/storage/settings.py`, UI modules, tests (24 total)

### Changed
- Main window completely rewritten; `IndexStore` schema gets `sort_order`; `QueryManager` honors enabled set

## [0.0.2] — 2026-11-13

### Added
- DSL (ABBYY Lingvo) and Babylon (.bgl) format support via PyGlossary
- `PyGlossaryDictionary` generic base
- CHANGELOG, LICENSE, tests for new parsers

### Changed
- `pyglossary` promoted to required dependency

## [0.0.1] — 2026-11-12

### Added
- Project bootstrap, native StarDict parser, SQLite FTS5 index, basic 3-pane UI, 11 tests
