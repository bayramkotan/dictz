# dictz

[![PyPI version](https://img.shields.io/pypi/v/dictz.svg)](https://pypi.org/project/dictz/)
[![Python versions](https://img.shields.io/pypi/pyversions/dictz.svg)](https://pypi.org/project/dictz/)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/bayramkotan/dictz/releases)
[![GitHub release](https://img.shields.io/github/v/release/bayramkotan/dictz?include_prereleases)](https://github.com/bayramkotan/dictz/releases)
[![Downloads](https://img.shields.io/pypi/dm/dictz.svg)](https://pypi.org/project/dictz/)
[![GitHub stars](https://img.shields.io/github/stars/bayramkotan/dictz?style=social)](https://github.com/bayramkotan/dictz)

Cross-platform, multi-format, offline dictionary application — a modern GoldenDict-style desktop tool built with Python and Qt.

> [Türkçe açıklama aşağıda](#türkçe)

## Download

Portable builds for every release live on the [GitHub Releases page](https://github.com/bayramkotan/dictz/releases/latest):

| Platform | Artifact | Notes |
|----------|----------|-------|
| **Windows 10/11** | `dictz-windows-x86_64.zip` | Unzip anywhere, run `dictz.exe`. Unsigned — accept the SmartScreen prompt. |
| **Linux (any)** | `dictz-linux-x86_64.AppImage` | `chmod +x dictz-*.AppImage && ./dictz-*.AppImage` |
| **Linux (tarball)** | `dictz-linux-x86_64.tar.gz` | Extract, run `dictz/dictz` |
| **macOS Apple Silicon** | `dictz-macos-arm64.zip` | Unzip, right-click **Open** the `.app` once to bypass Gatekeeper. |
| **macOS Intel** | — | `pip install dictz` / `pipx install dictz` |

Or install from PyPI: `pip install dictz` (or `pipx install dictz` on PEP-668 distros).
Rust crate: `cargo add dictz` ([crates.io](https://crates.io/crates/dictz)).

Check any install with `dictz --selftest` (exit code 0 = dictionaries can be read).

## Highlights

- **Multi-format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Multi-dictionary** — load any number of dictionaries at once; a single query hits them all
- **Fast** — SQLite FTS5 index with Unicode + diacritic-insensitive search
- **Offline** — no network calls, no telemetry
- **Modern UI** — tabbed definitions, dockable sidebar (dictionaries / favorites / history), back/forward navigation, light + dark themes, configurable zoom and fonts
- **Portable** — every release ships standalone binaries for Windows, Linux (AppImage + tar.gz) and macOS (Apple Silicon), each self-tested in CI before publishing

## Features at a glance

- **Tabbed definition view** — open each lookup in its own tab (Ctrl+T / Ctrl+W / Ctrl+Tab)
- **Live suggestions** — debounced as-you-type, powered by FTS5 prefix matching
- **Back / Forward** — browser-style history navigation (Alt+Left / Alt+Right)
- **Favorites** — star words with Ctrl+D, review them in the sidebar
- **History** — every lookup logged, re-openable with one click, clearable with one button
- **Dictionary manager** — add / remove / reorder / enable / disable
- **Zoom** — Ctrl+= / Ctrl+- / Ctrl+0
- **Themes** — Light, Dark, Auto
- **Rich HTML** — QWebEngineView renders DSL markup with images and formatting

## Install (from source)

```bash
git clone https://github.com/bayramkotan/dictz.git
cd dictz
pip install -e .
dictz
```

## Supported formats

| Format | Extension | Parser |
|--------|-----------|--------|
| StarDict | `.ifo` / `.idx` / `.dict(.dz)` | native |
| ABBYY Lingvo DSL | `.dsl` / `.dsl.dz` | [PyGlossary](https://github.com/ilius/pyglossary) |
| Babylon | `.bgl` | [PyGlossary](https://github.com/ilius/pyglossary) |

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
pytest -v
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

### Öne çıkanlar

- **Çoklu format** — StarDict, DSL, Babylon
- **Çoklu sözlük** — tek arama hepsinde çalışır
- **Hızlı** — SQLite FTS5 + diakritik duyarsız
- **Çevrimdışı** — ağ yok, telemetri yok
- **Modern arayüz** — sekmeler, dock, favoriler, geçmiş, koyu/açık tema
- **Portable** — Windows/Linux/macOS için tek dosya binaryler her sürümde

### Lisans

[GNU General Public License v3.0 veya sonrası](LICENSE).
