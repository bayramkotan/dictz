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

**v0.0.5**: working portable binaries, and one tag publishes to PyPI, crates.io and GitHub Releases.
**Next suggested version:** v0.0.6, fuzzy search (RapidFuzz) plus audio/images in DSL.

### Release status (2026-11-15)

The v0.0.5 tag run finished successfully. It was the second of two runs; the first one, triggered by the push to main, stalled and was deleted by hand.

| Channel | Status |
|---|---|
| PyPI | 0.0.5 published from CI (0.0.2–0.0.4 were uploaded by hand) |
| crates.io | `dictz` 0.0.5, the first release, published from CI |
| GitHub Releases | v0.0.5: AppImage, Linux tar.gz, Windows zip, macOS arm64 zip |

---

## ⚠️ READ THIS FIRST: Release / CI mistakes we have already made (do not repeat them)

Every item below was hit, and fixed, during the v0.0.4 → v0.0.5 releases. Before touching the workflow or packaging, read this whole list.

### A. Workflow structure

**A1. There must be ONE workflow file: `.github/workflows/ci.yml`.**
- Mistake: there were separate `test.yml` and `release.yml` files, so one push showed up as two different workflows in Actions.
- Rule: lint, tests, Rust, the smoke test, publishing, binaries and the GitHub Release all live in `ci.yml`. **Never** add a second `.yml`.
- `tests/test_packaging.py::test_single_workflow_file` catches this and fails if there is a second file.

**A2. A release must produce ONE run. ⚠️ STILL OPEN.**
- Mistake: the `on: push:` block in `ci.yml` has both `branches: [main]` and `tags: ['v*']`. A release does `git push origin main` and then `git push origin vX.Y.Z`. GitHub creates a **separate push event per ref**, so the same commit gets **two runs**: `CI #4 (main)` and `CI #5 (v0.0.5)`.
- Symptom: two runs side by side. With `cancel-in-progress` and a shared runner queue, one of them (usually the main one) stalls, and it looks like "the workflow broke".
- In v0.0.5 we cleaned up by hand: the main run was deleted and the tag run finished.
- **Fix, to be applied in the next version (v0.0.6):** make the trigger in `ci.yml` exactly this:
  ```yaml
  on:
    push:
      tags: ['v*']          # NO branches: [main]
    pull_request:
      branches: [main]
    workflow_dispatch:
  ```
  Also change `test_ci_workflow_runs_tag_and_push_triggered` in `tests/test_packaging.py` to `test_ci_workflow_runs_once_per_release`, which asserts `triggers["push"] == {"tags": ["v*"]}`. Note that PyYAML reads the `on` key as `True`, so use `data.get("on", data.get(True))`.
- Result: a plain `git push origin main` triggers nothing. A tag push gives one run. You can still run tests by hand from Actions → CI → Run workflow.
- Until then, during a release: **cancel or delete the run whose title says `main`.** The real release is the one whose title says `vX.Y.Z`.

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

## Release checklist (in this order)

1. Bump the version in three places (C4) and add a `[X.Y.Z]` section to `CHANGELOG.md`.
2. Locally: `pytest -q` (all green) and `ruff check .`
3. `git commit` (title `vX.Y.Z: ...`) → `git tag vX.Y.Z`
4. `git push origin main` → `git push origin vX.Y.Z`
5. Actions: follow only the run whose **title says `vX.Y.Z`**. Until A2 is applied, cancel the `main` run.
6. If a job is red, paste its log. Don't fix things by hand (C3). Fix the cause, then re-push the tag (A3).
7. Check: PyPI, crates.io, GitHub Releases (4 files).

---

## One-time setup (done)

GitHub → https://github.com/bayramkotan/dictz/settings/secrets/actions

| Secret | Where to get it |
|---|---|
| `PYPI_API_TOKEN` | https://pypi.org/manage/account/token/ → Scope: Project `dictz` |
| `CARGO_REGISTRY_TOKEN` | https://crates.io/settings/tokens → scope `publish-new` + `publish-update` |

You no longer need the Trusted Publisher entry on PyPI or the `pypi` / `crates-io` environments on GitHub.

---

## Done this session (v0.0.5)

- Fixes: portable binaries open (E1, E2); DSL/Babylon works in the binary; the PyGlossary 4.x keyword (D1); PyPI publishes with a token (C1); crates.io first release (C2); the stalled-release problem (B1, A4).
- New: `dictz --selftest` and `dictz --version`; the selftest runs on every binary in CI; the `preflight` job; the version has a single source; the `dictz-rs/` Rust crate scaffold.
- Verified locally: PyInstaller build + AppImage selftest 4/4 PASS, a wheel in a clean venv PASS, `cargo publish --dry-run` PASS, 61 pytest, ruff clean, 4 cargo tests.

---

## Earlier sessions (summary)

- **v0.0.4**: PyInstaller spec + AppImage script + release workflow (the binaries didn't open; fixed in v0.0.5)
- **v0.0.3**: Modern UI (tabs, sidebar, themes, favorites, history, QWebEngine)
- **v0.0.2**: DSL + Babylon (PyGlossary)
- **v0.0.1**: StarDict, SQLite FTS5, basic UI

---

## TODO (summary)

Full list: [`TODO.md`](../TODO.md).

- v0.0.6: **first, the A2 fix (tags-only trigger)**, then fuzzy search and DSL audio/images
- v0.0.7: TTS, clipboard watching, global hotkey
- v0.0.8: Anki + OCR
- v0.0.9: XDXF, Slob, DictD
- v0.1.0: preference panels, shortcut editor, Windows signing, macOS notarization, Intel macOS binary (universal2)
- Rust: v0.1.x StarDict → v0.2.x DSL+Babylon → v0.3.x FTS5+fuzzy → v1.0 parity

---

## Git Notes

- Commit messages are **in English**, long and detailed, with repeated `-m` flags (no `-F`).
- A version commit title is `vX.Y.Z: ...`. A plain push carries no version in its title, has no tag and doesn't bump the version.
- No Claude/Anthropic attribution anywhere (no commit trailer, no README, no metadata).
- Latest tag: `v0.0.5`
