# dictz

[![PyPI version](https://img.shields.io/pypi/v/dictz.svg)](https://pypi.org/project/dictz/)
[![crates.io](https://img.shields.io/crates/v/dictz.svg)](https://crates.io/crates/dictz)
[![Python versions](https://img.shields.io/pypi/pyversions/dictz.svg)](https://pypi.org/project/dictz/)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/bayramkotan/dictz/releases)
[![GitHub release](https://img.shields.io/github/v/release/bayramkotan/dictz?include_prereleases)](https://github.com/bayramkotan/dictz/releases)
[![Downloads](https://img.shields.io/pypi/dm/dictz.svg)](https://pypi.org/project/dictz/)
[![GitHub stars](https://img.shields.io/github/stars/bayramkotan/dictz?style=social)](https://github.com/bayramkotan/dictz)

Cross-platform, multi-format, offline dictionary application — a modern GoldenDict-style desktop tool.

dictz ships as **two editions with the same features**, released together from one tag:

| | Python edition | Rust edition |
|---|---|---|
| Install | `pip install dictz` / `pipx install dictz` | `cargo install dictz` |
| GUI | Qt (PySide6 + QtWebEngine) | egui (single native binary) |
| Source | [`dictz/`](dictz/) | [`dictz-rs/`](dictz-rs/) |
| Portable builds | AppImage, Windows zip, macOS .app | `dictz-rs-*` single-file archives |

Both read dictionaries identically (checked in CI against shared golden files) and share one data folder, so dictionaries, history and favorites added in one edition appear in the other.

> [Türkçe açıklama aşağıda](#türkçe)

## Download

Portable builds for every release live on the [GitHub Releases page](https://github.com/bayramkotan/dictz/releases/latest):

| Platform | Python edition | Rust edition |
|----------|----------------|--------------|
| **Windows 10/11** | `dictz-windows-x86_64.zip` → `dictz.exe` | `dictz-rs-windows-x86_64.zip` → `dictz-rs\dictz.exe` |
| **Linux (any)** | `dictz-linux-x86_64.AppImage` (`chmod +x`, run) or `.tar.gz` | `dictz-rs-linux-x86_64.tar.gz` → `dictz-rs/dictz` |
| **macOS Apple Silicon** | `dictz-macos-arm64.zip` → right-click **Open** `dictz.app` | `dictz-rs-macos-arm64.tar.gz` → `dictz-rs/dictz` |
| **macOS Intel** | `pip install dictz` / `pipx install dictz` | `cargo install dictz` |

Binaries are unsigned: accept the SmartScreen / Gatekeeper prompt on first launch.

- PyPI: `pip install dictz` (or `pipx install dictz` on PEP-668 distros)
- crates.io: `cargo install dictz` (app) · `cargo add dictz` (library). On Linux, building needs the ALSA headers (`libasound2-dev` on Debian/Ubuntu).

Check any install with `dictz doctor` or `dictz selftest` (exit code 0 = dictionaries can be read). Update any install with `dictz update`.

## Highlights

- **Multi-format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Multi-dictionary** — load any number of dictionaries at once; a single query hits them all
- **Fast** — SQLite FTS5 index with Unicode + diacritic-insensitive search
- **Fuzzy search** — "did you mean" suggestions when nothing matches; suggestions ranked closest-first
- **Pictures and sounds** — DSL `[s]` media from `*.dsl.files/` or `*.dsl.files.zip`: pictures inline, sounds with one click
- **Offline** — no network calls, no telemetry
- **Modern UI** — tabbed definitions, dictionary bar, "found in" panel, collapsible articles, find in page, sidebar (dictionaries / favorites / history), back/forward, light + dark themes, accent color, fonts, zoom
- **Scan popup** — copy a word in any program (Ctrl+C+C, every copy, or just select it on Linux) and its articles pop up next to the mouse; tray icon, global hotkeys, single instance
- **Detailed settings** — 75 settings in 9 pages with search and per-setting reset, shared by both editions in one `settings.json`
- **Full command line** — `dictz lookup`, `dicts`, `add`, `config`, `doctor`, **`dictz update`**, shell completion… the same in both editions
- **Two editions** — Python and Rust, same features, same release, shared data
- **Portable** — every release ships standalone binaries of both editions for Windows, Linux and macOS (Apple Silicon), each self-tested in CI before publishing

## Features at a glance

- **Tabbed definition view** — open each lookup in its own tab (Ctrl+T / Ctrl+W / Ctrl+Tab)
- **Live suggestions** — debounced as-you-type, powered by FTS5 prefix matching
- **Back / Forward** — browser-style history navigation (Alt+Left / Alt+Right)
- **Favorites** — star words with Ctrl+D, review them in the sidebar
- **History** — every lookup logged, re-openable with one click, clearable with one button
- **Dictionary manager** — add / remove / reorder / enable / disable; dictionaries are reopened on the next start
- **Zoom** — Ctrl+= / Ctrl+- / Ctrl+0
- **Themes** — Light, Dark, Auto
- **Rich HTML** — DSL markup with colors, pictures and sounds (QtWebEngine in Python, a native renderer in Rust)
- **Dictionary bar** — one click turns a dictionary on/off, right-click keeps only that one
- **Find in page** — Ctrl+Shift+F, F3 / Shift+F3, highlighted matches with a counter
- **Background loading** — the window opens at once; dictionary folders are scanned on start

## Command line

Both editions have the same commands, options and output. Without a command the window opens; `dictz FILE…` opens it with those dictionaries.

```bash
dictz lookup apple            # articles as text (-d DICT, -p prefix, -f fuzzy, -n N, --json, --html)
dictz suggest app             # headwords starting with "app" (+ ≈ fuzzy fill)
dictz fuzzy aple              # "did you mean" candidates with scores
dictz random                  # a random article
dictz shell                   # interactive prompt (:fuzzy, :suggest, :dicts, :random, :q)

dictz add ~/sozlukler -r      # add files or whole folders (--disabled, --reindex)
dictz dicts                   # position, on/off, name, format, words, path (--json, --names)
dictz enable / disable / remove NAME   # names may be partial
dictz move NAME top           # up / down / top / bottom / position
dictz info NAME|FILE          # metadata
dictz reindex [NAME…]         # rebuild the search index
dictz scan FOLDER --add       # find dictionary files, add the new ones
dictz dump NAME -F txt -o out.txt      # tsv / json / txt

dictz history [--clear | --remove WORD]
dictz favorites [list | add | remove | clear] [WORD…]
dictz config list             # every setting with value, range and description
dictz config set search.fuzzy_cutoff 70
dictz config get | reset | path | export FILE | import FILE
dictz paths                   # data folder, settings, databases
dictz doctor                  # install kind, paths, index, history, selftest
dictz update [--check] [--yes]           # updates this copy where it is installed
dictz show                    # bring the running window to the front (starts it if needed)
dictz popup [WORD]            # scan popup in the running dictz (no WORD = clipboard)
dictz watch on|off|toggle|status         # clipboard / selection scanning
eval "$(dictz completion bash)"          # also zsh, fish, powershell
```

Global options: `--data-dir DIR` (or `DICTZ_DATA_DIR`), `--no-color` (or `NO_COLOR`), `-v` / `--version`. Aliases: `l`, `s`, `f`, `ls`, `rm`, `fav`, `upgrade`.

`dictz update` knows how it was installed: pip / pipx / venv / `--user` / system / editable / AppImage / portable (Python), `cargo install` / portable binary (Rust).

### Scan popup (like GoldenDict)

Turn on **Tarama** in the toolbar (or `dictz watch on`, or Ctrl+F12). Then in any program:

- select a word and press **Ctrl+C twice** quickly (Ctrl+C+C), or
- with "Her kopyalamada ara" on: copy it once, or
- on Linux with "Seçilen metni ara" on: just select it with the mouse.

A small window with the articles opens next to the mouse: 📌 keeps it open, ⤢ opens the word in the main window, Esc or moving the mouse away closes it. dictz keeps running in the **tray** when "Kapatınca tepsiye gizle" is on.

Global hotkeys: Ctrl+F11 shows/hides the window, Ctrl+F12 turns scanning on/off, Ctrl+Shift+F11 looks the clipboard up (changeable in Tercihler → Genel kısayollar). They work on Windows, macOS and Linux X11. Wayland does not let programs grab keys: bind a desktop shortcut to `dictz popup`, `dictz show` or `dictz watch toggle` instead. Either edition's CLI talks to either edition's window.

## Settings

**Düzen → Tercihler** (Ctrl+,) or `dictz config`. 75 settings, stored in `settings.json` in the data folder and shared by both editions:

| Page | Examples |
|---|---|
| Genel | restore tabs / last search, check updates on start, confirm quit, remember window, always on top, load in background, tray icon, close to tray, start in tray, single instance |
| Görünüm | theme, accent color, UI font and size, toolbar / dictionary bar / found-in / sidebar / status bar, sidebar side, tab title length |
| Makale | font, size, line spacing, headword size, zoom, max width, dictionary headers, open collapsed, pictures, picture height, find highlight color |
| Arama | live search, delay, minimum letters, suggestion count, prefix fallback, fuzzy on/off / threshold / count, max articles, Enter → new tab, ignore case |
| Tarama penceresi | scanning on/off, Ctrl+C+C, every copy, selection (Linux), Ctrl+C+C interval, max text length, open near the mouse, always on top, hide when the mouse leaves + delay, size, record in history |
| Genel kısayollar | on/off, show/hide main window (Ctrl+F11), scanning on/off (Ctrl+F12), look up the clipboard (Ctrl+Shift+F11) |
| Sözlükler | dictionary folders scanned on start, recursive, reindex changed files |
| Ses | auto-play, volume, system player |
| Geçmiş ve Favoriler | history on/off, sidebar counts, database limit, clear on exit |

The window has a search box over all settings, ↺ reset per setting, page reset and reset all, a Shortcuts page and an Advanced page (data folder, rebuild indexes, check updates, clear history / favorites, export / import).

## Install (from source)

```bash
git clone https://github.com/bayramkotan/dictz.git
cd dictz
pip install -e .          # Python edition
dictz

cd dictz-rs
cargo run --release       # Rust edition
```

## Supported formats

| Format | Extension | Python edition | Rust edition |
|--------|-----------|----------------|--------------|
| StarDict | `.ifo` / `.idx` / `.dict(.dz)` | native | native |
| ABBYY Lingvo DSL | `.dsl` / `.dsl.dz` | [PyGlossary](https://github.com/ilius/pyglossary) | native (port of PyGlossary's reader) |
| Babylon | `.bgl` | [PyGlossary](https://github.com/ilius/pyglossary) | native (port of PyGlossary's reader) |

More formats (XDXF, Slob, DictD, MDict) are on the [TODO](TODO.md).

## Where to find dictionaries

- [FreeDict](https://freedict.org/) — StarDict dictionaries, many language pairs
- [StarDict.org](http://download.huzheng.org/) — large StarDict collection
- [Lingvo DSL packs](http://lingvo.helpmax.net/) — community DSL dictionaries

## Keyboard shortcuts

The same in both editions (Yardım → Kısayollar, F1):

| Shortcut | Action |
|---|---|
| Ctrl+F | Arama kutusuna odaklan |
| Enter | Ara (sonuç yoksa “Bunu mu demek istediniz?”) |
| Ctrl+Enter | Yeni sekmede ara |
| Ctrl+T | Yeni sekme |
| Ctrl+W | Sekmeyi kapat |
| Ctrl+Tab / Ctrl+Shift+Tab | Sonraki / önceki sekme |
| Alt+← / Alt+→ | Geri / ileri |
| Ctrl+R | Rastgele kelime |
| Ctrl+D | Favorilere ekle / çıkar |
| Ctrl+Shift+C | Kelimeyi kopyala |
| Ctrl+Shift+A | Makaleyi metin olarak kopyala |
| Ctrl+S | Makaleyi HTML olarak kaydet |
| Ctrl+Shift+F | Sayfada bul |
| F3 / Shift+F3 | Sonraki / önceki eşleşme |
| Ctrl+= / Ctrl+- / Ctrl+0 | Büyüt / küçült / sıfırla |
| Ctrl+B | Yan paneli göster/gizle |
| F11 | Tam ekran |
| Ctrl+O | Sözlük ekle |
| Ctrl+Shift+O | Klasör ekle |
| Ctrl+M | Sözlük yöneticisi |
| Ctrl+, | Tercihler |
| Ctrl+Q | Çıkış |

## Build portable binaries locally

See [`docs/PACKAGING.md`](docs/PACKAGING.md) for the full details. Quick version:

```bash
# Linux / macOS
bash scripts/build.sh

# Windows PowerShell
scripts\build.ps1
```

CI builds the same artifacts automatically on every `v*` tag.

## Development

```bash
pip install -e ".[dev]"
pytest -v                                  # Python edition

cd dictz-rs && cargo test                  # Rust edition (incl. parity tests)
python tests/data/parity/make_samples.py   # regenerate parity samples + golden files
```

## License

Licensed under the [GNU General Public License v3.0 or later](LICENSE).

## Author

[Bayram Kotan](https://bayramkotan.com) — [github.com/bayramkotan](https://github.com/bayramkotan)

---

## Türkçe

dictz, çoklu format destekli, çevrimdışı çalışan, çapraz platform bir sözlük uygulamasıdır. GoldenDict tarzı modern bir masaüstü aracıdır.

### İndir

Her sürüm için portable paketler [GitHub Releases](https://github.com/bayramkotan/dictz/releases/latest) sayfasında:

- **Windows 10/11** — `dictz-windows-x86_64.zip` → aç, `dictz.exe`'yi çalıştır
- **Linux** — `dictz-linux-x86_64.AppImage` → `chmod +x` ve çalıştır
- **macOS (Apple Silicon)** — `dictz-macos-arm64.zip`; Intel Mac için `pipx install dictz`

Veya PyPI: `pip install dictz` (veya `pipx install dictz`).

**Rust sürümü** — aynı özellikler, tek dosya: `cargo install dictz` veya Releases sayfasındaki `dictz-rs-*` arşivleri. İki sürüm aynı veri klasörünü kullanır; sözlükler, geçmiş ve favoriler ortaktır.

### Öne çıkanlar

- **Çoklu format** — StarDict, DSL, Babylon
- **Çoklu sözlük** — tek arama hepsinde çalışır
- **Hızlı** — SQLite FTS5 + diakritik duyarsız
- **Bulanık arama** — sonuç yoksa "Bunu mu demek istediniz?" önerileri
- **Resim ve ses** — DSL `[s]` dosyaları (`*.dsl.files/` veya `*.dsl.files.zip`)
- **Çevrimdışı** — ağ yok, telemetri yok
- **Modern arayüz** — sekmeler, sözlük çubuğu, "bulunduğu sözlükler" paneli, katlanır makaleler, sayfada bul, favoriler, geçmiş, koyu/açık tema, vurgu rengi
- **Tarama penceresi** — herhangi bir programda kelimeyi kopyalayın (Ctrl+C+C, her kopyalama ya da Linux'ta yalnızca seçmek) ve makale imlecin yanında açılsın; tepsi simgesi, genel kısayollar, tek kopya
- **Ayrıntılı ayarlar** — 9 sayfada 75 ayar; arama kutusu, ayar başına sıfırlama; iki sürüm aynı `settings.json` dosyasını kullanır
- **Tam komut satırı** — `dictz lookup`, `dicts`, `add`, `config`, `doctor`, **`dictz update`**, kabuk tamamlama… iki sürümde aynı (`dictz help`)
- **İki sürüm** — Python ve Rust, aynı özellikler, aynı sürüm numarası
- **Portable** — Windows/Linux/macOS için iki sürümün de binary'leri her release'te

### Lisans

[GNU General Public License v3.0 veya sonrası](LICENSE).
