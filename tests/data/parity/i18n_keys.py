"""Every interface text (Turkish) used by either edition — the translation keys.

Python: arguments of ``t("...")`` in ``dictz/``, plus the settings schema
(labels, help, choices, page titles) and the shortcut table.
Rust: arguments of ``tr("...")`` / ``tr_args("...")`` in ``dictz-rs/src/gui``,
plus the Rust-only constants named below.
"""
from __future__ import annotations

import ast
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))


def schema_keys() -> set[str]:
    from dictz.core.shortcuts import SHORTCUTS
    from dictz.storage.settings import SCHEMA, SECTIONS

    keys = {title for _id, title in SECTIONS}
    for s in SCHEMA:
        keys.add(s.label)
        if s.help:
            keys.add(s.help)
        if s.key != "general.language":
            keys.update(label for _v, label in s.choices)
    keys.update(desc for _k, desc in SHORTCUTS)
    keys.update(("Yeni sekme", "Kısayollar", "Gelişmiş", "Ad", "Biçim", "Kelime", "Diller", "Dosya"))
    return keys


_RS_STR = re.compile(r'\btr(?:_args)?\(\s*"((?:[^"\\]|\\.)*)"')


def _rs_unescape(s: str) -> str:
    return re.sub(r'\\(n|t|"|\\|\')', lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), s)


def rust_keys() -> set[str]:
    keys: set[str] = set()
    for path in (ROOT / "dictz-rs" / "src" / "gui").rglob("*.rs"):
        for m in _RS_STR.finditer(path.read_text(encoding="utf-8")):
            keys.add(_rs_unescape(m.group(1)))
    return keys


def py_t_keys() -> set[str]:
    keys: set[str] = set()
    for path in (ROOT / "dictz").rglob("*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if (isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "t"
                    and node.args and isinstance(node.args[0], ast.Constant) and isinstance(node.args[0].value, str)):
                k = node.args[0].value
                if "&" in k and "&&" not in k and "&lt;" not in k and "&gt;" not in k:
                    k = k.replace("&", "")
                keys.add(k)
    return keys


def all_keys() -> list[str]:
    keys = py_t_keys() | rust_keys() | schema_keys()
    return sorted(k for k in keys if k.strip())


if __name__ == "__main__":
    ks = all_keys()
    print(len(ks), "keys")
    if "-v" in sys.argv:
        for k in ks:
            print(repr(k))
