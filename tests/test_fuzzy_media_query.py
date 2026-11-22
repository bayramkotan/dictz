"""v0.0.6: fuzzy search, DSL media, dictionary order and restore."""
from __future__ import annotations

import base64
import struct
import zipfile
from pathlib import Path

import pytest

from dictz.core import fuzzy
from dictz.core.media import (
    MediaResolver,
    audio_url,
    media_kind,
    parse_audio_url,
    rewrite_media_html,
)
from dictz.core.query_manager import QueryManager
from dictz.storage.indexer import IndexStore


def write_stardict(base: Path, name: str, entries: list[tuple[str, str]]) -> Path:
    body, idx = b"", b""
    for word, definition in entries:
        data = definition.encode()
        idx += word.encode() + b"\x00" + struct.pack(">II", len(body), len(data))
        body += data
    base.with_suffix(".dict").write_bytes(body)
    base.with_suffix(".idx").write_bytes(idx)
    base.with_suffix(".ifo").write_text(
        f"StarDict's dict ifo file\nversion=2.4.2\nbookname={name}\nwordcount={len(entries)}\n",
        encoding="utf-8")
    return base.with_suffix(".ifo")


# ------------------------------------------------------------ fuzzy

def test_ratio_values() -> None:
    assert fuzzy.ratio("aple", "apple") == 88.88888888888889
    assert fuzzy.ratio("Apple", "APPLE") == 100.0
    assert fuzzy.ratio("", "") == 100.0


def test_extract_and_rank() -> None:
    r = fuzzy.extract("appel", ["apple", "apply", "maple", "zebra", "ample"], limit=3)
    assert r[0] == ("apple", 80.0)
    assert all(score >= fuzzy.DEFAULT_CUTOFF for _, score in r)
    assert "zebra" not in [w for w, _ in r]
    assert fuzzy.rank("car", ["carpet", "car", "cart"]) == ["car", "cart", "carpet"]


# ------------------------------------------------------------ media

def test_media_kind() -> None:
    assert media_kind("a.PNG") == "image"
    assert media_kind("x/y.wav") == "audio"
    assert media_kind("z.txt") == "other"


def test_resolver_dir_zip_and_escape(tmp_path: Path) -> None:
    (tmp_path / "En.dsl.files").mkdir()
    (tmp_path / "En.dsl.files" / "Pic.PNG").write_bytes(b"png")
    with zipfile.ZipFile(tmp_path / "En.dsl.files.zip", "w") as z:
        z.writestr("snd/hello.wav", b"wav")
    (tmp_path / "secret.txt").write_bytes(b"s")
    r = MediaResolver(tmp_path / "En.dsl.dz")
    assert r.read("pic.png") == b"png"
    assert r.read("Hello.WAV") == b"wav"
    assert r.read("secret.txt") == b"s"
    assert r.read("../etc/passwd") is None
    assert r.read("/etc/passwd") is None
    assert r.read("missing.png") is None


def test_rewrite_media_html(tmp_path: Path) -> None:
    with zipfile.ZipFile(tmp_path / "d.dsl.files.zip", "w") as z:
        z.writestr("p.png", b"\x89PNG")
    r = MediaResolver(tmp_path / "d.dsl")
    html = ('<img align="top" src="p.png" alt="p.png" /> <img src="none.png"/>'
            '<object type="audio/x-wav" data="s.wav" width="40" height="40"><param name="autoplay" value="false" /></object>'
            '<a href="sound://x.mp3">x</a>')
    out = rewrite_media_html(html, "My Dict", r)
    assert "data:image/png;base64," + base64.b64encode(b"\x89PNG").decode() in out
    assert 'src="none.png"' in out
    assert "<object" not in out
    assert audio_url("My Dict", "s.wav") in out
    assert audio_url("My Dict", "x.mp3") in out
    assert parse_audio_url(audio_url("My Dict", "a b/ç.wav")) == ("My Dict", "a b/ç.wav")
    assert parse_audio_url("bword://x") is None


