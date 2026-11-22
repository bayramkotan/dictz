# Changelog

All notable changes to dictz are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **dictz-mobile/** — the Android and iOS apps (first build, for testing): one Flutter interface on the same Rust core (`dictz-mobile/rust`: a C interface with JSON in and out over the `dictz` library, used through `dart:ffi`). Search box always on top with Enter and random-word buttons, word list while typing (background isolate), articles one strip per dictionary (`bword://` links, DSL pictures, back), favourites and history, dictionaries added from the phone's files (StarDict set with `.syn`, DSL, Babylon or a `.zip`; copied into the app), on / off, drag to reorder, remove with or without the files, the 27 themes and the 25 interface languages (the desktop's `themes.json` and catalogues). `tool/prepare.sh` makes the platform folders (`flutter create`) and builds the core (cargo-ndk for Android, a `DictzCore.xcframework` for iOS). New jobs in the CI workflow (`mobile-core`, `mobile-android`, `mobile-ios`; on tags and manual runs, not on pull requests, never holding up the release): core tests, APK, unsigned iOS app as artifacts; a tag attaches the APK to the release.

## [0.2.0] — 2026-11-22

StarDict **synonyms** and one broken source less — in both editions.

### Added
- **StarDict synonyms (`.syn`, also `.syn.dz`):** a word that is not a headword is looked up among the dictionary's synonyms — *went* opens the article of *go*, *mice* the one of *mouse* (exact first, then ignoring case; each article once). Synonyms are offered by the word list, the index and "Bunu mu demek istediniz?" too. Many StarDict dictionaries (Wiktionary builds above all) keep their inflected forms only there, so far more words are found.
- Article stores keep the synonyms (`synonyms` table, same in both editions); a store from an earlier version still works. The `.syn` is part of a StarDict's stamp, so **only StarDicts that have one are read again** on the next start (in the background, as after a file change).

### Removed
- **FreeMdict cloud** is no longer a built-in source: its server answers HTTP 500 to folder listings. A setup that still has it loses it (and its cached list) at the next start, and the default server falls back to dictz-data; a copy pointed at another address is kept as the user's own source. It can still be added by hand (*Kaynaklar → Ekle…*, type Nextcloud share).

## [0.1.9] — 2026-11-22

**27 themes** and default sources that really work — in both editions.

### Added
- **27 interface themes** (Otomatik = the system's light or dark, plus Açık, Koyu, Sepya, Solarized Light / Dark, Gruvbox Light / Dark, GitHub Light / Dark, Catppuccin Latte / Mocha, Rosé Pine Dawn / Rosé Pine, Nord Light / Nord, One Light / One Dark, Nane, Lavanta, Dracula, Monokai, Tokyo Night, Everforest, Gece mavisi, Gece siyahı, Yüksek kontrast (açık / koyu)), from one table shared by both editions: `dictz/resources/themes.json` (Rust copy `dictz-rs/src/gui/themes.json`, made by `make_samples.py`). Each theme colours the window and the article page and brings its own accent colour, used while *Vurgu rengi* is left at its default. Pick one in **Görünüm → Tema**, under the toolbar's theme button (a menu now, light themes first, then the dark ones) or in Tercihler → Görünüm.

### Fixed
- **Default sources still showed errors after v0.1.8:** built-in sources are stored in `sources.json` the first time it is read, so a changed built-in address never reached an existing setup. A stored built-in still on a retired address now moves to today's address (its on / off switch stays, its cached error is dropped; an address the user typed is kept) — `RETIRED_URLS` in `sources.py` / `sources.rs`.
- **StarDict archive:** download.huzheng.org no longer answers at all; the built-in source now reads the huzheng mirror recovered from the Internet Archive, **https://stardict.uber.space/**. The web-folder reader also follows static sites whose folders are `…/index.html` pages.

## [0.1.8] — 2026-11-22

Everything about dictionaries in **one window**, a search that **never freezes**, and a search box that **stays put** — in both editions.

### Changed
- **Fast search:** the word list and "Bunu mu demek istediniz?" are worked out on their own thread (`dictz/core/suggest.py` ↔ `dictz-rs/src/suggest.rs`) with its own read-only connection to the index; typing never waits for them, and only the newest request is answered. The fuzzy candidate list (every headword of the enabled dictionaries) is built **once** in the background — after loading and a moment after a dictionary is switched on or off — with its lower-cased copy, instead of on the first typo and again after every change; each dictionary's own list is kept, so switching one dictionary does not read all the others again. A "no result" page gets its suggestions as soon as they are ready. The Rust matcher skips candidates by length before any work and no longer lower-cases every candidate on every keystroke.
- **The search box is fixed** above the side tabs, with an **Enter button (⏎)** beside it: it stays when Sözlükler, Favoriler or Geçmiş is open, and typing brings the word list back.
- **Tercihler** no longer has a *Sözlükler* page that only pointed elsewhere: the dictionary settings are in **Sözlükler → Ayarlar** (and the Sözlükler menu's *Sözlük ayarları…*).
- Default sources: **StarDict archive** is read over `http://download.huzheng.org/` (the site has no HTTPS, so the old `https://` address was refused); **dictz-data** reads the catalog kept in the dictz repository (`docs/dictz-data/catalog.json`) until its own repository exists, so it no longer answers 404.
- **One Sözlükler window** (Sözlükler → Sözlükler…, Ctrl+M; the cloud button) replaces the dictionary manager, the Dictionary Center and the dictionary pages of Tercihler. Seven tabs: **Sözlüklerim** (search order with drag and ▲ ▼, on / off, only this, info, reindex, open folder, add files / folder, remove from the list or **with the files**, catalog licence and ↑ update per row), **Klasörler** (scanned folders, subfolders box, scan now), **Katalog**, **Kaynaklar**, **İndirmeler**, **İçe / Dışa aktar**, **Ayarlar** (dictionary and catalog settings, saved as you change them). The old *Kurulu* tab is folded into Sözlüklerim and Katalog (update one / all).
- **One Sözlükler menu**: Sözlükler… (Ctrl+M), Sözlük ekle… (Ctrl+O), Klasör ekle… (Ctrl+Shift+O), Bağlantıdan sözlük ekle… (Ctrl+Shift+L), Sözlük klasörleri…, Sözlük indir (katalog)… (Ctrl+Shift+M), Kaynaklar (sunucular)…, Kurulumu dışa / içe aktar…, Tümünü etkinleştir / devre dışı bırak, Sözlük ayarları…. Dosya keeps only the article, favourites, desktop shortcut and Çıkış; Tercihler loses the Sözlükler and Sözlük Merkezi pages.
- **Dictionary icons do something:** in the dictionary bar a click goes to that dictionary's article, **Ctrl+click** switches it on / off (an off dictionary is drawn faded), right click opens a menu (aç / kapa, yalnızca bunu etkinleştir, bilgi, Sözlüklerimde göster, kaldır). Switching re-runs the current search.

### Added
- **Drag to reorder** dictionaries in the Sözlüklerim tab, the dictionary bar, the "Bulunduğu yer" panel and the left pane; the order is saved at once (`move_before` / `set_order`, same in both editions).
- Removing a dictionary *with its files* deletes only its own files (`own_files`: source files, `.syn`, media archive), the same in both editions (`tests/test_center.py`, `dictz-rs/tests/center.rs`).
- New `enter` interface icon; 25 new texts in all 25 interface languages.

## [0.1.7] — 2026-11-22

The **Dictionary Center**: find, download, install, update and remove free dictionaries from many servers — or from your own server, folder or link — with every dictionary's **licence**; export and import the whole setup. In **both editions**, with the same files, commands and results.

### Added
- **Dictionary Center window** (Dosya → Sözlük Merkezi…, Ctrl+Shift+M, the cloud button on the toolbar) with five tabs:
  - **Katalog** — every dictionary of the enabled sources in one sortable table (name, languages, kind, format, words, size, licence, source, date); filters for text, source and target language (with counts and ⇄), source, licence status, format, "only those dictz opens", "hide installed"; a detail pane with the licence and what it allows (credit, share-alike, commercial use), the files and their checksums; ✓ installed, ↑ update available; install with a button or a double click, remove, open the page, copy the links.
  - **Kaynaklar (sunucular)** — the servers: on / off, add, edit, delete, up / down, make default (★), refresh, export / import the list (without passwords), restore the built-in ones. The source editor: address, type (with "guess"), file filter, language pattern, languages and licence for sources that do not say, depth, sign-in (user + password, token, custom header, share password), notes, and a **Dene** button that reads the source at once.
  - **İndirmeler** — the download queue: several at once, progress, speed, pause / resume / cancel / retry; an interrupted download continues where it stopped (HTTP Range); size and SHA-256 / SHA-512 / MD5 are checked; zip, tar, tar.gz, tar.xz and tar.bz2 are unpacked safely (no path leaves the install folder); the history below.
  - **Kurulu** — what was installed from the catalog, with version, date, licence, folder and available update; update one or all, remove with or without the files, open the folder or the licence.
  - **İçe / Dışa aktar** — export / import the **whole setup** as one zip (dictionary list, settings, sources, install records, history and favourites, optionally the dictionary files), merging or replacing; export the dictionary list with licences as CSV / JSON / TXT; build a `catalog.json` for your own server and check one; the licence table.
- **Bağlantıdan sözlük ekle…** (Ctrl+Shift+L): any link to a dictionary file or archive (also `file:`) is downloaded, unpacked and added; a link that looks like a server is offered as a new source.
- **10 source types:** dictz catalog (`catalog.json`, with `include`), FreeDict database, GitHub releases, GitHub folder, web folder listing, WebDAV, Nextcloud share, Internet Archive search, local folder, Telegram channel (listed; files open in Telegram).
- **9 built-in sources:** dictz-data (github.com/bayramkotan/dictz-data, the default server), FreeDict, Wiktionary Dictionaries (Vuizur), Wiktionary StarDict (xxyzz), ECDICT, the StarDict archive at download.huzheng.org, the Internet Archive, the FreeMdict cloud and the Dictionaries_GoldenDict Telegram channel. Every address can be changed or removed.
- **Licences:** 26 licence ids (CC0, CC BY, CC BY-SA, CC BY-NC, GPL, LGPL, AGPL, GFDL, MIT, BSD, Apache, ODbL, WordNet, EDRDG, FreeDict, Wiktionary, public domain, free, personal use, unknown) with status **open / free / unknown / nonfree** and what each allows; other spellings are recognised. Unknown and personal-use dictionaries are marked, can be hidden, and dictz asks before downloading them.
- **ISO 639 language table** (`languages.json`): `en`, `eng`, `English`, `İngilizce`-style names all become one code, shown as "English (en)".
- **Command line:** `dct sources [list|add|edit|remove|enable|disable|default|up|down|refresh|export|import|reset]`, `dct catalog [TEXT] [--from --to --license --format --supported]`, `dct catalog show|languages|updates|build|check`, `dct install ID|NAME|URL…`, `dct uninstall`, `dct downloads [clear]`, `dct licenses`, `dct setup export|import|show`, `dct dicts --export FILE [--format csv|json|txt]`; shell completion for all of them.
- **10 new settings** (page "Sözlük Merkezi"): use internet sources, refresh catalogs every N days, downloads at once, folder for downloaded dictionaries, check checksums, keep archives, add after install, show unknown licences, proxy, timeout — **90 settings in 10 pages**.
- `docs/CATALOG.md` — the catalog format, sidecar `<file>.json`, licences, source types; `docs/dictz-data/` — the starting point of the dictz-data server repository.
- Six new interface icons (download, upload, link, server, center, license); 211 new texts in all 25 interface languages.

### Fixed
- `file:` links are read the same way in both editions and on every system: `file:///C:/x`, `file://C:/x`, `file://C:\x`, `file:C:\x`, `file:///C|/x`, `file://localhost/x` and UNC `file://host/share/x` (the Rust edition read `file://C:\…` as a host name on Windows).

### Changed
- dictz now goes online also for the Dictionary Center's sources, only when it is used; `center.online` switches it off.

### Rust edition
- New default feature `center` (pure-Rust: regex, sha2, md-5, lzma-rs, bzip2 with the pure-Rust backend); `cli` and `gui` turn it on.
- Python ↔ Rust golden tests for every source type (recorded answers in `tests/data/center/`), the licence table, the built-in sources and the language codes.

## [0.1.6] — 2026-11-21

Speaker-only sound buttons, one dictz window ever, and a tray icon from the start — in **both editions**.

### Changed
- **Sound buttons show only a speaker icon** (dictz's own `sound` icon, in the link colour); the file name is in the tooltip ("Sesi çal — house_gb.wav") instead of next to every sound in the article.

### Fixed
- **Only one dictz runs.** A second start — by hand, at log-in, or of the other edition — no longer opens a second window: both editions take an operating-system lock on `instance.lock` in the data folder, and a second start hands its request (show the window, add files, open the popup) to the running one, even while that one is still starting or busy. Before, a window that did not answer within a second (still loading) let a second one open.
- **Tray icon from the start:** when the desktop's tray is not ready yet (at log-in), the Python edition tries again every 2 seconds for a minute instead of giving up.
- Start with the system, the tray icon, close / start to tray and single instance are all in **Preferences → General**.

### Changed (Rust edition)
- Minimum Rust version 1.89 (`File::try_lock`).

## [0.1.5] — 2026-11-21

Dictionaries are added in the background, several at once, and the pictures and sounds of GoldenDict packs are found — in **both editions**.

### Fixed
- **Pictures and sounds of GoldenDict packs** (e.g. Oxford Advanced Learner's 8th): sounds did not play and pictures showed as broken, because the media zip was named `X.dsl.dz.files.zip` (with `.dz`, GoldenDict's name) and dictz looked only for `X.dsl.files.zip` / `X.files.zip`. Now every name GoldenDict tries is looked for — `X.dsl.dz`, `X.dsl` and `X`, each as a `.files/` folder and a `.files.zip`, in any letter case — then StarDict's `res/` and the dictionary's folder.
- A media zip is opened once and its list of names kept, so articles with many pictures and sounds open fast even with zips of 100 000+ files.
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
