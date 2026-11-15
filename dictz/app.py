"""dictz application entry point."""
from __future__ import annotations

import os
import sys
from pathlib import Path

from PySide6.QtWidgets import QApplication

from .core.query_manager import QueryManager
from .storage.history import HistoryStore
from .storage.indexer import IndexStore
from .storage.settings import Preferences
from .ui.main_window import MainWindow

__version__ = "0.0.5"
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


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName(APP_NAME)
    app.setApplicationVersion(__version__)
    app.setOrganizationName("Bayram Kotan")
    app.setOrganizationDomain("bayramkotan.com")

    data_dir = user_data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)
    index_path = data_dir / "dictz_index.db"
    history_path = data_dir / "dictz_history.db"

    index_store = IndexStore(index_path)
    index_store.open()

    history_store = HistoryStore(history_path)
    history_store.open()

    prefs = Preferences.load()
    query_manager = QueryManager(index_store=index_store)

    window = MainWindow(query_manager, history_store, prefs)
    window.show()

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