# ------------------------------------------------------------ query manager

def test_order_enable_fuzzy_and_restore(tmp_path: Path) -> None:
    a = write_stardict(tmp_path / "a", "A", [("apple", "elma"), ("apply", "uygula")])
    b = write_stardict(tmp_path / "b", "B", [("apple", "Apfel"), ("banana", "muz")])
    store = IndexStore(tmp_path / "index.db")
    store.open()
    qm = QueryManager(store)
    qm.add_dictionary(a)
    qm.add_dictionary(b)
    assert qm.loaded_names() == ["A", "B"]
    qm.move_dictionary("B", -1)
    assert qm.loaded_names() == ["B", "A"]
    qm.move_before("B", None)  # drag to the end
    assert qm.loaded_names() == ["A", "B"]
    qm.move_before("B", "A")  # drag onto the first one
    assert qm.loaded_names() == ["B", "A"]
    qm.set_order(["A", "nope"])
    assert qm.loaded_names() == ["A", "B"]
    qm.set_order(["B", "A"])
    assert qm.search("apple")[0].definition == "Apfel"
    qm.set_enabled("B", False)
    assert len(qm.search("apple")) == 1

    s = qm.suggest("app", 30)
    assert s[0].word == "apple" and not s[0].fuzzy
    assert qm.fuzzy("aple", 5)[0][0] == "apple"
    assert any(x.fuzzy and x.word == "apple" for x in qm.suggest("aplpe", 30))
    # disabled dictionaries never leak into suggestions
    assert all(x.word != "banana" for x in qm.suggest("ban", 30))
    store.close()

    store2 = IndexStore(tmp_path / "index.db")
    store2.open()
    qm2 = QueryManager(store2)
    loaded, errors = qm2.restore_from_index()
    assert errors == []
    assert loaded == ["B", "A"]
    assert not qm2.is_enabled("B")
    assert qm2.is_enabled("A")
    store2.close()


def test_restore_reports_missing_files(tmp_path: Path) -> None:
    a = write_stardict(tmp_path / "a", "A", [("x", "y")])
    store = IndexStore(tmp_path / "i.db")
    store.open()
    QueryManager(store).add_dictionary(a)
    for f in tmp_path.glob("a.*"):
        f.unlink()
    loaded, errors = QueryManager(store).restore_from_index()
    assert loaded == [] and len(errors) == 1
    store.close()


def test_search_headword_empty_filter_means_none(tmp_path: Path) -> None:
    a = write_stardict(tmp_path / "a", "A", [("apple", "elma")])
    store = IndexStore(tmp_path / "i.db")
    store.open()
    QueryManager(store).add_dictionary(a)
    assert store.search_headword("app", dictionaries=[]) == []
    assert store.search_headword("app", dictionaries=["A"]) == [("apple", "A")]
    store.close()


@pytest.mark.parametrize("zip_name", ["OALD.dsl.dz.files.zip", "OALD.DSL.DZ.FILES.ZIP", "OALD.files.zip"])
def test_resolver_finds_goldendict_zip_names(tmp_path: Path, zip_name: str) -> None:
    """``X.dsl.dz`` with ``X.dsl.dz.files.zip`` (the usual GoldenDict packs,
    e.g. OALD8): the zip keeps the ``.dz`` in its name; names match in any case."""
    (tmp_path / "OALD.dsl.dz").write_bytes(b"")
    with zipfile.ZipFile(tmp_path / zip_name, "w") as z:
        z.writestr("z_house__gb_2.wav", b"wav")
        z.writestr("pics/x_verb_forms_house.jpg", b"jpg")
    r = MediaResolver(tmp_path / "OALD.dsl.dz")
    assert r.read("z_house__gb_2.wav") == b"wav"
    assert r.read("x_verb_forms_house.jpg") == b"jpg"
    assert r.read("X_VERB_FORMS_HOUSE.JPG") == b"jpg"
    r.close()


