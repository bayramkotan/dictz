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


def palette(prefs: Preferences) -> dict[str, str]:
    """The interface colours (same values as the Rust edition's ``gui::window``)."""
    dark = is_dark(prefs["appearance.theme"])
    accent = prefs["appearance.accent_color"]
    if dark:
        c = {"bg": "#1e2127", "panel": "#25282f", "border": "#363a43", "text": "#e3e5e9", "muted": "#9aa1ac", "hover": "#2f333b", "header": "#2a2e36", "icon": "#c9cdd4"}
        c["sel_bg"], c["sel_fg"] = _mix(accent, "#000000", 0.45), "#ffffff"
    else:
        c = {"bg": "#f5f6f8", "panel": "#ffffff", "border": "#d7dbe0", "text": "#1f2328", "muted": "#5f6670", "hover": "#e8ecf1", "header": "#eef1f4", "icon": "#3d4450"}
        c["sel_bg"], c["sel_fg"] = _mix(accent, "#ffffff", 0.78), _mix(accent, "#000000", 0.5)
    c["accent"] = accent
    return c


def icon_color(prefs: Preferences) -> str:
    return palette(prefs)["icon"]


def qss(prefs: Preferences) -> str:
    """Compact, GoldenDict-like style: small paddings, square corners, thin lines."""
    c = palette(prefs)
    accent = c["accent"]
    family = prefs["appearance.ui_font_family"]
    size = int(prefs["appearance.ui_font_size"])
    fam = f'"{family}", ' if family else ""
    bg, panel, border, text, muted, hover = c["bg"], c["panel"], c["border"], c["text"], c["muted"], c["hover"]
    sel_bg, sel_fg, header = c["sel_bg"], c["sel_fg"], c["header"]
    return f"""
* {{ font-family: {fam}"Segoe UI", "Inter", "Cantarell", "Noto Sans", "Helvetica Neue", sans-serif; font-size: {size}px; }}
QMainWindow, QDialog, QWidget {{ background: {bg}; color: {text}; }}
QMainWindow::separator {{ background: {border}; width: 1px; height: 1px; }}
QMainWindow::separator:hover {{ background: {accent}; }}
QToolBar {{ background: {bg}; border: none; border-bottom: 1px solid {border}; padding: 1px 4px; spacing: 1px; }}
QToolBar::separator {{ background: {border}; width: 1px; margin: 5px 4px; }}
QToolBar QToolButton {{ background: transparent; border: 1px solid transparent; border-radius: 3px; padding: 3px; color: {text}; }}
QToolBar QToolButton:hover {{ background: {hover}; border-color: {border}; }}
QToolBar QToolButton:pressed, QToolBar QToolButton:checked {{ background: {sel_bg}; border-color: {_mix(accent, bg, 0.5)}; }}
QToolButton#dict_toggle {{ padding: 2px; border-radius: 3px; }}
QToolButton#dict_toggle:!checked {{ background: transparent; border-color: transparent; }}
QToolButton#dict_toggle:checked {{ background: {sel_bg}; border-color: {_mix(accent, bg, 0.55)}; }}
QMenuBar {{ background: {bg}; border: none; padding: 0; }}
QMenuBar::item {{ padding: 3px 8px; background: transparent; }}
QMenuBar::item:selected {{ background: {hover}; }}
QMenu {{ background: {panel}; border: 1px solid {border}; padding: 3px 0; }}
QMenu::item {{ padding: 4px 22px 4px 22px; }}
QMenu::item:selected {{ background: {sel_bg}; color: {sel_fg}; }}
QMenu::separator {{ height: 1px; background: {border}; margin: 3px 6px; }}
QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox, QFontComboBox {{ background: {panel}; border: 1px solid {border}; border-radius: 3px; padding: 3px 6px; selection-background-color: {sel_bg}; selection-color: {sel_fg}; color: {text}; }}
QLineEdit#search_box {{ padding: 4px 6px; font-size: {size + 1}px; }}
QLineEdit:focus, QSpinBox:focus, QDoubleSpinBox:focus, QComboBox:focus {{ border-color: {accent}; }}
QListWidget, QTreeWidget, QTableWidget {{ background: {panel}; border: 1px solid {border}; border-radius: 0; padding: 0; color: {text}; outline: 0; }}
QListWidget::item, QTreeWidget::item {{ padding: 2px 5px; border: none; }}
QListWidget::item:hover, QTreeWidget::item:hover, QTableWidget::item:hover {{ background: {hover}; }}
QListWidget::item:selected, QTreeWidget::item:selected, QTableWidget::item:selected {{ background: {sel_bg}; color: {sel_fg}; }}
QListWidget#word_list::item {{ padding: 1px 5px; }}
QHeaderView::section {{ background: {header}; color: {muted}; border: none; border-right: 1px solid {border}; border-bottom: 1px solid {border}; padding: 3px 5px; }}
QTabWidget::pane {{ border: 1px solid {border}; border-radius: 0; background: {panel}; top: -1px; }}
QTabBar::tab {{ background: {bg}; color: {muted}; border: 1px solid {border}; border-bottom: none; padding: 3px 10px; margin-right: -1px; min-width: 30px; }}
QTabBar::tab:selected {{ background: {panel}; color: {text}; border-top: 2px solid {accent}; }}
QTabBar::tab:hover:!selected {{ background: {hover}; }}
QTabWidget#side_tabs QTabBar::tab {{ padding: 4px 6px; min-width: 24px; }}
QTabBar::close-button {{ subcontrol-position: right; }}
QStatusBar {{ background: {bg}; border-top: 1px solid {border}; color: {muted}; }}
QStatusBar::item {{ border: none; }}
QStatusBar QLabel {{ color: {muted}; padding: 0 4px; }}
QDockWidget {{ titlebar-close-icon: none; }}
QDockWidget::title {{ background: {header}; padding: 3px 6px; border-bottom: 1px solid {border}; text-align: left; font-weight: 600; color: {muted}; }}
QScrollBar:vertical {{ background: transparent; width: 9px; margin: 0; }}
QScrollBar::handle:vertical {{ background: {_mix(border, text, 0.2)}; border-radius: 4px; min-height: 24px; margin: 1px; }}
QScrollBar::handle:vertical:hover {{ background: {_mix(border, text, 0.4)}; }}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{ height: 0; }}
QScrollBar:horizontal {{ background: transparent; height: 9px; }}
QScrollBar::handle:horizontal {{ background: {_mix(border, text, 0.2)}; border-radius: 4px; min-width: 24px; margin: 1px; }}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {{ width: 0; }}
QPushButton {{ background: {panel}; border: 1px solid {border}; border-radius: 3px; padding: 3px 10px; color: {text}; }}
QPushButton:hover {{ background: {hover}; border-color: {muted}; }}
QPushButton:default {{ background: {accent}; color: white; border-color: {accent}; }}
QPushButton:checked {{ background: {sel_bg}; color: {sel_fg}; border-color: {accent}; }}
QPushButton:flat {{ border: none; background: transparent; color: {accent}; padding: 1px 4px; }}
QCheckBox {{ spacing: 5px; }}
QLabel#section_title {{ font-size: {size + 4}px; font-weight: 600; color: {text}; }}
QLabel#hint {{ color: {muted}; font-size: {max(9, size - 2)}px; }}
QWidget#find_bar {{ background: {header}; border-top: 1px solid {border}; }}
QProgressBar {{ border: 1px solid {border}; border-radius: 2px; background: {bg}; height: 8px; text-align: center; }}
QProgressBar::chunk {{ background: {accent}; }}
QToolTip {{ background: {panel}; color: {text}; border: 1px solid {border}; padding: 3px 5px; }}
"""


