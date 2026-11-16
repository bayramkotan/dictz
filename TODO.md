# dictz — TODO

Status legend: ⏳ in flight · 📋 planned · 💡 idea · ✅ done

---

## Goal: more than GoldenDict

dictz must do everything GoldenDict / GoldenDict-ng does, in **both editions**, plus the things GoldenDict does not have. One version = one theme; we review it together before the next one.

### Feature matrix (GoldenDict-ng vs dictz)

| Area | GoldenDict-ng | dictz now | Planned |
|---|---|---|---|
| StarDict, DSL, Babylon | ✓ | ✓ | `.syn`, `sametypesequence`, DSL abbreviations, `.spx` sound — v0.0.12 |
| MDict (.mdx/.mdd), XDXF, DictD, Slob | ✓ | — | v0.0.12 |
| ZIM, Lingoes LSA, SDict, Aard, zipped sound packs, sound folders, Epwing, GLS | ✓ | — | v0.0.13 |
| Dictionaries > 4 GB, millions of headwords | ✓ | partial (BGL loads into memory) | v0.0.13 |
| Scan popup (selection / clipboard / Ctrl+C+C), tray icon, global hotkeys | ✓ | ✓ (v0.0.8) | modifier-key-only scan, popup groups — v0.0.10 |
| Dictionary groups (switch, hotkeys, per-group order, group tabs) | ✓ | — | v0.0.10 |
| Custom dictionary names, dictionary metadata file | ✓ | — | v0.0.10 |
| Ignore case / diacritics / punctuation / whitespace | ✓ | case + diacritics | v0.0.11 |
| Hunspell morphology, stemming, spelling suggestions | ✓ | — (fuzzy instead) | v0.0.11 |
| Transliteration (Cyrillic, Greek, Romaji, Chinese conversion…) | ✓ | — | v0.0.11 |
| Wildcard / regex headword search | ✓ | prefix only | v0.0.11 |
| Full-text search inside definitions | ✓ (Xapian) | — | v0.0.14 |
| Wikipedia / Wiktionary (MediaWiki), websites, DICT servers, Forvo, external programs | ✓ | — | v0.0.15 |
| Voice engines (TTS) | ✓ | — | v0.0.16 |
| Anki | ✓ | — | v0.0.17 |
| Favorites folders, history / favorites export | ✓ | flat favorites, export | v0.0.17 |
| Article CSS (global + per dictionary), print, headwords dialog + export | ✓ | — | v0.0.18 |
| Shortcut editor, interface languages, portable mode | partial | listed only | v0.0.18 |
| Fuzzy "did you mean" with scores | — | ✓ | ➕ dictz only |
| Full command line (lookup, dump, config, doctor, update…), short name `dct` | a few switches | ✓ | ➕ dictz only |
| Desktop / menu shortcut from the app or `dct shortcut` | installer only | ✓ (v0.0.9) | ➕ dictz only |
| npm package (`npx dictz`, `npm i -g dictz` → `dictz` / `dct`) | — | — | 💡 later, separate job |
| Self-update (`dictz update`) | — | ✓ | ➕ dictz only |
| Find in page with counter, dictionary bar, found-in panel | ✓ | ✓ | — |
| Searchable 75-setting preferences, export / import | — | ✓ | ➕ dictz only |
| Scan popup driven from the CLI (`dictz popup`, `watch`, `show`) across both editions | — | ✓ | ➕ dictz only |
| Same app in Python and Rust, shared data files | — | ✓ | ➕ dictz only |

### v0.0.8 — scan popup (GoldenDict's main feature)
- 📋 Popup window that looks up the word under the selection (X11 primary selection, Windows / macOS via clipboard)
- 📋 Triggers, each switchable: Ctrl+C+C, clipboard change, selection change, modifier key (Ctrl / Alt / Shift) held while selecting
- 📋 Global hotkeys: show/hide main window, toggle scan, look up clipboard
- 📋 Popup: pin, auto-hide on mouse leave, own size and zoom, "open in main window", dictionary bar for the active group
- 📋 System tray icon (scan on/off, quit), start minimized to tray, close to tray
- 📋 Wayland: works through the clipboard / hotkey; selection scan only where the compositor allows it (stated in the settings page)
- 📋 CLI: `dictz popup WORD`, `dictz scan on|off` talking to the running instance (single-instance socket)

