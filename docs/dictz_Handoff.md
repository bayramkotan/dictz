# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**Project path (Windows):** `C:\Github\dictz\`
**Project path (Pardus):** `/home/bayram/Desktop/Github/dictz/`
**License:** GPL-3.0+
**Language:** Python 3.10+
**GUI:** PySide6

> **Not:** İsim şimdilik `dictz`. İlerleyen aşamalarda değişebilir; o gün gelirse paket/repo/handoff adlarını topluca güncelleriz.

---

## Current version

**v0.0.1** — Bootstrap (UI iskeleti + StarDict parser + SQLite FTS5 indeks)
**Next suggested version:** v0.0.2 (manuel testten gelen düzeltmeler / küçük iyileştirmeler)

---

## Dil/Platform Kararı (2026-11-12)

**Karar:** Python + PySide6 ile **v1.0'a kadar devam**. Rust port'u v1.0 sonrası ayrı bir proje olarak (`dictz-rs`) ele alınacak — karşılaştırma + öğrenme amaçlı, ikisini paralel sürdürmeyeceğiz.

**Değerlendirilen seçenekler:**
- Tam Rust pivot: reddedildi (GUI ekosistemi Qt kadar olgun değil; Babylon .bgl ters mühendislik gerektiren kötü bir format, PyGlossary yıllarca uğraşmış; diğer tüm projeler PySide6)
- Hibrit (Rust core + Python GUI, PyO3): şimdilik ertelendi; gerçek bir darboğaz görülürse spesifik parça için yeniden değerlendirilir
- Paralel iki uygulama: reddedildi (VenvStudio Pro Ekim'de aynı sebeple bırakılmıştı — tek public repo tercihi)

**Performans not:** dictz boyutunda projelerde Python darboğaz değil — sıcak noktaların hepsi zaten C/C++'ta (SQLite, rapidfuzz, Qt). Benchmark'la kanıtlanmış bir sorun çıkarsa o parça PyO3 ile Rust'a alınır.

---

## Bu Oturumda Yapılanlar (v0.0.1)

İlk iskelet sıfırdan kuruldu. İsim önce Lexora olarak seçildi, sonra `dictz` olarak değiştirildi; tüm paket/modül/yol/ID alanları topluca güncellendi. Versiyon başlangıçta 0.1.0 olarak yazılmıştı, ardından 0.0.1'e indirildi (daha alçakgönüllü başlangıç işareti). Oturum sonunda Rust'a geçiş değerlendirildi — Python'da kalma kararı verildi (yukarı bak).

### Mimari (4 katman)

1. **UI** (`dictz/ui/`) — PySide6, üç bölmeli ana pencere
2. **Çekirdek** (`dictz/core/`) — sorgu yöneticisi, paralel arama orkestrasyonu
3. **Parserlar** (`dictz/parsers/`) — her format için ayrı modül, ortak `Dictionary` ABC
4. **Depolama** (`dictz/storage/`) — SQLite FTS5 ile merkezi indeks

### Eklenenler

- `pyproject.toml` — setuptools build, opsiyonel bağımlılık grupları (`parsers`, `tts`, `ocr`, `anki`, `dev`, `all`)
- `dictz/parsers/base.py` — `Dictionary` ABC + `DictionaryEntry` + `DictionaryMetadata` dataclass'ları
- `dictz/parsers/stardict.py` — Native StarDict parser (.ifo/.idx/.dict + .dict.dz). PyGlossary bağımlılığı yok.
  - 32-bit ve 64-bit `idxoffsetbits` destekliyor
  - gzip ile `.dict.dz` açıyor
  - case-insensitive arama + prefix arama
- `dictz/storage/indexer.py` — `IndexStore` (SQLite FTS5), unicode61 + remove_diacritics
- `dictz/core/query_manager.py` — `QueryManager`, `parser_for()` dispatch, çoklu sözlük arama
- `dictz/ui/main_window.py` — Splitter ile 3-pane UI, debounced search (150ms), live preview, dosya menüsü, status bar
- `dictz/app.py` — Cross-platform user data dir (Win/macOS/Linux), uygulama girişi
- `dictz/__main__.py` — `python -m dictz` desteği
- `tests/test_stardict.py` — 7 test (load, lookup exact/case/prefix/missing, iter, context manager)
- `tests/test_indexer.py` — 3 test (register, index+search, remove)
- `README.md` — EN + TR
- `.gitignore` — Python + IDE + PyInstaller

### Doğrulama

11/11 test geçti, tüm Python dosyaları syntax-temiz (~1280 satır).

### Tasarım kararları

- **PyGlossary YOK v0.0.1'de**: StarDict'i native parse ediyoruz. PyGlossary sonraki sürümde DSL/Babylon için eklenecek.
- **FTS5 sadece headword'leri tutuyor**: Tanımlar parser üzerinden on-demand fetch ediliyor. Bu, büyük sözlüklerde bile indeks DB'sini küçük tutuyor.
- **HTML render QTextBrowser ile**: İleride QWebEngineView'a geçilecek (zengin HTML + CSS + JS için).
- **Debounce 150ms**: Hızlı yazımda her karakterde search atılmasını engelliyor.
- **User data dir cross-platform**: Win=`%APPDATA%\dictz`, macOS=`~/Library/Application Support/dictz`, Linux=`$XDG_DATA_HOME/dictz`.

---

## Bilinen Sınırlamalar / TODO

- [ ] **`.syn` synonym file** parse edilmiyor (StarDict opsiyonel özellik)
- [ ] **`sametypesequence`** yorumlanmıyor (HTML / plain / etc. tag'leri)
- [ ] **Encoding fallback**: UTF-8 dışı `.ifo` dosyaları için latin-1 fallback yok
- [ ] **Fuzzy arama** (RapidFuzz)
- [ ] **Paralel sözlük arama** (QThreadPool — şu an sequential)
- [ ] **DSL + Babylon parser** (PyGlossary)
- [ ] **Rich HTML render** (QWebEngineView)
- [ ] **TTS** (edge-tts neural sesler)
- [ ] **Pano izleme + global hotkey**
- [ ] **Ekran OCR** (Tesseract)
- [ ] **Anki entegrasyonu**
- [ ] **Sözlük yöneticisi UI + tercihler + tema**
- [ ] **v1.0 sonrası**: `dictz-rs` ayrı Rust port projesi (karşılaştırma + öğrenme)

---

## Test Çalıştırma

```bash
pip install -e ".[dev]"
pytest -v
```

Beklenen: 11 test geçer (8 stardict + 3 indexer).

---

## Çalıştırma

```bash
pip install -e .
dictz
# veya:
python -m dictz
```

Test sözlüğü için **FreeDict** (https://freedict.org/) önerilir — küçük StarDict dosyaları sağlar.

---

## Git Notes

- Commit mesajları **İngilizce**
- İlk commit önerisi: `feat: bootstrap dictz v0.0.1 with StarDict support`
- Tag: `v0.0.1`
- Remote: `git remote add origin https://github.com/bayramkotan/dictz.git`
