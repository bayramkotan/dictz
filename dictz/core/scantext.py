"""Turning copied / selected text into a word to look up (scan popup).

The Rust edition has the same rules (``dictz-rs/src/scantext.rs``), checked by
``tests/data/parity/scan_golden.json``.
"""
from __future__ import annotations

# Characters stripped from both ends of a selection.
TRIM = " \t\r\n .,;:!?¡¿\"'`()[]{}<>«»“”„‘’‚‹›…-–—/\\|*#@&%^~_=+"


def clean_scan_text(text: str | None, max_length: int = 80) -> str | None:
    """The word or phrase to look up, or ``None`` when the text is not one.

    Whitespace runs become one space, punctuation around the text is removed,
    and text longer than ``max_length`` characters (after cleaning) or with
    more than one line is ignored.
    """
    if not text:
        return None
    text = text.strip(TRIM)
    if not text or "\n" in text or "\r" in text:
        return None
    text = " ".join(text.split())
    if not text or len(text) > max_length:
        return None
    if not any(c.isalpha() for c in text):
        return None
    return text


class DoubleCopy:
    """Ctrl+C+C: the same text copied twice within ``interval_ms``."""

    def __init__(self, interval_ms: int = 500) -> None:
        self.interval_ms = interval_ms
        self._last_text: str | None = None
        self._last_ms: float = -1e18

    def feed(self, text: str, now_ms: float) -> bool:
        hit = text == self._last_text and 0 <= now_ms - self._last_ms <= self.interval_ms
        if hit:
            # a third copy does not trigger again
            self._last_text = None
            self._last_ms = -1e18
        else:
            self._last_text = text
            self._last_ms = now_ms
        return hit
