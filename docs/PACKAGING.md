# Packaging and releasing dictz

Everything is driven by one workflow, `.github/workflows/ci.yml`.

| Event | What runs |
|---|---|
| pull request, or Actions → CI → **Run workflow** | lint · Python tests (Linux/Windows/macOS × 3.10–3.14) · Rust check + test (incl. parity tests) + library-only build + `--selftest` · wheel install smoke + `dictz --selftest` |
| push to `main` (no tag) | nothing — so a release (main + tag push) shows up as exactly **one** run |
| push a `v*` tag | all of the above, then **preflight** → publish to **PyPI** (Python edition) + **crates.io** (Rust edition) → build portable binaries of both editions (each runs `--selftest`) → **GitHub Release** |

## One-time setup

Add two repository secrets at **Settings → Secrets and variables → Actions → New repository secret**:

| Secret | Where to get it |
|---|---|
| `PYPI_API_TOKEN` | https://pypi.org/manage/account/token/ → Add API token → scope: project `dictz` |
| `CARGO_REGISTRY_TOKEN` | https://crates.io/settings/tokens → New Token → scopes `publish-new`, `publish-update` (the crates.io account needs a verified e-mail) |

If either secret is missing, the `preflight` job fails within seconds with a message naming it — nothing is half-published.

## Cutting a release

1. Bump the version in **three** places (preflight refuses a tag that does not match):
   - `pyproject.toml` → `version`
   - `dictz/__init__.py` → `__version__`
   - `dictz-rs/Cargo.toml` → `version`
2. Commit, then:
   ```bash
   git tag vX.Y.Z
   git push origin main
   git push origin vX.Y.Z
   ```

Re-pushing the same tag cancels the previous run. PyPI and crates.io uploads are skipped if that version already exists, so re-running a release is safe.

## Artifacts

| Edition | Platform | File |
|---|---|---|
| Python | Linux (any distro, glibc ≥ 2.35) | `dictz-linux-x86_64.AppImage` — `chmod +x`, run |
| Python | Linux | `dictz-linux-x86_64.tar.gz` — extract, run `dictz/dictz` |
| Python | Windows 10/11 | `dictz-windows-x86_64.zip` — extract, run `dictz\dictz.exe` |
| Python | macOS Apple Silicon | `dictz-macos-arm64.zip` — extract, right-click `dictz.app` → Open |
| Python | any | `pip install dictz` / `pipx install dictz` |
| Rust | Linux (glibc ≥ 2.35) | `dictz-rs-linux-x86_64.tar.gz` — extract, run `dictz-rs/dictz` |
| Rust | Windows 10/11 | `dictz-rs-windows-x86_64.zip` — extract, run `dictz-rs\dictz.exe` |
| Rust | macOS Apple Silicon | `dictz-rs-macos-arm64.tar.gz` — extract, run `dictz-rs/dictz` |
| Rust | any | `cargo install dictz` (app, needs ALSA headers on Linux) · `cargo add dictz` (library) |

Every artifact and install method also provides the short name **`dct`** (= `dictz`):

