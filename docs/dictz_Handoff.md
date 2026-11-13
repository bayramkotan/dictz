# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**PyPI:** https://pypi.org/project/dictz/
**Project path (Windows):** `C:\Github\dictz\`
**Project path (Pardus):** `/home/bayram/Desktop/Github/dictz/`
**License:** GPL-3.0+
**Language:** Python 3.10+
**GUI:** PySide6

> **Not:** İsim şimdilik `dictz`. İlerleyen aşamalarda değişebilir; o gün gelirse paket/repo/handoff adlarını topluca güncelleriz.

---

## Current version

**v0.0.3** — Functional UI overhaul (tabs, sidebar, favorites, history, themes, QWebEngine)
**Next suggested version:** v0.0.4 — portable paketleme (AppImage, Windows portable zip, macOS .app)

---

## Dil/Platform Kararı (2026-11-12)

Python + PySide6 ile **v1.0'a kadar devam**. Rust port'u v1.0 sonrası ayrı bir proje olarak (`dictz-rs`) ele alınacak.

---

## Bu Oturumda Yapılanlar (v0.0.3)

İşlevsel UI overhaul. v0.0.2'nin minimal UI'ı tamamen değişti; GoldenDict tarzı modern bir masaüstü aracı iskeleti kuruldu.

### Yeni modüller

- `dictz/ui/themes.py` — elle ayarlanmış açık ve koyu QSS temaları + tanım paneli CSS'i. `auto` seçeneği sistem paletini okuyup karar veriyor.
- `dictz/ui/icons.py` — Qt standart ikonlarına isimli erişim (bundled icon set yok, her platformda çalışıyor).
- `dictz/ui/definition_view.py` — Tek bir tanım görüntüleyici. **QWebEngineView** varsa onu kullanıyor (zengin HTML, resim, CSS); yoksa QTextBrowser'a düşüyor.
- `dictz/ui/definition_tabs.py` — Sekmeli container, drag-to-reorder, close button, Ctrl+T / Ctrl+W.
- `dictz/ui/sidebar.py` — Sol tarafta QDockWidget. Üç sekme: Sözlükler (checkbox ile aktif/pasif), Favoriler, Geçmiş.
- `dictz/ui/dictionary_manager.py` — Sözlük ekle/kaldır/sırala/aktifleştir için ayrı dialog (Ctrl+M).
- `dictz/ui/preferences_dialog.py` — Tema, yazı tipi, yazı boyutu, debounce ayarı (Ctrl+,).
- `dictz/ui/main_window.py` — Tamamen yeniden yazıldı. Araç çubuğu + arama kutusu + öneriler + sekmeler + dock + status bar. Back/Forward navigasyon stack'i.
- `dictz/storage/history.py` — `HistoryStore`: SQLite'ta favoriler + geçmiş + not. `dictz_history.db` dosyası `dictz_index.db` ile aynı klasörde.
- `dictz/storage/settings.py` — `Preferences` dataclass'ı QSettings üzerinde typed arayüz; pencere geometrisi, dock durumu, zoom, son arama saklanıyor.

### Değişen modüller

- `dictz/storage/indexer.py` — `sort_order` sütunu eklendi; mevcut veritabanları `ALTER TABLE` ile upgrade ediliyor; `list_dictionaries` sıralı dönüyor; `set_sort_order` eklendi.
- `dictz/core/query_manager.py` — `enabled_names` seti takip ediliyor; `search()` ve `suggest()` sadece aktif sözlüklere gidiyor; `set_enabled()` + `is_enabled()`.
- `dictz/app.py` — HistoryStore açıyor, Preferences yüklüyor, hepsini MainWindow'a geçiriyor.
- `pyproject.toml` — `PySide6-Addons` eklendi (QtWebEngine için).

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
| Kelimeyi kopyala | Ctrl+Shift+C |

### Doğrulama

24/24 test geçti (8 stardict + 3 indexer + sort_order + 3 dsl + 6 parser_registry + 5 history).
Headless MainWindow construction testi başarılı: `QT_QPA_PLATFORM=offscreen` + `QTWEBENGINE_DISABLE_SANDBOX=1` ile pencere açılıyor, sidebar görünüyor, favoriler güncelliyor.

### Tasarım kararları

- **QWebEngineView + fallback**: Zengin HTML için QWebEngineView tercih edildi ama bazı PySide6 kurulumlarında (slim PyInstaller build'leri, bazı Linux repoları) yok. `HAS_WEBENGINE` bayrağıyla QTextBrowser'a otomatik düşüyoruz. Kullanıcı farkı görmez — sadece DSL'deki resimler/complex CSS kaybolur.
- **QSS elle ayarlı**: qt-material gibi üçüncü-parti tema paketi yok. Light ve Dark QSS doğrudan `themes.py` içinde. Bakım basit, bağımlılık yok, her platformda tutarlı.
- **Dock widget**: Kullanıcı sidebar'ı sağa taşıyabiliyor, floating yapabiliyor, kapatıp açabiliyor. QMainWindow'un hazır davranışı — kod yok.
- **QSettings ile ayarlar**: Windows'ta registry, macOS'ta plist, Linux'ta INI — her platformda doğru yer. Preferences dataclass hem typed erişim hem default değer sağlıyor.
- **Favoriler + geçmiş ayrı DB**: `dictz_history.db` indeks DB'sinden ayrı. İndeks her sözlük ekleme/çıkarmada yeniden yazılabilir, geçmiş ve favoriler kayıp olmaz.
- **Navigation stack**: `_nav_history` + `_nav_cursor`. Yeni bir kelime açınca forward history silinir (browser mantığı). `_applying_nav` bayrağı back/forward sırasında stack'e tekrar push edilmesini engelliyor.

---

## Bilinen Sınırlamalar / TODO (özet)

Tam liste: [`TODO.md`](../TODO.md).

Yakın sırada:
- v0.0.4: AppImage + Windows portable zip + macOS .app
- v0.0.5: Fuzzy arama (RapidFuzz), DSL'deki ses/resim
- v0.0.6: TTS, pano izleme, global hotkey
- v0.0.7: Anki + ekran OCR
- v0.0.8: XDXF, Slob, DictD parser'ları

---

## Test Çalıştırma

```bash
pip install -e ".[dev]"
pytest -v
```

Beklenen: 24 test geçer.

---

## Çalıştırma

```bash
pip install -e .
dictz
# veya:
python -m dictz
```

**Python 3.14 uyarısı:** PySide6 Python 3.14'te henüz tam destekli değil. Bayram'ın Windows'unda 3.14 kurulu ama PyPI'ye paketleme çalıştı (paket Python 3.10–3.12 hedefliyor). Uygulamayı çalıştırmak için Python 3.10–3.12 ile bir venv önerilir.

Test sözlüğü: **FreeDict** (https://freedict.org/) küçük StarDict dosyaları sağlar.

---

## Git Notes

- Commit mesajları **İngilizce**
- v0.0.3 commit başlığı: `v0.0.3: functional UI overhaul (tabs, sidebar, themes, favorites, history)`
- Tag: `v0.0.3`

## Deployment Notes

- **PyPI**: `python -m build && twine upload dist/*` (otomasyonsuz; API token ile)
- **GitHub**: push + tag yeter; GitHub Releases henüz manuel
- **crates.io**: dictz Python; crates.io Rust registry. v1.0 sonrası `dictz-rs` için.
