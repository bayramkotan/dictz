# dictz

Cross-platform, multi-format offline dictionary application — a GoldenDict-style desktop tool built with Python and Qt.

> [Türkçe açıklama aşağıda](#türkçe)

## Features (roadmap)

- ✅ **v0.0.1** — StarDict format support, multi-dictionary search, SQLite FTS5 index
- 🚧 **v0.2** — DSL (Lingvo) and Babylon (.bgl) format support via PyGlossary
- 🚧 **v0.3** — Fuzzy / approximate matching with RapidFuzz
- 🚧 **v0.4** — Rich HTML rendering for definitions (QWebEngineView)
- 🚧 **v0.5** — Text-to-speech (edge-tts neural voices)
- 🚧 **v0.6** — Clipboard monitoring + global hotkey
- 🚧 **v0.7** — Screen-area OCR (Tesseract)
- 🚧 **v0.8** — Anki integration (one-click flashcards)
- 🚧 **v0.9** — Dictionary manager UI + preferences
- 🚧 **v1.0** — PyInstaller packaging for Windows / Linux / macOS

## Quick start

```bash
# Clone
git clone https://github.com/bayramkotan/dictz.git
cd dictz

# Install (Python 3.10+)
pip install -e .

# Run
dictz
# or
python -m dictz
```

## Project structure

```
dictz/
├── dictz/
│   ├── core/           # query orchestration, fuzzy matching, cache
│   ├── parsers/        # one module per dictionary format
│   ├── storage/        # SQLite FTS5 indexer
│   ├── services/       # TTS, OCR, clipboard, Anki bridge
│   ├── ui/             # PySide6 widgets
│   └── app.py          # entry point
├── tests/
└── pyproject.toml
```

## Supported formats

| Format | Extension | Status | Parser |
|--------|-----------|--------|--------|
| StarDict | `.ifo` / `.idx` / `.dict(.dz)` | ✅ v0.0.1 | native |
| DSL (Lingvo) | `.dsl` | 🚧 v0.2 | PyGlossary |
| Babylon | `.bgl` | 🚧 v0.2 | PyGlossary |

## Running tests

```bash
pip install -e ".[dev]"
pytest
```

## License

GPL-3.0 or later — see `LICENSE`.

---

## Türkçe

dictz, çoklu format destekli, çevrimdışı çalışan, çapraz platform bir sözlük uygulamasıdır. GoldenDict tarzı bir masaüstü aracıdır ve Python + Qt ile geliştirilmektedir.

### Özellikler (yol haritası)

- ✅ **v0.0.1** — StarDict format desteği, çoklu sözlükte arama, SQLite FTS5 indeksi
- 🚧 **v0.2** — DSL (Lingvo) ve Babylon (.bgl) desteği
- 🚧 **v0.3** — RapidFuzz ile fuzzy arama
- 🚧 **v0.4** — Tanımlar için zengin HTML görüntüleme
- 🚧 **v0.5** — Metin okuma (TTS, neural sesler)
- 🚧 **v0.6** — Pano izleme + global kısayol
- 🚧 **v0.7** — Ekran OCR (Tesseract)
- 🚧 **v0.8** — Anki entegrasyonu
- 🚧 **v0.9** — Sözlük yöneticisi + tercihler
- 🚧 **v1.0** — Windows / Linux / macOS paketleri

### Hızlı başlangıç

```bash
git clone https://github.com/bayramkotan/dictz.git
cd dictz
pip install -e .
dictz
```