- pip/pipx: a second console script
- cargo: a second `[[bin]]`
- tar archives: a `dct → dictz` symlink
- Windows zips: a `dct.exe` copy (the PyInstaller copy shares `_internal\`)

CI runs `dct -V` on each build.

## Desktop shortcut

`dictz shortcut` (or Dosya → Masaüstü kısayolu oluştur) creates the shortcuts for the copy that is running. `--remove` deletes them, and `--desktop-only` / `--menu-only` limit them.

| Platform | Files |
|---|---|
| Linux | `$XDG_DATA_HOME/applications/dictz.desktop` + `<xdg-user-dir DESKTOP>/dictz.desktop` (trusted), icon `<data>/dictz.png` |
| Windows | `Desktop\dictz.lnk` + `Start Menu\Programs\dictz.lnk` (PowerShell `WScript.Shell`), icon `<data>\dictz.ico` |
| macOS | `~/Applications/dictz.app` (launcher script + `Info.plist` + `dictz.icns`) + a link on the desktop |

The AppImage shortcut points at `$APPIMAGE`, so it keeps working after `dictz update` replaces the file.

No Intel macOS binary: GitHub's Intel macOS runners do not start reliably for public repositories. Intel Mac users install with `pip`/`pipx`.

## Self-test

```bash
dictz --selftest      # exit 0 = OK (same as `dictz selftest`)
dictz --version
dictz doctor          # install kind, paths, index, history + selftest
```

`--selftest` loads a generated StarDict and DSL dictionary, checks the DSL and Babylon plugins, and creates an SQLite FTS5 index — without opening a window. CI runs it against every frozen binary, the AppImage, and the pip-installed console script. On Windows both `dictz.exe` builds are windowed programs; for CLI commands they attach to the console they were started from (Python: `AttachConsole` + `CONOUT$`, Rust: `windows-sys`), so `dictz lookup …` prints in cmd / PowerShell. For CI, `DICTZ_SELFTEST_REPORT=path\to\report.txt` still writes the selftest report to a file.

## Scan popup, tray and hotkeys — platform notes

| | Python edition | Rust edition |
|---|---|---|
| Clipboard / Ctrl+C+C | Qt clipboard signals | Windows: clipboard format listener · macOS: pasteboard change count · Linux X11: XFixes (x11rb) · otherwise polling |
| Selection scan (Linux) | Qt selection signal | primary selection polled (arboard) |
| Tray | QSystemTrayIcon | Linux: StatusNotifierItem over D-Bus (ksni) · Windows/macOS: tray-icon |
| Global hotkeys | Windows: RegisterHotKey (ctypes) · X11: XGrabKey (python-xlib) · macOS: pynput (`dictz[hotkeys]`) | global-hotkey (Windows, macOS, X11) |
| Wayland | no global hotkeys or "near the mouse"; bind desktop shortcuts to `dictz popup` / `show` / `watch toggle` | same; hide-to-tray minimizes |

No new system packages are needed to build either edition. A Linux tray needs a StatusNotifierItem host at runtime (KDE Plasma, GNOME with the AppIndicator extension, Xfce, …); without one dictz runs without the tray icon.

## Updating an installed copy

`dictz update` (both editions) detects how it was installed and updates it in place — nothing is moved:

| Edition | Installed with | `dictz update` runs |
|---|---|---|
| Python | pip in a venv / `--user` / system | `python -m pip install -U dictz` (system: with `sudo`, plus `--break-system-packages` on PEP 668 distros) |
| Python | pipx | `pipx upgrade dictz` |
| Python | editable checkout | nothing; tells you to `git pull` |
| Python | AppImage | downloads the new AppImage and replaces the running file |
| Python | portable zip / tar.gz | downloads the new archive to your Downloads folder and says where to extract it |
| Rust | `cargo install` | `cargo install dictz --locked --force` |
| Rust | portable binary | downloads the release archive and swaps the executable, and a sibling `dct` copy (Windows keeps `dictz.old` until the next start) |

`dictz update --check` only compares versions (PyPI or crates.io); `--yes` skips the question.

## Dictionary Center (v0.1.7; the Sözlükler window since v0.1.8) — what it needs

- **Python edition:** nothing new. Downloads use `urllib.request` (system certificates, `center.proxy` or the `*_proxy` variables), unpacking `zipfile` / `tarfile` / `lzma` / `bz2`, checksums `hashlib` — all standard library. `dictz/resources/languages.json` (ISO 639 table) ships with the other resources (`MANIFEST.in`, the PyInstaller spec's `resources/**/*`).
- **Rust edition:** the `center` feature (on by default; `cli` and `gui` turn it on) adds pure-Rust crates only — `ureq` (already used by `update`), `tar`, `regex`, `sha2`, `md-5`, `lzma-rs` (xz) and `bzip2` 0.6 with its default pure-Rust backend (`libbz2-rs-sys`, no C compiler or system libbz2). `src/center/languages.json` is built in with `include_str!` and listed in the crate's `include` — **without it the crates.io package does not build**.
- **Network:** both editions go online only when the Dictionary Center reads a source or downloads (and for `dct update`); `center.online` = false switches all of it off. The Rust edition trusts the bundled Mozilla roots (`webpki-roots`), the Python edition the system store.
- **Icons:** six new interface icons (`download`, `upload`, `link`, `server`, `center`, `license`) — SVGs in `dictz/resources/icons/`, PNGs for Rust from `make_samples.py`.
- **Tests:** `tests/test_center.py` (with a local HTTP server for downloads) and `dictz-rs/tests/center.rs` read recorded answers in `tests/data/center/` and must match `center_golden.json`, `licenses_golden.json`, `sources_golden.json` and `langs_golden.json` — no test reaches the internet.

## Suggestion thread (v0.1.8) — what it needs

- Nothing new to install. Python: `dictz/core/suggest.py` uses `threading` and a second, read-only `sqlite3` connection to the index (`file:…?mode=ro`); RapidFuzz runs there. Rust: `src/suggest.rs` uses `std::thread`, `mpsc` and a read-only `rusqlite` connection (`SQLITE_OPEN_READ_ONLY | NO_MUTEX`). Both are part of the core (no feature flag).
- One new interface icon, `enter` (SVG + PNG from `make_samples.py`).
- The built-in StarDict archive source is plain `http://` (the site has no HTTPS); packagers that block plain HTTP will see only that source fail.

## Parity between the editions

`tests/data/parity/` holds `sample.dsl` (+ `sample.dsl.files.zip`), `sample.bgl` and the golden files the Python edition produces from them (`*_golden.json`). `dictz-rs/tests/parity.rs` requires the Rust parsers to produce exactly the same headwords, definitions and metadata, and the same fuzzy scores; `tests/test_parity.py` checks the Python side still matches. Since v0.0.7 the same folder also pins the article-text conversion (`text_golden.json`), the settings schema (`settings_schema.json`), the CLI command list (`cli_commands.json`), the shell completion scripts (`completion_*.txt`, also copied to `dictz-rs/src/completion/`) the shortcuts (`shortcuts.json`) and, since v0.1.1, the exact text of the shared dictionary list `dictionaries.json` plus its unique-name and path-id rules (`registry_golden.json`), and an article store written by the Python edition (`articles_golden.db`, built from `sample.dsl`) that the Rust edition must read with its own parser's answers; since v0.1.7 also the Dictionary Center's adapters, licence table, built-in sources and language codes (`center_golden.json`, `licenses_golden.json`, `sources_golden.json`, `langs_golden.json`). After an intentional change, regenerate with `python tests/data/parity/make_samples.py`.

## Local builds

```bash
# Rust edition
cd dictz-rs && cargo build --release && ./target/release/dictz --selftest && cd ..

# Linux / macOS
pyinstaller packaging/pyinstaller/dictz.spec --noconfirm --clean
./dist/dictz/dictz --selftest                      # Linux
./dist/dictz.app/Contents/MacOS/dictz --selftest   # macOS
bash packaging/appimage/build_appimage.sh           # Linux AppImage → dist/

# Windows (PowerShell)
pyinstaller packaging\pyinstaller\dictz.spec --noconfirm --clean
```

## PyInstaller notes

- Entry point is `packaging/pyinstaller/launch_dictz.py` (absolute import). `dictz/__main__.py` uses a relative import and cannot be a PyInstaller entry script.
- PyGlossary discovers plugins by scanning `.py` files on disk, so the spec ships PyGlossary's sources as data (`collect_data_files("pyglossary", include_py_files=True)`), and `dictz.parsers.pyglossary_base` makes PyGlossary resolve its paths from the bundle.
- The macOS bundle version comes from `dictz/__init__.py`.
- The interface icons are `dictz/resources/icons/*.svg` (shipped by `MANIFEST.in` and the spec's `resources/**/*`); the Rust binary carries the same pictures as PNGs built in (`dictz-rs/src/uiicons`, `include_bytes!`), so neither edition needs icon files next to it.
- QtMultimedia is bundled (DSL sounds play in-app); without a multimedia backend sounds open in the system player.
- Binaries are unsigned: Windows SmartScreen and macOS Gatekeeper warn on first launch.
