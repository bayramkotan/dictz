# Packaging dictz

dictz ships three portable artifacts per release:

| Platform | Artifact | How it's built |
|----------|----------|----------------|
| **Linux** | `dictz-linux-x86_64.tar.gz` (PyInstaller --onedir tarball) | `pyinstaller packaging/pyinstaller/dictz.spec` + `tar czf` |
| **Linux** | `dictz-linux-x86_64.AppImage` (single-file executable) | Above + `packaging/appimage/build_appimage.sh` |
| **Windows** | `dictz-windows-x86_64.zip` (PyInstaller --onedir zip) | `pyinstaller packaging/pyinstaller/dictz.spec` + `Compress-Archive` |
| **macOS** | `dictz-macos-x86_64.zip` + `dictz-macos-arm64.zip` (.app bundles) | `pyinstaller packaging/pyinstaller/dictz.spec` + `zip -qr` |

All three come out of the same PyInstaller spec; the only platform-specific
piece is the final archive format and (on Linux) the AppImage wrapper.

## Automatic CI builds (recommended)

Every tag that matches `v*` triggers `.github/workflows/release.yml`, which:

1. Builds on `ubuntu-22.04`, `windows-latest`, `macos-13`, `macos-14` in parallel
2. Runs PyInstaller + the AppImage script on Linux
3. Uploads artifacts
4. Creates a GitHub Release with the artifacts attached and auto-generated
   release notes from commits since the previous tag

To cut a release:

```bash
git tag v0.0.4
git push origin v0.0.4
```

That's it — ~10 minutes later the four archives + one AppImage are on
<https://github.com/bayramkotan/dictz/releases>.

## Local builds

### Linux / macOS

```bash
bash scripts/build.sh
# outputs under dist/
# BUILD_APPIMAGE=0 bash scripts/build.sh    # skip AppImage
```

### Windows

```powershell
scripts\build.ps1
# outputs under dist/
```

## About the spec file

`packaging/pyinstaller/dictz.spec` is the one source of truth. It:

- Uses `--onedir` (not `--onefile`): faster startup, fewer antivirus false
  positives, easier to diff
- Pulls in every PyGlossary plugin via `collect_submodules("pyglossary.plugins")`
- Explicitly lists Qt WebEngine / Network / PrintSupport because PyInstaller's
  PySide6 hook doesn't always catch them
- Excludes unused Qt modules (Quick, QML, Charts, Multimedia, …) to shave
  ~80 MB off the final size
- Produces a `.app` bundle with proper `Info.plist` on macOS, including the
  bundle identifier and dark-mode flag

## Known build caveats

- **Linux distros older than Ubuntu 22.04** may lack new enough GLIBC/GCC
  runtime libs. The AppImage is built on 22.04 and targets glibc 2.35+.
- **Windows SmartScreen / antivirus**: because the artifact is not signed,
  Windows may warn the first time. Users can click "More info → Run anyway"
  or add an exception.
- **macOS code signing / notarization** is NOT done. Running the app for
  the first time will show the Gatekeeper warning; users control-click →
  Open to bypass once.
- **Size**: PyInstaller + PySide6 + QtWebEngine ≈ 220–280 MB uncompressed,
  80–120 MB compressed per platform. Most of it is Qt and Chromium.

## Future work

- Code signing on Windows (Authenticode) when a cert is available
- macOS notarization (requires Apple Developer account)
- Smaller build without QtWebEngine as an option (falls back to QTextBrowser)
- arm64 Linux builds (Raspberry Pi)
