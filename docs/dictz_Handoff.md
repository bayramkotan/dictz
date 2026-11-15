# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**PyPI:** https://pypi.org/project/dictz/
**crates.io:** https://crates.io/crates/dictz
**Releases:** https://github.com/bayramkotan/dictz/releases
**Project path (Windows):** `C:\Github\dictz\`
**Project path (CachyOS):** `/home/bayram/Github/dictz/`
**License:** GPL-3.0+
**Language:** Python 3.10+ (CI: 3.10–3.13) · Rust (dictz-rs crate)
**GUI:** PySide6 (+ QtWebEngine)

---

## Current version

**v0.0.5** — çalışan portable binary'ler + tek tag ile PyPI / crates.io / GitHub Releases yayını
**Next suggested version:** v0.0.6 — fuzzy arama (RapidFuzz) + DSL'deki ses/resim

### Yayın durumu (2026-11-15)

| Kanal | Durum |
|---|---|
| PyPI | 0.0.2, 0.0.3, 0.0.4 yayında — 0.0.5 tag push'u ile gelecek |
| crates.io | `dictz` adı boş, ilk yayın 0.0.5 tag push'u ile |
| GitHub Releases | boş — ilk release 0.0.5 tag push'u ile |

---

## Bu Oturumda Yapılanlar (v0.0.5)

### Bulunan ve düzeltilen gerçek hatalar

1. **Portable binary hiç açılmıyordu.** PyInstaller giriş noktası `dictz/__main__.py` idi; içindeki göreli import (`from .app import main`) donmuş exe'de `ImportError` veriyor. → `packaging/pyinstaller/launch_dictz.py` (mutlak import).
2. **Binary'de DSL/Babylon açılmıyordu.** PyGlossary donmuş modda eklenti ve veri klasörlerini exe'nin yanında arıyor; PyInstaller 6 ise `_internal/` (macOS: `Contents/Frameworks`) altına koyuyor. Ayrıca `plugins-meta/index.json`'ı kontrolsüz açıp çöküyordu. → `pyglossary_base._import_glossary_class()` PyGlossary'yi kökünü bundle'dan hesaplayacak şekilde içe aktarıyor; frozen modda `usePluginsJson=False`; spec eklenti `.py` kaynaklarını diske koyuyor (`include_py_files=True`).
3. **PyGlossary 4.x'te `formatName` yok** (`format`); yanlış anahtar sessizce yok sayılıp uzantıdan tahmin ediliyordu. → Runtime'da imzaya bakıp doğru anahtar seçiliyor.
4. **PyPI `invalid-publisher`.** Trusted Publishing PyPI sitesinde elle kayıt istiyordu. → API token (`PYPI_API_TOKEN` secret).
5. **crates.io'da hiçbir şey yoktu.** Token yokken publish job'ı sessizce atlıyordu. → `preflight` job token eksikse saniyeler içinde net hata veriyor.
6. **Release "durdu".** Intel macOS (macos-13) runner'ı public repo'da hiç atanmıyor, tüm release onu bekliyordu; ayrıca yeniden tag push'unda eski run kuyruğu tıkıyordu. → Intel macOS binary çıkarıldı, `cancel-in-progress: true`.

### Yeni

- `dictz --selftest` (GUI açmadan StarDict + DSL okur, DSL/Babylon eklentilerini ve FTS5'i doğrular) ve `dictz --version`.
- CI her platformda derlenen binary'yi, AppImage'ı ve pip ile kurulan `dictz` komutunu `--selftest` ile çalıştırıyor — çalışmayan binary yayınlanamaz.
- `preflight` job: secret'lar + tag/sürüm eşleşmesi (pyproject, `dictz/__init__.py`, `dictz-rs/Cargo.toml`).
- Sürüm tek kaynak: `dictz/__init__.py`. `app.py`, About kutusu ve macOS Info.plist oradan okuyor.
- `dictz-rs/` Rust crate iskeleti.

### Yerelde doğrulandı (CachyOS benzeri Linux, Python 3.13)

- PyInstaller build → `./dist/dictz/dictz --selftest` → 4/4 PASS
- AppImage (190 MB) → `--selftest` → 4/4 PASS
- Wheel → temiz venv'e kurulum → `dictz --version` + `dictz --selftest` → PASS
- `cargo publish --dry-run` → PASS
- 61 pytest, ruff temiz, 4 cargo test

Windows .exe ve macOS .app CI'de aynı selftest ile doğrulanacak.

---

## Tek seferlik kurulum (yayın için şart)

GitHub → https://github.com/bayramkotan/dictz/settings/secrets/actions → **New repository secret**

| Secret | Nereden |
|---|---|
| `PYPI_API_TOKEN` | https://pypi.org/manage/account/token/ → "Add API token" → Scope: Project `dictz` → `pypi-...` değeri |
| `CARGO_REGISTRY_TOKEN` | https://crates.io/settings/tokens → "New Token" → scope `publish-new` + `publish-update` (crates.io hesabında e-posta doğrulanmış olmalı) |

PyPI'deki Trusted Publisher kaydı ve GitHub'daki `pypi` / `crates-io` environment'ları **artık gerekmiyor** (silinebilir).

## Release akışı

```
git tag v0.0.X && git push origin v0.0.X
```
→ preflight → test + rust → PyPI + crates.io + 3 platform binary (her biri selftest) → GitHub Release

---

## pip ile kurulumda `dictz` komutu bulunamazsa

`pip install --user` / `--break-system-packages` komutu `~/.local/bin/dictz` olarak koyar. Bu klasör PATH'te değilse:
- fish: `fish_add_path ~/.local/bin`
- veya her zaman çalışır: `python -m dictz`
- önerilen: `pipx install dictz`

---

## Önceki Oturumlar (özet)

- **v0.0.4** — PyInstaller spec + AppImage betiği + release workflow (binary'ler açılmıyordu — v0.0.5'te düzeltildi)
- **v0.0.3** — Modern UI (sekmeler, sidebar, temalar, favoriler, geçmiş, QWebEngine)
- **v0.0.2** — DSL + Babylon (PyGlossary)
- **v0.0.1** — StarDict, SQLite FTS5, temel UI

---

## TODO (özet)

Tam liste: [`TODO.md`](../TODO.md).

- v0.0.6: Fuzzy arama, DSL ses/resim
- v0.0.7: TTS, pano izleme, global kısayol
- v0.0.8: Anki + OCR
- v0.0.9: XDXF, Slob, DictD
- v0.1.0: tercih panelleri, kısayol düzenleyici, Windows imzalama, macOS notarization, Intel macOS binary (Rosetta/universal2)
- Rust: v0.1.x StarDict → v0.2.x DSL+Babylon → v0.3.x FTS5+fuzzy → v1.0 parity

---

## Git Notes

- Commit mesajları **İngilizce**
- v0.0.5 commit başlığı: `v0.0.5: working portable binaries + one-tag release to PyPI, crates.io and GitHub Releases`
- Tag: `v0.0.5`
