"""Licences of downloadable dictionaries.

Every catalog entry carries a licence id (SPDX where one exists). This table
says what each one allows; the Rust edition has the same table
(``dictz-rs/src/center/licenses.rs``, checked by ``licenses_golden.json``).

``status``:

* ``open``    — a free / open licence: use, share and (usually) change
* ``free``    — free to download and use, the terms are not a known licence
* ``unknown`` — the source does not say; the user decides
* ``nonfree`` — personal use only or similar

Nothing here is legal advice: the text at ``url`` is what counts.
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class License:
    id: str
    name: str
    url: str
    status: str  # open | free | unknown | nonfree
    attribution: bool = False  # the authors must be named when shared
    share_alike: bool = False  # changed copies keep the same licence
    commercial: bool = True  # commercial use allowed


LICENSES: tuple[License, ...] = (
    License("GPL-2.0-or-later", "GNU GPL 2.0 or later", "https://www.gnu.org/licenses/old-licenses/gpl-2.0.html",
            "open", True, True),
    License("GPL-3.0-or-later", "GNU GPL 3.0 or later", "https://www.gnu.org/licenses/gpl-3.0.html",
            "open", True, True),
    License("GPL-2.0-only", "GNU GPL 2.0", "https://www.gnu.org/licenses/old-licenses/gpl-2.0.html",
            "open", True, True),
    License("GPL-3.0-only", "GNU GPL 3.0", "https://www.gnu.org/licenses/gpl-3.0.html", "open", True, True),
    License("LGPL-2.1-or-later", "GNU LGPL 2.1 or later", "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
            "open", True, True),
    License("AGPL-3.0-or-later", "GNU AGPL 3.0 or later", "https://www.gnu.org/licenses/agpl-3.0.html",
            "open", True, True),
    License("GFDL-1.3-or-later", "GNU Free Documentation License 1.3", "https://www.gnu.org/licenses/fdl-1.3.html",
            "open", True, True),
    License("CC-BY-SA-3.0", "Creative Commons BY-SA 3.0", "https://creativecommons.org/licenses/by-sa/3.0/",
            "open", True, True),
    License("CC-BY-SA-4.0", "Creative Commons BY-SA 4.0", "https://creativecommons.org/licenses/by-sa/4.0/",
            "open", True, True),
    License("CC-BY-3.0", "Creative Commons BY 3.0", "https://creativecommons.org/licenses/by/3.0/",
            "open", True, False),
    License("CC-BY-4.0", "Creative Commons BY 4.0", "https://creativecommons.org/licenses/by/4.0/",
            "open", True, False),
    License("CC-BY-NC-SA-4.0", "Creative Commons BY-NC-SA 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/",
            "free", True, True, False),
    License("CC-BY-NC-4.0", "Creative Commons BY-NC 4.0", "https://creativecommons.org/licenses/by-nc/4.0/",
            "free", True, False, False),
    License("CC0-1.0", "Creative Commons Zero (public domain)", "https://creativecommons.org/publicdomain/zero/1.0/",
            "open"),
    License("public-domain", "Public domain", "https://en.wikipedia.org/wiki/Public_domain", "open"),
    License("MIT", "MIT License", "https://opensource.org/license/mit", "open", True),
    License("BSD-3-Clause", "BSD 3-Clause", "https://opensource.org/license/bsd-3-clause", "open", True),
    License("Apache-2.0", "Apache License 2.0", "https://www.apache.org/licenses/LICENSE-2.0", "open", True),
    License("ODbL-1.0", "Open Database License 1.0", "https://opendatacommons.org/licenses/odbl/1-0/",
            "open", True, True),
    License("WordNet", "WordNet License (Princeton)", "https://wordnet.princeton.edu/license-and-commercial-use",
            "open", True),
    License("EDRDG", "EDRDG Licence (JMdict, KANJIDIC)", "https://www.edrdg.org/edrdg/licence.html",
            "open", True, True),
    License("freedict", "FreeDict (GPL, CC BY-SA … — each dictionary's own)", "https://freedict.org/",
            "open", True, True),
    License("wiktionary", "Wiktionary (CC BY-SA 4.0 / GFDL)",
            "https://en.wiktionary.org/wiki/Wiktionary:Copyrights", "open", True, True),
    License("free", "Free to use (terms not stated as a licence)", "", "free"),
    License("personal", "Free for personal use only", "", "nonfree", True, False, False),
    License("unknown", "Unknown — the source does not say", "", "unknown"),
)

_BY_ID = {lic.id.lower(): lic for lic in LICENSES}
# other spellings catalogs use
_ALIASES = {
    "gpl": "GPL-2.0-or-later", "gpl2": "GPL-2.0-or-later", "gpl-2": "GPL-2.0-or-later",
    "gpl-2.0": "GPL-2.0-or-later", "gplv2": "GPL-2.0-or-later", "gplv2+": "GPL-2.0-or-later",
    "gpl-2.0+": "GPL-2.0-or-later", "gpl3": "GPL-3.0-or-later", "gpl-3": "GPL-3.0-or-later",
    "gpl-3.0": "GPL-3.0-or-later", "gplv3": "GPL-3.0-or-later", "gplv3+": "GPL-3.0-or-later",
    "gpl-3.0+": "GPL-3.0-or-later", "gfdl": "GFDL-1.3-or-later", "gfdl-1.3": "GFDL-1.3-or-later",
    "cc-by-sa": "CC-BY-SA-4.0", "cc by-sa 3.0": "CC-BY-SA-3.0", "cc by-sa 4.0": "CC-BY-SA-4.0",
    "cc-by-sa 3.0": "CC-BY-SA-3.0", "cc-by": "CC-BY-4.0", "cc0": "CC0-1.0", "pd": "public-domain",
    "public domain": "public-domain", "mit license": "MIT", "apache": "Apache-2.0", "": "unknown",
}


def get(license_id: str) -> License:
    """The licence for an id or one of its spellings; unknown ids keep their
    text as the name with status ``unknown``."""
    key = (license_id or "").strip().lower()
    if key in _BY_ID:
        return _BY_ID[key]
    if key in _ALIASES:
        return _BY_ID[_ALIASES[key].lower()]
    return License(license_id.strip(), license_id.strip(), "", "unknown")


def norm(license_id: str) -> str:
    """The canonical id (``gplv3`` → ``GPL-3.0-or-later``)."""
    return get(license_id).id


STATUS_ORDER = ("open", "free", "unknown", "nonfree")


def status_matches(license_id: str, wanted: str) -> bool:
    """Filter: ``all``; ``open``; ``free`` = open or free; ``unknown``; ``nonfree``."""
    st = get(license_id).status
    if wanted in ("", "all"):
        return True
    if wanted == "free":
        return st in ("open", "free")
    return st == wanted