### v0.0.9 — CLI short name, desktop shortcut ✅
- ✅ `dct` = `dictz` in both editions (pip/pipx console script, second cargo `[[bin]]`, portable archives)
- ✅ every version spelling: `-v -V --version --Version --VERSION -version -Version -VERSION`, `dct version`
- ✅ `dct shortcut [--remove] [--desktop-only] [--menu-only]` + Dosya → Masaüstü kısayolu oluştur (Linux .desktop, Windows .lnk, macOS .app bundle)
- ✅ completion scripts register both names
- ✅ READMEs rewritten with per-platform install (Linux / Windows / macOS × portable / pipx / cargo)

### v0.0.10 — dictionary groups
- 📋 Groups with name, icon, order and their own dictionary list; "All" group
- 📋 Group selector in the toolbar, popup and CLI (`dictz lookup -g NAME`, `dictz groups`)
- 📋 Hotkey per group; a tab remembers its group
- 📋 Automatic groups by language pair (En→Tr, Tr→En…)
- 📋 Custom display name per dictionary; `dictz.toml` metadata next to a dictionary (name, language, disable full-text)

### v0.0.11 — smarter matching
- 📋 Ignore punctuation and whitespace (case and diacritics already work); each one a setting
- 📋 Hunspell morphology: lemma lookup ("went" → "go", "kitaplarımızdan" → "kitap"), spelling suggestions; dictionaries downloadable from the app
- 📋 Turkish stemming (Zemberek-style suffix stripping) as a dictz extra
- 📋 Wildcard (`*`, `?`) and regex headword search, also in the CLI
- 📋 Transliteration: Cyrillic ↔ Latin, Greek, Romaji, Ottoman Latin, Chinese simplified ↔ traditional
- 📋 Multi-word phrases: look up the whole phrase, then each word

### v0.0.12 — formats I
- 📋 MDict `.mdx` / `.mdd` (with CSS, images, sounds) — the most requested format
- 📋 XDXF, DictD (`.index` + `.dict.dz`), Slob / Aard 2
- 📋 StarDict `.syn` synonyms and `sametypesequence`; latin-1 fallback for `.ifo`
- 📋 DSL: abbreviation files (`_abrv.dsl`), `.spx` sounds, case-insensitive media extensions

### v0.0.13 — formats II and large dictionaries
- 📋 ZIM (Kiwix: offline Wikipedia), Lingoes LSA sound packs, SDict, Aard, GLS
- 📋 Sound folders and zipped sound packs as pronunciation dictionaries
- 💡 Epwing
- 📋 Lazy loading for big files (BGL, MDX) and dictionaries > 4 GB; millions of headwords without memory blow-up

### v0.0.14 — full-text search
- 📋 Search inside definitions (SQLite FTS5 content index per dictionary, built in the background, can be turned off per dictionary)
- 📋 Modes: words, phrase, wildcard, regex, "near"; results list with context snippets
- 📋 Full-text search window and `dictz search` in the CLI

### v0.0.15 — online sources
- 📋 Wikipedia / Wiktionary (MediaWiki API) per language
- 📋 Any website as a dictionary (URL template with `%GDWORD%`-style placeholder), shown in a tab or opened in the browser
- 📋 DICT protocol servers, Forvo pronunciations (API key), external programs as dictionaries
- 📋 Proxy settings and a "no network" switch; everything online is off by default

### v0.0.16 — pronunciation (TTS)
- 📋 System voices (SAPI / NSSpeech / speech-dispatcher) and edge-tts neural voices
- 📋 Voice per language and per dictionary; "pronounce" button and hotkey; auto-pronounce

### v0.0.17 — learning
- 📋 Anki via AnkiConnect (word + selected article + sound), deck / model / field mapping
- 📋 Favorites folders, drag and drop, import / export (txt, csv, json); history export
- 📋 Built-in flashcards with spaced repetition from favorites (dictz extra)
- 📋 Word of the day (dictz extra)

### v0.0.18 — customization
- 📋 Article CSS: user stylesheet, per-dictionary stylesheet, ready styles (Modern, Lingvo, Compact)
- 📋 Print article, headwords window with filter and export
- 📋 Shortcut editor
- 📋 Interface languages (Türkçe, English first)
- 📋 Portable mode (settings and indexes next to the program)

