"""The catalog: every enabled source's entries, kept in ``center/cache``.

A source is read only when asked (Refresh, or when its cache is older than
``center.refresh_days``); offline, the last list is shown. Sources are read
several at a time; each result is written to ``cache/<source>.json`` at once.
"""
from __future__ import annotations

import datetime as _dt
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from . import adapters
from .model import matches, sort_key
from .net import Http
from .sources import Sources, center_dir
from .store import JsonFile


def now_iso() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _age_days(stamp: str) -> float:
    try:
        t = _dt.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
    except ValueError:
        return 1e9
    return (_dt.datetime.now(_dt.timezone.utc) - t).total_seconds() / 86400


class Catalog:
    def __init__(self, data_dir: Path | str, sources: Sources | None = None, http: Http | None = None):
        self.data_dir = Path(data_dir)
        self.sources = sources or Sources(data_dir)
        self.http = http or Http()

    def cache_file(self, sid: str) -> JsonFile:
        return JsonFile(center_dir(self.data_dir) / "cache" / f"{sid}.json", dict)

    def cached(self, sid: str) -> dict:
        """``{"source", "fetched", "ok", "error", "entries"}`` (empty when never read)."""
        d = self.cache_file(sid).read()
        if not isinstance(d, dict):
            d = {}
        d.setdefault("source", sid)
        d.setdefault("fetched", "")
        d.setdefault("ok", False)
        d.setdefault("error", "")
        d.setdefault("entries", [])
        return d

    def stale(self, sid: str, days: float) -> bool:
        c = self.cached(sid)
        return not c["fetched"] or (days > 0 and _age_days(c["fetched"]) >= days)

    def refresh_one(self, source: dict) -> dict:
        """Read one source now; the cache keeps the old entries when it fails."""
        old = self.cached(source["id"])
        pw, tok = self.sources.secret(source["id"])
        try:
            entries = adapters.read(source, self.http, pw, tok)
            res = {"source": source["id"], "fetched": now_iso(), "ok": True, "error": "", "entries": entries}
        except Exception as exc:  # noqa: BLE001 — shown on the Sources page
            res = {**old, "ok": False, "error": str(exc) or type(exc).__name__}
            res.setdefault("fetched", "")
        self.cache_file(source["id"]).write(res)
        return res

    def refresh(self, ids: list[str] | None = None, *, stale_days: float = 0,
                progress: Callable[[str, dict], None] | None = None, workers: int = 4) -> dict[str, dict]:
        """Read the enabled sources (or ``ids``) — only stale ones when
        ``stale_days`` > 0. ``progress(source_id, result)`` after each."""
        todo = [s for s in self.sources.all()
                if (s["id"] in ids if ids is not None else s["enabled"])
                and (stale_days <= 0 or self.stale(s["id"], stale_days))]
        out: dict[str, dict] = {}
        if not todo:
            return out
        with ThreadPoolExecutor(max(1, min(workers, len(todo))), thread_name_prefix="dictz-catalog") as pool:
            for s, res in zip(todo, pool.map(self.refresh_one, todo), strict=True):
                out[s["id"]] = res
                if progress is not None:
                    progress(s["id"], res)
        return out

    def entries(self, include_disabled: bool = False) -> list[dict]:
        """Every entry of the (enabled) sources, sources in their list order;
        the same id twice: the first source wins."""
        out, seen = [], set()
        for s in self.sources.all():
            if not s["enabled"] and not include_disabled:
                continue
            for e in self.cached(s["id"])["entries"]:
                if e.get("id") in seen:
                    continue
                seen.add(e["id"])
                out.append(e)
        return out

    def get(self, entry_id: str) -> dict | None:
        return next((e for e in self.entries(include_disabled=True) if e["id"] == entry_id), None)

    def find(self, key: str) -> list[dict]:
        """By id, or by name (exact, then case-insensitive substring)."""
        es = self.entries(include_disabled=True)
        hit = [e for e in es if e["id"] == key]
        if hit:
            return hit
        low = key.lower()
        exact = [e for e in es if e["name"].lower() == low]
        return exact or [e for e in es if low in e["name"].lower()]

    def search(self, **filters) -> list[dict]:
        return sorted((e for e in self.entries() if matches(e, **filters)), key=sort_key)

    def languages(self) -> tuple[list[str], list[str]]:
        """Source and target language codes in the catalog (for the filters)."""
        s, t = set(), set()
        for e in self.entries():
            if e["source_lang"]:
                s.add(e["source_lang"])
            t.update(e["target_lang"])
        return sorted(s), sorted(t)
