//! Catalog entries: one downloadable dictionary. Same keys, rules and
//! normalisation as the Python edition (`dictz/center/model.py`).

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

use super::{langs, licenses};

/// One file of an entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileRef {
    /// File name.
    pub name: String,
    /// Where to download it.
    pub url: String,
    /// Bytes (0 = unknown).
    pub size: u64,
    /// Checksums (lower-case hex, '' = not given).
    pub sha256: String,
    /// SHA-512.
    pub sha512: String,
    /// MD5.
    pub md5: String,
    /// `zip`, `tar.gz`, `tar.xz`, `tar.bz2`, `tar`, `7z`, `rar` or ''.
    pub archive: String,
}

/// One downloadable dictionary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// `<source>:<id>`
    pub id: String,
    /// Source id.
    pub source: String,
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Home page.
    pub homepage: String,
    /// Its own page.
    pub page: String,
    /// Source language code.
    pub source_lang: String,
    /// Target language codes.
    pub target_lang: Vec<String>,
    /// bilingual | monolingual | multilingual | unknown
    pub kind: String,
    /// Format.
    pub format: String,
    /// `true` = dictz opens it, `false` = not yet, `null` = cannot tell.
    pub supported: Option<bool>,
    /// Headwords.
    pub headwords: u64,
    /// Version.
    pub version: String,
    /// `YYYY-MM-DD`
    pub date: String,
    /// Bytes.
    pub size: u64,
    /// Files.
    pub files: Vec<FileRef>,
    /// Licence id.
    pub license: String,
    /// Licence text.
    pub license_url: String,
    /// Author.
    pub author: String,
    /// Tags.
    pub tags: Vec<String>,
    /// A few headwords.
    pub sample: String,
    /// Looked up when installed (`archive-org:<id>`).
    pub resolve: String,
}

impl Entry {
    /// As JSON (all keys).
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// From JSON written by either edition.
    pub fn from_value(v: &Value) -> Option<Self> {
        serde_json::from_value(v.clone()).ok()
    }
}

/// Every key of an entry.
pub const ENTRY_KEYS: &[&str] = &[
    "id",
    "source",
    "name",
    "description",
    "homepage",
    "page",
    "source_lang",
    "target_lang",
    "kind",
    "format",
    "supported",
    "headwords",
    "version",
    "date",
    "size",
    "files",
    "license",
    "license_url",
    "author",
    "tags",
    "sample",
    "resolve",
];

/// Formats dictz opens today.
pub const SUPPORTED_FORMATS: &[&str] = &["stardict", "dsl", "babylon"];
/// Every format name.
pub const KNOWN_FORMATS: &[&str] = &[
    "stardict", "dsl", "babylon", "mdict", "dictd", "xdxf", "slob", "tabfile", "mobi", "epub",
    "zim", "lingoes", "json", "archive", "unknown",
];

const ARCHIVES: &[(&str, &str)] = &[
    (".tar.gz", "tar.gz"),
    (".tgz", "tar.gz"),
    (".tar.xz", "tar.xz"),
    (".txz", "tar.xz"),
    (".tar.bz2", "tar.bz2"),
    (".tbz2", "tar.bz2"),
    (".tbz", "tar.bz2"),
    (".tar", "tar"),
    (".zip", "zip"),
    (".7z", "7z"),
    (".rar", "rar"),
];

/// `x.tar.xz` → `tar.xz`; '' when the file is not an archive.
pub fn archive_of(name: &str) -> String {
    let low = name.to_lowercase();
    ARCHIVES
        .iter()
        .find(|(ext, _)| low.ends_with(ext))
        .map(|(_, k)| k.to_string())
        .unwrap_or_default()
}

const FORMAT_WORDS: &[(&str, &str)] = &[
    ("stardict", "stardict"),
    ("startdict", "stardict"),
    ("babylon", "babylon"),
    ("bgl", "babylon"),
    ("lingvo", "dsl"),
    ("dsl", "dsl"),
    ("mdict", "mdict"),
    ("mdx", "mdict"),
    ("dictd", "dictd"),
    ("xdxf", "xdxf"),
    ("slob", "slob"),
    ("tabfile", "tabfile"),
    ("tsv", "tabfile"),
    ("mobi", "mobi"),
    ("kindle", "mobi"),
    ("epub", "epub"),
    ("zim", "zim"),
    ("lingoes", "lingoes"),
    ("ld2", "lingoes"),
    ("json", "json"),
];

fn re(pat: &'static str, cell: &'static OnceLock<Regex>) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("pattern"))
}

