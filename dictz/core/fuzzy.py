""""Did you mean" matching.

The score is RapidFuzz's ``fuzz.ratio`` (normalised InDel similarity × 100) on
lower-cased strings. The Rust edition (``dictz-rs/src/fuzzy.rs``) computes the
same number, and ``tests/data/parity/fuzzy_golden.json`` checks both.
"""
from __future__ import annotations

from collections.abc import Iterable

from rapidfuzz import fuzz, process

DEFAULT_CUTOFF = 60.0
DEFAULT_LIMIT = 10


def ratio(a: str, b: str) -> float:
    """Case-insensitive similarity of two strings, 0–100."""
    return fuzz.ratio(a.lower(), b.lower())


def extract(query: str, candidates: Iterable[str], limit: int = DEFAULT_LIMIT,
            cutoff: float = DEFAULT_CUTOFF) -> list[tuple[str, float]]:
    """Best ``limit`` candidates scoring at least ``cutoff``, best first.

    Ties keep candidate order (RapidFuzz ``process.extract`` semantics).
    """
    words = list(candidates)
    if not words:
        return []
    lowered = [w.lower() for w in words]
    hits = process.extract(query.lower(), lowered, scorer=fuzz.ratio, limit=None, score_cutoff=cutoff)
    hits.sort(key=lambda h: (-h[1], h[2]))
    return [(words[idx], score) for _, score, idx in hits[:limit]]


def rank(query: str, words: list[str]) -> list[str]:
    """Order prefix suggestions closest-first: score descending, then shorter,
    then case-insensitive alphabetical, then exact string."""
    q = query.lower()
    return sorted(words, key=lambda w: (-fuzz.ratio(q, w.lower()), len(w), w.lower(), w))
