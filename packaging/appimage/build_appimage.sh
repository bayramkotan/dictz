#!/usr/bin/env bash
# Build dictz.AppImage from a PyInstaller --onedir build.
#
# Prerequisites:
#   - PyInstaller build already run (dist/dictz/ exists)
#   - linuxdeploy + linuxdeploy-plugin-appimage in PATH (downloaded below if missing)
#
# Usage:  bash packaging/appimage/build_appimage.sh [VERSION]
#         VERSION defaults to the one in dictz/__init__.py
set -euo pipefail

cd "$(dirname "$0")/../.."

VERSION="${1:-$(sed -nE 's/^__version__ = "([^"]+)"/\1/p' dictz/__init__.py)}"
ARCH="${ARCH:-$(uname -m)}"
APPDIR="build/dictz.AppDir"
OUT="dist/dictz-${VERSION}-${ARCH}.AppImage"
TOOLS="build/appimage-tools"

mkdir -p "$TOOLS"

# Download linuxdeploy if missing
if [ ! -x "$TOOLS/linuxdeploy" ]; then
    echo "==> Downloading linuxdeploy..."
    curl -fsSL -o "$TOOLS/linuxdeploy" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage"
    chmod +x "$TOOLS/linuxdeploy"
fi

# Ensure the PyInstaller build exists
if [ ! -d "dist/dictz" ]; then
    echo "==> Running PyInstaller build first..."
    pyinstaller packaging/pyinstaller/dictz.spec --noconfirm --clean
fi

# Lay out an AppDir pointing at the PyInstaller bundle
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

# Copy the entire --onedir tree under usr/bin
cp -R dist/dictz/. "$APPDIR/usr/bin/"

# A minimal .desktop
cat > "$APPDIR/usr/share/applications/dictz.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=dictz
GenericName=Offline Dictionary
Comment=Multi-format offline dictionary (GoldenDict-style)
Exec=dictz
Icon=dictz
Categories=Office;Dictionary;Education;
Terminal=false
StartupNotify=true
Keywords=dictionary;stardict;dsl;babylon;translation;
EOF

# A placeholder icon — a solid-color PNG so linuxdeploy accepts us
python3 - <<'PY'
import struct, zlib, os
w = h = 256
blue = bytes((59, 130, 246))  # #3B82F6
scanlines = b""
for _ in range(h):
    scanlines += b"\x00" + (blue * w)
def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
sig = b"\x89PNG\r\n\x1a\n"
ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
idat = zlib.compress(scanlines, 9)
png = sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
path = "build/dictz.AppDir/usr/share/icons/hicolor/256x256/apps/dictz.png"
os.makedirs(os.path.dirname(path), exist_ok=True)
open(path, "wb").write(png)
PY

export OUTPUT="$OUT"
export LINUXDEPLOY_OUTPUT_VERSION="$VERSION"

# linuxdeploy likes the output file named after the app
mkdir -p dist
"$TOOLS/linuxdeploy" \
    --appdir "$APPDIR" \
    --desktop-file "$APPDIR/usr/share/applications/dictz.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/dictz.png" \
    --output appimage \
    --custom-apprun <(cat <<'APPRUN_EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/dictz" "$@"
APPRUN_EOF
)

# linuxdeploy writes dictz-<version>-<arch>.AppImage into cwd; relocate
GENERATED=$(ls -1t *.AppImage | head -n 1 || true)
if [ -n "${GENERATED:-}" ] && [ -f "$GENERATED" ]; then
    mv -f "$GENERATED" "$OUT"
fi

echo "==> Built $OUT"
ls -lh "$OUT"
