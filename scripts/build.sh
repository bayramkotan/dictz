#!/usr/bin/env bash
# Local build — Linux / macOS.
#
# - Linux:  dist/dictz/ (--onedir) + dist/dictz-<ver>-<arch>.tar.gz + optional AppImage
# - macOS:  dist/dictz.app + dist/dictz-<ver>-macos.zip
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="$(sed -nE 's/^__version__ = "([^"]+)"/\1/p' dictz/__init__.py)"
ARCH="$(uname -m)"
OS="$(uname -s)"

echo "==> Building dictz $VERSION on $OS/$ARCH"

python -m pip install --upgrade --quiet pyinstaller
python -m pip install --upgrade --quiet -e .

rm -rf build/ dist/
pyinstaller packaging/pyinstaller/dictz.spec --noconfirm --clean

case "$OS" in
    Linux)
        cd dist && tar czf "dictz-${VERSION}-linux-${ARCH}.tar.gz" dictz/
        cd ..
        echo "==> Linux portable: dist/dictz-${VERSION}-linux-${ARCH}.tar.gz"
        if [ "${BUILD_APPIMAGE:-1}" = "1" ]; then
            bash packaging/appimage/build_appimage.sh "$VERSION"
        fi
        ;;
    Darwin)
        cd dist && zip -qr "dictz-${VERSION}-macos.zip" dictz.app/
        cd ..
        echo "==> macOS bundle: dist/dictz-${VERSION}-macos.zip"
        ;;
    *)
        echo "Unsupported OS: $OS" >&2
        exit 2
        ;;
esac

ls -lh dist/*.tar.gz dist/*.zip dist/*.AppImage 2>/dev/null || true
