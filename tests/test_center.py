"""Dictionary Center: sources, adapters (answers from files), catalog,
install (a local HTTP server), import / export, catalog build / check."""
from __future__ import annotations

import hashlib
import http.server
import io
import json
import tarfile
import threading
import zipfile
from pathlib import Path

import pytest

from dictz.center import adapters, langs, licenses, model
from dictz.center.catalog import Catalog
from dictz.center.install import Installer, unpack
from dictz.center.net import Http
from dictz.center.sources import BUILTIN, Sources, clean, guess_type
from dictz.center.transfer import (
    build_catalog,
    check_catalog,
    export_list,
    export_setup,
    import_setup,
)

HERE = Path(__file__).resolve().parent
FIX = HERE / "data" / "center"
GOLDEN = HERE / "data" / "parity" / "center_golden.json"


class FakeHttp:
    """Answers from tests/data/center (fixtures.json: "METHOD url" → file)."""

    def __init__(self):
        self.map = json.loads((FIX / "fixtures.json").read_text(encoding="utf-8"))["responses"]
        self.calls: list[str] = []

    def request(self, method, url, body=None, headers=None):
        key = f"{method} {url}"
        self.calls.append(key)
        if key not in self.map:
            from dictz.center.net import NetError

            raise NetError(f"HTTP 404: {url}", 404)
        return (FIX / self.map[key]).read_bytes()

    def get(self, url, headers=None):
        return self.request("GET", url, None, headers)


def fixture_sources() -> list[dict]:
    return [clean(s) for s in json.loads((FIX / "fixtures.json").read_text(encoding="utf-8"))["sources"]]


def all_entries() -> dict[str, list[dict]]:
    http = FakeHttp()
    return {s["id"]: adapters.read(s, http) for s in fixture_sources()}


def test_adapters_match_the_golden_file() -> None:
    """The Rust edition reads the same answers into the same entries."""
    got = all_entries()
    want = json.loads(GOLDEN.read_text(encoding="utf-8"))
    assert got == want


def test_adapter_details() -> None:
    e = all_entries()
    cat = {x["id"]: x for x in e["dictz-data"]}
    w = cat["dictz-data:en-tr-wordlist"]
    assert w["source_lang"] == "en" and w["target_lang"] == ["tr"] and w["kind"] == "bilingual"
    assert w["files"][0]["url"] == "https://srv.example.org/data/files/en-tr-wordlist.tar.xz"
    assert w["files"][0]["sha256"] == "ab12" and w["files"][0]["archive"] == "tar.xz"
    assert cat["dictz-data:ota-tr"]["kind"] == "multilingual" and cat["dictz-data:ota-tr"]["license"] == "GPL-3.0-or-later"
    assert cat["dictz-data:bg-mono"]["kind"] == "monolingual" and cat["dictz-data:bg-mono"]["format"] == "babylon"
    assert cat["dictz-data:bg-mono"]["files"][0]["url"] == "https://srv.example.org/data/extra/bg.bgl"
    assert not model.installable(cat["dictz-data:no-files"])
    fd = {x["id"]: x for x in e["freedict"]}
    assert fd["freedict:afr-eng"]["format"] == "stardict" and fd["freedict:afr-eng"]["files"][0]["sha512"] == "bb22"
    assert fd["freedict:afr-eng"]["name"] == "FreeDict Afrikaans → English"
    assert fd["freedict:eng-tur"]["format"] == "slob" and fd["freedict:eng-tur"]["supported"] is False
    assert len(fd) == 2  # no releases / software entries skipped
    gh = {x["name"]: x for x in e["wiktionary-xxyzz"]}
    assert set(gh) == {"en_en", "de_en"} and gh["de_en"]["source_lang"] == "de" and gh["en_en"]["kind"] == "monolingual"
    vz = {x["name"]: x for x in e["wiktionary-vuizur"]}
    assert vz["Ancient Greek-English Wiktionary dictionary"]["source_lang"] == "grc"
    assert len(vz) == 2
    hz = {x["name"]: x for x in e["huzheng"]}
    assert "cedict-gb" in hz and len(hz["cedict-gb"]["files"]) == 3 and hz["cedict-gb"]["format"] == "stardict"
    assert hz["stardict-English-Turkish-2.4.2"]["source_lang"] == "en"
    assert "x" not in hz and all("other.example.com" not in f["url"] for x in hz.values() for f in x["files"])
    fm = {x["name"]: x for x in e["freemdict"]}
    assert len(fm["Longman DOCE5"]["files"]) == 2 and fm["Longman DOCE5"]["supported"] is False
    assert fm["Oxford En-En"]["format"] == "dsl" and fm["Oxford En-En"]["size"] == 777
    ao = {x["id"]: x for x in e["archive-org"]}
    assert ao["archive-org:stardict-en-ru"]["license"] == "CC-BY-SA-3.0"
    assert ao["archive-org:stardict-en-ru"]["description"] == "Big pack of dictionaries"
    assert ao["archive-org:babylon_glossaries"]["resolve"] == "archive-org:babylon_glossaries"
    tg = e["telegram"]
    assert [x["name"] for x in tg] == ["Deutsch-Russisch", "Oxford English-Turkish"]
    assert tg[1]["size"] == int(12.5 * (1 << 20)) and not model.installable(tg[1])


