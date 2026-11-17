# dictz — TODO

Status legend: ⏳ in flight · 📋 planned · 💡 idea · ✅ done

---

## Goal: more than GoldenDict

dictz must do everything GoldenDict / GoldenDict-ng does, in **both editions**, plus the things GoldenDict does not have. One version = one theme; we review it together before the next one.

### Feature matrix (GoldenDict-ng vs dictz)

| Area | GoldenDict-ng | dictz now | Planned |
|---|---|---|---|
| StarDict, DSL, Babylon | ✓ | ✓ | `.syn`, `sametypesequence`, DSL abbreviations, `.spx` sound — v0.1.3 |
| MDict (.mdx/.mdd), XDXF, DictD, Slob | ✓ | — | v0.1.3 |
| ZIM, Lingoes LSA, SDict, Aard, zipped sound packs, sound folders, Epwing, GLS | ✓ | — | v0.1.4 |
| Dictionaries > 4 GB, millions of headwords | ✓ | partial (BGL loads into memory) | v0.1.4 |
| Scan popup (selection / clipboard / Ctrl+C+C), tray icon, global hotkeys | ✓ | ✓ (v0.0.8) | modifier-key-only scan, popup groups — v0.1.1 |
| Dictionary groups (switch, hotkeys, per-group order, group tabs) | ✓ | — | v0.1.1 |
| Custom dictionary names, dictionary metadata file | ✓ | — | v0.1.1 |
| Ignore case / diacritics / punctuation / whitespace | ✓ | case + diacritics | v0.1.2 |
| Hunspell morphology, stemming, spelling suggestions | ✓ | — (fuzzy instead) | v0.1.2 |
| Transliteration (Cyrillic, Greek, Romaji, Chinese conversion…) | ✓ | — | v0.1.2 |
| Wildcard / regex headword search | ✓ | prefix only | v0.1.2 |
| Full-text search inside definitions | ✓ (Xapian) | — | v0.1.5 |
| Wikipedia / Wiktionary (MediaWiki), websites, DICT servers, Forvo, external programs | ✓ | — | v0.1.6 |
| Voice engines (TTS) | ✓ | — | v0.1.7 |
| Anki | ✓ | — | v0.1.8 |
| Favorites folders, history / favorites export | ✓ | flat favorites, export | v0.1.8 |
| Article CSS (global + per dictionary), print, headwords dialog + export | ✓ | — | v0.1.9 |
| Shortcut editor, portable mode | partial | listed only | v0.1.9 |
| Fuzzy "did you mean" with scores | — | ✓ | ➕ dictz only |
| Interface in 25 languages with a flag picker (RTL for ar/fa/ur) | yes, in Preferences only | ✓ | ➕ dictz only |
| Dictionary icons (own file, BGL icon, generated monogram badge), language pair + format on every article card | file icons only | ✓ | ➕ dictz only |
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

> **Version numbers moved up:** v0.1.1 became the dictionary-list fix. Next come **v0.1.2 — speed** (✅ done after v0.1.1 as a plain push: article stores, no full parse at start) then **v0.1.3 — start from the JSON files only + start with the system / tray** (✅), then **v0.1.4 — article display like GoldenDict**, then the **Dictionary Center** (maintenance, online catalogs, downloads — the section right below), then themes and fonts. The groups below and everything after them shift to later numbers in the same order; the matrix above keeps the old numbers until they are renumbered.

## Dictionary Center — maintenance, online catalogs, downloads (big job, both editions)

> Asked for on 2026-11-17: "dictionary maintenance must be very, very rich"; add a link and download straight from the internet; pull dictionaries from **my own server, like isnady**; every detail down to **which language into which language**. Planned as its own group of versions **after v0.1.4 (article display)**, split into phases A–H below; each phase is one delivery, in **both editions** (GUI + CLI, same data files, parity tests).

