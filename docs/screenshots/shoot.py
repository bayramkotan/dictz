"""Take the README screenshots of the Python edition (no display needed).

    QT_QPA_PLATFORM=offscreen python docs/screenshots/shoot.py [OUT_DIR]

Builds the invented demo dictionaries (``demo_dicts.py``) into a fresh data
folder, opens the window at 1360×820 with a scale of 2 and writes
``main-light.png``, ``python-dark.png`` (Nord theme), ``themes.png``,
``turkish.png``, ``arabic.png``, ``languages.png``, ``preferences.png``,
``scan-popup.png`` and ``dictionaries.png`` (2720×1640 each). The Rust
edition's pictures come from ``shoot_rust.sh``.
"""
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
DATA = Path(tempfile.mkdtemp(prefix="dictz-"))
os.environ["DICTZ_DATA_DIR"] = str(DATA / "data")
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ["QT_SCALE_FACTOR"] = "2"
os.environ.setdefault("QTWEBENGINE_CHROMIUM_FLAGS", "--no-sandbox")
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from PySide6.QtCore import QPoint, Qt, QTimer  # noqa: E402
from PySide6.QtGui import QColor, QPainter, QPixmap  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

W, H = 1360, 820


def wait(app: QApplication, ms: int) -> None:
    from PySide6.QtCore import QEventLoop

    loop = QEventLoop()
    QTimer.singleShot(ms, loop.quit)
    loop.exec()


def compose(base: QPixmap, top: QPixmap, at: QPoint, dim: bool = True) -> QPixmap:
    """``top`` (a popup or dialog) drawn over ``base`` at ``at`` (logical px)."""
    out = QPixmap(base)
    p = QPainter(out)
    if dim:
        p.fillRect(out.rect(), QColor(0, 0, 0, 70))
    ratio = base.devicePixelRatio()
    p.drawPixmap(QPoint(int(at.x() * ratio), int(at.y() * ratio)) / ratio, top)
    p.end()
    return out


def main() -> None:
    import demo_dicts

    app = QApplication([])  # before demo_dicts, which needs one for its pictures
    dicts = DATA / "Dictionaries"
    demo_dicts.main(dicts)
    from dictz.core import i18n
    from dictz.core.query_manager import QueryManager
    from dictz.storage.history import HistoryStore
    from dictz.storage.indexer import IndexStore
    from dictz.storage.settings import SETTINGS_FILE, Preferences
    from dictz.ui.main_window import MainWindow
    from dictz.ui.preferences_dialog import PreferencesDialog

    d = DATA / "data"
    d.mkdir(parents=True, exist_ok=True)
    st = IndexStore(d / "dictz_index.db")
    st.open()
    hs = HistoryStore(d / "dictz_history.db")
    hs.open()
    for word in ("light", "book", "yield", "pear", "apple"):
        hs.record(word)
    prefs = Preferences.load(d / SETTINGS_FILE)
    prefs.set("general.load_in_background", False)
    prefs.set("center.refresh_days", 0)
    prefs.set("general.restore_last_search", False)
    prefs.set("dictionaries.folders", [str(dicts)])
    prefs.set("dictionaries.scan_on_startup", True)
    qm = QueryManager(index_store=st)

    def window(lang: str, theme: str, word: str = "apple") -> MainWindow:
        i18n.set_language(lang)
        app.setLayoutDirection(Qt.LayoutDirection.RightToLeft if i18n.is_rtl(lang)
                               else Qt.LayoutDirection.LeftToRight)
        prefs.set("general.language", lang)
        prefs.set("appearance.theme", theme)
        w = MainWindow(qm, hs, prefs)
        w.resize(W, H)
        w.show()
        w.start_loading()
        wait(app, 1500)
        w.search_box.clear()  # the last search may already be in the box
        w.search_box.setText(word)
        w._run_search_explicit()
        wait(app, 1500)
        return w

    def save(pix: QPixmap, name: str) -> None:
        pix.save(str(OUT / name))
        print("wrote", OUT / name, pix.width(), "x", pix.height())

    w = window("en", "light")
    save(w.grab(), "main-light.png")
    # the language picker over the window
    btn = getattr(w, "language_button", None)
    menu = btn.menu() if btn is not None else None
    if menu is not None:
        menu.ensurePolished()
        menu.resize(menu.sizeHint())  # not limited to the offscreen screen
        pos = btn.mapTo(w, QPoint(btn.width() - menu.sizeHint().width(), btn.height()))
        save(compose(w.grab(), menu.grab(), pos, dim=False), "languages.png")
    # Preferences
    dlg = PreferencesDialog(prefs, w)
    dlg.resize(940, 640)
    dlg.show()
    wait(app, 300)
    for i in range(dlg.nav.count()):
        if dlg.nav.item(i).text() in ("Ses", "Sound", "Audio"):
            dlg.nav.setCurrentRow(i)
    wait(app, 200)
    save(compose(w.grab(), dlg.grab(), QPoint((W - 940) // 2, (H - 640) // 2)), "preferences.png")
    dlg.close()
    # the scan popup
    w.popup_lookup("yield")
    wait(app, 1200)
    pop = w.scan_popup
    if pop is not None and pop.isVisible():
        pop.resize(560, 420)
        wait(app, 600)
        shot = pop.grab()
        p = QPainter(shot)
        p.setPen(QColor(0, 0, 0, 90))
        p.drawRect(0, 0, pop.width() - 1, pop.height() - 1)
        p.end()
        save(compose(w.grab(), shot, QPoint(W - 620, 90), dim=False), "scan-popup.png")
        pop.hide()
    # the Sözlükler window
    c = w.open_center(0)
    c.resize(W, H)
    wait(app, 600)
    save(c.grab(), "dictionaries.png")
    c.close()
    w.stop_services()
    w.close()

    w = window("en", "nord")
    save(w.grab(), "python-dark.png")
    w.stop_services()
    w.close()
    # four themes in one picture
    tiles = []
    for th in ("solarized-light", "dracula", "mint", "gruvbox-dark"):
        w = window("en", th)
        tiles.append(w.grab())
        w.stop_services()
        w.close()
    grid = QPixmap(tiles[0].size())
    grid.setDevicePixelRatio(tiles[0].devicePixelRatio())
    grid.fill(Qt.GlobalColor.black)
    p = QPainter(grid)
    half_w, half_h = W // 2, H // 2
    for i, t in enumerate(tiles):
        p.drawPixmap(QPoint((i % 2) * half_w, (i // 2) * half_h),
                     t.scaled(t.width() // 2, t.height() // 2, Qt.AspectRatioMode.IgnoreAspectRatio,
                              Qt.TransformationMode.SmoothTransformation))
    p.end()
    save(grid, "themes.png")

    w = window("tr", "light")
    save(w.grab(), "turkish.png")
    w.stop_services()
    w.close()
    w = window("ar", "light")
    save(w.grab(), "arabic.png")
    w.stop_services()
    w.close()
    shutil.rmtree(DATA, ignore_errors=True)


if __name__ == "__main__":
    main()
