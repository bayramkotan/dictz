"""DSL (ABBYY Lingvo) dictionary parser.

DSL is a text-based markup format used by ABBYY Lingvo. Files end in .dsl
(or .dsl.dz when dictzip-compressed) and contain headwords followed by
indented, markup-rich definitions.

We delegate the actual parsing to PyGlossary's ABBYYLingvoDSL plugin, which
handles the markup (phonetic brackets, cross-references, formatting tags)
and converts it to HTML.
"""
from __future__ import annotations

from .pyglossary_base import PyGlossaryDictionary


class DSLDictionary(PyGlossaryDictionary):
    """ABBYY Lingvo DSL dictionary (.dsl / .dsl.dz)."""

    _format = "ABBYYLingvoDSL"
    format_label = "dsl"
