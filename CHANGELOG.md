# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] — 2026-11-13

### Added
- **DSL (ABBYY Lingvo) format support** via PyGlossary — reads `.dsl` and `.dsl.dz` files
- **Babylon format support** via PyGlossary — reads `.bgl` binary dictionaries
- `PyGlossaryDictionary` generic base parser (shared code between PyGlossary-backed formats)
- Parser-registry dispatch handles `.dz`-compressed variants automatically
- `supported_extensions()` helper for UI file-dialog filters
- Test suite for the DSL parser and parser dispatcher
- `CHANGELOG.md` and `LICENSE` added to the repo

### Changed
- `pyglossary` moved from optional to required dependency
- File-open dialog filter now lists StarDict, DSL and Babylon separately
- About-box version bumped to 0.0.2 and format list expanded
- README rewritten for multi-platform publication (PyPI, GitHub) with badges and install instructions
- PyPI classifiers expanded: Windows / Linux / macOS, Turkish and English natural languages

## [0.0.1] — 2026-11-12

### Added
- Project bootstrap (4-layer architecture: UI, core, parsers, storage)
- Native StarDict parser (no external dependency) — reads `.ifo`, `.idx`, `.dict` and `.dict.dz`
- SQLite FTS5 central index with `unicode61 remove_diacritics 2` tokenizer
- `QueryManager` orchestrates lookups across multiple loaded dictionaries
- PySide6 three-pane UI: search box, suggestion list, definition view
- Debounced incremental search (150 ms)
- Cross-platform per-user data directory (Windows / macOS / Linux)
- Test suite for the StarDict parser and index store (11 tests)