/// The dictionary format a file name says.
pub fn format_of(name: &str) -> String {
    let low = name.to_lowercase();
    for (ext, fmt) in [
        (".ifo", "stardict"),
        (".dict.dz", "stardict"),
        (".idx", "stardict"),
        (".dsl.dz", "dsl"),
        (".dsl", "dsl"),
        (".bgl", "babylon"),
        (".mdx", "mdict"),
        (".mdd", "mdict"),
        (".index", "dictd"),
        (".xdxf", "xdxf"),
        (".slob", "slob"),
        (".mobi", "mobi"),
        (".azw3", "mobi"),
        (".epub", "epub"),
        (".zim", "zim"),
        (".ld2", "lingoes"),
        (".tsv", "tabfile"),
    ] {
        if low.ends_with(ext) {
            return fmt.into();
        }
    }
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    let words: Vec<&str> = re("[^a-z0-9]+", &SPLIT).split(&low).collect();
    for (w, fmt) in FORMAT_WORDS {
        if words.contains(w) {
            return fmt.to_string();
        }
    }
    if archive_of(name).is_empty() {
        "unknown".into()
    } else {
        "archive".into()
    }
}

/// `Some(true)` = dictz opens it, `Some(false)` = not yet, `None` = cannot tell.
pub fn is_supported(fmt: &str) -> Option<bool> {
    if SUPPORTED_FORMATS.contains(&fmt) {
        Some(true)
    } else if fmt == "archive" || fmt == "unknown" {
        None
    } else {
        Some(false)
    }
}

/// Python `str(v).strip()` for the JSON values catalogs use.
pub fn vstr(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Bool(b)) => if *b { "True" } else { "False" }.into(),
        Some(o) => o.to_string().trim().to_string(),
    }
}

/// Python `max(0, int(str(v).replace(",", "")))`, 0 when not a number.
pub fn vint(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_i64().map(|x| x.max(0) as u64))
            .unwrap_or(0), // 5.5 / 5.0: Python's int("5.0") fails too
        Some(Value::String(s)) => {
            let t = s.trim().replace(',', "");
            if t.is_empty() {
                0
            } else {
                t.parse::<i64>().map(|x| x.max(0) as u64).unwrap_or(0)
            }
        }
        _ => 0,
    }
}

fn vlist(v: Option<&Value>) -> Vec<Value> {
    match v {
        None | Some(Value::Null) => vec![],
        Some(Value::String(s)) if s.is_empty() => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(o) => vec![o.clone()],
    }
}

/// One file from what a catalog gave (a string = its address).
pub fn norm_file(f: &Value) -> FileRef {
    let empty = Map::new();
    let (m, url) = match f {
        Value::String(s) => (&empty, s.trim().to_string()),
        Value::Object(o) => (o, vstr(o.get("url"))),
        _ => (&empty, String::new()),
    };
    let mut name = vstr(m.get("name"));
    if name.is_empty() {
        let last = url.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        name = last.split('?').next().unwrap_or("").to_string();
    }
    let name = super::util::unquote(&name);
    let mut archive = vstr(m.get("archive"));
    if archive.is_empty() {
        archive = archive_of(&name);
    }
    FileRef {
        size: vint(m.get("size")),
        sha256: vstr(m.get("sha256")).to_lowercase(),
        sha512: vstr(m.get("sha512")).to_lowercase(),
        md5: vstr(m.get("md5")).to_lowercase(),
        archive,
        name,
        url,
    }
}

/// bilingual | monolingual | multilingual | unknown
pub fn kind_of(source: &str, targets: &[String]) -> String {
    if source.is_empty() || targets.is_empty() {
        "unknown".into()
    } else if targets.len() > 1 {
        "multilingual".into()
    } else if targets[0] == source {
        "monolingual".into()
    } else {
        "bilingual".into()
    }
}

