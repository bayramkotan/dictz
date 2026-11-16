# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for dictz — cross-platform (Windows / Linux / macOS).

Build with:
    pyinstaller packaging/pyinstaller/dictz.spec --noconfirm

Output goes to dist/dictz/ (Windows + Linux) or dist/dictz.app (macOS).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

# Resolve project root from the spec file's own location
SPEC_DIR = Path(os.path.abspath(SPECPATH))
PROJECT_ROOT = SPEC_DIR.parent.parent

# Single source of truth for the version: dictz/__init__.py
import re as _re
VERSION = _re.search(
    r'^__version__ = "([^"]+)"',
    (PROJECT_ROOT / "dictz" / "__init__.py").read_text(encoding="utf-8"),
    _re.MULTILINE,
).group(1)

block_cipher = None

# Everything PyGlossary / Qt / SQLite needs that PyInstaller might miss
hidden_imports = [
    "pyglossary.glossary_v2",
    "pyglossary.plugins.stardict",
    "pyglossary.plugins.babylon_bgl",
    "pyglossary.plugins.dsl",
    "pyglossary.plugins.appledict_bin",
    "PySide6.QtWebEngineWidgets",
    "PySide6.QtWebEngineCore",
    "PySide6.QtWebChannel",
    "PySide6.QtNetwork",
    "PySide6.QtPrintSupport",
    # in-app playback of DSL sounds (dictz.ui.main_window._play_audio)
    "PySide6.QtMultimedia",
    "sqlite3",
    "rapidfuzz",
    "rapidfuzz.fuzz",
    "rapidfuzz.process",
]

# Pull in PyGlossary's entire plugin tree at runtime
from PyInstaller.utils.hooks import collect_submodules, collect_data_files
hidden_imports += collect_submodules("pyglossary.plugins")
# global hotkeys on Linux (dictz.core.hotkeys imports Xlib lazily)
if sys.platform.startswith("linux"):
    hidden_imports += collect_submodules("Xlib")

# PyGlossary discovers its format plugins by scanning the plugins directory on
# disk (pkgutil.iter_modules), so the plugin .py sources must exist as real
# files in the bundle — not only inside the PYZ archive. include_py_files=True
# ships them; dictz.parsers.pyglossary_base points PyGlossary at that folder.
datas = collect_data_files("pyglossary", include_py_files=True)
# interface languages and their flags (dictz/resources/i18n, dictz/resources/flags)
datas += collect_data_files("dictz", includes=["resources/**/*"])

# Strip imports we never use (kept small, don't trim essentials)
excludes = [
    "tkinter",
    "PySide6.QtQuick",
    "PySide6.QtQml",
    "PySide6.Qt3DCore",
    "PySide6.Qt3DRender",
    "PySide6.Qt3DAnimation",
    "PySide6.QtCharts",
    "PySide6.QtDataVisualization",
    "PySide6.QtMultimediaWidgets",
    "PySide6.QtSensors",
    "PySide6.QtPositioning",
    "PySide6.QtLocation",
    "PySide6.QtNfc",
    "PySide6.QtBluetooth",
    "PySide6.QtSerialPort",
    "PySide6.QtRemoteObjects",
    "PySide6.QtQuick3D",
    "PySide6.QtWebSockets",
    "matplotlib",
    "numpy",
    "scipy",
    "pandas",
    "IPython",
    "jupyter",
]

a = Analysis(
    # Absolute-import launcher; dictz/__main__.py uses a relative import that
    # fails when PyInstaller runs it as a top-level script.
    [str(SPEC_DIR / "launch_dictz.py")],
    pathex=[str(PROJECT_ROOT)],
    binaries=[],
    datas=datas,
    hiddenimports=hidden_imports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=excludes,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="dictz",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,          # upx breaks Qt libs on some platforms
    console=False,       # GUI app — no console window on Windows
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="dictz",
)

# macOS .app bundle
if sys.platform == "darwin":
    app = BUNDLE(
        coll,
        name="dictz.app",
        icon=None,
        bundle_identifier="com.bayramkotan.dictz",
        info_plist={
            "CFBundleName": "dictz",
            "CFBundleDisplayName": "dictz",
            "CFBundleIdentifier": "com.bayramkotan.dictz",
            "CFBundleVersion": VERSION,
            "CFBundleShortVersionString": VERSION,
            "NSHighResolutionCapable": True,
            "LSMinimumSystemVersion": "11.0",
        },
    )
