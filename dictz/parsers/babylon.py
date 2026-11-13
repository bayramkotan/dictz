"""Babylon (.bgl) dictionary parser.

Babylon is a proprietary binary format from the Babylon translator. The format
was reverse-engineered years ago; PyGlossary includes a mature parser that
handles the binary encoding, embedded images, and multi-byte character sets.
"""
from __future__ import annotations

from .pyglossary_base import PyGlossaryDictionary


class BabylonDictionary(PyGlossaryDictionary):
    """Babylon binary dictionary (.bgl)."""

    _format = "BabylonBgl"
    format_label = "babylon"
