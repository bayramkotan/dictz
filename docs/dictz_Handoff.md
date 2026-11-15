# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**PyPI:** https://pypi.org/project/dictz/
**Releases:** https://github.com/bayramkotan/dictz/releases
**Project path (Windows):** `C:\Github\dictz\`
**Project path (CachyOS):** `/home/bayram/Github/dictz/`
**License:** GPL-3.0+
**Language:** Python 3.10+ (CI tests 3.10/3.11/3.12)
**GUI:** PySide6 (+ QtWebEngine when available)

> **Not:** İsim şimdilik `dictz`. İlerleyen aşamalarda değişebilir; o gün gelirse paket/repo/handoff adlarını topluca güncelleriz.

---

## Current version

**v0.0.4** — Cross-platform portable builds (AppImage, Windows portable zip, macOS .app) + GitHub Actions release pipeline
**Next suggested version:** v0.0.5 — fuzzy arama (RapidFuzz) + DSL'deki ses/resim

---

## Bu Oturumda Yapılanlar (v0.0.4)

Portable paketleme ve yayın otomasyonu. Bayram Windows + Pardus + macOS için tek dosya binary'ler istedi; artık her `v*` tag push'unda GitHub Actions 4 platform için build yapıyor ve Release'e ekliyor.

### Eklenenler

- `packaging/pyinstaller/dictz.spec` — tek PyInstaller spec. PyGlossary plugin'lerinin tamamını `collect_submodules("pyglossary.plugins")` ile dahil ediyor; QtWebEngine + Network + PrintSupport'u hidden_imports'a ekliyor; QtQuick/QtCharts/QtMultimedia gibi kullanmadıklarımızı excludes ile çıkarıyor (bundle ~80 MB küçülüyor). macOS'ta `BUNDLE` ile Info.plist düzgün bir `.app` üretiyor.
- `packaging/appimage/build_appimage.sh` — PyInstaller `--onedir` çıktısını AppImage'a sarıyor. `linuxdeploy` yoksa indiriyor; `.desktop` ve ikon (python ile generated placeholder PNG) oluşturup `linuxdeploy --output appimage` ile tek dosya üretiyor.
- `scripts/build.sh` ve `scripts/build.ps1` — yerel one-command build. Linux'ta tarball + opsiyonel AppImage, macOS'ta .app zip, Windows'ta onedir zip üretir. CI ile aynı çıktıyı verir.
- `.github/workflows/release.yml` — tag matrix build (ubuntu-22.04, windows-latest, macos-13, macos-14). Her runner'da pip install → pyinstaller → platform-specific paketleme → artifact upload. `release` job artifact'ları topluyor, `softprops/action-gh-release@v2` ile GitHub Release oluşturuyor, auto-generated notes ekliyor.
- `.github/workflows/test.yml` — her push/PR'da Linux/macOS/Windows × Python 3.10/3.11/3.12 matrix (9 kombinasyon) testleri çalıştırıyor.
- `docs/PACKAGING.md` — build akışı, artifact tablosu, bilinen sınırlamalar (SmartScreen, Gatekeeper, glibc floor), gelecekteki iyileştirme adımları.
- `tests/test_packaging.py` — spec + script + workflow dosyalarının varlığını ve doğru entry point'leri işaret ettiğini doğrulayan 6 test.
- `.gitignore` — AppImage cache ekleri (`build/appimage-tools/`, `*.AppImage`).

### Değişenler

- `pyproject.toml` — `Releases` URL eklendi; version 0.0.4.
- `README.md` — en başa "Download" bölümü eklendi (her platform için artifact tablosu), Releases badge'i eklendi.
- About-box version string'leri 0.0.4.

### Doğrulama

24/24 test + 6 yeni packaging testi = **30/30**. Tüm Python dosyaları syntax-temiz.

### Tasarım kararları

- **PyInstaller `--onedir`**: Daha hızlı başlangıç, daha az antivirus false positive, daha az bellek. Tradeoff: tek dosya değil, klasör. Kullanıcı tarafında zip/tar/AppImage sarmalı ile tek dosya gibi görünüyor.
- **AppImage yerine Flatpak değil**: AppImage her Linux dağıtımında çalışır, kurulum gerektirmez. Flatpak sandbox avantajı olsa da kurulum + runtime yükü var.
- **macOS imzalanmamış**: Apple Developer Program $99/yıl, Bayram için henüz mantıklı değil. Kullanıcı ilk açışta right-click → Open.
- **Windows imzalanmamış**: Code signing certificate maliyetli. SmartScreen uyarısı alacak; "More info → Run anyway" ile geçilir. v0.1.0 roadmap'inde sertifika var.
- **Matrix build runners**: ubuntu-22.04 (glibc 2.35 floor, AppImage uyumu), windows-latest (2022), macos-13 (Intel), macos-14 (ARM64). macOS universal binary yerine iki ayrı build — binary ~50 MB daha küçük.
- **Tag format**: `v*` tetikliyor, prerelease işareti `alpha/beta/rc` içeren tag'lerde otomatik veriliyor.

### Build boyutları (CI'de gözlemlenecek)

- Linux tarball: ~90 MB sıkıştırılmış, ~250 MB açık
- Linux AppImage: ~95 MB tek dosya
- Windows zip: ~85 MB sıkıştırılmış, ~230 MB açık
- macOS zip (per arch): ~95 MB sıkıştırılmış, ~260 MB açık

(Qt + QtWebEngine + Chromium çoğunluğu alıyor. QtWebEngine olmadan ~120 MB'a kadar iniyordu — opsiyonel build v0.1.0 roadmap'te.)

---

## Önceki Oturumlarda Yapılanlar

### v0.0.3 — Functional UI overhaul (2026-11-13)

Modern GoldenDict-tarzı UI: sekmeler, dockable sidebar (Sözlükler/Favoriler/Geçmiş), back/forward navigation, koyu/açık tema, kalıcı favoriler + geçmiş, zoom, QWebEngineView renderer, klavye kısayolları, dictionary manager dialog. 11 UI modülü yeniden yazıldı. 24/24 test.

### v0.0.2 — DSL + Babylon (2026-11-13)

PyGlossary ile DSL (.dsl / .dsl.dz) ve Babylon (.bgl) parser'ları eklendi. `PyGlossaryDictionary` generic base — gelecek formatlar 3-4 satırla eklenebilir.

### v0.0.1 — Bootstrap (2026-11-12)

4 katmanlı mimari (UI/core/parsers/storage), native StarDict parser, SQLite FTS5 index, basic 3-pane UI.

---

## Bilinen Sınırlamalar / TODO

Tam liste: [`TODO.md`](../TODO.md).

Yakın sırada:
- v0.0.5: Fuzzy arama (RapidFuzz), DSL'deki ses/resim
- v0.0.6: TTS, pano izleme, global hotkey
- v0.0.7: Anki + ekran OCR
- v0.0.8: XDXF, Slob, DictD parser'ları
- v0.0.9: Dictionary groups, full-text search, Hunspell morfoloji
- v0.1.0: Panel'li preferences, shortcut editor, Windows code signing, macOS notarization

---

## Test Çalıştırma

```bash
pip install -e ".[dev]"
pytest -v
```

Beklenen: 30 test geçer.

---

## Release Çıkarma

Portable binary release'leri otomatik:

```bash
git tag v0.0.4
git push origin v0.0.4
```

GitHub Actions ~10 dakika içinde 5 artifact (Linux tarball + AppImage, Windows zip, macOS Intel zip, macOS ARM zip) üretip Release'e ekler.

PyPI release:

```bash
python -m build
twine upload dist/*
```

---

## Git Notes

- Commit mesajları **İngilizce**
- v0.0.4 commit başlığı: `v0.0.4: portable packaging (AppImage, Windows portable, macOS .app) + CI release pipeline`
- Tag: `v0.0.4`

---

## Deployment Durumu

- **GitHub repo**: https://github.com/bayramkotan/dictz (v0.0.3'te)
- **PyPI**: https://pypi.org/project/dictz/0.0.3/ (v0.0.3 canlı)
- **GitHub Releases**: v0.0.4 push edilince otomatik üretilecek
- **crates.io**: dictz Python; v1.0 sonrası `dictz-rs` için
