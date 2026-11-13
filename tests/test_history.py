"""Tests for the history + favorites store."""
from __future__ import annotations

from pathlib import Path

from dictz.storage.history import HistoryStore


def test_record_and_recent(tmp_path: Path) -> None:
    s = HistoryStore(tmp_path / "h.db")
    s.open()
    s.record("apple")
    s.record("book")
    s.record("cat")
    s.record("apple")  # duplicate — should collapse but still bubble up
    recent = s.recent(10)
    assert recent[0] == "apple"
    assert set(recent) == {"apple", "book", "cat"}
    s.close()


def test_favorites_toggle(tmp_path: Path) -> None:
    s = HistoryStore(tmp_path / "h.db")
    s.open()
    assert s.toggle_favorite("elma") is True
    assert s.is_favorite("elma")
    assert "elma" in s.favorites()
    assert s.toggle_favorite("elma") is False
    assert not s.is_favorite("elma")
    s.close()


def test_favorite_note(tmp_path: Path) -> None:
    s = HistoryStore(tmp_path / "h.db")
    s.open()
    s.add_favorite("kitap", "to review")
    assert s.get_note("kitap") == "to review"
    s.set_note("kitap", "already reviewed")
    assert s.get_note("kitap") == "already reviewed"
    s.close()


def test_clear_history(tmp_path: Path) -> None:
    s = HistoryStore(tmp_path / "h.db")
    s.open()
    s.record("a")
    s.record("b")
    assert len(s.recent()) == 2
    s.clear_history()
    assert s.recent() == []
    s.close()


def test_favorites_persist_sort(tmp_path: Path) -> None:
    """Newest favorite comes first."""
    import time
    s = HistoryStore(tmp_path / "h.db")
    s.open()
    s.add_favorite("alpha")
    time.sleep(0.01)
    s.add_favorite("beta")
    favs = s.favorites()
    assert favs[0] == "beta"
    assert favs[1] == "alpha"
    s.close()
