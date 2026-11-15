"""Light and dark Qt style sheets."""
from __future__ import annotations


LIGHT_QSS = """
* { font-family: "Segoe UI", "Inter", "Cantarell", "Helvetica Neue", sans-serif; }
QMainWindow, QDialog, QWidget { background: #fafbfc; color: #1a1d23; }
QToolBar { background: #ffffff; border: none; border-bottom: 1px solid #e3e6eb; padding: 4px 6px; spacing: 4px; }
QToolBar QToolButton { background: transparent; border: 1px solid transparent; border-radius: 6px; padding: 6px 10px; color: #2d3139; }
QToolBar QToolButton:hover { background: #eef2f7; border-color: #dde3ea; }
QToolBar QToolButton:pressed { background: #dfe6ef; }
QToolBar QToolButton:checked { background: #dbeafe; border-color: #93c5fd; }
QMenuBar { background: #ffffff; border-bottom: 1px solid #e3e6eb; }
QMenuBar::item { padding: 6px 12px; background: transparent; }
QMenuBar::item:selected { background: #eef2f7; }
QMenu { background: #ffffff; border: 1px solid #d9dee5; padding: 4px; }
QMenu::item { padding: 6px 24px; border-radius: 4px; }
QMenu::item:selected { background: #dbeafe; color: #1e3a8a; }
QLineEdit { background: #ffffff; border: 1px solid #d9dee5; border-radius: 8px; padding: 8px 12px; selection-background-color: #bfdbfe; font-size: 14px; }
QLineEdit:focus { border-color: #60a5fa; }
QListWidget, QTreeWidget, QTableWidget { background: #ffffff; border: 1px solid #e3e6eb; border-radius: 8px; padding: 4px; outline: 0; }
QListWidget::item, QTreeWidget::item { padding: 6px 8px; border-radius: 4px; }
QListWidget::item:hover, QTreeWidget::item:hover { background: #f1f5f9; }
QListWidget::item:selected, QTreeWidget::item:selected { background: #dbeafe; color: #1e3a8a; }
QTabWidget::pane { border: 1px solid #e3e6eb; border-radius: 8px; background: #ffffff; top: -1px; }
QTabBar::tab { background: #f1f5f9; color: #4b5563; border: 1px solid #e3e6eb; border-bottom: none; border-top-left-radius: 8px; border-top-right-radius: 8px; padding: 6px 14px; margin-right: 2px; min-width: 90px; }
QTabBar::tab:selected { background: #ffffff; color: #1e3a8a; border-color: #60a5fa; }
QTabBar::tab:hover:!selected { background: #e6ebf2; }
QStatusBar { background: #ffffff; border-top: 1px solid #e3e6eb; color: #4b5563; }
QDockWidget::title { background: #f1f5f9; padding: 6px 10px; border-bottom: 1px solid #e3e6eb; text-align: left; font-weight: 600; }
QScrollBar:vertical { background: transparent; width: 10px; margin: 0; }
QScrollBar::handle:vertical { background: #cbd5e1; border-radius: 5px; min-height: 20px; }
QScrollBar::handle:vertical:hover { background: #94a3b8; }
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }
QScrollBar:horizontal { background: transparent; height: 10px; }
QScrollBar::handle:horizontal { background: #cbd5e1; border-radius: 5px; min-width: 20px; }
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal { width: 0; }
QPushButton { background: #ffffff; border: 1px solid #d9dee5; border-radius: 6px; padding: 6px 14px; color: #1a1d23; }
QPushButton:hover { background: #f1f5f9; border-color: #94a3b8; }
QPushButton:default { background: #2563eb; color: white; border-color: #1d4ed8; }
"""

