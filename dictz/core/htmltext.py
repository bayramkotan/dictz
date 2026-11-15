"""Article HTML → plain text, for the command line.

A line-by-line port of ``dictz-rs/src/html.rs`` (``parse`` + ``to_text``), so
both editions print articles identically. ``tests/data/parity/text_golden.json``
checks it in both test suites.
"""
from __future__ import annotations

import math
from dataclasses import dataclass, field

from ._entities import NAMED_ENTITIES

BLOCK_TAGS = {
    "p", "div", "li", "ul", "ol", "dl", "dd", "dt", "blockquote", "h1", "h2", "h3",
    "h4", "h5", "h6", "tr", "table", "pre", "hr", "center", "section", "article",
    "header", "footer",
}
SKIP_TAGS = {"script", "style", "head", "title"}
_HEX = set("0123456789abcdefABCDEF")
_DEC = set("0123456789")


def _ws(c: str) -> bool:
    """Rust ``char::is_whitespace`` (Python's isspace minus \\x1c-\\x1f)."""
    return c.isspace() and c not in "\x1c\x1d\x1e\x1f"


def _strip(s: str) -> str:
    a, b = 0, len(s)
    while a < b and _ws(s[a]):
        a += 1
    while b > a and _ws(s[b - 1]):
        b -= 1
    return s[a:b]


def _ascii_lower(s: str) -> str:
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c for c in s)


def _char(cp: int | None) -> str | None:
    if cp is None or cp > 0x10FFFF or 0xD800 <= cp <= 0xDFFF:
        return None
    return chr(cp)


def decode_entities(s: str) -> str:
    """Decode ``&name;``, ``&#NN;`` and ``&#xHH;``; anything else stays."""
    if "&" not in s:
        return s
    out: list[str] = []
    rest = s
    while True:
        i = rest.find("&")
        if i < 0:
            break
        out.append(rest[:i])
        tail = rest[i:]
        end = None
        for j in range(1, len(tail)):
            c = tail[j]
            if c in ";&<" or _ws(c):
                end = j
                break
        decoded = None
        if end is not None and tail[end] == ";":
            name = tail[1:end]
            cp = None
            if name[:2] in ("#x", "#X"):
                h = name[2:]
                if h and all(c in _HEX for c in h):
                    cp = int(h, 16)
            elif name.startswith("#"):
                d = name[1:]
                if d and all(c in _DEC for c in d):
                    cp = int(d)
            else:
                cp = NAMED_ENTITIES.get(name)
                if cp is None:
                    cp = NAMED_ENTITIES.get(name.lower())
            ch = _char(cp)
            if ch is not None:
                decoded = (ch, end + 1)
        if decoded:
            out.append(decoded[0])
            rest = tail[decoded[1]:]
        else:
            out.append("&")
            rest = tail[1:]
    out.append(rest)
    return "".join(out)


def _parse_attrs(s: str) -> list[tuple[str, str]]:
    c = s
    out: list[tuple[str, str]] = []
    i, n = 0, len(c)
    while i < n:
        while i < n and (_ws(c[i]) or c[i] == "/"):
            i += 1
        start = i
        while i < n and not _ws(c[i]) and c[i] not in "=/":
            i += 1
        if start == i:
            i += 1
            continue
        name = _ascii_lower(c[start:i])
        while i < n and _ws(c[i]):
            i += 1
        value = ""
        if i < n and c[i] == "=":
            i += 1
            while i < n and _ws(c[i]):
                i += 1
            if i < n and c[i] in "\"'":
                q = c[i]
                i += 1
                vs = i
                while i < n and c[i] != q:
                    i += 1
                value = c[vs:i]
                i += 1
            else:
                vs = i
                while i < n and not _ws(c[i]):
                    i += 1
                value = c[vs:i]
        out.append((name, decode_entities(value)))
    return out


def _style_prop(style: str, prop: str) -> str | None:
    for decl in style.split(";"):
        if ":" not in decl:
            continue
        k, v = decl.split(":", 1)
        if _strip(k).lower() == prop:
            return _strip(v)
    return None