def test_resolver_finds_dz_files_folder(tmp_path: Path) -> None:
    (tmp_path / "OALD.dsl.dz.files").mkdir()
    (tmp_path / "OALD.dsl.dz.files" / "a.wav").write_bytes(b"w")
    assert MediaResolver(tmp_path / "OALD.dsl.dz").read("a.wav") == b"w"


def test_suggestion_thread_answers_like_suggest(tmp_path: Path) -> None:
    """The background suggestion thread gives what ``QueryManager.suggest``
    gives; only the newest request of a kind is answered."""
    import threading

    from dictz.core.suggest import Candidates, SuggestWorker

    a = write_stardict(tmp_path / "A", "A", [("apple", "elma"), ("apply", "uygula"), ("banana", "muz")])
    store = IndexStore(tmp_path / "index.db")
    qm = QueryManager(store)
    qm.add_dictionary(a)
    got: dict[str, object] = {}
    done = threading.Event()

    def on_result(kind, query, result):
        got[kind] = (query, result)
        if "suggest" in got and "fuzzy" in got:
            done.set()

    w = SuggestWorker(store.db_path, on_result, qm.candidates)
    w.start()
    snap = qm.enabled_snapshot()
    w.submit("suggest", "ap", snap, limit=30, fuzzy_fill_below=10, fuzzy_limit=10, fuzzy_cutoff=60.0)
    w.submit("fuzzy", "aplpe", snap, limit=5, cutoff=60.0)
    assert done.wait(10)
    w.stop()
    assert got["suggest"][0] == "ap"
    assert [s.word for s in got["suggest"][1]] == [s.word for s in qm.suggest("ap", 30)]
    assert got["fuzzy"] == ("aplpe", qm.fuzzy("aplpe", 5))
    # the candidate list is built once per set of dictionaries and shared
    assert qm.candidates.ready(snap)
    assert qm.fuzzy("aple", 5, wait=False)[0][0] == "apple"
    c = Candidates()
    assert c.get(snap, wait=True)[1] == ["apple", "apply", "banana"]
    qm.set_enabled("A", False)
    assert not qm.candidates.ready(qm.enabled_snapshot())
    store.close()


def test_stardict_synonyms_parser_store_and_index(tmp_path: Path) -> None:
    """StarDict .syn: "went" finds "go" in the parser and in the article store,
    and the index / "did you mean" know the synonyms (parity sample)."""
    import json

    from dictz.parsers.stardict import StarDictDictionary
    from dictz.storage import articles

    parity = Path(__file__).parent / "data" / "parity"
    g = json.loads((parity / "stardict_golden.json").read_text(encoding="utf-8"))
    parity_sd = parity / "stardict"
    parsed = StarDictDictionary(parity_sd / "sample.ifo")
    parsed.load()
    store = tmp_path / "s.db"
    articles.build(parsed, store)
    stored = articles.StoredDictionary(store, parity_sd / "sample.ifo")
    for d in (parsed, stored):
        assert [list(p) for p in d.synonyms()] == g["synonyms"]
        assert d.search_words() == g["search_words"]
        for key, want in g["lookups"].items():
            q, exact = key.rsplit("|", 1)
            assert [e.headword for e in d.lookup(q, exact == "1")] == want, key
    assert stored.lookup("went")[0].definition == "gitmek"
    assert "sample.syn" in [p.name for p in articles.source_files(parity_sd / "sample.ifo")]
    # a store from before v0.2.0 (no synonyms table) still works
    import sqlite3

    con = sqlite3.connect(store)
    con.execute("DROP TABLE synonyms")
    con.commit()
    con.close()
    old = articles.StoredDictionary(store, parity_sd / "sample.ifo")
    assert old.synonyms() == [] and old.lookup("went") == [] and old.lookup("go")[0].definition == "gitmek"
    # the index offers synonyms
    idx = IndexStore(tmp_path / "index.db")
    qm = QueryManager(idx)
    qm.add_dictionary(parity_sd / "sample.ifo")
    assert "went" in [s.word for s in qm.suggest("wen")]
    assert qm.search("went")[0].headword == "go"
    idx.close()
