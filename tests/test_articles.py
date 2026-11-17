"""Article store: built once, read on demand, same answers as the parsers."""
from __future__ import annotations

import os
from pathlib import Path

import pytest

from dictz.core.query_manager import QueryManager
from dictz.storage import articles
from dictz.storage.indexer import IndexStore
from dictz.storage.registry import path_id

from .test_stardict import _build_tiny_stardict

PARITY = Path(__file__).resolve().parent / "data" / "parity"


def _queries(d) -> list[str]:
    words = d.headwords()
    out = ["", "zzzz-none", "a", "A", "ap"]
    for w in words[:: max(1, len(words) // 15)]:
        out += [w, w.upper(), w.lower(), w[:2], w[:1].upper()]
    return out


def _same(parsed, stored) -> None:
    assert stored.headwords() == parsed.headwords()
    assert stored.metadata.word_count == len(parsed.headwords())
    for q in _queries(parsed):
        for exact in (True, False):
            a = [(e.headword, e.definition) for e in parsed.lookup(q, exact=exact)]
            b = [(e.headword, e.definition) for e in stored.lookup(q, exact=exact)]
            assert a == b, (q, exact)
    assert [(e.headword, e.definition) for e in stored.iter_entries()] == \
        [(e.headword, e.definition) for e in parsed.iter_entries()]


@pytest.mark.parametrize("sample", ["sample.dsl", "sample.bgl", "stardict"])
def test_store_answers_like_the_parser(tmp_path, sample) -> None:
    src = _build_tiny_stardict(tmp_path) if sample == "stardict" else PARITY / sample
    parsed = QueryManager.open_file(src)
    store = articles.store_path(tmp_path, path_id(src))
    n = articles.build(parsed, store)
    assert n == len(parsed.headwords())
    assert articles.is_fresh(store, src)
    stored = articles.StoredDictionary(store, src)
    for k in ("name", "description", "author", "source_language", "target_language", "format"):
        assert getattr(stored.metadata, k) == (getattr(parsed.metadata, k) or ""), k
    _same(parsed, stored)
    stored.close()


def test_store_goes_stale_when_the_file_changes(tmp_path) -> None:
    src = _build_tiny_stardict(tmp_path)
    store = articles.store_path(tmp_path, path_id(src))
    articles.build(QueryManager.open_file(src), store)
    assert articles.is_fresh(store, src)
    dict_file = src.with_suffix(".dict")
    st = dict_file.stat()
    os.utime(dict_file, ns=(st.st_atime_ns, st.st_mtime_ns + 5_000_000_000))
    assert not articles.is_fresh(store, src)
    d, built = QueryManager.open_cached(src, tmp_path)
    assert built and isinstance(d, articles.StoredDictionary)
    d2, built2 = QueryManager.open_cached(src, tmp_path)
    assert not built2 and d2.lookup("apple")[0].definition.startswith("elma")


def test_start_reads_the_store_not_the_file(tmp_path) -> None:
    src = _build_tiny_stardict(tmp_path)
    data = tmp_path / "data"
    store = IndexStore(data / "dictz_index.db")
    store.open()
    qm = QueryManager(store)
    qm.add_dictionary(src)
    assert articles.store_path(data, path_id(src)).is_file()
    qm.close_all()
    qm2 = QueryManager(store)
    loaded, errors = qm2.restore_from_index()
    assert errors == [] and loaded == ["Tiny Test Dictionary"]
    assert isinstance(qm2.dictionaries[loaded[0]], articles.StoredDictionary)
    assert qm2.search("Apple")[0].headword == "apple"
    # removing the dictionary removes its store
    qm2.remove_dictionary(loaded[0])
    assert not articles.store_path(data, path_id(src)).exists()
    store.close()


def test_golden_store_is_readable() -> None:
    """``articles_golden.db`` is a store the Python edition wrote; the Rust
    edition's parity test reads the same file."""
    g = PARITY / "articles_golden.db"
    stored = articles.StoredDictionary(g, PARITY / "sample.dsl")
    _same(QueryManager.open_file(PARITY / "sample.dsl"), stored)
    stored.close()
