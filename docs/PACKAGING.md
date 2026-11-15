# Packaging and releasing dictz

Everything is driven by one workflow, `.github/workflows/ci.yml`.

| Event | What runs |
|---|---|
| pull request, or Actions → CI → **Run workflow** | lint · Python tests (Linux/Windows/macOS × 3.10–3.13) · Rust check + test (incl. parity tests) + library-only build + `--selftest` · wheel install smoke + `dictz --selftest` |
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

No Intel macOS binary: GitHub's Intel macOS runners do not start reliably for public repositories. Intel Mac users install with `pip`/`pipx`.

## Self-test

```bash
dictz --selftest      # exit 0 = OK
dictz --version
```

`--selftest` loads a generated StarDict and DSL dictionary, checks the DSL and Babylon plugins, and creates an SQLite FTS5 index — without opening a window. CI runs it against every frozen binary, the AppImage, and the pip-installed console script. On Windows (`dictz.exe` has no console) set `DICTZ_SELFTEST_REPORT=path\to\report.txt` to get the report in a file.

## Parity between the editions

`tests/data/parity/` holds `sample.dsl` (+ `sample.dsl.files.zip`), `sample.bgl` and the golden files the Python edition produces from them (`*_golden.json`). `dictz-rs/tests/parity.rs` requires the Rust parsers to produce exactly the same headwords, definitions and metadata, and the same fuzzy scores; `tests/test_parity.py` checks the Python side still matches. After an intentional change, regenerate with `python tests/data/parity/make_samples.py`.

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
- QtMultimedia is bundled (DSL sounds play in-app); without a multimedia backend sounds open in the system player.
- Binaries are unsigned: Windows SmartScreen and macOS Gatekeeper warn on first launch.