/// A complete, canonical entry from what a catalog / adapter gave.
pub fn normalize(e: &Value, source_id: &str, default_license: &str) -> Entry {
    let empty = Map::new();
    let m = e.as_object().unwrap_or(&empty);
    let files: Vec<FileRef> = vlist(m.get("files")).iter().map(norm_file).collect();
    let mut name = vstr(m.get("name"));
    if name.is_empty() {
        name = files.first().map(|f| f.name.clone()).unwrap_or_default();
    }
    let src = langs::norm(&vstr(m.get("source_lang")));
    let mut targets: Vec<String> = Vec::new();
    for t in vlist(m.get("target_lang")) {
        let code = langs::norm(&vstr(Some(&t)));
        if !code.is_empty() && !targets.contains(&code) {
            targets.push(code);
        }
    }
    let mut fmt = vstr(m.get("format")).to_lowercase();
    if fmt == "bgl" {
        fmt = "babylon".into();
    }
    if !KNOWN_FORMATS.contains(&fmt.as_str()) {
        fmt.clear();
    }
    if fmt.is_empty() {
        let mut guesses: Vec<String> = files.iter().map(|f| format_of(&f.name)).collect();
        guesses.push(format_of(&name));
        fmt = guesses
            .iter()
            .find(|g| *g != "archive" && *g != "unknown")
            .cloned()
            .unwrap_or_default();
        if fmt.is_empty() {
            fmt = if guesses.iter().any(|g| g == "archive") {
                "archive".into()
            } else {
                "unknown".into()
            };
        }
    }
    let lic_raw = vstr(m.get("license"));
    let lic = licenses::norm(if !lic_raw.is_empty() {
        &lic_raw
    } else if !default_license.is_empty() {
        default_license
    } else {
        "unknown"
    });
    let supported = match m.get("supported") {
        Some(Value::Bool(b)) => Some(*b),
        _ => is_supported(&fmt),
    };
    let mut size = vint(m.get("size"));
    if size == 0 {
        size = files.iter().map(|f| f.size).sum();
    }
    let mut eid = vstr(m.get("id"));
    if eid.is_empty() {
        static NON: OnceLock<Regex> = OnceLock::new();
        eid = re("[^a-z0-9._-]+", &NON)
            .replace_all(&name.to_lowercase(), "-")
            .trim_matches('-')
            .to_string();
    }
    if !source_id.is_empty() && !eid.starts_with(&format!("{}:", source_id)) {
        eid = format!("{}:{}", source_id, eid);
    }
    let mut tags: Vec<String> = vlist(m.get("tags"))
        .iter()
        .map(|t| vstr(Some(t)))
        .filter(|t| !t.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    let mut kind = vstr(m.get("kind"));
    if kind.is_empty() {
        kind = kind_of(&src, &targets);
    }
    let mut license_url = vstr(m.get("license_url"));
    if license_url.is_empty() {
        license_url = licenses::get(&lic).url;
    }
    Entry {
        id: eid,
        source: if source_id.is_empty() {
            vstr(m.get("source"))
        } else {
            source_id.to_string()
        },
        name,
        description: vstr(m.get("description")),
        homepage: vstr(m.get("homepage")),
        page: vstr(m.get("page")),
        source_lang: src,
        target_lang: targets,
        kind,
        format: fmt,
        supported,
        headwords: vint(m.get("headwords")),
        version: vstr(m.get("version")),
        date: super::util::take(&vstr(m.get("date")), 10),
        size,
        files,
        license: lic,
        license_url,
        author: vstr(m.get("author")),
        tags,
        sample: vstr(m.get("sample")),
        resolve: vstr(m.get("resolve")),
    }
}

/// Something to download.
pub fn installable(e: &Entry) -> bool {
    !e.files.is_empty() || !e.resolve.is_empty()
}

/// `en → tr`, `en → de, fr`, `tr` (monolingual) or ''.
pub fn pair_text(e: &Entry) -> String {
    if e.source_lang.is_empty() {
        return String::new();
    }
    if e.kind == "monolingual" || e.target_lang.is_empty() {
        return e.source_lang.clone();
    }
    format!("{} → {}", e.source_lang, e.target_lang.join(", "))
}

/// The catalog filters.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Words that must all appear.
    pub text: String,
    /// Source language code.
    pub src: String,
    /// Target language code.
    pub dst: String,
    /// Source id.
    pub source: String,
    /// all | open | free | unknown | nonfree
    pub license: String,
    /// Format.
    pub fmt: String,
    /// Only what dictz opens.
    pub supported_only: bool,
}

/// Same rules as the Python edition's `matches`.
pub fn matches(e: &Entry, f: &Filter) -> bool {
    if !f.src.is_empty() && e.source_lang != f.src {
        return false;
    }
    if !f.dst.is_empty() && !e.target_lang.contains(&f.dst) {
        return false;
    }
    if !f.source.is_empty() && e.source != f.source {
        return false;
    }
    if !f.fmt.is_empty() && e.format != f.fmt {
        return false;
    }
    if f.supported_only && e.supported == Some(false) {
        return false;
    }
    if !licenses::status_matches(
        &e.license,
        if f.license.is_empty() {
            "all"
        } else {
            &f.license
        },
    ) {
        return false;
    }
    if !f.text.is_empty() {
        let hay = [
            e.name.as_str(),
            e.description.as_str(),
            e.author.as_str(),
            e.id.as_str(),
            &e.tags.join(" "),
            &pair_text(e),
        ]
        .join(" ")
        .to_lowercase();
        return f
            .text
            .to_lowercase()
            .split_whitespace()
            .all(|w| hay.contains(w));
    }
    true
}

/// Sort key: (lower name, id).
pub fn sort_key(e: &Entry) -> (String, String) {
    (e.name.to_lowercase(), e.id.clone())
}

/// `1536` → `1.5 KB`; `-` for 0 (both editions print sizes the same way).
pub fn human_size(n: u64) -> String {
    if n == 0 {
        return "-".into();
    }
    if n < 1024 {
        return format!("{} B", n);
    }
    let mut v = n as f64;
    for unit in ["KB", "MB", "GB", "TB"] {
        v /= 1024.0;
        if v < 1024.0 || unit == "TB" {
            return format!("{:.1} {}", v, unit);
        }
    }
    format!("{} B", n)
}

/// An object `Value` builder for adapters.
pub fn obj(v: Value) -> Value {
    if v.is_object() {
        v
    } else {
        json!({})
    }
}
