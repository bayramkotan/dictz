"""HTTP for the Dictionary Center: lists, catalogs and downloads.

Standard library only (``urllib``). The same requests as the Rust edition
(``ureq``): user agent ``dictz/<version>``, the source's sign-in, a proxy
from the settings (``center.proxy``: empty = the system's, ``none`` =
direct, else ``http://host:port``), resumable downloads (``Range``).

Adapters take an object with ``get(url, headers)`` and ``request(method,
url, body, headers)``; tests pass a fake one that answers from files.
"""
from __future__ import annotations

import base64
import contextlib
import os
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path

from .. import __version__

USER_AGENT = f"dictz/{__version__} (+https://github.com/bayramkotan/dictz)"
CHUNK = 256 * 1024


def local_path_text(url: str, windows: bool | None = None) -> str | None:
    """The path of a ``file:`` link as text, the same in both editions:
    ``file:///home/x``, ``file:/home/x``, ``file://localhost/x`` → ``/home/x``;
    ``file:///C:/x``, ``file://C:/x``, ``file://C:\\x``, ``file:C:\\x``,
    ``file:///C|/x`` → ``C:/x`` (``C:\\x`` with ``windows``); ``file://host/share/x``
    → ``//host/share/x``. Query and fragment are dropped, ``%xx`` decoded.
    ``None`` for other links."""
    from urllib.parse import unquote

    if url[:5].lower() != "file:":
        return None
    if windows is None:
        windows = os.name == "nt"
    rest = url[5:].split("#", 1)[0].split("?", 1)[0]
    if rest.startswith("//"):
        r = rest[2:]
        if r.startswith("/") or (len(r) >= 2 and r[0].isascii() and r[0].isalpha() and r[1] in ":|"):
            path = r
        else:
            host, _, tail = r.partition("/")
            path = "/" + tail if host.lower() in ("", "localhost") else "//" + host + "/" + tail
    else:
        path = rest
    path = unquote(path)
    if len(path) >= 3 and path[0] in "/\\" and path[1].isascii() and path[1].isalpha() and path[2] in ":|":
        path = path[1:]
    if len(path) >= 2 and path[0].isascii() and path[0].isalpha() and path[1] == "|":
        path = path[0] + ":" + path[2:]
    if windows:
        path = path.replace("/", "\\")
    return path


def local_path(url: str) -> Path | None:
    """:func:`local_path_text` as a ``Path`` on this system."""
    t = local_path_text(url)
    return None if t is None else Path(t)


class NetError(Exception):
    """A request failed; ``status`` is the HTTP status (0 = no answer)."""

    def __init__(self, message: str, status: int = 0):
        super().__init__(message)
        self.status = status


class Cancelled(Exception):  # noqa: N818 — a stop, not an error
    pass


def auth_headers(source: dict, password: str = "", token: str = "") -> dict[str, str]:
    """The headers a source's sign-in adds."""
    auth = source.get("auth") or "none"
    if auth == "basic" and (source.get("user") or password):
        raw = f"{source.get('user', '')}:{password}".encode()
        return {"Authorization": "Basic " + base64.b64encode(raw).decode()}
    if auth == "bearer" and token:
        return {"Authorization": f"Bearer {token}"}
    if auth == "header" and source.get("user") and token:
        return {source["user"]: token}
    return {}


class Http:
    def __init__(self, timeout: float = 30.0, proxy: str = "", headers: dict | None = None):
        self.timeout = timeout
        self.headers = headers or {}
        handlers: list = []
        if proxy == "none":
            handlers.append(urllib.request.ProxyHandler({}))
        elif proxy:
            handlers.append(urllib.request.ProxyHandler({"http": proxy, "https": proxy}))
        self._opener = urllib.request.build_opener(*handlers)

    def _req(self, method: str, url: str, body: bytes | None, headers: dict | None) -> urllib.request.Request:
        h = {"User-Agent": USER_AGENT, **self.headers, **(headers or {})}
        if "api.github.com" in url and os.environ.get("GITHUB_TOKEN") and "Authorization" not in h:
            h["Authorization"] = "Bearer " + os.environ["GITHUB_TOKEN"]  # more requests per hour
        return urllib.request.Request(url, data=body, headers=h, method=method)

    def request(self, method: str, url: str, body: bytes | None = None, headers: dict | None = None) -> bytes:
        local = local_path(url)
        if local is not None:  # a catalog on this computer
            try:
                return local.read_bytes()
            except OSError as e:
                raise NetError(f"{e.strerror or e}: {local}") from e
        try:
            with self._opener.open(self._req(method, url, body, headers), timeout=self.timeout) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            raise NetError(f"HTTP {e.code} {e.reason}: {url}", e.code) from e
        except (urllib.error.URLError, OSError, ValueError) as e:
            reason = getattr(e, "reason", e)
            raise NetError(f"{reason}: {url}") from e

    def get(self, url: str, headers: dict | None = None) -> bytes:
        return self.request("GET", url, None, headers)

    def download(self, url: str, part: Path, headers: dict | None = None,
                 progress: Callable[[int, int], None] | None = None,
                 cancelled: Callable[[], bool] | None = None) -> int:
        """Download ``url`` into ``part``, continuing a partial file
        (``Range``). Returns the size; raises :class:`Cancelled` when
        ``cancelled()`` says so (the partial file stays, to resume)."""
        part.parent.mkdir(parents=True, exist_ok=True)
        have = part.stat().st_size if part.exists() else 0
        h = dict(headers or {})
        if have:
            h["Range"] = f"bytes={have}-"
        try:
            resp = self._opener.open(self._req("GET", url, None, h), timeout=self.timeout)
        except urllib.error.HTTPError as e:
            if e.code == 416 and have:  # already complete
                return have
            raise NetError(f"HTTP {e.code} {e.reason}: {url}", e.code) from e
        except (urllib.error.URLError, OSError, ValueError) as e:
            raise NetError(f"{getattr(e, 'reason', e)}: {url}") from e
        with resp:
            status = getattr(resp, "status", 200)
            if have and status != 206:  # the server ignored Range: start again
                have = 0
            length = resp.headers.get("Content-Length")
            total = (int(length) + have) if length and length.isdigit() else 0
            done = have
            with open(part, "ab" if have else "wb") as f:
                while True:
                    if cancelled is not None and cancelled():
                        raise Cancelled()
                    try:
                        chunk = resp.read(CHUNK)
                    except OSError as e:
                        raise NetError(f"{e}: {url}") from e
                    if not chunk:
                        break
                    f.write(chunk)
                    done += len(chunk)
                    if progress is not None:
                        progress(done, total)
        if total and done < total:
            raise NetError(f"incomplete download ({done} of {total} bytes): {url}")
        return done


def quote_url(url: str) -> str:
    """Spaces and other characters in a path made safe (``a b.zip`` → ``a%20b.zip``)."""
    from urllib.parse import quote, urlsplit, urlunsplit

    p = urlsplit(url)
    return urlunsplit((p.scheme, p.netloc, quote(p.path, safe="/%:@!$&'()*+,;="), p.query, p.fragment))


def remove(path: Path) -> None:
    with contextlib.suppress(OSError):
        path.unlink()
