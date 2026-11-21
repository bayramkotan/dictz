"""Background loading: several dictionaries prepared at once, the list and the
index written by one thread, more work taken while running, cancel / stop."""
from __future__ import annotations

import threading
from pathlib import Path

import pytest

from dictz.core import loading
from dictz.core.query_manager import QueryManager
from dictz.storage import articles
from dictz.storage.indexer import IndexStore
from dictz.storage.registry import path_id

from .test_stardict import _build_tiny_stardict

PARITY = Path(__file__).resolve().parent / "data" / "parity"


def _copies(tmp_path: Path, n: int) -> list[Path]:
    out = []
    for i in range(n):
        d = tmp_path / f"src{i}"
        d.mkdir()
        out.append(_build_tiny_stardict(d))
    return out


def _job(data: Path, **kw):
    got, failed, states = [], [], []
    job = loading.LoadJob(data / "dictz_index.db", mode=kw.pop("mode", "thread"),
                          on_loaded=lambda d, p, e, r: got.append((d, p, e, r)),
                          on_failed=lambda p, e: failed.append((p, e)),
                          on_progress=states.append, **kw)
    return job, got, failed, states


def test_worker_count() -> None:
    assert loading.worker_count(3, 10) == 3
    assert loading.worker_count(8, 2) == 2  # never more than the work
    assert 1 <= loading.worker_count(0, 100) <= loading.MAX_AUTO_WORKERS
    assert loading.worker_count(0, 0) == 1


@pytest.mark.parametrize("mode", ["thread", "process"])
def test_new_files_are_prepared_in_parallel_and_listed(tmp_path, mode) -> None:
    files = _copies(tmp_path, 3) + [PARITY / "sample.dsl"]
    data = tmp_path / "data"
    IndexStore(data / "dictz_index.db").open()
    job, got, failed, states = _job(data, files=files, workers=2, mode=mode)
    job.run()
    assert failed == []
    assert sorted(p for _d, p, _e, _r in got) == sorted(str(f) for f in files)
    for f in files:
        assert articles.is_fresh(articles.store_path(data, path_id(f)), f)
    store = IndexStore(data / "dictz_index.db")
    store.open()
    names = [r["name"] for r in store.list_dictionaries(enabled_only=False)]
    assert len(names) == 4 and len(set(names)) == 4  # "Tiny Test Dictionary (2)", …
    assert all(store.is_indexed(n) for n in names)
    assert states[-1].running == [] and states[-1].done == states[-1].total == 4
    assert any(s.phase == "prepare" for s in states)
    for d, *_ in got:
        d.close()
    store.close()


def test_list_rows_open_from_json_and_changed_files_are_rebuilt(tmp_path) -> None:
    import os

    files = _copies(tmp_path, 2)
    data = tmp_path / "data"
    job, got, *_ = _job(data, files=files)
    job.run()
    for d, *_ in got:
        d.close()
    store = IndexStore(data / "dictz_index.db")
    store.open()
    rows = [dict(r) for r in store.list_dictionaries(enabled_only=False)]
    store.close()
    dict_file = files[1].with_suffix(".dict")
    st = dict_file.stat()
    os.utime(dict_file, ns=(st.st_atime_ns, st.st_mtime_ns + 5_000_000_000))
    job, got, failed, _ = _job(data, rows=rows)
    job.run()
    assert failed == [] and len(got) == 2
    lazy = {p: getattr(d, "lazy", False) for d, p, _e, _r in got}
    assert lazy[str(files[0])] is True  # unchanged: from dictionaries.json alone
    assert lazy[str(files[1])] is False  # changed: prepared by a worker again
    assert got[0][0].lookup("apple")[0].definition.startswith("elma")
    for d, *_ in got:
        d.close()


def test_work_added_while_running_is_taken_by_the_same_job(tmp_path) -> None:
    first, second = _copies(tmp_path, 2)
    data = tmp_path / "data"
    gate = threading.Event()
    calls = []

    def slow_scan():
        gate.wait(5)
        calls.append("scan")
        return [first]

    job, got, *_ = _job(data, scans=[slow_scan])
    t = threading.Thread(target=job.run)
    t.start()
    assert job.add(files=[second])  # taken while the scan waits
    gate.set()
    t.join(20)
    assert not job.add(files=[second])  # finished: the window starts a new job
    assert calls == ["scan"] and sorted(p for _d, p, _e, _r in got) == sorted([str(first), str(second)])
    for d, *_ in got:
        d.close()


def test_rebuild_keeps_the_list_name(tmp_path) -> None:
    (f,) = _copies(tmp_path, 1)
    data = tmp_path / "data"
    store = IndexStore(data / "dictz_index.db")
    store.open()
    qm = QueryManager(store)
    qm.add_dictionary(f)
    name = qm.loaded_names()[0]
    qm.dictionaries[name].suspend()
    assert qm.dictionaries[name].lookup("apple") == []  # suspended: answers nothing
    job, got, failed, _ = _job(data)
    job.add(rebuild=[(name, str(f), True)])
    job.run()
    assert failed == [] and got[0][3] == name and got[0][0].metadata.name == name
    qm.attach(got[0][0], str(f), True)
    assert qm.loaded_names() == [name]  # kept its place
    assert qm.search("apple")[0].headword == "apple"
    qm.close_all()
    store.close()


def test_cancel_adds_no_more_new_files(tmp_path) -> None:
    files = _copies(tmp_path, 3)
    data = tmp_path / "data"
    job, got, *_ = _job(data, files=files, workers=1)
    job.cancel()
    job.run()
    assert got == []


def test_a_broken_file_is_reported_and_the_rest_goes_on(tmp_path) -> None:
    (good,) = _copies(tmp_path, 1)
    bad = tmp_path / "broken.ifo"
    bad.write_text("not a StarDict header")  # and no .idx / .dict next to it
    data = tmp_path / "data"
    job, got, failed, _ = _job(data, files=[bad, good])
    job.run()
    assert [p for _d, p, _e, _r in got] == [str(good)]
    assert failed and failed[0][0] == str(bad)
    for d, *_ in got:
        d.close()
