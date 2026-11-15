# Packaging and releasing dictz

Everything is driven by one workflow, `.github/workflows/ci.yml`.

| Event | What runs |
|---|---|
| push to `main`, pull request | lint · Python tests (Linux/Windows/macOS × 3.10–3.13) · Rust check + test · wheel install smoke + `dictz --selftest` |
| push a `v*` tag | all of the above, then **preflight** → publish to **PyPI** + **crates.io** → build portable binaries (each runs `--selftest`) → **GitHub Release** |

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

| Platform | File |
|---|---|
| Linux (any distro, glibc ≥ 2.35) | `dictz-linux-x86_64.AppImage` — `chmod +x`, run |
| Linux | `dictz-linux-x86_64.tar.gz` — extract, run `dictz/dictz` |
| Windows 10/11 | `dictz-windows-x86_64.zip` — extract, run `dictz\dictz.exe` |
| macOS Apple Silicon | `dictz-macos-arm64.zip` — extract, right-click `dictz.app` → Open |
| any (Python) | `pip install dictz` / `pipx install dictz` |
| Rust | `cargo add dictz` |

No Intel macOS binary: GitHub's Intel macOS runners do not start reliably for public repositories. Intel Mac users install with `pip`/`pipx`.

## Self-test

```bash
dictz --selftest      # exit 0 = OK
dictz --version
```

`--selftest` loads a generated StarDict and DSL dictionary, checks the DSL and Babylon plugins, and creates an SQLite FTS5 index — without opening a window. CI runs it against every frozen binary, the AppImage, and the pip-installed console script. On Windows (`dictz.exe` has no console) set `DICTZ_SELFTEST_REPORT=path\to\report.txt` to get the report in a file.

## Local builds

```bash
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
- Binaries are unsigned: Windows SmartScreen and macOS Gatekeeper warn on first launch.
