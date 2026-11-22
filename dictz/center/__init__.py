"""The Dictionary Center: sources (servers), catalogs, downloads, installs,
import / export — Qt-free, shared by the window and the command line.

Same files and rules as the Rust edition (``dictz-rs/src/center/``):

* ``DATA/center/sources.json``      the servers / sources (built-in + the user's)
* ``DATA/center/credentials.json``  user names, passwords, tokens (never exported)
* ``DATA/center/cache/<id>.json``   each source's last catalog
* ``DATA/center/installed.json``    dictionaries installed from a catalog
* ``DATA/center/downloads.json``    download history
* ``DATA/dictionaries/<source>/<entry>/``  where downloads are installed

The catalog format (``dictz-catalog.json``) is described in ``docs/CATALOG.md``.
"""
from __future__ import annotations

CENTER_DIR = "center"
CATALOG_SCHEMA = 1
