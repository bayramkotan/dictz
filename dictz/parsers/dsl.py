"""DSL (ABBYY Lingvo) dictionary parser."""
from __future__ import annotations

from .pyglossary_base import PyGlossaryDictionary


class DSLDictionary(PyGlossaryDictionary):
    _format = "ABBYYLingvoDSL"
    format_label = "dsl"
    _unescape_terms = True
