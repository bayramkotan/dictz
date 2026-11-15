# dictz — Handoff

**Repo:** https://github.com/bayramkotan/dictz.git
**PyPI:** https://pypi.org/project/dictz/
**crates.io:** https://crates.io/crates/dictz
**Releases:** https://github.com/bayramkotan/dictz/releases
**Project path (Windows):** `C:\Github\dictz\`
**Project path (CachyOS):** `/home/bayram/Github/dictz/`
**License:** GPL-3.0+
**Language:** dictz is built **twice**, once in **Python** and once in **Rust** (details below)
**GUI (Python):** PySide6 (+ QtWebEngine, QtMultimedia) · **GUI (Rust):** egui / eframe 0.31 (+ rfd, rodio)

---

## ⚠️ Two editions: Python + Rust

This application is developed in **both Python and Rust**. Rust is not a side library or a "maybe later" idea. It becomes the same full application.

| | Python edition | Rust edition |
|---|---|---|
| Folder | `dictz/` | `dictz-rs/` |
| Package | PyPI `dictz` | crates.io `dictz` |
| Goal | full app (GUI) | full app (CLI + GUI), same features |
| Version | `dictz/__init__.py` | `dictz-rs/Cargo.toml`, **always the same number** |
| CI | `ci.yml` | `ci.yml` (same file, same tag) |

Rules:
- **Every feature is built in both editions in the same version.** Python must never get ahead of Rust again.
- Both editions must read dictionaries identically. `tests/data/parity/` holds hand-made `sample.dsl` (+ `sample.dsl.files.zip`) and `sample.bgl`, plus `*_golden.json` files with the Python edition's output. `dictz-rs/tests/parity.rs` (Rust) and `tests/test_parity.py` (Python) must both match them **byte for byte**. After an intentional change, regenerate them with `python tests/data/parity/make_samples.py` and commit.
- Both editions share **one data folder**: `~/.local/share/dictz`, `%APPDATA%\dictz`, `~/Library/Application Support/dictz`. They use the same `dictz_index.db` and `dictz_history.db` with the same schema. A dictionary, favorite or history entry added in one edition shows up in the other. Preferences are separate: Python uses QSettings, Rust uses `dictz-rs.ini`.
- Both run the same `--selftest` checks: StarDict, DSL, Babylon, FTS5, Fuzzy and Media. Python has one extra check, the PyGlossary plugins.
- **Status (v0.0.6): parity reached.** Rust has native StarDict/DSL/Babylon parsers, FTS5, history/favorites, a full GUI, fuzzy search and DSL media.

Rust code map (`dictz-rs/src/`):

| File | What it is | Python counterpart |
|---|---|---|
| `stardict.rs` | StarDict parser | `dictz/parsers/stardict.py` |
| `dsl.rs` | DSL parser, a port of PyGlossary `plugins/dsl` (lexer + title transformer) | PyGlossary |
| `babylon.rs` | BGL parser, a port of PyGlossary `plugins/babylon_bgl` | PyGlossary |
| `text.rs` | Python-compatible strip/escape/decode, PyGlossary entry filters, Persian clean-up | — |
| `wordtable.rs` | headwords; first one wins; `sorted(key=str.lower)` order | `_index` / `_sorted_words` |
| `index.rs`, `history.rs` | SQLite (rusqlite bundled, FTS5) | `dictz/storage/indexer.py`, `history.py` |
| `query.rs` | QueryManager: order, enable, restore, suggest, fuzzy | `dictz/core/query_manager.py` |
| `fuzzy.rs` | RapidFuzz `fuzz.ratio` (InDel/LCS), `extract`, `rank` | `dictz/core/fuzzy.py` |
| `media.rs` | DSL/StarDict media lookup | `dictz/core/media.py` |
| `html.rs` | article HTML → styled lines (for egui) | QtWebEngine |
| `gui/` | egui window: `app.rs` (window), `render.rs` (article), `fonts.rs`, `audio.rs` (rodio), `system.rs` | `dictz/ui/*` |
| `selftest.rs`, `main.rs` | `--selftest`, `--version`, `FILE…` | `dictz/selftest.py`, `dictz/app.py` |

---

## Current version

**v0.0.6**: the Rust edition is a full app at parity with Python; fuzzy "did you mean" search and DSL pictures/sounds in both editions.
**Next suggested version:** v0.0.7, TTS + clipboard watching + global hotkey (**in both editions**).

### Release status (2026-11-15)

| Channel | Status |
|---|---|
| PyPI | 0.0.5 (from CI). 0.0.6 comes with the v0.0.6 tag push. |
| crates.io | 0.0.5 (scaffold). 0.0.6, the first full app, comes with the tag push. |
| GitHub Releases | v0.0.5 has 4 files. v0.0.6 will have 7: the same 4 Python files plus `dictz-rs-linux-x86_64.tar.gz`, `dictz-rs-windows-x86_64.zip` and `dictz-rs-macos-arm64.tar.gz`. |

---

## ⚠️ READ THIS FIRST: Release / CI mistakes we have already made (do not repeat them)

Every item below was hit and fixed during the v0.0.4 → v0.0.6 releases. Read this whole list before touching the workflow or the packaging.

### A. Workflow structure

**A1. There must be ONE workflow file: `.github/workflows/ci.yml`.**
- Mistake: there were separate `test.yml` and `release.yml` files, so one push showed up as two different workflows in Actions.
- Rule: lint, tests, Rust, the smoke test, publishing, binaries and the GitHub Release all live in `ci.yml`. **Never** add a second `.yml`.
- `tests/test_packaging.py::test_single_workflow_file` catches this and fails if there is a second file.

**A2. A release must produce ONE run. ✅ Fixed in v0.0.6.**
- Mistake: the `on: push:` block in `ci.yml` had both `branches: [main]` and `tags: ['v*']`. A release does `git push origin main` and then `git push origin vX.Y.Z`. GitHub creates a **separate push event for each ref**, so the same commit got **two runs**: `CI #4 (main)` and `CI #5 (v0.0.5)`. One of them stalled, and it looked like "the workflow broke". In v0.0.5 we deleted the main run by hand.
- Fix (in `ci.yml` since v0.0.6):
  ```yaml
  on:
    push:
      tags: ['v*']          # NO branches: [main]
    pull_request:
      branches: [main]
    workflow_dispatch:
  ```
  `tests/test_packaging.py::test_ci_workflow_runs_once_per_release` asserts `triggers["push"] == {"tags": ["v*"]}`. PyYAML reads the `on` key as `True`, so the test uses `data.get("on", data.get(True))`.
- Result: a plain `git push origin main` triggers nothing, and a tag push gives one run. Tests can still be run by hand from Actions → CI → Run workflow. **Never add `branches:` back under `push:`.**
- GitHub uses the workflow file **of the commit being pushed**, so the main push of the v0.0.6 release already uses the new trigger and starts nothing.

**A3. The publish and binary jobs only run on a tag.**
- Every publish/build job has `if: startsWith(github.ref, 'refs/tags/v')`.
- Mistake: when we pushed only main, the publish jobs showed as "skipped" and we thought "it didn't publish". **Nothing is published without a tag push.**
- If the tag is already on the remote but points at the wrong commit:
  ```bash
  git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z
  git tag vX.Y.Z && git push origin vX.Y.Z
  ```

**A4. `concurrency: cancel-in-progress: true` must stay.**
- Mistake: with `false`, the old run blocked the queue every time the tag was re-pushed, and the new one waited "pending" forever.

### B. Runners

**B1. Never use `macos-13` (Intel).**
- Mistake: on public repos, the macos-13 runner often never gets assigned. The whole release waited on it and the screen said "hepsi durdu" (everything stalled).
- Rule: macOS is only `macos-latest` (Apple Silicon arm64). Intel Mac users use `pip`/`pipx`. If an Intel binary is ever wanted, the plan is universal2 (v0.1.0).
- `test_ci_workflow_never_uses_starved_intel_mac_runner` checks this. Careful: the test greps the text, so even writing "macos-13" in a **comment** fails it.

**B2. Do not use labels like `macos-14-arm64`.**
- Mistake: "The configuration 'macos-14-arm64-us-default' is not supported". Use `macos-latest`.

**B3. Linux tests need Qt system libraries.**
- Mistake: `libEGL.so.1: cannot open shared object file`. Install `libegl1 libgl1 libxkbcommon-x11-0 libxcb-*` and the like with apt, and set `QT_QPA_PLATFORM=offscreen`, `QTWEBENGINE_DISABLE_SANDBOX=1` and `QTWEBENGINE_CHROMIUM_FLAGS=--no-sandbox --disable-gpu`.
- pytest-qt was **removed**. It needs a display and adds nothing here.

### C. Publishing (PyPI / crates.io)

**C1. PyPI uses an API token, NOT Trusted Publishing (OIDC).**
- Mistake: `invalid-publisher: valid token, but no corresponding publisher`. Trusted Publishing needs a manual publisher registration on the PyPI site, which we never did.
- Rule: `pypa/gh-action-pypi-publish@release/v1` + `password: ${{ secrets.PYPI_API_TOKEN }}` + `skip-existing: true`. **No** `environment:` and **no** `id-token: write` (a test checks both).

**C2. crates.io uses the `CARGO_REGISTRY_TOKEN` secret.**
- Mistake: with no token, the job **silently** skipped and crates.io showed no project at all.
- Rule: the `preflight` job checks for both secrets and fails within seconds if either is missing. `publish-crates` checks the crates.io API first and skips if the version already exists, otherwise it runs `cargo publish`.
- Use a generic User-Agent in the crates.io API request (`dictz-ci`). **Never put a personal e-mail address in a UA, URL or script.**

**C3. Uploading to PyPI by hand is not allowed.**
- Mistake: 0.0.2–0.0.4 were uploaded with `twine upload` from a local machine. Later, when CI didn't publish, PyPI was stuck on 0.0.3/0.0.4.
- Rule: every channel is published **only from CI, by the tag**.

**C4. The version must match in THREE places.**
- `pyproject.toml` → `version`
- `dictz/__init__.py` → `__version__` (the single source: app.py, the About box and the macOS Info.plist read from here)
- `dictz-rs/Cargo.toml` → `version`
- `preflight` refuses any tag that doesn't match all three. `test_rust_version_matches_python` checks it too. `CHANGELOG.md` must have a `[X.Y.Z]` section (a test checks this).

**C5. `dictz-rs/Cargo.toml` `include` must not point outside the crate.**
- Mistake: the `../LICENSE` entry broke `cargo publish`. `include` lists only `src/**/*.rs`, `Cargo.toml` and `README.md`.

### D. Python dependencies

**D1. PyGlossary is pinned to `>=4.7.0,<5.0.0`.**
- Mistake: PyGlossary 5.x uses PEP 695 (`type X = ...`) syntax, which is a `SyntaxError` on Python 3.10/3.11.
- PyGlossary 4.x's `directRead` parameter is `format=`; in 5.x it is `formatName=`. A wrong keyword is **silently** ignored and the format is guessed from the extension. `pyglossary_base._format_kwarg()` reads the signature at runtime and picks the right one. Don't remove it.

### E. Portable binaries (PyInstaller / AppImage)

**E1. The entry point is `packaging/pyinstaller/launch_dictz.py`.**
- Mistake: when `dictz/__main__.py` was the entry, the relative import (`from .app import main`) raised `ImportError` in the frozen exe and **the binary never opened**.

**E2. PyGlossary in a frozen build.**
- Mistake: in frozen mode PyGlossary looks for its plugin/data folders next to the exe, but PyInstaller 6 puts them under `_internal/` (macOS: `Contents/Frameworks`). It also opened `plugins-meta/index.json` unchecked and crashed. Result: DSL/Babylon wouldn't open in the binary.
- Fixes (all three are required):
  1. The spec uses `collect_data_files("pyglossary", include_py_files=True)`, because PyGlossary scans plugin `.py` files **on disk**.
  2. `pyglossary_base._import_glossary_class()` temporarily hides `sys.frozen` during the first import, so PyGlossary resolves its root from `__file__` (inside the bundle).
  3. In frozen mode it calls `Glossary.init(usePluginsJson=False)`.

**E3. The macOS Info.plist version is not hardcoded.**
- Mistake: the plist said 0.0.4 for a long time. The spec reads `VERSION` from `dictz/__init__.py` with a regex.

**E4. No binary ships without a selftest.**
- `dictz --selftest`, without opening the GUI, reads StarDict + DSL, checks the DSL/Babylon plugins and checks FTS5 (exit 0 = OK).
- CI: the Linux binary, the AppImage, the macOS .app and the Windows exe (`Start-Process -Wait` + `DICTZ_SELFTEST_REPORT`, because the exe has no console) are all run with `--selftest`, and so is the `dictz` command from the pip wheel.

**E5. The AppImage is built with `appimagetool` and `APPIMAGE_EXTRACT_AND_RUN=1`** (there's no FUSE on CI). `build_appimage.sh` must be **executable** (`git update-index --chmod=+x` if needed). On Windows, NTFS doesn't carry the exec bit, so that test is skipped on Windows.

**E6. Wrapped long lines in YAML.** The download URL in the release body was split across two lines and broke. URLs in `ci.yml` must stay on a single line.

### F. User side

**F1. `pip install dictz` installs it, but the `dictz` command isn't found.**
- Cause: `pip install --user` / `--break-system-packages` puts the command in `~/.local/bin/dictz`, and that folder isn't on PATH.
- Fix:
  - fish: `fish_add_path ~/.local/bin`
  - or `python -m dictz`, which always works
  - recommended: `pipx install dictz`

**F2. Git conflicts.** Git pull/rebase in the local repo can hit add/add conflicts. Conflict markers left in `pyproject.toml` broke the build. After a rebase, run `grep -rn "<<<<<<<" .` to check.

---

### G. Rust edition (v0.0.6)

**G1. Rust audio needs the ALSA headers on Linux.**
- `rodio` → `cpal` → `alsa-sys`. Building without `libasound2-dev` + `pkg-config` fails with "alsa.pc not found".
- In CI, the `rust`, `publish-crates` and `build-rust-binaries` jobs install `libasound2-dev pkg-config` (a test checks this). `cargo publish` also **compiles** the crate, so the publish job needs them too.
- Users: Debian/Ubuntu `sudo apt install libasound2-dev pkg-config`; Arch/CachyOS `alsa-lib` (usually already installed). To build without sound: `cargo install dictz --no-default-features --features gui`.

**G2. `Cargo.lock` is committed and published.**
- It is a binary crate, so the lock file goes into the package (`include` lists `Cargo.lock`), and CI uses `--locked`.
- After adding or upgrading a dependency, run `cargo update -p <crate>` or `cargo build` and commit the new `Cargo.lock`. Otherwise CI fails with "lock file needs to be updated".

**G3. `dictz-rs/target/` must never be committed** (`.gitignore` covers it). It is gigabytes in size.

**G4. The Windows Rust exe has no console** (`windows_subsystem = "windows"`), like the Python exe.
- `--selftest` output therefore goes to `DICTZ_SELFTEST_REPORT=<file>`. CI uses `Start-Process -Wait` in pwsh, or `|| rc=$?` in bash. GitHub's bash runs with `-e`, so a plain `; rc=$?` would exit before the report is printed.

**G5. Parity tests read `../tests/data/parity`.**
- They are in the repo, not in the crate package (`include` = `src/**`, `Cargo.toml`, `Cargo.lock`, `README.md`), so `cargo publish` doesn't need them.
- `.gitattributes` marks `*.dsl *.bgl *.zip` as binary: a UTF-16 DSL changed by git's CRLF conversion would break the parity tests on Windows.

**G6. egui is pinned to 0.31.** The API changes between versions (`id_salt`, `Frame::NONE`, `corner_radius`, `Arc<FontData>`). Upgrading means a compile-and-fix round, so do it on its own, not inside another job.

**G7. Python-side issues fixed in v0.0.6.**
- QtWebEngine ignored link clicks; `_LinkPage.acceptNavigationRequest` now hands them to the window.
- The PyInstaller spec excluded QtMultimedia; it is now a hidden import.
- PyGlossary stores DSL headwords XML-escaped; they are now unescaped (`_unescape_terms`).
- Dictionaries were not reopened on start; `restore_from_index()` does it now.

---

## Release checklist (in this order)

1. Bump the version in three places (C4) and add a `[X.Y.Z]` section to `CHANGELOG.md`.
2. Locally: `pytest -q` (all green), `ruff check .`, and `cd dictz-rs && cargo test` (parity tests included)
3. `git commit` (title `vX.Y.Z: ...`) → `git tag vX.Y.Z`
4. `git push origin main` → `git push origin vX.Y.Z`
5. Actions: there is exactly one run, titled `vX.Y.Z` (A2).
6. If a job is red, paste its log. Don't fix things by hand (C3). Fix the cause, then re-push the tag (A3).
7. Check PyPI, crates.io and GitHub Releases. The release should have 7 files: 4 for Python and 3 for Rust.

---

## One-time setup (done)

GitHub → https://github.com/bayramkotan/dictz/settings/secrets/actions

| Secret | Where to get it |
|---|---|
| `PYPI_API_TOKEN` | https://pypi.org/manage/account/token/ → Scope: Project `dictz` |
| `CARGO_REGISTRY_TOKEN` | https://crates.io/settings/tokens → scope `publish-new` + `publish-update` |

You no longer need the Trusted Publisher entry on PyPI or the `pypi` / `crates-io` environments on GitHub.

---

## Done this session (v0.0.6)

- **Rust edition at parity.** The native DSL and BGL readers are line-by-line ports of PyGlossary's readers. The egui GUI has the same menus, shortcuts and Turkish labels as Python, plus drag-and-drop. Portable Rust binaries and the `dictz FILE…` argument were added.
- **Fuzzy search (both editions):**
  - The not-found page shows "Bunu mu demek istediniz?" with the matches and their %.
  - Suggestions are ranked closest-first, and fuzzy matches marked `≈` fill up the list when there are fewer than 10 prefix hits.
  - The score is RapidFuzz `fuzz.ratio` on lower-cased text. It was compared with RapidFuzz on 20,000 random pairs and matches every one exactly.
- **DSL media (both editions):** see the `media` modules for the lookup order. Pictures are shown inline (Python as `data:` URIs). Sounds have a ▶ button and play with QtMultimedia / rodio, falling back to the system player.
- Restore on start, dictionary order, the link-click fix and the DSL `&` headword fix (G7).
- The CI tags-only trigger (A2) and the Rust jobs (G1–G4).
- Verified locally:
  - 80 pytest, ruff clean, 50 cargo tests (46 unit + 4 parity)
  - `cargo publish --dry-run` PASS
  - Rust and Python selftests PASS
  - the Rust and Python GUIs were run under Xvfb (search, fuzzy page, media button, link clicks)
  - parity on real PyGlossary test files: Currency_In_Each_Country, Farsi_Aviation_Dictionary, Flavours_of_Malaysia and Solar_Physics_Glossary (`.bgl`), plus 4 `.dsl` files. Rust = Python on every headword, definition and metadata field.

## Earlier sessions (summary)

- **v0.0.5**: working portable binaries, `--selftest`, one-tag release with API tokens, Rust scaffold
- **v0.0.4**: PyInstaller spec + AppImage script + release workflow (the binaries didn't open; fixed in v0.0.5)
- **v0.0.3**: Modern UI (tabs, sidebar, themes, favorites, history, QWebEngine)
- **v0.0.2**: DSL + Babylon (PyGlossary)
- **v0.0.1**: StarDict, SQLite FTS5, basic UI

---

## TODO (summary)

Full list: [`TODO.md`](../TODO.md).

Every feature lands in **both** editions in the same version:

| Version | Both editions |
|---|---|
| v0.0.7 | TTS, clipboard watching, global hotkey |
| v0.0.8 | Anki + OCR |
| v0.0.9 | XDXF, Slob, DictD |
| v0.0.10 | dictionary groups, full-text search inside definitions, morphology |
| v0.1.0 | preference panels, shortcut editor, signing, notarization, Intel macOS (universal2) |

Rust-only follow-ups: right-to-left shaping for Arabic/Persian (egui draws it left-to-right), and maybe an AppImage for the Rust binary.

---

## Git Notes

- Commit messages are **in English**, long and detailed, with repeated `-m` flags (no `-F`).
- A version commit title is `vX.Y.Z: ...`. A plain push carries no version in its title, has no tag and doesn't bump the version.
- No Claude/Anthropic attribution anywhere (no commit trailer, no README, no metadata).
- Latest tag: `v0.0.6`