def test_resolve_archive_org() -> None:
    files = adapters.resolve_archive_org("stardict-en-ru", FakeHttp())
    assert [f["name"] for f in files] == ["Mueller.tar.bz2"]
    assert files[0]["url"] == "https://archive.org/download/stardict-en-ru/stardict-en-ru/Mueller.tar.bz2"


def test_languages_and_licenses() -> None:
    assert langs.norm("eng") == "en" and langs.norm("Türkçe") == "tr" and langs.norm("Akkadian") == "akk"
    assert langs.label("en") == "English (en)"
    assert licenses.norm("gplv2") == "GPL-2.0-or-later" and licenses.get("whatever").status == "unknown"
    assert licenses.status_matches("MIT", "free") and not licenses.status_matches("unknown", "open")
    assert adapters.license_from_url("https://creativecommons.org/licenses/by/4.0/") == "CC-BY-4.0"
    assert adapters.parse_size("1.5 GB") == int(1.5 * (1 << 30))


def test_formats() -> None:
    assert model.format_of("freedict-deu-eng-1.0.stardict.tar.xz") == "stardict"
    assert model.format_of("x.dsl.dz") == "dsl" and model.format_of("y.zip") == "archive"
    assert model.archive_of("a.TAR.BZ2") == "tar.bz2" and model.archive_of("a.dict.dz") == ""


def test_sources_file(tmp_path: Path) -> None:
    s = Sources(tmp_path)
    ids = [x["id"] for x in s.all()]
    assert ids == [b["id"] for b in BUILTIN] and s.default_id() == "dictz-data"
    mine = s.add({"name": "My server", "type": "dictz-catalog", "url": "https://me.example/catalog.json"})
    assert mine["id"] == "my-server" and not mine["builtin"]
    assert s.add({"name": "My server", "url": "x"})["id"] == "my-server-2"
    s.update("freedict", url="https://mirror.example/fd.json", enabled=False)
    assert s.get("freedict")["url"] == "https://mirror.example/fd.json" and not s.get("freedict")["enabled"]
    s.remove("huzheng")
    assert s.get("huzheng") is None and Sources(tmp_path).get("huzheng") is None  # stays removed
    s.set_default("my-server")
    assert s.default_id() == "my-server" and s.all()[0]["id"] == "my-server"
    s.set_secret("my-server", "pw", "")
    assert s.secret("my-server") == ("pw", "")
    out = tmp_path / "list.json"
    assert s.export(out, include_builtin=False) == 2
    assert "pw" not in out.read_text(encoding="utf-8")
    other = Sources(tmp_path / "b")
    other.import_(out)
    assert other.get("my-server") is not None
    s.reset()
    assert s.get("huzheng") is not None and s.get("my-server") is not None
    assert guess_type("https://x/freedict-database.json") == "freedict"
    assert guess_type("https://github.com/a/b") == "github-folder"
    assert guess_type("https://cloud.x.com/index.php/s/abc") == "nextcloud-share"


def test_catalog_cache_and_search(tmp_path: Path) -> None:
    src = Sources(tmp_path)
    for b in BUILTIN:
        src.remove(b["id"])
    for s in fixture_sources():
        src.add(s)
    cat = Catalog(tmp_path, src, FakeHttp())
    res = cat.refresh()
    assert all(r["ok"] for r in res.values()), {k: r["error"] for k, r in res.items()}
    assert not cat.stale("freedict", 7)
    hits = cat.search(src="en", dst="tr")
    assert {h["name"] for h in hits} >= {"English–Turkish Word List", "stardict-English-Turkish-2.4.2"}
    assert all(licenses.status_matches(h["license"], "open") for h in cat.search(license="open"))
    assert cat.find("FreeDict Afrikaans → English")[0]["id"] == "freedict:afr-eng"
    # a failing source keeps its last list
    cat.http.map.pop("GET https://freedict.org/freedict-database.json")
    r = cat.refresh(["freedict"])["freedict"]
    assert not r["ok"] and len(r["entries"]) == 2


# ---------------------------------------------------------------- install

