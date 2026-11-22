//! Licences of downloadable dictionaries — the same table as the Python
//! edition (`dictz/center/licenses.py`, checked by `licenses_golden.json`).
//!
//! `status`: `open` (a free / open licence), `free` (free to use, terms not a
//! known licence), `unknown` (the source does not say), `nonfree` (personal
//! use only or similar). Nothing here is legal advice.

/// One licence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct License {
    /// SPDX id where one exists.
    pub id: String,
    /// Display name.
    pub name: String,
    /// The licence text.
    pub url: String,
    /// open | free | unknown | nonfree
    pub status: String,
    /// The authors must be named when shared.
    pub attribution: bool,
    /// Changed copies keep the same licence.
    pub share_alike: bool,
    /// Commercial use allowed.
    pub commercial: bool,
}

type Row = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    bool,
    bool,
);

/// The table (id, name, url, status, attribution, share-alike, commercial).
pub const LICENSES: &[Row] = &[
    (
        "GPL-2.0-or-later",
        "GNU GPL 2.0 or later",
        "https://www.gnu.org/licenses/old-licenses/gpl-2.0.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "GPL-3.0-or-later",
        "GNU GPL 3.0 or later",
        "https://www.gnu.org/licenses/gpl-3.0.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "GPL-2.0-only",
        "GNU GPL 2.0",
        "https://www.gnu.org/licenses/old-licenses/gpl-2.0.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "GPL-3.0-only",
        "GNU GPL 3.0",
        "https://www.gnu.org/licenses/gpl-3.0.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "LGPL-2.1-or-later",
        "GNU LGPL 2.1 or later",
        "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "AGPL-3.0-or-later",
        "GNU AGPL 3.0 or later",
        "https://www.gnu.org/licenses/agpl-3.0.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "GFDL-1.3-or-later",
        "GNU Free Documentation License 1.3",
        "https://www.gnu.org/licenses/fdl-1.3.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "CC-BY-SA-3.0",
        "Creative Commons BY-SA 3.0",
        "https://creativecommons.org/licenses/by-sa/3.0/",
        "open",
        true,
        true,
        true,
    ),
    (
        "CC-BY-SA-4.0",
        "Creative Commons BY-SA 4.0",
        "https://creativecommons.org/licenses/by-sa/4.0/",
        "open",
        true,
        true,
        true,
    ),
    (
        "CC-BY-3.0",
        "Creative Commons BY 3.0",
        "https://creativecommons.org/licenses/by/3.0/",
        "open",
        true,
        false,
        true,
    ),
    (
        "CC-BY-4.0",
        "Creative Commons BY 4.0",
        "https://creativecommons.org/licenses/by/4.0/",
        "open",
        true,
        false,
        true,
    ),
    (
        "CC-BY-NC-SA-4.0",
        "Creative Commons BY-NC-SA 4.0",
        "https://creativecommons.org/licenses/by-nc-sa/4.0/",
        "free",
        true,
        true,
        false,
    ),
    (
        "CC-BY-NC-4.0",
        "Creative Commons BY-NC 4.0",
        "https://creativecommons.org/licenses/by-nc/4.0/",
        "free",
        true,
        false,
        false,
    ),
    (
        "CC0-1.0",
        "Creative Commons Zero (public domain)",
        "https://creativecommons.org/publicdomain/zero/1.0/",
        "open",
        false,
        false,
        true,
    ),
    (
        "public-domain",
        "Public domain",
        "https://en.wikipedia.org/wiki/Public_domain",
        "open",
        false,
        false,
        true,
    ),
    (
        "MIT",
        "MIT License",
        "https://opensource.org/license/mit",
        "open",
        true,
        false,
        true,
    ),
    (
        "BSD-3-Clause",
        "BSD 3-Clause",
        "https://opensource.org/license/bsd-3-clause",
        "open",
        true,
        false,
        true,
    ),
    (
        "Apache-2.0",
        "Apache License 2.0",
        "https://www.apache.org/licenses/LICENSE-2.0",
        "open",
        true,
        false,
        true,
    ),
    (
        "ODbL-1.0",
        "Open Database License 1.0",
        "https://opendatacommons.org/licenses/odbl/1-0/",
        "open",
        true,
        true,
        true,
    ),
    (
        "WordNet",
        "WordNet License (Princeton)",
        "https://wordnet.princeton.edu/license-and-commercial-use",
        "open",
        true,
        false,
        true,
    ),
    (
        "EDRDG",
        "EDRDG Licence (JMdict, KANJIDIC)",
        "https://www.edrdg.org/edrdg/licence.html",
        "open",
        true,
        true,
        true,
    ),
    (
        "freedict",
        "FreeDict (GPL, CC BY-SA … — each dictionary's own)",
        "https://freedict.org/",
        "open",
        true,
        true,
        true,
    ),
    (
        "wiktionary",
        "Wiktionary (CC BY-SA 4.0 / GFDL)",
        "https://en.wiktionary.org/wiki/Wiktionary:Copyrights",
        "open",
        true,
        true,
        true,
    ),
    (
        "free",
        "Free to use (terms not stated as a licence)",
        "",
        "free",
        false,
        false,
        true,
    ),
    (
        "personal",
        "Free for personal use only",
        "",
        "nonfree",
        true,
        false,
        false,
    ),
    (
        "unknown",
        "Unknown — the source does not say",
        "",
        "unknown",
        false,
        false,
        true,
    ),
];

