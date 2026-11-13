# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.3] — 2026-11-13

### Added
- **Tabbed definition view** — each lookup opens in its own tab, Ctrl+T / Ctrl+W / Ctrl+Tab
- **QWebEngineView-based renderer** — rich HTML rendering of definitions, with QTextBrowser fallback when QtWebEngine is unavailable
- **Dockable sidebar** with three panes (Ctrl+B to toggle):
  - Sözlükler: every loaded dictionary with a checkbox to enable/disable
  - Favoriler: starred words, newest first
  - Geçmiş: every lookup, newest first, with one-click clear
- **Back / Forward navigation** — Alt+← / Alt+→, browser-style
- **Favorites + history** — persistent in SQLite (`dictz_history.db`), survives restarts
- **Dictionary manager dialog** — add / remove / reorder / enable / disable in one place (Ctrl+M)
- **Light + dark themes** — `auto` picks by system palette, click the toolbar icon to cycle
- **Preferences dialog** — theme, font family, font size, search debounce (Ctrl+,)
- **Zoom** — Ctrl+= / Ctrl+- / Ctrl+0, shown in the status bar
- **Toolbar with search box** — main interaction surface; text + shortcut labels on every action
- **Keyboard shortcuts** — Ctrl+F (focus), Ctrl+Enter (new tab), Ctrl+D (favorite), Ctrl+M, Ctrl+B, Ctrl+Shift+C
- **Window state persistence** — geometry, dock layout, zoom, last search restored on relaunch
- `dictz/storage/history.py` — SQLite-backed `HistoryStore` with favorites + notes + history
- `dictz/storage/settings.py` — typed `Preferences` dataclass over QSettings
- `dictz/ui/themes.py` — hand-tuned light + dark QSS, definition-pane CSS
- `dictz/ui/definition_view.py` — single-pane HTML renderer
- `dictz/ui/definition_tabs.py` — tab bar + close/move/zoom
- `dictz/ui/sidebar.py` — the dock widget
- `dictz/ui/dictionary_manager.py` — manager dialog
- `dictz/ui/preferences_dialog.py` — settings dialog
- `dictz/ui/icons.py` — Qt standard-icon wrapper
- `tests/test_history.py` — 5 tests for the history/favorites store
- `TODO.md` — roadmap and open items

### Changed
- **Main window completely rewritten** — three-pane dock layout instead of the old splitter
- `IndexStore` schema gets a `sort_order` column; existing databases are migrated in place with `ALTER TABLE IF NOT EXISTS`-style fallback
- `QueryManager` now tracks which dictionaries are enabled; `search()` and `suggest()` honor that set
- About-box, welcome screen, window title all moved to v0.0.3
- README expanded with keyboard-shortcut tables and UI feature list
- `pyproject.toml` pins `PySide6-Addons` for `QtWebEngine` availability

### Fixed
- Welcome text now visible when no dictionary is loaded (previously only when a dictionary was loaded)
- Closing the last tab resets it instead of leaving an empty tab bar

## [0.0.2] — 2026-11-13

### Added
- DSL (ABBYY Lingvo) format support via PyGlossary — `.dsl` and `.dsl.dz`
- Babylon (.bgl) format support via PyGlossary
- `PyGlossaryDictionary` generic base parser
- Parser-registry dispatch handles `.dz`-compressed variants automatically
- `supported_extensions()` helper
- `CHANGELOG.md` and `LICENSE` added
- Tests for DSL parser and parser dispatcher

### Changed
- `pyglossary` moved from optional to required dependency
- File-open dialog lists StarDict, DSL and Babylon separately
- README rewritten for multi-platform publication (PyPI, GitHub) with badges

## [0.0.1] — 2026-11-12

### Added
- Project bootstrap (4-layer architecture: UI, core, parsers, storage)
- Native StarDict parser — reads `.ifo`, `.idx`, `.dict` and `.dict.dz`
- SQLite FTS5 central index with `unicode61 remove_diacritics 2`
- `QueryManager` orchestrates multi-dictionary lookups
- PySide6 three-pane UI, debounced incremental search
- Cross-platform per-user data directory
- Test suite (11 tests)
