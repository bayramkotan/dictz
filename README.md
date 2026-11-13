# dictz

[![PyPI version](https://img.shields.io/pypi/v/dictz.svg)](https://pypi.org/project/dictz/)
[![Python versions](https://img.shields.io/pypi/pyversions/dictz.svg)](https://pypi.org/project/dictz/)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/bayramkotan/dictz)
[![Downloads](https://img.shields.io/pypi/dm/dictz.svg)](https://pypi.org/project/dictz/)

Cross-platform, multi-format, offline dictionary application — a GoldenDict-style desktop tool built with Python and Qt.

> [Türkçe açıklama aşağıda](#türkçe)

![Platforms: Windows, Linux, macOS](https://img.shields.io/badge/runs_on-Windows_·_Linux_·_macOS-success)

## Highlights

- **Multi-format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Multi-dictionary** — load any number of dictionaries at once; a single query hits them all
- **Fast** — SQLite FTS5 index with Unicode + diacritic-insensitive search
- **Offline** — no network calls, no telemetry
- **Cross-platform** — one codebase runs on Windows 11, Linux (CachyOS/Arch/Debian/Ubuntu/Fedora), and macOS

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

More formats (XDXF, Slob, DictD, MDict, …) are planned — see the [roadmap](#roadmap).

## Where to find dictionaries

- [FreeDict](https://freedict.org/) — StarDict dictionaries, many language pairs
- [StarDict.org](http://download.huzheng.org/) — large StarDict collection
- [Lingvo DSL packs](http://lingvo.helpmax.net/) — ABBYY Lingvo community dictionaries

## Roadmap

- [x] **v0.0.1** — StarDict, multi-dictionary search, SQLite FTS5 index
- [x] **v0.0.2** — DSL + Babylon format support via PyGlossary
- [ ] Fuzzy / approximate matching (RapidFuzz)
- [ ] Rich HTML rendering (QWebEngineView) with images and audio
- [ ] Text-to-speech (edge-tts neural voices)
- [ ] Clipboard monitoring + global hotkey
- [ ] Screen-area OCR (Tesseract)
- [ ] Anki integration (one-click flashcards)
- [ ] Dictionary manager UI + preferences
- [ ] Dark mode
- [ ] Standalone binaries for Windows / Linux / macOS (PyInstaller)

## Development

```bash
# Install with dev extras
pip install -e ".[dev]"

# Run the test suite
pytest -v
```

Project layout:

```
dictz/
├── dictz/
│   ├── core/           # query orchestration, parser dispatch
│   ├── parsers/        # one module per dictionary format
│   ├── storage/        # SQLite FTS5 indexer
│   ├── services/       # (planned) TTS, OCR, clipboard, Anki
│   ├── ui/             # PySide6 widgets
│   └── app.py          # entry point
├── tests/
└── pyproject.toml
```

## Contributing

Issues and pull requests are welcome at [github.com/bayramkotan/dictz](https://github.com/bayramkotan/dictz).

## License

Licensed under the [GNU General Public License v3.0 or later](LICENSE).

## Author

[Bayram Kotan](https://bayramkotan.com) — [github.com/bayramkotan](https://github.com/bayramkotan)

---

## Türkçe

dictz, çoklu format destekli, çevrimdışı çalışan, çapraz platform bir sözlük uygulamasıdır. GoldenDict tarzı bir masaüstü aracıdır ve Python + Qt ile geliştirilmektedir.

### Öne çıkanlar

- **Çoklu format** — StarDict (`.ifo`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`), Babylon (`.bgl`)
- **Çoklu sözlük** — istediğin kadar sözlük yükle; tek bir arama hepsinde çalışır
- **Hızlı** — SQLite FTS5 indeksi + Unicode + diakritik duyarsız arama
- **Çevrimdışı** — hiçbir ağ çağrısı yok, telemetri yok
- **Çapraz platform** — Windows 11, Linux (CachyOS/Arch/Debian/Ubuntu/Fedora) ve macOS üzerinde aynı kod

### Kurulum

```bash
pip install dictz
```

### Çalıştırma

```bash
dictz
```

### Yol haritası

- [x] **v0.0.1** — StarDict format desteği, çoklu sözlükte arama, SQLite FTS5 indeksi
- [x] **v0.0.2** — DSL + Babylon format desteği (PyGlossary ile)
- [ ] Fuzzy arama (RapidFuzz)
- [ ] Tanımlar için zengin HTML görüntüleme
- [ ] Metin okuma (TTS, neural sesler)
- [ ] Pano izleme + global kısayol
- [ ] Ekran OCR (Tesseract)
- [ ] Anki entegrasyonu
- [ ] Sözlük yöneticisi UI + tercihler
- [ ] Koyu tema
- [ ] Windows / Linux / macOS tek dosya binary (PyInstaller)

### Lisans

[GNU General Public License v3.0 veya sonrası](LICENSE).