def definition_css(prefs: Preferences) -> str:
    """Article style like GoldenDict: a thin header strip per dictionary, then
    the text — no cards, no shadows, little padding."""
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
        bg, fg, muted, card, border, strip = "#1b1d22", "#e3e5e9", "#9aa1ac", "#25282f", "#363a43", "#262a31"
        head_fg, link = _mix(accent, "#ffffff", 0.55), _mix(accent, "#ffffff", 0.35)
    else:
        bg, fg, muted, card, border, strip = "#ffffff", "#1f2328", "#6a717c", "#f3f5f8", "#d7dbe0", "#eef1f4"
        head_fg, link = _mix(accent, "#000000", 0.35), accent
    width = f"max-width: {maxw}px;" if maxw else ""
    return f"""
html {{ background: {bg}; }}
body {{ background: {bg}; color: {fg}; font-family: {family}"Segoe UI", "Inter", "Noto Sans", sans-serif;
        font-size: {size}px; line-height: {spacing}; padding: 0; margin: 0; }}
main {{ {width} }}
a {{ color: {link}; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
h1, h2, h3 {{ color: {head_fg}; font-weight: 600; margin: .4em 0; }}
details.dict {{ margin: 0; border: none; }}
details.dict + details.dict {{ border-top: 1px solid {border}; }}
details.dict > summary {{ list-style: none; cursor: pointer; display: flex; align-items: center; gap: 6px;
        padding: 2px 8px 2px 6px; border-left: 3px solid var(--dc, {accent});
        background: linear-gradient(90deg, var(--dt, {strip}) 0%, {strip} 70%);
        border-bottom: 1px solid {border}; font-size: 0.82em; font-weight: 600; color: {muted};
        user-select: none; line-height: 1.5; position: sticky; top: 0; z-index: 1; }}
details.dict > summary::-webkit-details-marker {{ display: none; }}
details.dict:not([open]) > summary {{ opacity: .8; border-bottom: none; }}
.dicon {{ flex: none; width: 16px; height: 16px; border-radius: 3px; display: inline-flex; align-items: center;
        justify-content: center; background: var(--dc, {accent}); color: #fff; font-size: 8.5px; font-weight: 700;
        letter-spacing: 0; line-height: 1; }}
.dicon.img {{ background: transparent; }}
.dicon img {{ width: 16px; height: 16px; object-fit: contain; }}
.dname {{ white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; color: {fg}; }}
.chip {{ flex: none; font-size: 0.86em; font-weight: 500; padding: 0 4px; border-radius: 2px; white-space: nowrap;
        color: var(--dk, {muted}); letter-spacing: .2px; }}
.chip.fmt {{ display: none; }}
details.dict > summary .count {{ flex: none; color: {muted}; font-weight: 400; }}
details.dict > summary .actions {{ margin-left: auto; white-space: nowrap; font-weight: 400; opacity: 0;
        transition: opacity .12s; }}
details.dict > summary:hover .actions {{ opacity: .9; }}
details.dict > summary .actions a {{ margin-left: 10px; color: {muted}; }}
details.dict > summary .actions a:hover {{ color: {link}; }}
details.dict > summary .chev {{ flex: none; color: {muted}; font-size: .9em; display: inline-block;
        transform: rotate(-90deg); }}
details.dict[open] > summary .chev {{ transform: none; }}
.dbody {{ padding: 4px 12px 8px 14px; }}
.nohead {{ border: none; }}
.nohead > summary {{ display: none !important; }}
.nohead > .dbody {{ padding: 4px 12px; }}
.article {{ padding: 2px 0 4px 0; }}
.article + .article {{ border-top: 1px dotted {border}; margin-top: 4px; padding-top: 6px; }}
.headword {{ font-size: {head:.2f}em; font-weight: 700; color: {head_fg}; margin: 2px 0 2px 0; line-height: 1.25; }}
.definition pre {{ white-space: pre-wrap; font-family: inherit; margin: 0; }}
.definition img {{ max-width: 100%; max-height: {img_h}px; vertical-align: middle; }}
.definition p {{ margin: .25em 0; }}
.definition ul, .definition ol {{ margin: .2em 0; padding-left: 1.4em; }}
a.dz-audio {{ display: inline-block; padding: 0 6px; margin: 0 2px; border-radius: 3px; background: {card};
        border: 1px solid {border}; font-size: 0.85em; }}
ul.did-you-mean {{ list-style: none; padding-left: 0; }}
ul.did-you-mean li {{ margin: 3px 0; }}
.score {{ color: {muted}; font-size: 0.8em; margin-left: 6px; }}
.muted {{ color: {muted}; }}
.page {{ padding: 10px 14px; }}
::selection {{ background: {hl}; color: #000; }}
::-webkit-scrollbar {{ width: 9px; height: 9px; background: transparent; }}
::-webkit-scrollbar-thumb {{ background: {_mix(border, fg, 0.2)}; border-radius: 4px; border: 1px solid {bg}; }}
::-webkit-scrollbar-thumb:hover {{ background: {_mix(border, fg, 0.4)}; }}
"""