def _stardict_tar(tmp: Path) -> bytes:
    from .test_stardict import _build_tiny_stardict

    (tmp / "sd").mkdir(parents=True, exist_ok=True)
    ifo = _build_tiny_stardict(tmp / "sd")
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:xz") as t:
        for f in sorted(ifo.parent.iterdir()):
            t.add(f, arcname=f"tiny/{f.name}")
        info = tarfile.TarInfo("../evil.txt")
        info.size = 4
        t.addfile(info, io.BytesIO(b"evil"))
    return buf.getvalue()


class _Server:
    def __init__(self, files: dict[str, bytes]):
        files_ = files

        class H(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):  # noqa: N802
                body = files_.get(self.path)
                if body is None:
                    self.send_response(404)
                    self.end_headers()
                    return
                start = 0
                rng = self.headers.get("Range")
                if rng:
                    start = int(rng.split("=")[1].split("-")[0])
                    self.send_response(206)
                else:
                    self.send_response(200)
                self.send_header("Content-Length", str(len(body) - start))
                self.end_headers()
                self.wfile.write(body[start:])

        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.url = f"http://127.0.0.1:{self.httpd.server_address[1]}"
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()

    def close(self):
        self.httpd.shutdown()


def test_install_update_uninstall(tmp_path: Path) -> None:
    data = _stardict_tar(tmp_path)
    srv = _Server({"/tiny.stardict.tar.xz": data})
    try:
        url = srv.url + "/tiny.stardict.tar.xz"
        e = model.normalize({"id": "tiny", "name": "Tiny", "source_lang": "en", "target_lang": "tr",
                             "license": "CC0-1.0", "version": "1",
                             "files": [{"url": url, "size": len(data),
                                        "sha256": hashlib.sha256(data).hexdigest()}]}, "test")
        inst = Installer(tmp_path / "data", http=Http(timeout=10))
        # a partial download is resumed
        from dictz.storage.registry import fnv1a64

        part = inst.part_dir / f"{fnv1a64(url.encode()):016x}.part"
        part.parent.mkdir(parents=True)
        part.write_bytes(data[:100])
        stages = []
        rec = inst.install(e, lambda st, d, t: stages.append(st))
        assert "download" in stages and "unpack" in stages
        assert [Path(f).name for f in rec["files"]] == ["tiny.ifo"]
        assert Path(rec["files"][0]).is_file() and not (Path(rec["folder"]).parent / "evil.txt").exists()
        assert rec["license"] == "CC0-1.0" and inst.installed()["test:tiny"]["version"] == "1"
        assert inst.history()[0]["status"] == "done"
        assert inst.record_for_path(rec["files"][0])["id"] == "test:tiny"
        newer = dict(e, version="2")
        assert inst.updates([newer]) and not inst.updates([e])
        # a bad checksum is refused and nothing is installed over the old one
        bad = model.normalize(dict(e, files=[dict(e["files"][0], sha256="00")], version="3"), "")
        bad["id"], bad["source"] = e["id"], e["source"]
        with pytest.raises(Exception, match="SHA256"):
            inst.install(bad)
        assert Path(rec["files"][0]).is_file()
        assert inst.uninstall("test:tiny")["id"] == "test:tiny"
        assert not Path(rec["folder"]).exists() and inst.installed() == {}
    finally:
        srv.close()


def test_unpack_zip_skips_escapes(tmp_path: Path) -> None:
    z = tmp_path / "a.zip"
    with zipfile.ZipFile(z, "w") as f:
        f.writestr("ok/x.dsl", "x")
        f.writestr("../bad.dsl", "x")
        f.writestr("/abs.dsl", "x")
    out = unpack(z, "zip", tmp_path / "out")
    assert [p.relative_to(tmp_path / "out").as_posix() for p in out] == ["ok/x.dsl"]


# ---------------------------------------------------------------- transfer

def test_setup_export_import_with_files(tmp_path: Path) -> None:
    from dictz.core.query_manager import QueryManager
    from dictz.storage.history import HistoryStore
    from dictz.storage.indexer import IndexStore

    from .test_stardict import _build_tiny_stardict

    a = tmp_path / "a"
    (tmp_path / "src").mkdir()
    ifo = _build_tiny_stardict(tmp_path / "src")
    store = IndexStore(a / "dictz_index.db")
    store.open()
    qm = QueryManager(store)
    qm.add_dictionary(ifo)
    qm.close_all()
    store.close()
    h = HistoryStore(a / "dictz_history.db")
    h.record("apple")
    h.add_favorite("pear", "note")
    h.close()
    Sources(a).add({"name": "Mine", "url": "https://me.example/c.json"})
    Sources(a).set_secret("mine", "secret-pw", "")
    out = tmp_path / "setup.zip"
    m = export_setup(out, a, parts=("dictionaries", "settings", "sources", "installed", "history", "files"))
    assert m["dictionaries"][0]["files"] and "secret-pw" not in out.read_bytes().decode("latin-1")
    b = tmp_path / "b"
    res = import_setup(out, b)
    assert len(res["added"]) == 1 and res["files"] >= 3 and res["history"] == 1 and res["favorites"] == 1
    new_path = Path(res["added"][0]["path"])
    assert new_path.is_file() and b in new_path.parents
    assert Sources(b).get("mine") is not None
    again = import_setup(out, b)  # merge: nothing twice
    assert again["added"] == [] and again["history"] == 0
    n = export_list(tmp_path / "list.csv", b)
    assert n == 1 and "Tiny" in (tmp_path / "list.csv").read_text(encoding="utf-8")


