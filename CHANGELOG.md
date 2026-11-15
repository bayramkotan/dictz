# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.6] — 2026-11-15

**dictz now ships as two full editions with the same features:** Python (`pip install dictz`, Qt) and Rust (`cargo install dictz`, egui). Both are released from the same tag with the same version.

### Added — Rust edition (`dictz-rs/`, crates.io)
- **Full application**, no longer a scaffold. The binary `dictz` has `--version`, `--selftest` and `FILE…` arguments. The GUI has the same layout, Turkish labels and shortcuts as the Python edition:
  - menu bar, toolbar (back/forward, search, favorite, zoom, theme, preferences), live suggestions
  - tabbed articles
  - sidebar with Sözlükler / Favoriler / Geçmiş
  - dictionary manager (enable/disable, add, remove, up/down, add by path) and preferences (theme, font file, size, search delay)
  - about box, light/dark/auto themes, zoom, drag-and-drop of dictionary files
- **Native parsers**, written in Rust without Python or PyGlossary:
  - **StarDict**
  - **DSL**: a port of PyGlossary's DSL reader
  - **Babylon `.bgl`**: a port of PyGlossary's Babylon reader, covering charsets, `<charset>` tags, entities, part of speech, title and transcription fields, `$index$` markers, type-11 blocks and the Persian clean-up
- **SQLite FTS5 index**, **history** and **favorites** with the same schema and the same files as the Python edition. Both editions share one data folder, so dictionaries, history and favorites appear in both.
- A small HTML renderer for articles (colors, bold/italic, indented paragraphs, links, pictures, sounds), with readable colors in the dark theme.
- System fonts are used as fallbacks for wide script coverage (Turkish, Cyrillic, Greek, Arabic/Persian, CJK), plus a bold face.
- **Portable Rust binaries** on every release: `dictz-rs-linux-x86_64.tar.gz`, `dictz-rs-windows-x86_64.zip`, `dictz-rs-macos-arm64.tar.gz`. Each one runs `--selftest` in CI before it is published.
- Library use without GUI dependencies: `default-features = false`.

### Added — both editions (v0.0.6 features)
- **Fuzzy search / "did you mean"**:
  - When nothing matches, the not-found page lists the closest headwords with their similarity (%).
  - Suggestions are ranked closest-first and filled up with fuzzy matches, marked `≈`.
  - Scores are RapidFuzz `fuzz.ratio`, identical in both editions.
- **DSL pictures and sounds**:
  - `[s]picture.png[/s]` is shown inline and `[s]sound.wav[/s]` gets a ▶ play button.
  - Files are looked up in `X.dsl.files/`, then `X.dsl.files.zip`, then StarDict `res/`, then the dictionary folder; the match is case-insensitive.
  - Python plays sounds with QtMultimedia, Rust with rodio; both fall back to the system player.
- **Dictionaries come back after a restart.** They were lost on every start before.
- **Dictionary order is kept**: the manager's up/down now changes the search order and the sidebar order.
- **Parity tests**: `tests/data/parity/` holds a hand-made DSL, BGL and media zip plus golden output from the Python edition. `dictz-rs/tests/parity.rs` and `tests/test_parity.py` require byte-identical headwords, definitions and metadata. They were also checked against real PyGlossary test dictionaries (4 BGL, 4 DSL: identical).
- `--selftest` gained the **Babylon parser**, **Fuzzy matching** and **Media resolver** checks (6 checks in Rust, 7 in Python).

### Fixed
- **Python:** clicking links in articles did nothing with QtWebEngine; links now open (`bword://`, `http(s)://`, sounds).
- **Python:** DSL headwords containing `&`, `<` or `>` were stored XML-escaped (`rock &amp; roll`).
- Suggestions could come from disabled dictionaries when all dictionaries were disabled; the filter now runs in SQL.
- The PyInstaller spec excluded QtMultimedia; it is bundled now so sounds play in the portable builds.

### Changed
- **CI triggers on tag pushes only** (plus pull requests and manual runs), so a release is exactly **one** run. A plain push to `main` starts nothing.
- The Rust CI jobs install the ALSA headers, run the parity and library-only tests and the selftest, and publish with `cargo publish --locked`.
- `.gitattributes` marks dictionary samples as binary; `dictz-rs/target/` is ignored.

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