### A. Languages: "from which language into which language" (the base of everything else)
- 📋 One language model for both editions: ISO 639-1 / 639-3 codes (`en` / `eng`), native and English names, script (Latn, Cyrl, Arab, Hans, Hant, Jpan, Kore, Deva…), RTL flag, flag of the main country (flag-icons, MIT). Shared table `languages.json` (+ golden) so Python and Rust name and sort languages the same.
- 📋 Every dictionary gets **source → target**: from its own metadata (StarDict `.ifo`, DSL `#INDEX_LANGUAGE` / `#CONTENTS_LANGUAGE`, BGL info, MDict header, FreeDict `eng-tur` names), else from its file / book name (`En2Tr`, `eng-tur`, `English-Turkish`, `İngilizce → Türkçe`, `Англо-русский`, `英汉`), else "unknown" with a one-click fix by hand. Already partly in `dictident` — extend and move to the language table.
- 📋 Kinds: **bilingual** (en → tr), **monolingual / explanatory** (tr → tr), **multilingual** (en → de, fr, es…), **bidirectional** (one file both ways), **thesaurus / synonyms**, **encyclopedia** (Wikipedia, ZIM), **pronunciation / sound packs**, **phrasebook**, **etymology**, **terminology** (medicine, law, IT, religion…). Stored per dictionary; filterable.
- 📋 Language pair filter everywhere (catalog, installed list, dictionary bar, popup): **From** and **To** pickers with flags and search, "↔ swap", "any", "both directions", "monolingual only". Counts per choice ("Türkçe → *: 37 sözlük").
- 📋 Pair matrix view: rows = source languages, columns = target languages, cell = number of installed / available dictionaries; click a cell → that list. Shows at a glance which pairs are missing ("no Kazakh → Turkish dictionary yet").
- 📋 Automatic groups by pair (en → tr, tr → en …) for the group feature; the search uses the pair of the word's script when "auto pair" is on (Cyrillic word → dictionaries whose source is Cyrillic).
- 📋 Per-pair default order (which dictionary first for en → tr), kept in `dictionaries.json`.

### B. Sources (where dictionaries come from) — like isnady's data sources
- 📋 **Sources page** (Dictionary Center → Sources, and Preferences → Dictionaries): built-in sources (editable, they may move) + **"My sources"** (user-added), each with on/off, name, type, URL, last refresh, item count, status (OK / unreachable / needs sign-in), "Refresh", "Edit", "Remove", "Import all".
- 📋 **My own server (default source, like isnady-data)**: a dictz catalog on his server / GitHub (`dictz-data` repo or `dictz.bayramkotan.com` / any HTTPS URL). Format **`dictz-catalog.json`** (versioned, documented in `docs/CATALOG.md`, JSON schema + golden in `tests/data/parity/`):
  - `catalog`: `name`, `version`, `updated`, `homepage`, `description`, optional `parent` catalogs to include.
  - one entry per dictionary: `id` (stable), `name`, `source_lang`, `target_lang` (one or a list), `kind`, `format` (stardict, dsl, bgl, mdict, xdxf, dictd, slob, zim, tabfile…), `headwords`, `edition` / `version`, `date`, `size` (bytes, download and unpacked), `files` (list of `url` + optional `mirrors`, `sha256` / `sha512`, `archive`: zip / tar.gz / tar.xz / tar.bz2 / 7z / none, `inner_path`), `license` (SPDX id + text URL), `author` / `publisher`, `description` per interface language, `tags`, `icon` URL, `sample` (a few headwords + one article for the preview), `min_dictz` version, `replaces` (old id), `deprecated`.
  - signed catalogs (optional): `catalog.json.sig` with an ed25519 key shown in the source's settings; dictz warns when the signature does not match.
  - a tool to make the catalog: `dct catalog build FOLDER --base-url URL` reads a folder of dictionaries (both editions), fills languages / counts / checksums / sizes / samples and writes `dictz-catalog.json`; `dct catalog check URL` validates one.
