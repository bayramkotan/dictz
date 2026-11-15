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

Check any install with `dictz --selftest` (exit code 0 = dictionaries can be read).

## Highlights

- **Multi-format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Multi-dictionary** — load any number of dictionaries at once; a single query hits them all
- **Fast** — SQLite FTS5 index with Unicode + diacritic-insensitive search
- **Fuzzy search** — "did you mean" suggestions when nothing matches; suggestions ranked closest-first
- **Pictures and sounds** — DSL `[s]` media from `*.dsl.files/` or `*.dsl.files.zip`: pictures inline, sounds with one click
- **Offline** — no network calls, no telemetry
- **Modern UI** — tabbed definitions, dockable sidebar (dictionaries / favorites / history), back/forward navigation, light + dark themes, configurable zoom and fonts
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
- **Rich HTML** — QWebEngineView renders DSL markup with images and formatting

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

| Action | Shortcut |
|--------|----------|
| Focus search box | Ctrl+F |
| Open in current tab | Enter |
| Open in new tab | Ctrl+Enter |
| New tab / Close tab | Ctrl+T / Ctrl+W |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Back / Forward | Alt+← / Alt+→ |
| Toggle favorite | Ctrl+D |
| Toggle sidebar | Ctrl+B |
| Zoom in / out / reset | Ctrl+= / Ctrl+- / Ctrl+0 |
| Preferences | Ctrl+, |
| Add dictionary | Ctrl+O |
| Dictionary manager | Ctrl+M |
| Copy current word | Ctrl+Shift+C |

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
- **Modern arayüz** — sekmeler, dock, favoriler, geçmiş, koyu/açık tema
- **İki sürüm** — Python ve Rust, aynı özellikler, aynı sürüm numarası
- **Portable** — Windows/Linux/macOS için iki sürümün de binary'leri her release'te

### Lisans

[GNU General Public License v3.0 veya sonrası](LICENSE).
