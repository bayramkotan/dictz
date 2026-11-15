"""Babylon (.bgl) dictionary parser."""
from __future__ import annotations

from .pyglossary_base import PyGlossaryDictionary


class BabylonDictionary(PyGlossaryDictionary):
    _format = "BabylonBgl"
    format_label = "babylon"