- 📋 **Source types** dictz can read (each a small adapter; same list in Python and Rust):
  - `dictz-catalog` — the JSON above (own server, GitHub raw / Pages, any web server).
  - `freedict` — FreeDict API `https://freedict.org/freedict-database.json` (one entry per pair like `eng-tur`; releases per platform: `stardict` `.tar.xz`, `dictd`, `slob`, `src`; size + SHA-512 per release) → languages from the pair name, licence GPL / per dictionary.
  - `github-releases` — `owner/repo`: assets of the latest (or every) release (e.g. xxyzz/wiktionary_stardict: dated tags, StarDict assets, GPL-3.0); asset name → languages by a per-source pattern.
  - `github-folder` — files in a repo folder (e.g. Vuizur/Wiktionary-Dictionaries: `<Language>-English` StarDict `.tar.gz`, TSV, Kindle `.mobi`; CC-BY-SA 3.0 / GFDL) via the GitHub contents API (token optional, rate limit shown).
  - `nextcloud-share` — a public Nextcloud / ownCloud share link like `cloud.freemdict.com/index.php/s/<token>`: listing via WebDAV `PROPFIND` on `/public.php/webdav/` (token as user name, optional share password), download per file, folders browsable; MDict `.mdx` / `.mdd` pairs kept together.
  - `webdav` — any WebDAV folder with user name + password.
  - `http-index` — an Apache / nginx / lighttpd directory listing (e.g. old StarDict mirrors like download.huzheng.org): links parsed, folders walked to a set depth, files grouped by stem (`.ifo` + `.idx` + `.dict.dz`, `.tar.bz2`).
  - `archive-org` — Internet Archive: search (advanced search API, `mediatype`, query such as `"StarDict"`, `"Babylon BGL"`, `"MDict mdx"`) and item file lists (metadata API); every file of an item shown with size and format; the user picks.
  - `direct-url` — **"Add from link…"**: paste any URL (file or archive); dictz looks at the name, size and content type, downloads, unpacks and adds it.
  - `local` — a folder or archive on disk / USB / network share (same install path as downloads).
  - `telegram` — public channels (e.g. `t.me/s/Dictionaries_GoldenDict`): the web preview lists posts and file names but files can only be fetched with a Telegram account (MTProto). Plan: show the channel's list read-only with "open in Telegram"; a real download only with the user's own Telegram API id / hash (optional extra, off by default).
  - 💡 later: Google Drive / Yandex.Disk / Dropbox public links, FTP, S3, torrent / magnet (aria2 if installed).
- 📋 **Credentials** per source: none, basic (user + password), bearer token, custom header, share password; kept in the OS keyring (Windows Credential Manager, macOS Keychain, Secret Service) — never in `settings.json`; the CLI can take them from env vars.
- 📋 Sources are kept in **`sources.json`** in the data folder (same file in both editions, lock + backup like `dictionaries.json`); export / import a source list to share it.
- 📋 **Which sources are built in:** only those whose dictionaries are clearly free to redistribute (FreeDict, Wiktionary builds, his own server). Community archives (Huzheng, Internet Archive uploads, FreeMdict, Telegram) are **not** built in: the user adds them himself under "My sources" (dictz supports their protocols); every catalog entry shows its licence, and dictz shows "licence unknown" where a source does not say.

### C. Catalog (browse and choose)
- 📋 **Catalog page**: all dictionaries of all enabled sources in one list (cached, refreshed in the background, offline: last list): name, **from → to** with flags, kind, format, headwords, size, licence, source, date / edition, installed / update available / not installed.
- 📋 Filters: from / to language, kind, format, source, licence (free / unknown / all), size range, headword range, "installed", "updates", text search (name, description, tags); sort by name, pair, size, headwords, date, popularity (download count when the source gives one).
- 📋 Preview before downloading: description, licence text link, sample headwords and one article rendered like a real article, file list with sizes, where it will be installed.
- 📋 Multi-select → "Install selected"; "install everything for en → tr"; recommended set for a pair ("best free en → tr": FreeDict + Wiktionary).
- 📋 Duplicates across sources shown once with the source choice (same `id` / same checksum / same name + pair).