def _css_length_em(v: str) -> float | None:
    v = _strip(v)

    def num(x: str) -> float | None:
        x = _strip(x)
        try:
            return float(x)
        except ValueError:
            return None

    if v.endswith("em"):
        return num(v[:-2])
    if v.endswith("px"):
        f = num(v[:-2])
        return None if f is None else f / 16.0
    if v.endswith("pt"):
        f = num(v[:-2])
        return None if f is None else f / 12.0
    f = num(v)
    return None if f is None else f / 16.0


@dataclass
class Line:
    indent: float = 0.0
    # ("text", str, link) | ("image", src) | ("audio", src)
    items: list[tuple] = field(default_factory=list)


@dataclass
class _Frame:
    tag: str
    link: str | None
    underline: bool
    strike: bool
    indent: float
    block: bool


class _Builder:
    def __init__(self) -> None:
        self.lines: list[Line] = []
        self.cur = Line()
        self.pending_space = False
        self.pre = 0

    def flush(self, force: bool, indent: float) -> None:
        if self.cur.items or force:
            line = self.cur
            if line.items and line.items[-1][0] == "text":
                kind, text, *rest = line.items[-1]
                line.items[-1] = (kind, text.rstrip(" "), *rest)
            self.lines.append(line)
            self.cur = Line()
        self.cur.indent = indent
        self.pending_space = False

    def push_text(self, text: str, frame: _Frame) -> None:
        if self.pre > 0:
            parts = text.split("\n")
            buf = ""
            for k, part in enumerate(parts):
                if k > 0:
                    if buf:
                        self.push_run(buf, frame)
                        buf = ""
                    self.flush(True, self.cur.indent)
                buf += part
            if buf:
                self.push_run(buf, frame)
            return
        buf = []
        for ch in text:
            if _ws(ch) and ch != " ":
                self.pending_space = True
            else:
                if self.pending_space and buf:
                    buf.append(" ")
                elif self.pending_space and self.cur.items:
                    last = self.cur.items[-1]
                    if last[0] == "text" and last[2] is None and not last[3] and not last[4]:
                        self.cur.items[-1] = ("text", last[1] + " ", *last[2:])
                    else:
                        self.cur.items.append(("text", " ", None, False, False))
                self.pending_space = False
                buf.append(" " if ch == " " else ch)
        if buf:
            self.push_run("".join(buf), frame)

    def push_run(self, text: str, frame: _Frame) -> None:
        self.cur.items.append(("text", text, frame.link, frame.underline, frame.strike))

    def push_item(self, item: tuple) -> None:
        if self.pending_space and self.cur.items:
            self.cur.items.append(("text", " ", None, False, False))
        self.pending_space = False
        self.cur.items.append(item)


