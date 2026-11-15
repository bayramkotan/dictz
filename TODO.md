# dictz — TODO

Status legend: ⏳ in flight · 📋 planned · 💡 idea · ✅ done

---

## Near-term (v0.0.x)

### v0.0.8 — services
- 📋 TTS with edge-tts (40+ languages, neural voices)
- 📋 Per-dictionary / per-language voice mapping
- 📋 Clipboard monitor (Ctrl-C triggers lookup) with opt-in toggle
- 📋 Global hotkey (Ctrl+Alt+K)

### v0.0.9 — Anki + OCR
- 📋 Anki integration (AnkiConnect HTTP, add selected headword + definition)
- 📋 Screen-area OCR (mss + Tesseract)
- 📋 Per-deck + per-model mapping in preferences

### v0.0.10 — formats
- 📋 XDXF parser (PyGlossary already does)
- 📋 Slob / Aard2 (PyGlossary)
- 📋 DictD `.index` / `.dict`
- 💡 MDict (.mdx / .mdd)

### v0.0.11 — groups, search, UX
- 📋 Dictionary groups (per-language presets, like GoldenDict groups)
- 📋 Full-text search inside definitions (not just headwords)
- 📋 Multi-word phrase lookup
- 📋 Morphology / stemming (Hunspell) for lemma matching
- 📋 Per-dictionary priority weighting

---

## Rust edition (`dictz-rs/`), built alongside the Python edition

dictz is built **twice**: in Python (`dictz` on PyPI, Qt) and in Rust (`dictz-rs/`, `dictz` on crates.io, egui). Both are full applications with the same features, version number, tag, CI file and selftest checks. Every new feature is done in **both** editions in the same version.

- ✅ v0.0.5: crate scaffold, version lockstep, crates.io publishing from CI
- ✅ v0.0.6: **parity reached**:
  - native StarDict, DSL and Babylon parsers
  - FTS5 index, history and favorites in shared files
  - full egui GUI
  - fuzzy search and DSL media (the v0.0.6 features, in both editions)
  - portable Rust binaries and golden-file parity tests
- ✅ v0.0.7: CLI (same commands and output), `dictz update`, 54-setting shared `settings.json`, detailed preferences window, dictionary bar, found-in panel, find in page, background loading — in both editions
- 📋 v0.0.8+: every feature below lands in both editions in the same version
- 💡 Rust: right-to-left shaping for Arabic/Persian articles (egui draws RTL text left-to-right)
- 💡 Rust: AppImage for the Rust binary too (the tar.gz is a single file already)

---

## v0.1.0 — first polished release
- ✅ Preferences dialog with pages, search, reset, Shortcuts and Advanced pages (v0.0.7)
- ✅ Export / import settings (v0.0.7, also `dictz config export/import`)
- ✅ Diagnostics: `dictz doctor` (v0.0.7)
- 📋 Shortcut editor (shortcuts are listed, not yet editable)
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

- DSL `[s]` media: only `wav`/`mp3` sounds and `jpg/jpeg/gif/tif/tiff/png/bmp` pictures, with case-sensitive extensions (PyGlossary's rule, kept for parity); Lingvo `.spx` sounds are not played
- Rust GUI: Arabic/Persian text is not shaped right-to-left
- Rust GUI: a font given by name is matched against font file names (no fontconfig lookup); a file path always works
- `dictz update` on a Python portable zip / tar.gz build only downloads the new archive (a running PyInstaller folder cannot replace itself); the AppImage is replaced in place

- `.syn` synonym files in StarDict are ignored
- `sametypesequence` in StarDict `.ifo` is not interpreted
- UTF-8 assumed for `.ifo` headers (no latin-1 fallback)
- Large `.bgl` files load eagerly into memory
- QWebEngine missing on some PySide6 slim installs → falls back to QTextBrowser
- Windows SmartScreen warning on first run (unsigned)
- macOS Gatekeeper warning on first run (unnotarized)

---

## Recently done

- ✅ **v0.0.7** — Python 3.14 Babylon fix + no more start-up hang (background loading); full CLI incl. `dictz update` in both editions; 54 shared settings with a detailed Preferences window; dictionary bar, found-in panel, collapsible articles, find in page, save/copy article, random word, full screen, Git/Sözlükler menus
- ✅ **v0.0.6** — Rust edition is a full app (native StarDict/DSL/Babylon, FTS5, GUI, portable binaries); fuzzy "did you mean" + DSL pictures/sounds in both editions; dictionaries restored on start; order kept; link clicks fixed; parity golden tests; CI triggers on tags only
- ✅ **v0.0.5** — portable binaries actually start and read DSL/Babylon (PyInstaller entry point + PyGlossary bundle paths fixed); `dictz --selftest` run on every binary in CI; one tag → PyPI + crates.io + GitHub Releases via API-token secrets; `dictz-rs/` Rust crate scaffold
- ✅ **v0.0.4** — cross-platform portable builds (Linux tarball + AppImage, Windows portable zip, macOS .app); PyInstaller spec; GitHub Actions matrix release pipeline
- ✅ **v0.0.3** — tabbed UI, sidebar, themes, zoom, window state, QWebEngine renderer, keyboard shortcuts, dictionary manager dialog
- ✅ **v0.0.2** — DSL + Babylon parsers via PyGlossary
- ✅ **v0.0.1** — StarDict native parser, SQLite FTS5 index, 3-pane UI