def test_build_and_check_catalog(tmp_path: Path) -> None:
    from .test_stardict import _build_tiny_stardict

    folder = tmp_path / "srv"
    (folder / "en-tr").mkdir(parents=True)
    ifo = _build_tiny_stardict(folder / "en-tr")
    (folder / "pack.tar.gz").write_bytes(b"not really")
    (folder / "pack.tar.gz.json").write_text('{"license": "MIT", "source_lang": "de", "target_lang": "en"}')
    cat = build_catalog(folder, "https://dl.example.org/dicts", "test", "CC-BY-4.0")
    rows = {r["name"]: r for r in cat["dictionaries"]}
    tiny = rows[next(n for n in rows if n != "pack")]
    assert tiny["headwords"] > 0 and tiny["license"] == "CC-BY-4.0"
    assert all(f["sha256"] and f["url"].startswith("https://dl.example.org/dicts/en-tr/") for f in tiny["files"])
    assert rows["pack"]["license"] == "MIT" and rows["pack"]["source_lang"] == "de"
    n, problems = check_catalog(cat)
    assert n == 2 and not [p for p in problems if p.startswith("error")]
    n, problems = check_catalog({"dictionaries": [{"id": "a"}, {"id": "a"}]})
    assert any("twice" in p for p in problems) and any("no files" in p for p in problems)
    assert ifo.exists()


def test_file_links_same_in_both_editions() -> None:
    from dictz.center.net import local_path_text

    golden = json.loads((Path(__file__).parent / "data" / "parity" / "filelinks_golden.json").read_text("utf-8"))
    for g in golden:
        assert local_path_text(g["url"], False) == g["posix"], g["url"]
        assert local_path_text(g["url"], True) == g["windows"], g["url"]


def test_own_files_of_a_dictionary() -> None:
    from dictz.center.transfer import own_files

    p = Path(__file__).parent / "data" / "parity" / "sample.dsl"
    assert [f.name for f in own_files(p)] == ["sample.dsl", "sample.dsl.files.zip"]


def test_retired_builtin_addresses_move(tmp_path: Path) -> None:
    """A stored built-in on an old address gets today's address (its switch
    stays, its cached error goes); an address the user typed stays."""
    from dictz.center.sources import BUILTIN, Sources

    center = tmp_path / "center"
    (center / "cache").mkdir(parents=True)
    old = [dict(clean(b)) for b in BUILTIN]
    for s in old:
        if s["id"] == "huzheng":
            s.update(url="https://download.huzheng.org/", enabled=False)
        if s["id"] == "dictz-data":
            s.update(url="https://my.example.org/catalog.json")
    old.append(dict(clean({"id": "freemdict", "name": "FreeMdict cloud", "type": "nextcloud-share",
                           "url": "https://cloud.freemdict.com/index.php/s/pgKcDcbSDTCzXCs", "builtin": True})))
    old.append(dict(clean({"id": "mine", "name": "Mine", "type": "local", "url": "/x"})))
    (center / "sources.json").write_text(json.dumps({"version": 1, "default": "freemdict", "removed": [],
                                                     "sources": old}), encoding="utf-8")
    (center / "cache" / "freemdict.json").write_text('{"ok": false}', encoding="utf-8")
    (center / "cache" / "huzheng.json").write_text('{"ok": false, "error": "refused"}', encoding="utf-8")
    src = Sources(tmp_path)
    hz = src.get("huzheng")
    assert hz["url"] == "https://stardict.uber.space/" and hz["enabled"] is False
    assert not (center / "cache" / "huzheng.json").exists()
    assert src.get("dictz-data")["url"] == "https://my.example.org/catalog.json"
    # a built-in that was dropped goes, with its cache; the default falls back
    assert src.get("freemdict") is None and src.get("mine") is not None
    assert not (center / "cache" / "freemdict.json").exists()
    assert src.data()["default"] == "dictz-data"
    stored = json.loads((center / "sources.json").read_text(encoding="utf-8"))
    assert next(s for s in stored["sources"] if s["id"] == "huzheng")["url"] == "https://stardict.uber.space/"
