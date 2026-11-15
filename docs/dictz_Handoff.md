# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**PyPI:** https://pypi.org/project/dictz/
**crates.io:** https://crates.io/crates/dictz
**Releases:** https://github.com/bayramkotan/dictz/releases
**Project path (Windows):** `C:\Github\dictz\`
**Project path (CachyOS):** `/home/bayram/Github/dictz/`
**License:** GPL-3.0+
**Language:** Python 3.10+ (CI tests 3.10–3.13) · Rust 1.75+ (dictz-rs crate)
**GUI:** PySide6 (+ QtWebEngine)

---

## Current version

**v0.0.5** — Unified release pipeline (PyPI + GitHub Releases + crates.io) + Rust crate scaffold
**Next suggested version:** v0.0.6 — fuzzy arama (RapidFuzz) + DSL'deki ses/resim

---

## Bu Oturumda Yapılanlar (v0.0.5)

Tag push'u her yere dağıtır. Daha önce `git push origin v0.0.X` sadece GitHub Releases'e artifact atıyordu; PyPI yüklemesi manuel idi, crates.io ise hiç yoktu. Şimdi `v*` tag push'u → PyPI + GitHub Releases + crates.io hepsi paralel + otomatik.

### Yeni

- **`dictz-rs/`** Rust crate iskeleti:
  - `Cargo.toml` — `name = "dictz"`, version Python ile aynı, GPL-3.0-or-later, keywords, docs.rs
  - `src/lib.rs` — `Dictionary` trait, `DictionaryEntry`, `DictionaryMetadata`, `DictzError` (`std::error::Error` impl), `VERSION` const. `#![forbid(unsafe_code)]`, dependency-free, 4 unit test
  - `README.md` — rozetler, status table, install snippet
- **`.github/workflows/ci.yml`** — **tek birleşik workflow**. Eski `test.yml` + `release.yml` silindi. 9 job, event'e göre şartlı çalışır:
  - Her push / PR'da: `lint` → `test` + `rust` → `install-smoke`
  - Sadece `v*` tag push'ta (ek olarak): `build-python-binaries`, `build-python-dist`, `publish-pypi`, `publish-crates`, `github-release`
  - Gate: `if: startsWith(github.ref, 'refs/tags/v')`
- `tests/test_packaging.py` — `test_single_workflow_file` + 8 renamed `test_ci_workflow_*` assertion'ı; sadece `ci.yml` olduğunu doğruluyor

### Versiyon bump

- `pyproject.toml` 0.0.4 → 0.0.5
- `dictz/__init__.py` __version__ → 0.0.5
- `dictz/app.py` __version__ → 0.0.5 (about-box)
- `dictz-rs/Cargo.toml` version → 0.0.5

### Tasarım kararları

- **PyPI Trusted Publishing**: API token yerine OIDC. Tek seferlik ayarlama: pypi.org/manage/account/publishing/ → Add publisher → GitHub repo bayramkotan/dictz, workflow release.yml, environment: pypi. Secret yönetimi yok, token sızıntı riski yok.
- **crates.io**: `CARGO_REGISTRY_TOKEN` GitHub repo Settings → Secrets → Actions'ta ayarlanmalı. Secret eksikse publish-crates job'ı warning ile skip eder (fail etmez).
- **Rust crate mono-repo**: Ayrı repo yerine aynı repoda `dictz-rs/` olarak tutuldu. Python ve Rust birlikte evrimleşiyor, version numarası lockstep (test ile enforced).
- **Dependency-free Rust scaffold**: v0.0.5'te thiserror vs. yok; manuel `fmt::Display` + `std::error::Error` impl. İlk parser (StarDict) geldiğinde flate2 + memchr eklenecek.
- **Rust CI'da test sadece unit testler**: integration + benchmark v0.1.x'te.

### Doğrulama

54/54 test geçti, ruff clean, iki YAML workflow geçerli.

---

## Tag push ile ne olur

```bash
git tag v0.0.5 && git push origin v0.0.5
```

**~10 dakika içinde:**
- https://pypi.org/project/dictz/0.0.5/ canlı
- https://crates.io/crates/dictz canlı (CARGO_REGISTRY_TOKEN set ise)
- https://github.com/bayramkotan/dictz/releases/tag/v0.0.5 → 5 artifact
- **Her dağıtım kanalı tek tag'den besleniyor, hepsi senkron**

---

## Ön koşul ayarları (tek seferlik)

### PyPI Trusted Publishing

1. https://pypi.org/manage/project/dictz/settings/publishing/ aç
2. "Add a new publisher" → GitHub Actions → owner `bayramkotan`, repo `dictz`, workflow `release.yml`, environment `pypi`
3. Save. API token'a gerek yok.

### crates.io token

1. https://crates.io/me (GitHub ile giriş)
2. Account Settings → API Tokens → New Token → scope "publish-update"
3. GitHub repo → Settings → Secrets and variables → Actions → New repository secret → `CARGO_REGISTRY_TOKEN` = token

Her ikisi set edilince `git push origin v0.0.5` üç yere birden yayın yapar.

---

## Önceki Oturumlar (özet)

- **v0.0.4** — Portable paketler (AppImage, Windows portable, macOS .app) + PyInstaller spec + matrix release pipeline
- **v0.0.3** — Modern UI overhaul (sekmeler, sidebar, temalar, favoriler, geçmiş, QWebEngine)
- **v0.0.2** — DSL + Babylon format desteği (PyGlossary)
- **v0.0.1** — StarDict native parser, SQLite FTS5 index, basic UI

---

## TODO (özet)

Tam liste: [`TODO.md`](../TODO.md).

- v0.0.6: Fuzzy arama (RapidFuzz), DSL ses/resim
- v0.0.7: TTS, pano izleme, global hotkey
- v0.0.8: Anki + OCR
- v0.0.9: XDXF, Slob, DictD parser'ları
- v0.1.0: UI polish (panel'li tercihler, shortcut editor), Windows code signing, macOS notarization

Rust tarafı:
- v0.1.x (dictz-rs): StarDict parser
- v0.2.x: DSL + Babylon
- v0.3.x: SQLite FTS5 + fuzzy
- v1.0 (dictz-rs): Python edition ile feature parity

---

## Git Notes

- Commit mesajları **İngilizce**
- v0.0.5 commit başlığı: `v0.0.5: unified release pipeline (PyPI + GitHub Releases + crates.io) + Rust crate scaffold`
- Tag: `v0.0.5`
