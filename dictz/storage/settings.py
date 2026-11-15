"""User preferences — QSettings wrapper with typed defaults."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from PySide6.QtCore import QSettings


@dataclass
class Preferences:
    theme: str = "auto"
    font_family: str = ""
    font_size: int = 14
    zoom_factor: float = 1.0
    search_debounce_ms: int = 150
    show_sidebar: bool = True
    sidebar_width: int = 280
    window_geometry: bytes | None = None
    window_state: bytes | None = None
    last_search: str = ""

    @classmethod
    def load(cls) -> "Preferences":
        qs = QSettings()
        p = cls()
        p.theme = qs.value("ui/theme", p.theme, type=str)
        p.font_family = qs.value("ui/font_family", p.font_family, type=str)
        p.font_size = qs.value("ui/font_size", p.font_size, type=int)
        p.zoom_factor = float(qs.value("ui/zoom_factor", p.zoom_factor))
        p.search_debounce_ms = qs.value("search/debounce_ms", p.search_debounce_ms, type=int)
        p.show_sidebar = qs.value("ui/show_sidebar", p.show_sidebar, type=bool)
        p.sidebar_width = qs.value("ui/sidebar_width", p.sidebar_width, type=int)
        p.window_geometry = qs.value("window/geometry", None)
        p.window_state = qs.value("window/state", None)
        p.last_search = qs.value("search/last", "", type=str)
        return p

    def save(self) -> None:
        qs = QSettings()
        qs.setValue("ui/theme", self.theme)
        qs.setValue("ui/font_family", self.font_family)
        qs.setValue("ui/font_size", self.font_size)
        qs.setValue("ui/zoom_factor", self.zoom_factor)
        qs.setValue("search/debounce_ms", self.search_debounce_ms)
        qs.setValue("ui/show_sidebar", self.show_sidebar)
        qs.setValue("ui/sidebar_width", self.sidebar_width)
        if self.window_geometry is not None:
            qs.setValue("window/geometry", self.window_geometry)
        if self.window_state is not None:
            qs.setValue("window/state", self.window_state)
        qs.setValue("search/last", self.last_search)

    def set(self, key: str, value: Any) -> None:
        setattr(self, key, value)
        self.save()