### D. Download manager
- 📋 Queue with parallel downloads (setting: 1–8), per-file and total progress, speed, ETA, pause / resume / cancel, retry with back-off, **resume after restart** (HTTP Range, `.part` files), mirrors tried in order.
- 📋 Verification: size, SHA-256 / SHA-512 when the source gives one; failed check → delete + retry once, then report.
- 📋 Unpacking: zip, tar.gz, tar.xz, tar.bz2, 7z (when available), single `.gz` / `.dz` / `.bz2`; StarDict sets (`.ifo` + `.idx[.gz]` + `.dict[.dz]` + `.syn`), DSL with `.files.zip` / `_abrv.dsl` / `.ann`, MDict `.mdx` + `.mdd`, BGL; pictures / icons next to them. Rust: `zip`, `tar`, `flate2`, `xz2` / `lzma-rs`, `bzip2`; Python: stdlib (`zipfile`, `tarfile`, `lzma`, `bz2`), `py7zr` optional.
- 📋 Install folder: setting `downloads.folder` (default `DATA/dictionaries/<source>/<id>/`), shown in the folder list as a dictionary folder (scanned like the others); disk-space check before starting; "open folder".
- 📋 After install: added to `dictionaries.json` (at the end, or right after other dictionaries of the same pair), article store built in the background, the dictionary bar shows it at once.
- 📋 Proxy (system / manual / none), bandwidth limit, "only on Wi-Fi / unmetered" (later), user agent `dictz/<version>`, timeouts; everything online is opt-in (off until the user opens the Dictionary Center or turns it on).
- 📋 Download history / log (what, from where, when, checksum, licence) kept in `downloads.json`.

### E. Updates
- 📋 Installed dictionaries remember their source, id, edition, date and checksum (`dictionaries.json` fields `source`, `source_id`, `edition`, `sha256`).
- 📋 "Check for dictionary updates" (button, on start optional, every N days optional): newer edition / date / checksum → "update available" badge; update keeps the dictionary's place, name, on/off and group; the old version can be kept as backup (setting).
- 📋 Replaced / renamed ids follow `replaces`; deprecated entries show a note.

### F. Maintenance (the "Health" page — very rich)
- 📋 **Health check** (`dct doctor dicts` too): missing files, moved files (find by name + size + checksum in the dictionary folders and offer "relink"), broken / unreadable files with the parser's reason, duplicates (same file twice, same dictionary in two formats, same checksum), dictionaries without languages, stale or orphan article stores (`articles/*.db` without a dictionary), stale headword index, failed loads, huge stores; each with a one-click fix.
- 📋 Rebuild one / all article stores and indexes; compact (VACUUM) the stores and the index; show disk use per dictionary (files + store + index + media).
- 📋 Per-dictionary settings (in its info / properties window): display name, icon, languages (fix by hand), kind, tags, notes, priority within its pair, font and size, RTL, "hide pictures", "full-text search on/off" (later), "open collapsed"; kept in `dictionaries.json`.
- 📋 Batch operations on a selection: enable / disable, move, set languages, add to group, rebuild, remove (with "also delete the files" for downloaded ones), export.
- 📋 Export / import the whole setup (list + order + settings + sources, optionally the files) as one `.dictz-setup.zip` to move to another computer.
- 📋 Statistics: dictionaries and headwords per pair / format / source, size totals, most used dictionaries (from history), dictionaries never shown in results.

### G. Conversion (PyGlossary-backed, like GoldenDict + more)
- 📋 "Convert…" for a dictionary: StarDict, DSL, BGL, MDict, XDXF, dictd, Slob, TSV / CSV, JSON, Kindle (`.mobi` via PyGlossary where available), Kobo, AppleDict source, plain HTML. Python edition through PyGlossary (`github.com/ilius/pyglossary`, GPL-3.0); the Rust edition calls the same conversion through `dct`'s Python helper when PyGlossary is installed, otherwise offers the formats it writes natively (StarDict, TSV, JSON).
- 📋 Options: target folder, compression (dictzip), keep media, merge several dictionaries into one, split by letter, filter by headword list, reverse a bilingual dictionary (tr → en from en → tr, marked as "reversed").
- 📋 `dct convert IN --to FORMAT [-o OUT]` and `dct merge A B -o C`.

