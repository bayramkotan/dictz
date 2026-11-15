"""Icon helper — uses Qt standard icons."""
from __future__ import annotations

from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication, QStyle

_ICON_MAP = {
    "open": QStyle.StandardPixmap.SP_DialogOpenButton,
    "save": QStyle.StandardPixmap.SP_DialogSaveButton,
    "back": QStyle.StandardPixmap.SP_ArrowBack,
    "forward": QStyle.StandardPixmap.SP_ArrowForward,
    "refresh": QStyle.StandardPixmap.SP_BrowserReload,
    "stop": QStyle.StandardPixmap.SP_BrowserStop,
    "star": QStyle.StandardPixmap.SP_DialogApplyButton,
    "copy": QStyle.StandardPixmap.SP_FileDialogDetailedView,
    "settings": QStyle.StandardPixmap.SP_FileDialogInfoView,
    "zoom-in": QStyle.StandardPixmap.SP_ArrowUp,
    "zoom-out": QStyle.StandardPixmap.SP_ArrowDown,
    "close": QStyle.StandardPixmap.SP_DialogCloseButton,
    "new": QStyle.StandardPixmap.SP_FileIcon,
    "dict": QStyle.StandardPixmap.SP_DriveFDIcon,
    "folder": QStyle.StandardPixmap.SP_DirIcon,
    "help": QStyle.StandardPixmap.SP_DialogHelpButton,
    "remove": QStyle.StandardPixmap.SP_TrashIcon,
    "up": QStyle.StandardPixmap.SP_ArrowUp,
    "down": QStyle.StandardPixmap.SP_ArrowDown,
    "history": QStyle.StandardPixmap.SP_FileDialogListView,
    "theme": QStyle.StandardPixmap.SP_DesktopIcon,
}


def icon(name: str) -> QIcon:
    app = QApplication.instance()
    if app is None:
        return QIcon()
    style = app.style()
    pixmap_enum = _ICON_MAP.get(name)
    if pixmap_enum is None:
        return QIcon()
    return style.standardIcon(pixmap_enum)