def parse(html: str) -> list[Line]:
    """Same structure as ``dictz::html::parse`` (styles omitted)."""
    stack = [_Frame("", None, False, False, 0.0, True)]
    b = _Builder()
    skip: str | None = None
    i, n = 0, len(html)
    while i < n:
        if html[i] == "<":
            if html.startswith("<!--", i):
                p = html.find("-->", i + 4)
                i = n if p < 0 else p + 3
                continue
            close = html.find(">", i)
            if close < 0:
                if skip is None:
                    b.push_text("<", stack[-1])
                i += 1
                continue
            raw = html[i + 1:close]
            i = close + 1
            raw_trim = _strip(raw)
            if raw_trim.startswith(("!", "?")):
                continue
            closing = raw_trim.startswith("/")
            body = raw_trim.lstrip("/")
            name_end = len(body)
            for k, c in enumerate(body):
                if _ws(c) or c == "/":
                    name_end = k
                    break
            name = _ascii_lower(body[:name_end])
            if not name or not ("a" <= name[0] <= "z"):
                if skip is None:
                    b.push_text(f"<{raw}>", stack[-1])
                continue
            if skip is not None:
                if closing and skip == name:
                    skip = None
                continue
            if name in SKIP_TAGS and not closing:
                if not raw_trim.endswith("/"):
                    skip = name
                continue
            attrs = _parse_attrs(body[name_end:])

            def attr(key: str, attrs: list = attrs) -> str | None:
                for k, v in attrs:
                    if k == key:
                        return v
                return None

            cur_indent = stack[-1].indent
            if closing:
                pos = next((k for k in range(len(stack) - 1, -1, -1) if stack[k].tag == name), None)
                if pos is not None and pos > 0:
                    was_block = stack[pos].block
                    del stack[pos:]
                    if name == "pre":
                        b.pre = max(0, b.pre - 1)
                    if was_block:
                        b.flush(False, stack[-1].indent)
                continue
            if name == "br":
                b.flush(True, cur_indent)
                continue
            if name == "hr":
                b.flush(False, cur_indent)
                b.cur.items.append(("text", "────────", None, False, False))
                b.flush(False, cur_indent)
                continue
            if name == "img":
                src = attr("src")
                if src:
                    b.push_item(("image", src))
                continue
            if name in ("object", "embed", "audio", "source"):
                src = attr("data")
                if src is None:
                    src = attr("src")
                if src:
                    b.push_item(("audio", src))
                if name in ("object", "audio"):
                    skip = name
                continue
            self_closing = raw_trim.endswith("/")
            top = stack[-1]
            link, underline, strike = top.link, top.underline, top.strike
            indent = cur_indent
            block = name in BLOCK_TAGS
            if name in ("u", "ins"):
                underline = True
            elif name in ("s", "strike", "del"):
                strike = True
            elif name == "a":
                h = attr("href")
                if h is not None:
                    link = h
            elif name in ("blockquote", "dd"):
                indent += 2.0
            elif name in ("ul", "ol"):
                indent += 1.5
            style = attr("style")
            if style is not None:
                for prop in ("padding-left", "margin-left", "text-indent"):
                    v = _style_prop(style, prop)
                    em = None if v is None else _css_length_em(v)
                    if em is not None and prop != "text-indent":
                        indent += em
            if block:
                if name == "p":
                    pos = next((k for k in range(len(stack) - 1, -1, -1) if stack[k].tag == "p"), None)
                    if pos is not None and pos > 0:
                        del stack[pos:]
                        indent = stack[-1].indent + (indent - cur_indent)
                b.flush(False, indent)
                if name == "pre":
                    b.pre += 1
                if name == "li":
                    b.cur.items.append(("text", "• ", None, False, False))
            if name == "a" and link is not None and link.startswith("sound://"):
                b.push_item(("audio", link[len("sound://"):]))
            if not self_closing:
                stack.append(_Frame(name, link, underline, strike, indent, block))
        else:
            nxt = html.find("<", i)
            if nxt < 0:
                nxt = n
            if skip is None:
                b.push_text(decode_entities(html[i:nxt]), stack[-1])
            i = nxt
    b.flush(False, b.cur.indent)
    out: list[Line] = []
    for line in b.lines:
        if not line.items and (not out or not out[-1].items):
            continue
        out.append(line)
    while out and not out[-1].items:
        out.pop()
    return out


def plain_to_lines(text: str) -> list[Line]:
    return [Line(0.0, [("text", ln, None, False, False)] if ln else []) for ln in text.split("\n")]


def looks_like_html(s: str) -> bool:
    return "<" in s and ">" in s


def lines_to_text(lines: list[Line]) -> str:
    out = []
    for line in lines:
        if not line.items:
            out.append("")
            continue
        parts = []
        for item in line.items:
            if item[0] == "text":
                parts.append(item[1])
            elif item[0] == "image":
                parts.append(f"[{item[1]}]")
            else:
                parts.append(f"[♪ {item[1]}]")
        out.append(" " * int(math.floor(line.indent * 2.0 + 0.5)) + "".join(parts))
    return "\n".join(out)


def to_text(definition: str) -> str:
    """Definition (HTML or plain) → terminal text."""
    lines = parse(definition) if looks_like_html(definition) else plain_to_lines(definition)
    return lines_to_text(lines)