### H. CLI (everything above, same commands in both editions)
- 📋 `dct sources [list | add URL --type T --name N [--user U --password-env VAR | --token-env VAR] | remove N | refresh [N] | export FILE | import FILE]`
- 📋 `dct catalog [search TEXT] [--from en --to tr] [--kind K] [--format F] [--source S] [--free] [--json]`
- 📋 `dct install ID|URL ... [--to FOLDER] [--no-verify]`, `dct uninstall NAME [--delete-files]`, `dct downloads [list | pause | resume | cancel]`
- 📋 `dct update-dicts [--check]`, `dct doctor dicts [--fix]`, `dct pairs [--matrix]`, `dct convert`, `dct merge`, `dct catalog build / check`
- 📋 Completion scripts, `cli_commands.json` golden and READMEs updated.

### Order of the phases (proposal)
1. **A** languages + pair filter (needed by everything else) · 2. **B** sources + own-server catalog + "Add from link…" · 3. **C + D** catalog page + download manager (FreeDict, GitHub, direct URL first; Nextcloud, WebDAV, http-index, archive.org next) · 4. **E** updates · 5. **F** health page and per-dictionary settings · 6. **G** conversion · 7. **H** remaining CLI. MDict (`.mdx/.mdd`) and dictd reading (from "formats I") must land before the FreeMdict / dictd sources are useful.

### Sources the user named (2026-11-17), with what is known about them
| Source | What it offers | How dictz reaches it | Built in? |
|---|---|---|---|
| FreeDict (`freedict.org/downloads`) | free (GPL etc.) bilingual dictionaries, hundreds of pairs; StarDict `.tar.xz`, dictd, Slob, sources | `freedict` adapter on `freedict-database.json` (sizes, SHA-512) | yes |
| Wiktionary StarDict (`github.com/xxyzz/wiktionary_stardict`) | Wiktionary dictionaries for KOReader / e-readers, dated releases, GPL-3.0 | `github-releases` | yes |
| Wiktionary Dictionaries (`github.com/Vuizur/Wiktionary-Dictionaries`) | `<Language>-English` StarDict, TSV, Kindle; CC-BY-SA 3.0 / GFDL | `github-folder` | yes |
| Huzheng StarDict archive (`download.huzheng.org`) | the oldest, largest StarDict archive, many languages; licences mixed / often unknown | `http-index` | no — "My sources" |
| Internet Archive (`archive.org`) | community uploads (StarDict, Babylon BGL, MDict packs) | `archive-org` (search + item files) | no — "My sources" |
| FreeMdict forum + cloud (`forum.freemdict.com`, `cloud.freemdict.com/index.php/s/pgKcDcbSDTCzXCs`) | the most active MDict / StarDict community, Nextcloud shares | `nextcloud-share` (WebDAV) | no — "My sources" |
| Telegram (`t.me/s/Dictionaries_GoldenDict`) | packs in 40+ languages: DSL, BGL, MDX, StarDict | read-only list; download only with the user's own Telegram API key (optional) | no |
| His own server (like isnady-data) | everything he publishes | `dictz-catalog` | **yes, the default source** |
| PyGlossary (`github.com/ilius/pyglossary`) | converter between dictionary formats | phase G | — |

### v0.1.1 → later — dictionary groups
- 📋 Groups with name, icon, order and their own dictionary list; "All" group
- 📋 Group selector in the toolbar, popup and CLI (`dictz lookup -g NAME`, `dictz groups`)
- 📋 Hotkey per group; a tab remembers its group
- 📋 Automatic groups by language pair (En→Tr, Tr→En…)
- 📋 Custom display name per dictionary; `dictz.toml` metadata next to a dictionary (name, language, disable full-text)