### Beyond GoldenDict (dictz extras, not scheduled yet)
- 💡 Screen-area OCR lookup (works where selection does not: images, PDFs, games)
- 💡 `dictz serve`: local HTTP / JSON API so other tools (and the browser extension) can query dictz
- 💡 Browser extension (double-click a word in the browser → dictz popup)
- 💡 Settings and favorites sync through a folder (Yandex.Disk, Syncthing, Dropbox)
- 💡 Dictionary downloader (free StarDict / Wiktionary dumps, Hunspell files)
- 💡 Mobile companion

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
- ✅ v0.0.9: `dct`, version flags, `dictz shortcut` — in both editions
- ✅ v0.0.8: scan popup, tray icon, global hotkeys, single instance, `dictz show / popup / watch` — in both editions
- ✅ v0.0.7: CLI (same commands and output), `dictz update`, 54-setting shared `settings.json`, detailed preferences window, dictionary bar, found-in panel, find in page, background loading — in both editions
- 📋 v0.0.8+: every feature above lands in both editions in the same version
- 💡 Rust: right-to-left shaping for Arabic/Persian articles (egui draws RTL text left-to-right)
- 💡 Rust: AppImage for the Rust binary too (the tar.gz is a single file already)

---

## v0.1.0 — first polished release
- ✅ Preferences dialog with pages, search, reset, Shortcuts and Advanced pages (v0.0.7)
- ✅ Export / import settings (v0.0.7, also `dictz config export/import`)
- ✅ Diagnostics: `dictz doctor` (v0.0.7)
- 📋 First-run tour
- 📋 Code signing on Windows (Authenticode)
- 📋 macOS notarization
- 📋 Intel macOS binary (universal2 build on Apple Silicon runner)

---

## Post-v1.0 ideas
- 💡 arm64 Linux builds (Raspberry Pi)

---

## Known bugs / limitations

- DSL `[s]` media: only `wav`/`mp3` sounds and `jpg/jpeg/gif/tif/tiff/png/bmp` pictures, with case-sensitive extensions (PyGlossary's rule, kept for parity); Lingvo `.spx` sounds are not played
- Rust GUI: Arabic/Persian text is not shaped right-to-left
- Rust GUI: a font given by name is matched against font file names (no fontconfig lookup); a file path always works
- Scan popup on Wayland: global hotkeys and "near the mouse" are not possible (Wayland does not allow them); the Rust edition minimizes instead of hiding to the tray there
- Rust edition on Windows: "hidden to the tray" moves the window off screen without a taskbar button (a hidden egui window stops drawing, and the popup is drawn from it)
- `dictz update` on a Python portable zip / tar.gz build only downloads the new archive (a running PyInstaller folder cannot replace itself); the AppImage is replaced in place

- `.syn` synonym files in StarDict are ignored (v0.0.12)
- `sametypesequence` in StarDict `.ifo` is not interpreted (v0.0.12)
- UTF-8 assumed for `.ifo` headers, no latin-1 fallback (v0.0.12)
- Large `.bgl` files load eagerly into memory (v0.0.13)
- QWebEngine missing on some PySide6 slim installs → falls back to QTextBrowser
- Windows SmartScreen warning on first run (unsigned)
- macOS Gatekeeper warning on first run (unnotarized)

---

## Recently done

- ✅ **v0.0.9** — `dct` short name, every `--version` spelling, `dct shortcut` + Dosya → Masaüstü kısayolu oluştur, completion for both names, per-platform READMEs — in both editions
- ✅ **v0.0.8** — scan popup (Ctrl+C+C, every copy, selection), tray icon, global hotkeys, single instance, `dictz show / popup / watch`, 21 new settings (75 in 9 pages) — in both editions
- ✅ **v0.0.7** — Python 3.14 Babylon fix + no more start-up hang (background loading); full CLI incl. `dictz update` in both editions; 54 shared settings with a detailed Preferences window; dictionary bar, found-in panel, collapsible articles, find in page, save/copy article, random word, full screen, Git/Sözlükler menus
- ✅ **v0.0.6** — Rust edition is a full app (native StarDict/DSL/Babylon, FTS5, GUI, portable binaries); fuzzy "did you mean" + DSL pictures/sounds in both editions; dictionaries restored on start; order kept; link clicks fixed; parity golden tests; CI triggers on tags only
- ✅ **v0.0.5** — portable binaries actually start and read DSL/Babylon (PyInstaller entry point + PyGlossary bundle paths fixed); `dictz --selftest` run on every binary in CI; one tag → PyPI + crates.io + GitHub Releases via API-token secrets; `dictz-rs/` Rust crate scaffold
- ✅ **v0.0.4** — cross-platform portable builds (Linux tarball + AppImage, Windows portable zip, macOS .app); PyInstaller spec; GitHub Actions matrix release pipeline
- ✅ **v0.0.3** — tabbed UI, sidebar, themes, zoom, window state, QWebEngine renderer, keyboard shortcuts, dictionary manager dialog
- ✅ **v0.0.2** — DSL + Babylon parsers via PyGlossary
- ✅ **v0.0.1** — StarDict native parser, SQLite FTS5 index, 3-pane UI
