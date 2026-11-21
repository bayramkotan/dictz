# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Adding dictionaries froze the window** (both editions): "Sözlük ekle", the dictionary manager's add, dropped files and "Reindex" read every file in the window's own thread — with big dictionaries the window stood still for minutes with no progress shown. Now all of it runs in the background:
  - new or changed dictionaries are read by **several workers at once** — separate processes in the Python edition (no waiting on Python's GIL), threads in the Rust edition; one coordinating thread writes `dictionaries.json` and the headword index;
  - folders are listed in the background too (adding a big folder tree, the scan at start, "Rescan");
  - work added while a load runs joins it instead of starting a second one;
  - "Reindex" (one dictionary or all) runs in the background; each dictionary keeps its place in the list.
  - Measured with 9 large dictionaries (1.5 million headwords): before, the window was frozen for the whole 36 s; now the longest pause is 0.4 s, and with 2 workers it is done in 21 s.
- **Progress in the status bar**: "Loading dictionaries (3/9): a.dsl, b.bgl", a bar with the share done (the dictionaries being read count with their part), a tooltip with each running dictionary's share, and a **stop** button (adds no more new dictionaries; the ones being read finish).

### Added
- Setting **Dictionaries prepared at once** (`dictionaries.build_workers`, Preferences → Dictionaries, 80 settings): 0 = automatic (processors − 1, at most 4).

## [0.1.4] — 2026-11-21

A compact window like GoldenDict, with dictz's own icons and an interface scale — in **both editions**.

### Changed
- **A new, compact window like GoldenDict** (both editions).
  - The **search box and the word list** are in the left pane (GoldenDict's search pane); Dictionaries, Favourites and History are icon tabs next to it. An empty search box lists the last searched words. Ctrl+F always brings the search pane back.
  - **One toolbar row**: back / forward, the **dictionary bar as small icon toggles** (click: on/off, right click: only this one), then random word, favourite, zoom out / normal / in, theme, scan popup, preferences and the language flag.
  - **dictz's own line icons** (`dictz/resources/icons/*.svg`, drawn in the theme's colour; the Rust edition uses the same pictures), instead of the system's mixed icons.
  - **Articles without cards**: a thin strip per dictionary (icon, name, language pair; copy and "search only here" appear on hover), the text right under it, small paddings. The tab row is hidden while there is only one tab.
  - Smaller paddings, square corners and thin lines everywhere; the same colours in both editions. The status bar shows the last search on the left and the dictionary count and zoom on the right.
  - Smaller default sizes: interface font 13, article font 15, line spacing 1.45, headword 1.2×, side pane 230 px. Settings that were still at the old defaults move to the new ones once; values you chose yourself are kept.
- **Interface scale** (Preferences → Appearance, `appearance.ui_scale`, 50–250 %, 79 settings): makes the whole window larger or smaller on high-resolution screens; on top of the system's own scale. The Python edition applies it after a restart and keeps fractional system scales (125 %, 150 %) sharp.
- README screenshots retaken with the new window.

## [0.1.3] — 2026-11-17

A start reads only the JSON files, and dictz can start with the system in the tray — in **both editions**.

### Changed
- **Nothing is re-scanned or re-read at start.** Every dictionary of the list opens from `dictionaries.json` alone: the list keeps each dictionary's languages, counts, author and a stamp of its files (size + modification time). When the stamp still matches, neither the dictionary file nor its article store is opened until the first lookup. Only when a file really changed is it read again — and the list is updated at once.
  - Dictionary icons are looked for once and kept in `icons/` in the data folder (no folder listing and no Babylon file read at every start).
  - The dictionary folders are not listed again when they have not changed: `scan_cache.json` keeps each folder's listing with its time; adding, removing or renaming a file in a folder is noticed.
  - Adding, removing, moving, switching off or reindexing a dictionary updates the JSON files at once.
  - A folder that changed less than 2 seconds before it was listed is listed again at the next scan: within one file-system clock tick its time may not move when a file is added, and the new file would be missed.

### Added
- **Start with the system** (Preferences → General, `general.autostart`, 78 settings): dictz starts by itself when you log in — Windows (Run key), Linux (`~/.config/autostart`), macOS (LaunchAgent) — hidden in the system tray; the tray icon is always shown then. If dictz is already running, the log-in start does nothing.

## [0.1.2] — 2026-11-17

Fast start, dictionaries that no longer get lost, and dictionary folders like GoldenDict — in **both editions**.

### Fixed
- **Dictionaries no longer get lost.** Several windows, the background loader, the CLI and the other edition could change the list at the same moment; one change then wiped out the other. Every change now takes turns through `dictionaries.json.lock` and reads the file again first. The Rust edition no longer writes two changes into the same temporary file. The previous list is kept as `dictionaries.json.bak` and is used if the file is ever broken.
- Windows: the selftest no longer fails because an article store is still open when its temporary folder is deleted; "Reindex" closes a dictionary's store before replacing it (Windows cannot replace an open file).
- **A dictionary that cannot be opened no longer disappears** from the window: the side panel and the dictionary manager show it as **⚠ Name — could not be opened**, with the reason on hover. It keeps its place and can be switched off or removed.

### Added
- **Dictionary folders, like GoldenDict's Sources → Files.** Preferences → Dictionaries and the dictionary manager show the list of folders; each folder has its own **Subfolders** box, plus **Add folder…**, **Remove** and **Scan now**. The folders are kept in `settings.json` and scanned at every start, so new dictionaries put into them appear by themselves.
  - Every **Add folder…** (menu, dictionary manager, a folder dropped on the window) now keeps the folder in that list and adds its dictionaries in the background, without asking.
  - New setting `dictionaries.flat_folders` (77 settings): the folders scanned without subfolders. `dictionaries.recursive` is the default for a new folder.
  - Removing a folder does not remove its dictionaries.

### Changed
- **Fast start, like GoldenDict** (both editions). A dictionary is now parsed **once**, when it is added or its file changes; its articles go into an article store `articles/<id>.db` in the data folder. Every later start opens those stores and reads nothing else; an article is read (and unpacked) only when it is shown.
  - Measured with 500,000 headwords (StarDict 300k + DSL 100k + Babylon 100k): Python start 7.2 s → 0.07 s, Rust 1.0 s → 0.01 s; the Python window has the dictionaries 0.03 s after it opens.
  - The store is rebuilt by itself when a dictionary's files change (size or modification time); "Reindex" (window, dictionary manager, `dictz reindex`) now reads the file again too.
  - Lookups give exactly the parsers' answers (exact word, then the first case-insensitive match, then up to 50 prefix matches).
  - A store written by one edition is read by the other; removing a dictionary deletes its store.
  - The first start after the update parses every dictionary one last time to build the stores.
- Building the headword index reads only the headwords, not the articles.

## [0.1.1] — 2026-11-16

The dictionary list moves to **`dictionaries.json`**, in **both editions**: dictionaries are no longer mixed up or lost.

### Fixed
- **Dictionaries no longer get mixed up or lost** (both editions). The dictionary list — order, on/off and names — now lives in **`dictionaries.json`** in the data folder, written at once after every change and shared by both editions and the CLI.
  - A dictionary is recognised by its **file path**, not its name. Two different files with the same name both stay in the list; the second one is shown as **`Name (2)`** (then `(3)`…), everywhere: list, article headers, search results.
  - A dictionary whose file is missing or fails to load keeps its place in the list; it is no longer moved or dropped when the others are saved.
  - New dictionaries are added at the end and never take another one's place.
  - The old list in `dictz_index.db` is moved to `dictionaries.json` once, on the first start (order and on/off kept, duplicate files dropped). The headword index stays in `dictz_index.db`.
- New parity golden `registry_golden.json`: both editions write `dictionaries.json` byte for byte the same.

## [0.1.0] — 2026-11-16

The **interface in 25 languages**, **dictionary icons and article cards**, and READMEs with screenshots — in **both editions** (Python and Rust). After 0.0.9 the version moves to 0.1.0, not 0.0.10.

### Added
- **25 interface languages** in **both editions**, chosen from the flag at the top right of the window (or Tercihler → Genel → Arayüz dili, `general.language`; `auto` follows the system language):
  - Türkçe, English, Azərbaycan türkcəsi, Deutsch, Français, Español, Italiano, Português, Nederlands, Polski
  - Русский, Українська, العربية, فارسی, اردو, Қазақша, Кыргызча, Oʻzbekcha, Türkmençe
  - 中文, 日本語, 한국어, Bahasa Indonesia, Bahasa Melayu, हिन्दी
  - Every menu, button, message, settings label and help text is translated; Arabic, Persian and Urdu are laid out right to left (Python) or shaped and reordered (Rust).
  - The catalogues are shared JSON files; a test fails if a text is missing in any language.
- **Dictionary icons and article cards** in **both editions**. Every place that names a dictionary shows the same badge: article headers, the dictionary bar, "found in" and the sidebar list.
  - The badge is the dictionary's own icon when it has one: a picture next to it with the same name (`Oxford.ifo` → `Oxford.ico` / `.png` / `.bmp` / `.jpg` / `.gif`, as in GoldenDict), or the icon stored inside a Babylon `.bgl` file.
  - Otherwise it is a generated badge: the dictionary's monogram (`Adil Hindistan En2Tr` → **AH**) on its own colour, which stays the same everywhere and between runs.
  - Each article is a card. Its header carries the badge, the name, the language pair (`EN → TR`, from the dictionary's metadata or its name: `En2Tr`, `English-Turkish`, `İngilizce → Türkçe`), the format (`StarDict` / `DSL` / `BGL`), the article count, copy / "only this one" and a collapse arrow, on a strip in the dictionary's colour.
  - Tooltips show name, languages, format and word count.
- `dictz/core/dictident.py` / `dictz-rs/src/dictident.rs` with a new parity golden (`dictident_golden.json`); the parity BGL sample now carries an icon.
- README and `dictz-rs/README.md` show screenshots: main window (light and dark), language picker, preferences, scan popup, Arabic interface, Turkish interface. `docs/screenshots/demo_dicts.py` rebuilds the invented demo dictionaries they use.

### Changed
- New setting `general.language` (76 settings).
- The Rust toolbar leaves room for longer translated labels; the language button is always shown.
- Roadmap renumbered: dictionary groups are v0.1.1, and later versions follow (x.y.9 is followed by x.(y+1).0).

## [0.0.9] — 2026-11-16

A **short command name, version flags and desktop shortcuts** in **both editions** (Python and Rust), plus rewritten READMEs.

### Added
- **`dct`**, the short name of `dictz`: the same program with the same commands.
  - Python: a second console script (`pip` / `pipx` install `dictz` and `dct`).
  - Rust: a second binary, `cargo install dictz` installs both.
  - Portable builds: `dct` next to `dictz` (a link on Linux/macOS, `dct.exe` on Windows); `dictz update` updates both.
  - Help and errors show the name you typed.
- **Every version spelling** works and prints `dictz X.Y.Z (… edition)`: `-v`, `-V`, `--version`, `--Version`, `--VERSION`, `-version`, `-Version`, `-VERSION`, `dct version`.
- **Desktop shortcut:** `dct shortcut` (`--desktop-only`, `--menu-only`, `--remove`) and **Dosya → Masaüstü kısayolu oluştur**.
  - Linux: `~/.local/share/applications/dictz.desktop` plus a trusted launcher on the desktop (`xdg-user-dir DESKTOP`, so localized folders like *Masaüstü* work), with the dictz icon.
  - Windows: `dictz.lnk` on the Desktop and in the Start menu, with a `.ico` icon.
  - macOS: a `~/Applications/dictz.app` launcher bundle (Launchpad, Spotlight) with a `.icns` icon, plus a link on the desktop.
  - The shortcut starts the copy you are running: the AppImage, the portable exe, the pipx / venv script or the cargo binary. Existing shortcuts are replaced.
- Shell completion registers both names (bash, zsh, fish, PowerShell).

### Changed
- README and `dictz-rs/README.md` were rewritten with install steps per platform:
  - Linux, Windows and macOS, each portable / pipx / pip / cargo
  - PATH notes for the Windows *Scripts* folder
  - Gatekeeper / SmartScreen notes
  - Wayland and `libxcb-cursor0` notes
  - data folders and a completion table
- CI checks `dct -V` on the wheel, the frozen builds and the Rust release binary.
- Rust: the program entry moved into the library (`dictz::entry`), so both binaries share it.
- Roadmap: dictionary groups move to v0.0.10, and every later version moves up by one.
- CI: no `run-name` any more. The Actions list shows only the commit title, and the preflight fails a release title longer than 72 characters.

## [0.0.8] — 2026-11-16

The **scan popup**, GoldenDict's main feature, in **both editions** (Python and Rust), with the same settings, commands and protocol.

### Added — scan popup
- When scanning is on, a word copied or selected in **any program** opens in a small window next to the mouse:
  - **Ctrl+C+C**: the same text copied twice quickly (interval adjustable, 500 ms by default)
  - **every copy** (optional)
  - **selection** on Linux (X11 and the Wayland desktops that allow it) — select a word, no key needed
- Copied text is cleaned: punctuation around it is stripped, spaces are joined; lines and text longer than the limit are ignored. Copies made inside dictz itself are ignored.
- The popup window:
  - search box, 📌 pin, ⤢ "open in the main window", ✕ / Esc
  - opens next to the mouse and stays on top (both switchable)
  - hides when the mouse leaves (delay adjustable) unless pinned
  - remembers its size
  - links inside an article open in the popup; sounds play
  - lookups go to the history (switchable)
- "Tarama" button in the toolbar and Görünüm menu; "look the clipboard up in the popup" menu item.

### Added — tray, hotkeys, single instance
- **Tray icon**: click = show / hide; menu with scanning on/off, look up clipboard, preferences, quit. Optional "close to tray" and "start in tray".
  - Python: Qt tray. Rust: StatusNotifierItem on Linux (KDE, GNOME with the AppIndicator extension, …), the native tray on Windows and macOS.
- **Global hotkeys** (work while dictz is not focused):
  - Ctrl+F11 shows / hides the main window
  - Ctrl+F12 turns scanning on / off
  - Ctrl+Shift+F11 looks the clipboard up in the popup
  - Changeable or off in Tercihler → Genel kısayollar.
  - Supported on Windows, macOS and Linux X11; on Wayland, bind a desktop shortcut to the new commands instead.
- **Single instance**: starting dictz again brings the running window to the front, passing on any dictionary files.
- New commands, the same in both editions:
  - `dictz show`: brings the running window to the front, starting it if needed
  - `dictz popup [WORD]`: opens the scan popup; with no WORD it uses the clipboard
  - `dictz watch on|off|toggle|status`
  - `dictz gui --popup WORD`
  - Either edition's command line talks to either edition's window over a local, token-protected connection (`instance.json` in the data folder).
- The window and tray use the dictz icon, with identical pixels in both editions.

### Added — settings
- 21 new settings, for **75 in 9 pages**:
  - New page "Tarama penceresi": on/off, Ctrl+C+C, every copy, selection, Ctrl+C+C interval, maximum length, near the mouse, always on top, auto-hide, hide delay, width, height, record in history.
  - New page "Genel kısayollar": on/off and the three hotkeys.
  - General page: tray icon, close to tray, start in tray, single instance.

### Changed
- Python: `python-xlib` is a dependency on Linux (global hotkeys through XGrabKey, no compiler needed). On macOS, `pip install "dictz[hotkeys]"` adds pynput.
- Rust: new default feature `scan`. It is pure Rust on Linux: x11rb and the D-Bus tray, with no new system packages.
- The Rust settings schema is now generated from the Python one by `tests/data/parity/make_samples.py`.
- New parity golden file `scan_golden.json` covers text cleaning, Ctrl+C+C timing, hotkey parsing and the icon pixels. Both test suites check it.

## [0.0.7] — 2026-11-15

Everything below is in **both editions** (Python `pip install dictz`, Rust `cargo install dictz`), with the same commands, settings and output.

### Fixed
- **Python 3.14: Babylon (`.bgl`) dictionaries could not be opened and start-up hung for 5–10 s.**
  - Python 3.14 moved the `_compression` module. PyGlossary's Babylon plugin failed with "Module '_compression' not found … skipping plugin 'babylon_bgl'". dictz now registers the new module under the old name before PyGlossary loads.
  - PyGlossary's log noise is silenced; set `DICTZ_DEBUG=1` to see it again.
  - Dictionaries are loaded **in the background**: the window opens at once and a progress bar shows the loading.
- `dictz add` stored relative paths, so a dictionary added from another folder was "missing" later. Paths are now stored as absolute paths.
- Re-adding dictionaries could switch disabled dictionaries back on.

### Added — command line (`dictz <command>`)
- A detailed command line, like venvstudio / isnady. Without a command the window opens, as before; `dictz FILE…` still opens the window with those dictionaries.
- Commands:
  - looking things up: `lookup` (alias `l`), `suggest` (`s`), `fuzzy` (`f`), `random`, `shell` (interactive prompt)
  - dictionaries: `dicts` (`ls`), `add`, `remove` (`rm`), `enable`, `disable`, `move`, `info`, `reindex`, `scan`, `dump` (tsv / json / txt)
  - your data: `history`, `favorites` (`fav`), `config` (`list` / `get` / `set` / `reset` / `path` / `export` / `import`), `paths`
  - maintenance: `doctor`, `selftest`, **`update`** (`upgrade`), `version`, `completion` (bash / zsh / fish / powershell), `help`
- **`dictz update`** finds out how this copy was installed and updates it in place:
  - Python: pip, pipx, venv, `--user`, editable checkout, system install (with `sudo`) or a portable / AppImage build (downloaded from the GitHub release)
  - Rust: `cargo install` or a portable binary (downloaded and swapped; Windows keeps a `.old` copy until the next start)
  - `--check` only looks; `--yes` does not ask.
- Global options: `--data-dir DIR` (also `DICTZ_DATA_DIR`), `--no-color` (also `NO_COLOR`), `-v/--version`. Most commands have `--json`.
- The Rust Windows build attaches to the calling console, so CLI output shows up in cmd / PowerShell.

### Added — settings (more detailed than GoldenDict)
- **54 settings in 7 pages** (Genel, Görünüm, Makale, Arama, Sözlükler, Ses, Geçmiş ve Favoriler), stored in one **`settings.json`** that both editions read and write.
  - general: restore tabs, restore last search, check for updates on start, confirm quit, remember window, always on top, load in background
  - appearance: theme, accent color, UI font and size, toolbar / dictionary bar / found-in panel / sidebar / status bar on-off, sidebar side, tab title length
  - article: font, size, line spacing, headword size, zoom, max width, dictionary headers, open collapsed, pictures, max picture height, find highlight color
  - search: live search, delay, minimum letters, suggestion count, prefix fallback, fuzzy on/off, threshold, count and fill, max articles, Enter opens a new tab, select all on focus, ignore case
  - dictionaries: dictionary folders (scanned on start), recursive, scan on start, reindex changed files
  - sound: auto-play, volume, system player
  - history: on/off, sidebar counts, database limit (old rows purged), clear on exit
- The **Preferences window** is built from the schema:
  - a search box across all settings
  - ↺ reset per setting, "reset this page" and "reset all"
  - a **Shortcuts** page
  - an **Advanced** page: open data folder, rebuild all indexes, check for updates, clear history / favorites, export / import settings
  - Uygula / Tamam / İptal
- Old settings (Python QSettings, Rust `dictz-rs.ini`) are imported once.

### Added — interface
- **Dictionary bar**: one toggle per dictionary; right-click = only this one; "Tümü" turns all on.
- **"Bulunduğu sözlükler" panel**: which dictionaries have the word; click to jump to that article.
- **Collapsible articles** per dictionary, with "kopyala" (copy as text) and "yalnızca bu" (search only this dictionary in a new tab).
- **Find in page** (Ctrl+Shift+F, F3 / Shift+F3) with highlighting and an n/m counter.
- Save article as HTML or text (Ctrl+S), copy article (Ctrl+Shift+A), random word (Ctrl+R), full screen (F11), always on top, view toggles, theme menu.
- Menus: Dosya, Düzen, Görünüm, **Git**, **Sözlükler**, Yardım (shortcuts F1, command-line help, check for updates, data folder, about).
- Sidebar: filter boxes, counts, right-click remove, favorites export; double-click a dictionary for its details.
- Dictionary manager: table (on, name, format, words, languages, file), add files / folder, top / up / down / bottom, reindex, info, remove with confirmation.
- Status bar: loading progress, "n madde · n sözlük · n ms", enabled dictionaries, zoom.
- Dictionary folders are scanned on start; files changed since indexing are reindexed.

### Changed
- CI tests Python 3.14 as well; the 3.14 classifier was added.
- More parity golden files (`tests/data/parity/`): article text, settings schema, CLI command list, shell completion scripts and shortcuts. Both editions are tested against them.

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
