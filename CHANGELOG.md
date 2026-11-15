# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.5] — 2026-11-15

### Fixed
- **Portable binaries did not start.** The PyInstaller entry point was `dictz/__main__.py`, whose relative import (`from .app import main`) crashes a frozen executable. New absolute-import launcher `packaging/pyinstaller/launch_dictz.py`.
- **DSL / Babylon files could not be opened from the portable binaries.** In a frozen build PyGlossary looked for its plugins and data (`plugins/`, `langs/langs.json`, `plugins-meta/index.json`) next to the executable instead of inside the PyInstaller bundle. `dictz.parsers.pyglossary_base` now imports PyGlossary so it resolves its root from the bundle, and the plugin `.py` sources are shipped as data so PyGlossary's directory scan finds them.
- PyGlossary 4.x vs 5.x keyword (`format` / `formatName`) for `directRead` is detected at runtime; the wrong one was silently ignored and fell back to extension sniffing.
- macOS bundle version (`CFBundleShortVersionString`) was hard-coded at 0.0.4; it is now read from `dictz/__init__.py`.
- About box showed a hard-coded 0.0.4; now uses `dictz.__version__`.

### Added
- **`dictz --selftest`** — headless check (no window) that loads a StarDict and a DSL dictionary, verifies the DSL + Babylon plugins and the SQLite FTS5 index. Exit code 0/1; report also written to `$DICTZ_SELFTEST_REPORT`.
- **`dictz --version`**
- **One release pipeline** in `.github/workflows/ci.yml`: a `v*` tag publishes to **PyPI**, **crates.io** and **GitHub Releases** (Linux AppImage + tar.gz, Windows zip, macOS Apple Silicon zip).
- Every frozen binary (and the AppImage, and the pip-installed console script) is executed with `--selftest` in CI before anything is published.
- `preflight` job: on a tag, fails within seconds if `PYPI_API_TOKEN` / `CARGO_REGISTRY_TOKEN` secrets are missing or the tag does not match the version in `pyproject.toml`, `dictz/__init__.py` and `dictz-rs/Cargo.toml`.
- **`dictz-rs/`** Rust crate (scaffold): `Dictionary` trait, `DictionaryEntry`, `DictionaryMetadata`, `DictzError`, `VERSION`; version kept in lockstep with the Python package.
- `tests/test_selftest.py` and new packaging assertions (61 tests).

### Changed
- Publishing uses **API-token secrets** instead of PyPI Trusted Publishing: no PyPI-side publisher registration and no GitHub environments needed.
- AppImage is built with `appimagetool` (packs the already-bundled PyInstaller tree) instead of `linuxdeploy`; works on CI without FUSE.
- Intel macOS binary dropped: GitHub's Intel macOS runners never start on public repositories and blocked the whole release. Intel Mac users: `pip install dictz`.
- Re-pushing a tag cancels the previous run instead of queueing behind it.
- `test.yml` + `release.yml` merged into `ci.yml`.

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
