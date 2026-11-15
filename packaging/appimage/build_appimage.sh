#!/usr/bin/env bash
# Build dist/dictz-<version>-<arch>.AppImage from the PyInstaller --onedir build.
#
# Uses appimagetool, which only packs the AppDir — PyInstaller has already
# bundled every library, so no dependency deployment is needed.
#
# Usage:  bash packaging/appimage/build_appimage.sh [VERSION]
set -euo pipefail

cd "$(dirname "$0")/../.."

VERSION="${1:-$(sed -nE 's/^__version__ = "([^"]+)"/\1/p' dictz/__init__.py)}"
ARCH="${ARCH:-$(uname -m)}"
APPDIR="build/dictz.AppDir"
OUT="dist/dictz-${VERSION}-${ARCH}.AppImage"
TOOLS="build/appimage-tools"
APPIMAGETOOL="$TOOLS/appimagetool"

# appimagetool is itself an AppImage; extract-and-run avoids needing FUSE
export APPIMAGE_EXTRACT_AND_RUN=1

if [ ! -d dist/dictz ]; then
    echo "==> dist/dictz missing — running PyInstaller first"
    pyinstaller packaging/pyinstaller/dictz.spec --noconfirm --clean
fi

mkdir -p "$TOOLS"
if [ ! -x "$APPIMAGETOOL" ]; then
    echo "==> Downloading appimagetool"
    curl -fsSL -o "$APPIMAGETOOL" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
    chmod +x "$APPIMAGETOOL"
fi

echo "==> Laying out $APPDIR"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
cp -a dist/dictz/. "$APPDIR/usr/bin/"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/dictz" "$@"
EOF
chmod +x "$APPDIR/AppRun"

cat > "$APPDIR/dictz.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=dictz
GenericName=Offline Dictionary
Comment=Multi-format offline dictionary (GoldenDict-style)
Exec=dictz
Icon=dictz
Categories=Office;Dictionary;Education;
Terminal=false
X-AppImage-Version=${VERSION}
EOF

# 256x256 solid-blue placeholder icon (no binary assets in the repo yet)
python3 - "$APPDIR/dictz.png" <<'PY'
import struct, sys, zlib
w = h = 256
row = b"\x00" + bytes((59, 130, 246)) * w
def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(row * h, 9))
       + chunk(b"IEND", b""))
open(sys.argv[1], "wb").write(png)
PY
cp "$APPDIR/dictz.png" "$APPDIR/.DirIcon"

mkdir -p dist
echo "==> Packing $OUT"
ARCH="$ARCH" "$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUT"

echo "==> Built $OUT"
ls -lh "$OUT"
