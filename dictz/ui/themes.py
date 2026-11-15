"""Light and dark Qt style sheets, built from the user's preferences."""
from __future__ import annotations

from ..storage.settings import Preferences


def _mix(hex_color: str, other: str, t: float) -> str:
    a = [int(hex_color[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(other[i:i + 2], 16) for i in (1, 3, 5)]
    c = [round(x + (y - x) * t) for x, y in zip(a, b, strict=True)]
    return "#" + "".join(f"{v:02x}" for v in c)


def is_dark(theme: str) -> bool:
    if theme == "dark":
        return True
    if theme == "light":
        return False
    try:
        from PySide6.QtCore import Qt
        from PySide6.QtGui import QGuiApplication

        app = QGuiApplication.instance()
        if app is not None:
            hints = app.styleHints()
            if hasattr(hints, "colorScheme"):
                scheme = hints.colorScheme()
                if scheme == Qt.ColorScheme.Dark:
                    return True
                if scheme == Qt.ColorScheme.Light:
                    return False
            bg = app.palette().window().color()
            return (bg.red() + bg.green() + bg.blue()) / 3 < 128
    except Exception:  # noqa: BLE001
        pass
    return False


def qss(prefs: Preferences) -> str:
    dark = is_dark(prefs["appearance.theme"])
    accent = prefs["appearance.accent_color"]
    family = prefs["appearance.ui_font_family"]
    size = int(prefs["appearance.ui_font_size"])
    fam = f'"{family}", ' if family else ""
    if dark:
        bg, panel, border, text, muted, hover = "#15181d", "#1b1f26", "#2a2f38", "#e5e7eb", "#9ca3af", "#262b34"
        sel_bg, sel_fg = _mix(accent, "#000000", 0.55), "#ffffff"
    else:
        bg, panel, border, text, muted, hover = "#fafbfc", "#ffffff", "#e3e6eb", "#1a1d23", "#4b5563", "#eef2f7"
        sel_bg, sel_fg = _mix(accent, "#ffffff", 0.8), _mix(accent, "#000000", 0.45)
    return f"""
* {{ font-family: {fam}"Segoe UI", "Inter", "Cantarell", "Noto Sans", "Helvetica Neue", sans-serif; font-size: {size}px; }}
QMainWindow, QDialog, QWidget {{ background: {bg}; color: {text}; }}
QToolBar {{ background: {panel}; border: none; border-bottom: 1px solid {border}; padding: 3px 6px; spacing: 3px; }}
QToolBar QToolButton {{ background: transparent; border: 1px solid transparent; border-radius: 6px; padding: 5px 9px; color: {text}; }}
QToolBar QToolButton:hover {{ background: {hover}; border-color: {border}; }}
QToolBar QToolButton:checked {{ background: {sel_bg}; border-color: {accent}; color: {sel_fg}; }}
QToolBar#dictionary_bar QToolButton {{ border-radius: 11px; padding: 2px 10px; }}
QToolBar#dictionary_bar QToolButton:!checked {{ color: {muted}; border: 1px dashed {border}; }}
QMenuBar {{ background: {panel}; border-bottom: 1px solid {border}; }}
QMenuBar::item {{ padding: 5px 11px; background: transparent; }}
QMenuBar::item:selected {{ background: {hover}; }}
QMenu {{ background: {panel}; border: 1px solid {border}; padding: 4px; }}
QMenu::item {{ padding: 6px 26px 6px 22px; border-radius: 4px; }}
QMenu::item:selected {{ background: {sel_bg}; color: {sel_fg}; }}
QMenu::separator {{ height: 1px; background: {border}; margin: 4px 8px; }}
QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox, QFontComboBox {{ background: {panel}; border: 1px solid {border}; border-radius: 7px; padding: 5px 9px; selection-background-color: {sel_bg}; selection-color: {sel_fg}; color: {text}; }}
QLineEdit#search_box {{ padding: 7px 12px; font-size: {size + 1}px; }}
QLineEdit:focus, QSpinBox:focus, QDoubleSpinBox:focus, QComboBox:focus {{ border-color: {accent}; }}
QListWidget, QTreeWidget, QTableWidget {{ background: {panel}; border: 1px solid {border}; border-radius: 8px; padding: 3px; color: {text}; outline: 0; }}
QListWidget::item, QTreeWidget::item {{ padding: 5px 8px; border-radius: 4px; }}
QListWidget::item:hover, QTreeWidget::item:hover, QTableWidget::item:hover {{ background: {hover}; }}
QListWidget::item:selected, QTreeWidget::item:selected, QTableWidget::item:selected {{ background: {sel_bg}; color: {sel_fg}; }}
QHeaderView::section {{ background: {panel}; color: {muted}; border: none; border-bottom: 1px solid {border}; padding: 5px; }}
QTabWidget::pane {{ border: 1px solid {border}; border-radius: 8px; background: {panel}; top: -1px; }}
QTabBar::tab {{ background: {bg}; color: {muted}; border: 1px solid {border}; border-bottom: none; border-top-left-radius: 8px; border-top-right-radius: 8px; padding: 5px 12px; margin-right: 2px; min-width: 70px; }}
QTabBar::tab:selected {{ background: {panel}; color: {text}; border-color: {accent}; }}
QTabBar::tab:hover:!selected {{ background: {hover}; }}
QStatusBar {{ background: {panel}; border-top: 1px solid {border}; color: {muted}; }}
QDockWidget::title {{ background: {panel}; padding: 6px 10px; border-bottom: 1px solid {border}; text-align: left; font-weight: 600; }}
QScrollBar:vertical {{ background: transparent; width: 10px; margin: 0; }}
QScrollBar::handle:vertical {{ background: {_mix(border, text, 0.25)}; border-radius: 5px; min-height: 20px; }}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{ height: 0; }}
QScrollBar:horizontal {{ background: transparent; height: 10px; }}
QScrollBar::handle:horizontal {{ background: {_mix(border, text, 0.25)}; border-radius: 5px; min-width: 20px; }}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {{ width: 0; }}
QPushButton {{ background: {panel}; border: 1px solid {border}; border-radius: 6px; padding: 5px 13px; color: {text}; }}
QPushButton:hover {{ background: {hover}; border-color: {muted}; }}
QPushButton:default {{ background: {accent}; color: white; border-color: {accent}; }}
QPushButton:checked {{ background: {sel_bg}; color: {sel_fg}; border-color: {accent}; }}
QCheckBox {{ spacing: 7px; }}
QLabel#section_title {{ font-size: {size + 5}px; font-weight: 600; color: {text}; }}
QLabel#hint {{ color: {muted}; font-size: {max(9, size - 2)}px; }}
QWidget#find_bar {{ background: {panel}; border-top: 1px solid {border}; }}
QProgressBar {{ border: 1px solid {border}; border-radius: 4px; background: {bg}; height: 10px; text-align: center; }}
QProgressBar::chunk {{ background: {accent}; border-radius: 3px; }}
"""


def definition_css(prefs: Preferences) -> str:
    dark = is_dark(prefs["appearance.theme"])
    accent = prefs["appearance.accent_color"]
    fam = prefs["article.font_family"] or prefs["appearance.ui_font_family"]
    family = f'"{fam}", ' if fam else ""
    size = int(prefs["article.font_size"])
    spacing = float(prefs["article.line_spacing"])
    head = float(prefs["article.headword_scale"])
    maxw = int(prefs["article.max_width"])
    img_h = int(prefs["article.max_image_height"])
    hl = prefs["article.highlight_color"]
    if dark:
        bg, fg, muted, card, border = "#1b1f26", "#e5e7eb", "#9ca3af", "#232832", "#343a45"
        head_fg, link = _mix(accent, "#ffffff", 0.55), _mix(accent, "#ffffff", 0.35)
    else:
        bg, fg, muted, card, border = "#ffffff", "#1a1d23", "#6b7280", "#f3f6fa", "#e3e6eb"
        head_fg, link = _mix(accent, "#000000", 0.35), accent
    width = f"max-width: {maxw}px; margin: 0 auto;" if maxw else ""
    return f"""
body {{ background: {bg}; color: {fg}; font-family: {family}"Segoe UI", "Inter", "Noto Sans", sans-serif;
        font-size: {size}px; line-height: {spacing}; padding: 18px 28px; margin: 0; }}
main {{ {width} }}
a {{ color: {link}; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
h1, h2, h3 {{ color: {head_fg}; font-weight: 600; }}
details.dict {{ margin: 0 0 14px 0; }}
details.dict > summary {{ list-style: none; cursor: pointer; display: flex; align-items: center; gap: 10px;
        background: {card}; border-left: 3px solid {accent}; padding: 7px 12px; border-radius: 5px;
        font-size: 0.78em; font-weight: 600; letter-spacing: .5px; text-transform: uppercase; color: {head_fg}; }}
details.dict > summary::-webkit-details-marker {{ display: none; }}
details.dict > summary .chev {{ transition: transform .15s; display: inline-block; }}
details.dict[open] > summary .chev {{ transform: rotate(90deg); }}
details.dict > summary .count {{ color: {muted}; font-weight: 400; text-transform: none; letter-spacing: 0; }}
details.dict > summary .actions {{ margin-left: auto; text-transform: none; letter-spacing: 0; font-weight: 400; white-space: nowrap; font-size: .95em; }}
details.dict > summary .actions a {{ margin-left: 10px; }}
.nohead > summary {{ display: none !important; }}
.article {{ padding: 6px 0 10px 14px; border-left: 2px solid {border}; margin: 8px 0 12px 4px; }}
.headword {{ font-size: {head:.2f}em; font-weight: 600; color: {head_fg}; margin: 2px 0 4px 0; }}
.definition pre {{ white-space: pre-wrap; font-family: inherit; margin: 0; }}
.definition img {{ max-width: 100%; max-height: {img_h}px; vertical-align: middle; }}
a.dz-audio {{ display: inline-block; padding: 0 9px; margin: 0 2px; border-radius: 10px; background: {card};
        border: 1px solid {border}; font-size: 0.85em; }}
ul.did-you-mean {{ list-style: none; padding-left: 0; }}
ul.did-you-mean li {{ margin: 5px 0; font-size: 1.1em; }}
.score {{ color: {muted}; font-size: 0.8em; margin-left: 6px; }}
.muted {{ color: {muted}; }}
::selection {{ background: {hl}; color: #000; }}
"""
