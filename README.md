# dictz

[![PyPI version](https://img.shields.io/pypi/v/dictz.svg)](https://pypi.org/project/dictz/)
[![Python versions](https://img.shields.io/pypi/pyversions/dictz.svg)](https://pypi.org/project/dictz/)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/bayramkotan/dictz)
[![Downloads](https://img.shields.io/pypi/dm/dictz.svg)](https://pypi.org/project/dictz/)
[![GitHub stars](https://img.shields.io/github/stars/bayramkotan/dictz?style=social)](https://github.com/bayramkotan/dictz)

Cross-platform, multi-format, offline dictionary application — a modern GoldenDict-style desktop tool built with Python and Qt.

> [Türkçe açıklama aşağıda](#türkçe)

## Highlights

- **Multi-format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Multi-dictionary** — load any number of dictionaries at once; a single query hits them all
- **Fast** — SQLite FTS5 index with Unicode + diacritic-insensitive search
- **Offline** — no network calls, no telemetry
- **Modern UI** — tabbed definitions, dockable sidebar (dictionaries / favorites / history), back/forward navigation, light + dark themes, configurable zoom and fonts
- **Cross-platform** — Windows 11, Linux (CachyOS/Arch/Debian/Ubuntu/Fedora), macOS

## Screens & features at a glance

- **Tabbed definition view** — open each lookup in its own tab (Ctrl+T / Ctrl+W / Ctrl+Tab)
- **Live suggestions** — debounced as-you-type, powered by FTS5 prefix matching
- **Back / Forward** — browser-style history navigation (Alt+Left / Alt+Right)
- **Favorites** — star words with Ctrl+D, review them in the sidebar
- **History** — every lookup logged, re-openable with one click, clearable with one button
- **Dictionary manager** — add / remove / reorder / enable / disable from a dialog or the sidebar
- **Zoom** — Ctrl+= / Ctrl+- / Ctrl+0
- **Themes** — Light, Dark, Auto (follows the OS)
- **Rich HTML** — QWebEngineView renders DSL markup with images and formatting

## Install

### From PyPI

```bash
pip install dictz
```

### From source

```bash
git clone https://github.com/bayramkotan/dictz.git
cd dictz
pip install -e .
```

### Run

```bash
dictz
# or equivalently
python -m dictz
```

## Supported formats

| Format | Extension | Parser |
|--------|-----------|--------|
| StarDict | `.ifo` / `.idx` / `.dict(.dz)` | native |
| ABBYY Lingvo DSL | `.dsl` / `.dsl.dz` | [PyGlossary](https://github.com/ilius/pyglossary) |
| Babylon | `.bgl` | [PyGlossary](https://github.com/ilius/pyglossary) |

More formats (XDXF, Slob, DictD, MDict, …) are on the [TODO](TODO.md).

## Where to find dictionaries

- [FreeDict](https://freedict.org/) — StarDict dictionaries, many language pairs
- [StarDict.org](http://download.huzheng.org/) — large StarDict collection
- [Lingvo DSL packs](http://lingvo.helpmax.net/) — ABBYY Lingvo community dictionaries

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

## Development

```bash
pip install -e ".[dev]"
pytest -v
```

Project layout:

```
dictz/
├── dictz/
│   ├── core/           # query orchestration, parser dispatch
│   ├── parsers/        # one module per dictionary format
│   ├── storage/        # SQLite FTS5 indexer, history, settings
│   ├── services/       # (planned) TTS, OCR, clipboard, Anki
│   ├── ui/             # PySide6 widgets — tabs, sidebar, themes
│   └── app.py          # entry point
├── tests/
├── docs/
├── CHANGELOG.md
├── TODO.md
└── pyproject.toml
```

## Contributing

Issues and pull requests are welcome at [github.com/bayramkotan/dictz](https://github.com/bayramkotan/dictz). See [TODO.md](TODO.md) for ideas.

## License

Licensed under the [GNU General Public License v3.0 or later](LICENSE).

## Author

[Bayram Kotan](https://bayramkotan.com) — [github.com/bayramkotan](https://github.com/bayramkotan)

---

## Türkçe

dictz, çoklu format destekli, çevrimdışı çalışan, çapraz platform bir sözlük uygulamasıdır. GoldenDict tarzı modern bir masaüstü aracıdır ve Python + Qt ile geliştirilmektedir.

### Öne çıkanlar

- **Çoklu format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Çoklu sözlük** — istediğin kadar sözlük yükle; tek bir arama hepsinde çalışır
- **Hızlı** — SQLite FTS5 indeksi + Unicode + diakritik duyarsız arama
- **Çevrimdışı** — hiçbir ağ çağrısı yok, telemetri yok
- **Modern arayüz** — sekmeli tanım görüntüleyici, yan panel (sözlükler/favoriler/geçmiş), ileri/geri gezinme, açık + koyu tema, yakınlaştırma, özelleştirilebilir yazı tipi
- **Çapraz platform** — Windows 11, Linux (CachyOS/Arch/Debian/Ubuntu/Fedora), macOS

### Kurulum

```bash
pip install dictz
```

### Çalıştırma

```bash
dictz
```

### Klavye kısayolları

| Eylem | Kısayol |
|-------|---------|
| Arama kutusuna odaklan | Ctrl+F |
| Mevcut sekmede aç | Enter |
| Yeni sekmede aç | Ctrl+Enter |
| Yeni sekme / Sekmeyi kapat | Ctrl+T / Ctrl+W |
| Önceki / sonraki sekme | Ctrl+Tab / Ctrl+Shift+Tab |
| Geri / İleri | Alt+← / Alt+→ |
| Favori aç/kapat | Ctrl+D |
| Yan paneli aç/kapat | Ctrl+B |
| Büyüt / küçült / sıfırla | Ctrl+= / Ctrl+- / Ctrl+0 |
| Tercihler | Ctrl+, |
| Sözlük ekle | Ctrl+O |
| Sözlük yöneticisi | Ctrl+M |

### Lisans

[GNU General Public License v3.0 veya sonrası](LICENSE).