### v0.1.2 — smarter matching
- 📋 Ignore punctuation and whitespace (case and diacritics already work); each one a setting
- 📋 Hunspell morphology: lemma lookup ("went" → "go", "kitaplarımızdan" → "kitap"), spelling suggestions; dictionaries downloadable from the app
- 📋 Turkish stemming (Zemberek-style suffix stripping) as a dictz extra
- 📋 Wildcard (`*`, `?`) and regex headword search, also in the CLI
- 📋 Transliteration: Cyrillic ↔ Latin, Greek, Romaji, Ottoman Latin, Chinese simplified ↔ traditional
- 📋 Multi-word phrases: look up the whole phrase, then each word

### v0.1.3 — formats I
- 📋 MDict `.mdx` / `.mdd` (with CSS, images, sounds) — the most requested format
- 📋 XDXF, DictD (`.index` + `.dict.dz`), Slob / Aard 2
- 📋 StarDict `.syn` synonyms and `sametypesequence`; latin-1 fallback for `.ifo`
- 📋 DSL: abbreviation files (`_abrv.dsl`), `.spx` sounds, case-insensitive media extensions

### v0.1.4 — formats II and large dictionaries
- 📋 ZIM (Kiwix: offline Wikipedia), Lingoes LSA sound packs, SDict, Aard, GLS
- 📋 Sound folders and zipped sound packs as pronunciation dictionaries
- 💡 Epwing
- 📋 Lazy loading for big files (BGL, MDX) and dictionaries > 4 GB; millions of headwords without memory blow-up

### v0.1.5 — full-text search
- 📋 Search inside definitions (SQLite FTS5 content index per dictionary, built in the background, can be turned off per dictionary)
- 📋 Modes: words, phrase, wildcard, regex, "near"; results list with context snippets
- 📋 Full-text search window and `dictz search` in the CLI

### v0.1.6 — online sources
- 📋 Wikipedia / Wiktionary (MediaWiki API) per language
- 📋 Any website as a dictionary (URL template with `%GDWORD%`-style placeholder), shown in a tab or opened in the browser
- 📋 DICT protocol servers, Forvo pronunciations (API key), external programs as dictionaries
- 📋 Proxy settings and a "no network" switch; everything online is off by default

### v0.1.7 — pronunciation (TTS)
- 📋 System voices (SAPI / NSSpeech / speech-dispatcher) and edge-tts neural voices
- 📋 Voice per language and per dictionary; "pronounce" button and hotkey; auto-pronounce

### v0.1.8 — learning
- 📋 Anki via AnkiConnect (word + selected article + sound), deck / model / field mapping
- 📋 Favorites folders, drag and drop, import / export (txt, csv, json); history export
- 📋 Built-in flashcards with spaced repetition from favorites (dictz extra)
- 📋 Word of the day (dictz extra)

### v0.1.9 — customization
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

## v0.2.0 — first polished release
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

- ⏳ **Article display** is not yet as clean as GoldenDict's (next big job)