DARK_QSS = """
* { font-family: "Segoe UI", "Inter", "Cantarell", "Helvetica Neue", sans-serif; }
QMainWindow, QDialog, QWidget { background: #15181d; color: #e5e7eb; }
QToolBar { background: #1b1f26; border: none; border-bottom: 1px solid #2a2f38; padding: 4px 6px; spacing: 4px; }
QToolBar QToolButton { background: transparent; border: 1px solid transparent; border-radius: 6px; padding: 6px 10px; color: #d1d5db; }
QToolBar QToolButton:hover { background: #262b34; border-color: #343a45; }
QToolBar QToolButton:checked { background: #1e3a5f; border-color: #3b82f6; }
QMenuBar { background: #1b1f26; border-bottom: 1px solid #2a2f38; }
QMenuBar::item { padding: 6px 12px; background: transparent; }
QMenuBar::item:selected { background: #262b34; }
QMenu { background: #1b1f26; border: 1px solid #2a2f38; padding: 4px; }
QMenu::item { padding: 6px 24px; border-radius: 4px; color: #d1d5db; }
QMenu::item:selected { background: #1e3a5f; color: #dbeafe; }
QLineEdit { background: #1b1f26; border: 1px solid #343a45; border-radius: 8px; padding: 8px 12px; selection-background-color: #1e3a5f; color: #e5e7eb; font-size: 14px; }
QLineEdit:focus { border-color: #3b82f6; }
QListWidget, QTreeWidget, QTableWidget { background: #1b1f26; border: 1px solid #2a2f38; border-radius: 8px; padding: 4px; color: #d1d5db; outline: 0; }
QListWidget::item, QTreeWidget::item { padding: 6px 8px; border-radius: 4px; }
QListWidget::item:hover, QTreeWidget::item:hover { background: #262b34; }
QListWidget::item:selected, QTreeWidget::item:selected { background: #1e3a5f; color: #dbeafe; }
QTabWidget::pane { border: 1px solid #2a2f38; border-radius: 8px; background: #1b1f26; top: -1px; }
QTabBar::tab { background: #15181d; color: #9ca3af; border: 1px solid #2a2f38; border-bottom: none; border-top-left-radius: 8px; border-top-right-radius: 8px; padding: 6px 14px; margin-right: 2px; min-width: 90px; }
QTabBar::tab:selected { background: #1b1f26; color: #dbeafe; border-color: #3b82f6; }
QTabBar::tab:hover:!selected { background: #262b34; }
QStatusBar { background: #1b1f26; border-top: 1px solid #2a2f38; color: #9ca3af; }
QDockWidget::title { background: #1b1f26; padding: 6px 10px; border-bottom: 1px solid #2a2f38; text-align: left; font-weight: 600; color: #d1d5db; }
QScrollBar:vertical { background: transparent; width: 10px; margin: 0; }
QScrollBar::handle:vertical { background: #3f4651; border-radius: 5px; min-height: 20px; }
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }
QScrollBar:horizontal { background: transparent; height: 10px; }
QScrollBar::handle:horizontal { background: #3f4651; border-radius: 5px; min-width: 20px; }
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal { width: 0; }
QPushButton { background: #1b1f26; border: 1px solid #343a45; border-radius: 6px; padding: 6px 14px; color: #e5e7eb; }
QPushButton:hover { background: #262b34; border-color: #4b5563; }
QPushButton:default { background: #2563eb; color: white; border-color: #1d4ed8; }
QLabel { color: #d1d5db; }
"""


def resolve(theme: str) -> str:
    if theme == "dark":
        return DARK_QSS
    if theme == "light":
        return LIGHT_QSS
    try:
        from PySide6.QtGui import QGuiApplication
        app = QGuiApplication.instance()
        if app is not None:
            palette = app.palette()
            bg = palette.window().color()
            if (bg.red() + bg.green() + bg.blue()) / 3 < 128:
                return DARK_QSS
    except Exception:
        pass
    return LIGHT_QSS


def definition_css(dark: bool) -> str:
    if dark:
        return """
        body { background: #1b1f26; color: #e5e7eb; font-family: "Segoe UI", "Inter", sans-serif; line-height: 1.65; padding: 24px 32px; margin: 0; }
        h1, h2, h3 { color: #93c5fd; font-weight: 600; }
        a { color: #60a5fa; text-decoration: none; }
        .dict-header { background: #262b34; border-left: 3px solid #3b82f6; padding: 8px 14px; margin: 24px 0 12px 0; font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.6px; color: #93c5fd; border-radius: 4px; }
        .headword { font-size: 22px; font-weight: 600; color: #dbeafe; margin: 4px 0 8px 0; }
        .definition { padding: 4px 0 8px 12px; border-left: 2px solid #343a45; margin-left: 4px; }
        .definition pre { white-space: pre-wrap; font-family: inherit; margin: 0; }
        """
    return """
    body { background: #ffffff; color: #1a1d23; font-family: "Segoe UI", "Inter", sans-serif; line-height: 1.65; padding: 24px 32px; margin: 0; }
    h1, h2, h3 { color: #1e3a8a; font-weight: 600; }
    a { color: #2563eb; text-decoration: none; }
    .dict-header { background: #eef2f7; border-left: 3px solid #2563eb; padding: 8px 14px; margin: 24px 0 12px 0; font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.6px; color: #1e3a8a; border-radius: 4px; }
    .headword { font-size: 22px; font-weight: 600; color: #1e3a8a; margin: 4px 0 8px 0; }
    .definition { padding: 4px 0 8px 12px; border-left: 2px solid #e3e6eb; margin-left: 4px; }
    .definition pre { white-space: pre-wrap; font-family: inherit; margin: 0; }
    """
