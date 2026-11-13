"""DSL (ABBYY Lingvo) dictionary parser."""
from __future__ import annotations

from .pyglossary_base import PyGlossaryDictionary


class DSLDictionary(PyGlossaryDictionary):
    """ABBYY Lingvo DSL dictionary (.dsl / .dsl.dz)."""

    _format = "ABBYYLingvoDSL"
    format_label = "dsl"