- DSL `[s]` media: only `wav`/`mp3` sounds and `jpg/jpeg/gif/tif/tiff/png/bmp` pictures, with case-sensitive extensions (PyGlossary's rule, kept for parity); Lingvo `.spx` sounds are not played
- Rust GUI: Arabic/Persian **dictionary article** text is not shaped right-to-left (the interface texts are, since v0.1.0)
- Rust GUI: a font given by name is matched against font file names (no fontconfig lookup); a file path always works
- Scan popup on Wayland: global hotkeys and "near the mouse" are not possible (Wayland does not allow them); the Rust edition minimizes instead of hiding to the tray there
- Rust edition on Windows: "hidden to the tray" moves the window off screen without a taskbar button (a hidden egui window stops drawing, and the popup is drawn from it)
- `dictz update` on a Python portable zip / tar.gz build only downloads the new archive (a running PyInstaller folder cannot replace itself); the AppImage is replaced in place

- `.syn` synonym files in StarDict are ignored (v0.1.3)
- `sametypesequence` in StarDict `.ifo` is not interpreted (v0.1.3)
- UTF-8 assumed for `.ifo` headers, no latin-1 fallback (v0.1.3)
- Large `.bgl` files load eagerly into memory (v0.1.4)
- QWebEngine missing on some PySide6 slim installs → falls back to QTextBrowser
- Windows SmartScreen warning on first run (unsigned)
- macOS Gatekeeper warning on first run (unnotarized)

---

## Recently done

- ✅ **v0.1.3** — a start reads only the JSON files: every dictionary opens from `dictionaries.json` alone (stamp of its files; no article store, no dictionary file read until the first lookup), icons are looked for once and kept (`icons/`, `icon` field), the dictionary folders are not listed again when unchanged (`scan_cache.json`); nothing is rebuilt or re-scanned unless a file really changed. New setting **Start with the system** (`general.autostart`, 78 settings): Windows Run key, Linux `~/.config/autostart/dictz.desktop`, macOS LaunchAgent; starts hidden in the tray (`--autostart`), the tray icon is always there then

- ✅ **v0.1.2** — dictionaries no longer get lost (list lock, always re-read before a change, `.bak` backup, failed dictionaries stay visible with ⚠); dictionary folders like GoldenDict's Sources → Files (several folders, each with its own Subfolders box, kept and scanned at every start, also in the dictionary manager); fast start released

- ✅ **after v0.1.1 (plain push, released in v0.1.2)** — fast start like GoldenDict, in both editions: a dictionary is parsed once into `articles/<id>.db`; later starts read articles on demand (500k headwords: Python 7.2 s → 0.07 s, Rust 1.0 s → 0.01 s); rebuilt by itself when the file changes

- ✅ **v0.1.1** — the dictionary list moved to `dictionaries.json` in both editions: dictionaries are known by file path, a duplicate name becomes `Name (2)`, missing / failed dictionaries keep their place, one-time migration from the SQLite table — fixes dictionaries getting mixed up or lost

- ✅ **v0.1.0** — interface in 25 languages with a flag picker (RTL for ar/fa/ur), dictionary icons + article cards (language pair, format, colour), READMEs with screenshots — in both editions
- ✅ **v0.0.9** — `dct` short name, every `--version` spelling, `dct shortcut` + Dosya → Masaüstü kısayolu oluştur, completion for both names, per-platform READMEs — in both editions
- ✅ **v0.0.8** — scan popup (Ctrl+C+C, every copy, selection), tray icon, global hotkeys, single instance, `dictz show / popup / watch`, 21 new settings (75 in 9 pages) — in both editions
- ✅ **v0.0.7** — Python 3.14 Babylon fix + no more start-up hang (background loading); full CLI incl. `dictz update` in both editions; 54 shared settings with a detailed Preferences window; dictionary bar, found-in panel, collapsible articles, find in page, save/copy article, random word, full screen, Git/Sözlükler menus
- ✅ **v0.0.6** — Rust edition is a full app (native StarDict/DSL/Babylon, FTS5, GUI, portable binaries); fuzzy "did you mean" + DSL pictures/sounds in both editions; dictionaries restored on start; order kept; link clicks fixed; parity golden tests; CI triggers on tags only
- ✅ **v0.0.5** — portable binaries actually start and read DSL/Babylon (PyInstaller entry point + PyGlossary bundle paths fixed); `dictz --selftest` run on every binary in CI; one tag → PyPI + crates.io + GitHub Releases via API-token secrets; `dictz-rs/` Rust crate scaffold
- ✅ **v0.0.4** — cross-platform portable builds (Linux tarball + AppImage, Windows portable zip, macOS .app); PyInstaller spec; GitHub Actions matrix release pipeline
- ✅ **v0.0.3** — tabbed UI, sidebar, themes, zoom, window state, QWebEngine renderer, keyboard shortcuts, dictionary manager dialog
- ✅ **v0.0.2** — DSL + Babylon parsers via PyGlossary
- ✅ **v0.0.1** — StarDict native parser, SQLite FTS5 index, 3-pane UI
