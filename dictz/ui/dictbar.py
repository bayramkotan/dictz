"""The dictionary bar in the toolbar: one icon per dictionary.

Click = go to that dictionary's article; Ctrl+click = switch it on / off;
right click = a menu; drag an icon onto another one = change the order. A
switched-off dictionary is drawn faded.
"""
from __future__ import annotations

from PySide6.QtCore import QMimeData, QPoint, QSize, Qt, Signal
from PySide6.QtGui import QDrag, QIcon, QPainter, QPalette
from PySide6.QtWidgets import QApplication, QHBoxLayout, QLabel, QToolButton, QWidget

from . import dicticons
from .icons import icon

MIME = "application/x-dictz-dictionary"


def drop_target(centers: list[tuple[str, float]], x: float, dragged: str) -> str | None:
    """The name to put ``dragged`` before when dropped at ``x``: the first
    icon whose centre is right of ``x`` (``None`` = at the end)."""
    for name, cx in centers:
        if x < cx:
            return name if name != dragged else next(
                (n for n, c in centers if c > cx), None)
    return None


class _DictButton(QToolButton):
    def __init__(self, name: str, enabled: bool, strip: DictionaryStrip):
        super().__init__(strip)
        self.name = name
        self.strip = strip
        self._press: QPoint | None = None
        self.setObjectName("dict_toggle")
        base = dicticons.qicon(name)
        self.setIcon(base if enabled else QIcon(base.pixmap(20, 20, QIcon.Mode.Disabled)))
        self.setIconSize(QSize(20, 20))
        self.setAutoRaise(True)
        self.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonIconOnly)
        self.setCheckable(False)
        self.setProperty("off", not enabled)

    def mousePressEvent(self, e):  # noqa: N802
        if e.button() == Qt.MouseButton.LeftButton:
            self._press = e.position().toPoint()
        super().mousePressEvent(e)

    def mouseMoveEvent(self, e):  # noqa: N802
        if self._press is not None and e.buttons() & Qt.MouseButton.LeftButton and \
                (e.position().toPoint() - self._press).manhattanLength() >= QApplication.startDragDistance():
            self._press = None
            self.setDown(False)
            drag = QDrag(self)
            md = QMimeData()
            md.setData(MIME, self.name.encode("utf-8"))
            drag.setMimeData(md)
            drag.setPixmap(self.icon().pixmap(20, 20))
            drag.exec(Qt.DropAction.MoveAction)
            return
        super().mouseMoveEvent(e)

    def mouseReleaseEvent(self, e):  # noqa: N802
        was = self._press is not None
        self._press = None
        super().mouseReleaseEvent(e)
        if was and e.button() == Qt.MouseButton.LeftButton and self.rect().contains(e.position().toPoint()):
            if e.modifiers() & Qt.KeyboardModifier.ControlModifier:
                self.strip.toggle_requested.emit(self.name)
            else:
                self.strip.jump_requested.emit(self.name)

    def contextMenuEvent(self, e):  # noqa: N802
        self.strip.menu_requested.emit(self.name, e.globalPos())


class DictionaryStrip(QWidget):
    jump_requested = Signal(str)
    toggle_requested = Signal(str)
    menu_requested = Signal(str, QPoint)
    moved = Signal(str, object)  # name, the name to put it before (None = at the end)
    enable_all_requested = Signal()

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setObjectName("dictionary_bar")
        self.setAcceptDrops(True)
        self._lay = QHBoxLayout(self)
        self._lay.setContentsMargins(0, 0, 0, 0)
        self._lay.setSpacing(1)
        self._drop_x: int | None = None

    def buttons(self) -> list[_DictButton]:
        return [self._lay.itemAt(i).widget() for i in range(self._lay.count())
                if isinstance(self._lay.itemAt(i).widget(), _DictButton)]

    def set_dictionaries(self, items: list[tuple[str, bool]], empty_text: str = "") -> None:
        while self._lay.count():
            w = self._lay.takeAt(0).widget()
            if w is not None:
                w.deleteLater()
        if not items:
            lab = QLabel(" " + empty_text)
            lab.setObjectName("hint")
            self._lay.addWidget(lab)
            return
        from ..core.i18n import t

        tip = t("Tıkla: makalesine git · Ctrl+tık: aç/kapat · Sağ tık: menü · Sürükle: sırayı değiştir")
        for name, enabled in items:
            b = _DictButton(name, enabled, self)
            off = "" if enabled else "\n" + t("(kapalı)")
            b.setToolTip(dicticons.look(name).tooltip() + off + "\n\n" + tip)
            self._lay.addWidget(b)
        all_b = QToolButton(self)
        all_b.setObjectName("dict_toggle")
        all_b.setAutoRaise(True)
        all_b.setIcon(icon("all"))
        all_b.setIconSize(QSize(16, 16))
        all_b.setToolTip(t("Tüm sözlükleri etkinleştir"))
        all_b.clicked.connect(self.enable_all_requested.emit)
        self._lay.addWidget(all_b)

    # ------------------------------------------------------------ drop
    def _centers(self) -> list[tuple[str, float]]:
        return [(b.name, b.geometry().center().x()) for b in self.buttons()]

    def dragEnterEvent(self, e):  # noqa: N802
        if e.mimeData().hasFormat(MIME):
            e.acceptProposedAction()

    def dragMoveEvent(self, e):  # noqa: N802
        if e.mimeData().hasFormat(MIME):
            e.acceptProposedAction()
            x = e.position().x()
            bs = self.buttons()
            nxt = next((b for b in bs if x < b.geometry().center().x()), None)
            self._drop_x = nxt.geometry().left() if nxt else (bs[-1].geometry().right() if bs else None)
            self.update()

    def dragLeaveEvent(self, e):  # noqa: N802
        self._drop_x = None
        self.update()

    def dropEvent(self, e):  # noqa: N802
        self._drop_x = None
        self.update()
        if not e.mimeData().hasFormat(MIME):
            return
        name = bytes(e.mimeData().data(MIME)).decode("utf-8")
        e.acceptProposedAction()
        self.moved.emit(name, drop_target(self._centers(), e.position().x(), name))

    def paintEvent(self, e):  # noqa: N802
        super().paintEvent(e)
        if self._drop_x is not None:
            p = QPainter(self)
            p.setPen(self.palette().color(QPalette.ColorRole.Highlight))
            p.drawLine(self._drop_x, 2, self._drop_x, self.height() - 2)
            p.drawLine(self._drop_x - 1, 2, self._drop_x - 1, self.height() - 2)
