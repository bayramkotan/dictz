# dictz — TODO

Status legend: ⏳ in flight · 📋 planned · 💡 idea · ✅ done

---

## Near-term (v0.0.x)

### v0.0.6 — fuzzy + rich media
- 📋 Fuzzy / approximate matching with RapidFuzz on no-exact-hit
- 📋 Highlighted suggestion ranking (closest first)
- 📋 Inline pronunciation audio playback from DSL `[s]sound.wav[/s]`
- 📋 Embedded images from DSL `[s]picture.jpg[/s]`

### v0.0.7 — services
- 📋 TTS with edge-tts (40+ languages, neural voices)
- 📋 Per-dictionary / per-language voice mapping
- 📋 Clipboard monitor (Ctrl-C triggers lookup) with opt-in toggle
- 📋 Global hotkey (Ctrl+Alt+K)

### v0.0.7 — Anki + OCR
- 📋 Anki integration (AnkiConnect HTTP, add selected headword + definition)
- 📋 Screen-area OCR (mss + Tesseract)
- 📋 Per-deck + per-model mapping in preferences

### v0.0.8 — formats
- 📋 XDXF parser (PyGlossary already does)
- 📋 Slob / Aard2 (PyGlossary)
- 📋 DictD `.index` / `.dict`
- 💡 MDict (.mdx / .mdd)

### v0.0.9 — groups, search, UX
- 📋 Dictionary groups (per-language presets, like GoldenDict groups)
- 📋 Full-text search inside definitions (not just headwords)
- 📋 Multi-word phrase lookup
- 📋 Morphology / stemming (Hunspell) for lemma matching
- 📋 Per-dictionary priority weighting

---

## Rust edition (`dictz-rs/`), built alongside the Python edition

dictz is built **twice**, in Python (`dictz`, PyPI) and in Rust (`dictz-rs/`, crates.io). The Rust side is not a library-only side project: it becomes the same full application (CLI + GUI), with the same features, version numbers and release. Every version moves the Rust track forward too.

- ✅ v0.0.5: crate scaffold (`Dictionary` trait, `DictionaryEntry`, `DictionaryMetadata`, `DictzError`), version kept in lockstep with Python, published to crates.io from CI
- 📋 v0.0.6: native StarDict parser (`.ifo` / `.idx` / `.dict` / `.dict.dz`) + `dictz-rs` CLI binary (`lookup`, `--version`, `--selftest`)
- 📋 v0.0.7: SQLite FTS5 index (rusqlite, bundled) + fuzzy matching, the same schema as Python
- 📋 v0.0.8: native DSL parser (`.dsl` / `.dsl.dz`, DSL markup → HTML)
- 📋 v0.0.9: native Babylon `.bgl` parser
- 📋 v0.1.0: decide the GUI toolkit and ship a GUI MVP (search box, results, definition view, dictionary manager); portable Rust binaries for Linux / Windows / macOS in the same GitHub Release
- 📋 v1.0: Rust edition has full feature parity with the Python edition (tabs, favorites, history, themes, TTS, clipboard, Anki, OCR)
- Rule: the same CI file (`ci.yml`), the same tag, the same selftest checks for both editions

---

## v0.1.0 — first polished release
- 📋 Preferences dialog gets panels (General / Search / Appearance / Shortcuts / Advanced)
- 📋 Shortcut editor
- 📋 Export / import settings
- 📋 Diagnostics panel (what's indexed, FTS5 stats)
- 📋 First-run tour
- 📋 Code signing on Windows (Authenticode)
- 📋 macOS notarization
- 📋 Intel macOS binary (universal2 build on Apple Silicon runner)

---

## Post-v1.0 ideas
- 💡 Mobile companion (Android via Chaquopy / Kivy; iOS)
- 💡 Browser extension (select → send to dictz)
- 💡 WebView-based "card mode" (Anki-style flashcard review in the app)
- 💡 arm64 Linux builds (Raspberry Pi)

---

## Known bugs / limitations

- `.syn` synonym files in StarDict are ignored
- `sametypesequence` in StarDict `.ifo` is not interpreted
- UTF-8 assumed for `.ifo` headers (no latin-1 fallback)
- Large `.bgl` files load eagerly into memory
- QWebEngine missing on some PySide6 slim installs → falls back to QTextBrowser
- Windows SmartScreen warning on first run (unsigned)
- macOS Gatekeeper warning on first run (unnotarized)

---

## Recently done

- ✅ **v0.0.5** — portable binaries actually start and read DSL/Babylon (PyInstaller entry point + PyGlossary bundle paths fixed); `dictz --selftest` run on every binary in CI; one tag → PyPI + crates.io + GitHub Releases via API-token secrets; `dictz-rs/` Rust crate scaffold
- ✅ **v0.0.4** — cross-platform portable builds (Linux tarball + AppImage, Windows portable zip, macOS .app); PyInstaller spec; GitHub Actions matrix release pipeline
- ✅ **v0.0.3** — tabbed UI, sidebar, themes, zoom, window state, QWebEngine renderer, keyboard shortcuts, dictionary manager dialog
- ✅ **v0.0.2** — DSL + Babylon parsers via PyGlossary
- ✅ **v0.0.1** — StarDict native parser, SQLite FTS5 index, 3-pane UI
