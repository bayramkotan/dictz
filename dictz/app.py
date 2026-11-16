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
    """Shared with the Rust edition (``dictz-rs/src/paths.rs``)."""
    override = os.environ.get("DICTZ_DATA_DIR")
    if override:
        return Path(override)
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
    """``dictz`` entry point: no arguments (or only files) → window, else CLI."""
    args = sys.argv[1:] if argv is None else argv
    from .cli import is_cli_invocation
    from .cli import main as cli_main

    if is_cli_invocation(args):
        _attach_console()
        return cli_main(args)
    return run_gui(args)


def _attach_console() -> None:
    """The Windows portable build is a windowed exe (no stdout). For CLI
    commands, write to the console it was started from, like the Rust edition."""
    if sys.platform != "win32" or sys.stdout is not None:
        return
    try:
        import ctypes

        if not ctypes.windll.kernel32.AttachConsole(-1):  # ATTACH_PARENT_PROCESS
            return
        sys.stdout = open("CONOUT$", "w", encoding="utf-8", errors="replace")  # noqa: SIM115
        sys.stderr = open("CONOUT$", "w", encoding="utf-8", errors="replace")  # noqa: SIM115
        sys.stdin = open("CONIN$", encoding="utf-8", errors="replace")  # noqa: SIM115
    except Exception:  # noqa: BLE001
        pass


def _hand_over(data_dir: Path, files: list[str], popup: str | None) -> bool:
    """Single instance: pass the request to the running dictz. True = done."""
    from .core.instance import send

    if send(data_dir, {"cmd": "ping"}, timeout=1.0) is None:
        return False
    if files:
        send(data_dir, {"cmd": "open", "files": [str(Path(f).absolute()) for f in files]})
    if popup is not None:
        send(data_dir, {"cmd": "popup", "word": popup})
    else:
        send(data_dir, {"cmd": "show"})
    return True


def run_gui(files: list[str] | None = None, popup: str | None = None) -> int:
    from PySide6.QtWidgets import QApplication

    from .core.query_manager import QueryManager
    from .storage.history import HistoryStore
    from .storage.indexer import IndexStore
    from .storage.settings import SETTINGS_FILE, Preferences, migrate_from_qsettings
    from .ui.main_window import MainWindow

    app = QApplication.instance() or QApplication(sys.argv[:1])
    app.setApplicationName(APP_NAME)
    app.setApplicationVersion(__version__)
    app.setOrganizationName("Bayram Kotan")
    app.setOrganizationDomain("bayramkotan.com")
    from .ui.icons import app_icon

    app.setWindowIcon(app_icon())

    data_dir = user_data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)
    if Preferences.load(data_dir / SETTINGS_FILE)["general.single_instance"] and _hand_over(
            data_dir, files or [], popup):
        return 0

    index_store = IndexStore(data_dir / "dictz_index.db")
    index_store.open()
    history_store = HistoryStore(data_dir / "dictz_history.db")
    history_store.open()

    prefs = Preferences.load(data_dir / SETTINGS_FILE)
    migrate_from_qsettings(prefs)
    query_manager = QueryManager(index_store=index_store)

    # The window opens at once; dictionaries load in the background.
    window = MainWindow(query_manager, history_store, prefs,
                        startup_files=[Path(f) for f in (files or [])])
    window.start_services(data_dir)
    if not window.starts_hidden():
        window.show()
    window.start_loading()
    if popup is not None:
        window.popup_lookup(popup)
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