const ALIASES: &[(&str, &str)] = &[
    ("gpl", "GPL-2.0-or-later"),
    ("gpl2", "GPL-2.0-or-later"),
    ("gpl-2", "GPL-2.0-or-later"),
    ("gpl-2.0", "GPL-2.0-or-later"),
    ("gplv2", "GPL-2.0-or-later"),
    ("gplv2+", "GPL-2.0-or-later"),
    ("gpl-2.0+", "GPL-2.0-or-later"),
    ("gpl3", "GPL-3.0-or-later"),
    ("gpl-3", "GPL-3.0-or-later"),
    ("gpl-3.0", "GPL-3.0-or-later"),
    ("gplv3", "GPL-3.0-or-later"),
    ("gplv3+", "GPL-3.0-or-later"),
    ("gpl-3.0+", "GPL-3.0-or-later"),
    ("gfdl", "GFDL-1.3-or-later"),
    ("gfdl-1.3", "GFDL-1.3-or-later"),
    ("cc-by-sa", "CC-BY-SA-4.0"),
    ("cc by-sa 3.0", "CC-BY-SA-3.0"),
    ("cc by-sa 4.0", "CC-BY-SA-4.0"),
    ("cc-by-sa 3.0", "CC-BY-SA-3.0"),
    ("cc-by", "CC-BY-4.0"),
    ("cc0", "CC0-1.0"),
    ("pd", "public-domain"),
    ("public domain", "public-domain"),
    ("mit license", "MIT"),
    ("apache", "Apache-2.0"),
    ("", "unknown"),
];

fn row(r: &Row) -> License {
    License {
        id: r.0.into(),
        name: r.1.into(),
        url: r.2.into(),
        status: r.3.into(),
        attribution: r.4,
        share_alike: r.5,
        commercial: r.6,
    }
}

/// Every licence.
pub fn all() -> Vec<License> {
    LICENSES.iter().map(row).collect()
}

/// The licence for an id or one of its spellings; unknown ids keep their
/// text as the name with status `unknown`.
pub fn get(id: &str) -> License {
    let key = id.trim().to_lowercase();
    if let Some(r) = LICENSES.iter().find(|r| r.0.to_lowercase() == key) {
        return row(r);
    }
    if let Some((_, to)) = ALIASES.iter().find(|(a, _)| *a == key) {
        return get(to);
    }
    let t = id.trim().to_string();
    License {
        id: t.clone(),
        name: t,
        url: String::new(),
        status: "unknown".into(),
        attribution: false,
        share_alike: false,
        commercial: true,
    }
}

/// The canonical id (`gplv3` → `GPL-3.0-or-later`).
pub fn norm(id: &str) -> String {
    get(id).id
}

/// Filter: `all`; `open`; `free` = open or free; `unknown`; `nonfree`.
pub fn status_matches(id: &str, wanted: &str) -> bool {
    let st = get(id).status;
    match wanted {
        "" | "all" => true,
        "free" => st == "open" || st == "free",
        w => st == w,
    }
}
