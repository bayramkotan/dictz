"""dictz application entry point.

    dictz              launch the GUI
    dictz --version    print the version and exit
    dictz --selftest   headless check that dictionaries can be read; exit 0/1
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

from . import __version__  # single source of truth: dictz/__init__.py

APP_NAME = "dictz"


def user_data_dir() -> Path:
    if sys.platform == "win32":
        base = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
        return base / APP_NAME
    elif sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / APP_NAME
    else:
        xdg = os.environ.get("XDG_DATA_HOME")
        base = Path(xdg) if xdg else Path.home() / ".local" / "share"
        return base / APP_NAME.lower()


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv

    # Non-GUI commands are handled before Qt is imported.
    if "--version" in args or "-V" in args:
        print(f"dictz {__version__}")
        return 0
    if "--selftest" in args:
        from .selftest import run

        return run()

    from PySide6.QtWidgets import QApplication

    from .core.query_manager import QueryManager
    from .storage.history import HistoryStore
    from .storage.indexer import IndexStore
    from .storage.settings import Preferences
    from .ui.main_window import MainWindow

    app = QApplication(sys.argv)
    app.setApplicationName(APP_NAME)
    app.setApplicationVersion(__version__)
    app.setOrganizationName("Bayram Kotan")
    app.setOrganizationDomain("bayramkotan.com")

    data_dir = user_data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)

    index_store = IndexStore(data_dir / "dictz_index.db")
    index_store.open()

    history_store = HistoryStore(data_dir / "dictz_history.db")
    history_store.open()

    prefs = Preferences.load()
    query_manager = QueryManager(index_store=index_store)
    # Dictionaries added earlier (in this or the Rust edition) come back.
    loaded, errors = query_manager.restore_from_index()
    for err in errors:
        print(f"[dictz] could not reopen {err}", file=sys.stderr)

    window = MainWindow(query_manager, history_store, prefs)
    window.show()
    if errors:
        window.statusBar().showMessage(f"{len(errors)} sözlük açılamadı: " + "; ".join(errors), 10000)
    elif loaded:
        window.statusBar().showMessage(f"{len(loaded)} sözlük yüklendi", 4000)

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
