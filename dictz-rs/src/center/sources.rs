//! Sources (servers) the catalog is read from — same file and rules as the
//! Python edition (`dictz/center/sources.py`): `DATA/center/sources.json`
//! with the built-in sources written in the first time, `removed` built-ins
//! remembered, `default` server first; passwords and tokens only in
//! `credentials.json` or `DICTZ_SOURCE_<ID>_PASSWORD` / `_TOKEN`.

use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{json, Map, Value};

use super::store::JsonFile;
use super::CENTER_DIR;

/// Source types.
pub const TYPES: &[&str] = &[
    "dictz-catalog",
    "freedict",
    "github-releases",
    "github-folder",
    "http-index",
    "webdav",
    "nextcloud-share",
    "archive-org",
    "local",
    "telegram",
];
/// Sign-in kinds.
pub const AUTH: &[&str] = &["none", "basic", "bearer", "header"];
/// The default server.
pub const DEFAULT_SERVER: &str = "freedict";

/// One source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Source {
    /// Id (a slug).
    pub id: String,
    /// Name.
    pub name: String,
    /// Type (`TYPES`).
    pub kind: String,
    /// Address.
    pub url: String,
    /// Read for the catalog.
    pub enabled: bool,
    /// Shipped with dictz.
    pub builtin: bool,
    /// Home page.
    pub homepage: String,
    /// Description.
    pub description: String,
    /// Licence when the source does not say.
    pub license: String,
    /// Source language when the source does not say.
    pub source_lang: String,
    /// Target language when the source does not say.
    pub target_lang: String,
    /// File names to list (archive.org: the search query).
    pub filter: String,
    /// Languages from names: `(?P<src>…)` `(?P<dst>…)`.
    pub name_pattern: String,
    /// Folders deep (listings) / pages (Telegram) / releases.
    pub depth: i64,
    /// none | basic | bearer | header
    pub auth: String,
    /// User name (basic) or header name (header).
    pub user: String,
    /// Notes.
    pub notes: String,
}

impl Source {
    /// As JSON (the file's keys; `type` for `kind`).
    pub fn to_value(&self) -> Value {
        json!({
            "id": self.id, "name": self.name, "type": self.kind, "url": self.url, "enabled": self.enabled,
            "builtin": self.builtin, "homepage": self.homepage, "description": self.description,
            "license": self.license, "source_lang": self.source_lang, "target_lang": self.target_lang,
            "filter": self.filter, "name_pattern": self.name_pattern, "depth": self.depth, "auth": self.auth,
            "user": self.user, "notes": self.notes,
        })
    }
}

fn s(m: &Map<String, Value>, k: &str) -> String {
    match m.get(k) {
        Some(Value::String(x)) => x.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(o) => o.to_string(),
    }
}

/// `text` → `a-b-c` (48 characters at most).
pub fn slug(text: &str) -> String {
    let re = Regex::new("[^a-z0-9]+").expect("pattern");
    let x = re
        .replace_all(&text.to_lowercase(), "-")
        .trim_matches('-')
        .to_string();
    let x: String = x.chars().take(48).collect();
    if x.is_empty() {
        "source".into()
    } else {
        x
    }
}

/// A source with every key, the right types.
pub fn clean(v: &Value) -> Source {
    let empty = Map::new();
    let m = v.as_object().unwrap_or(&empty);
    let raw_id = [s(m, "id"), s(m, "name"), s(m, "url")]
        .into_iter()
        .find(|x| !x.is_empty())
        .unwrap_or_default();
    let depth = match m.get("depth") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(2),
        Some(Value::String(x)) => x.trim().parse().unwrap_or(2),
        None | Some(Value::Null) => 2,
        _ => 2,
    }
    .clamp(0, 8);
    let mut auth = s(m, "auth");
    if !AUTH.contains(&auth.as_str()) {
        auth = "none".into();
    }
    let id = slug(&raw_id);
    let mut name = s(m, "name");
    if name.is_empty() {
        name = id.clone();
    }
    let b = |k: &str, d: bool| match m.get(k) {
        Some(Value::Bool(x)) => *x,
        Some(Value::Null) | None => d,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(x)) => !x.is_empty(),
        Some(_) => true,
    };
    Source {
        id,
        name,
        kind: s(m, "type"),
        url: s(m, "url"),
        enabled: b("enabled", true),
        builtin: b("builtin", false),
        homepage: s(m, "homepage"),
        description: s(m, "description"),
        license: s(m, "license"),
        source_lang: s(m, "source_lang"),
        target_lang: s(m, "target_lang"),
        filter: s(m, "filter"),
        name_pattern: s(m, "name_pattern"),
        depth,
        auth,
        user: s(m, "user"),
        notes: s(m, "notes"),
    }
}

/// The built-in sources (same as the Python edition's `BUILTIN`).
pub fn builtin() -> Vec<Source> {
    let mk = |v: Value| {
        let mut o = v.as_object().cloned().unwrap_or_default();
        o.insert("builtin".into(), Value::Bool(true));
        o.entry("enabled").or_insert(Value::Bool(true));
        o.entry("depth").or_insert(json!(2));
        clean(&Value::Object(o))
    };
    vec![
        mk(
            json!({"id": "freedict", "name": "FreeDict", "type": "freedict", "url": "https://freedict.org/freedict-database.json",
            "homepage": "https://freedict.org/", "license": "freedict",
            "description": "Free bilingual dictionaries, about 200 language pairs (StarDict, dictd, slob)."}),
        ),
        mk(
            json!({"id": "wiktionary-vuizur", "name": "Wiktionary Dictionaries (Vuizur)", "type": "github-folder",
            "url": "https://github.com/Vuizur/Wiktionary-Dictionaries", "homepage": "https://github.com/Vuizur/Wiktionary-Dictionaries",
            "license": "CC-BY-SA-3.0", "filter": " stardict\\.tar\\.gz$", "target_lang": "en", "depth": 1,
            "name_pattern": "^(?P<src>.+?)-(?P<dst>English) Wiktionary dictionary",
            "description": "Wiktionary turned into StarDict: almost every language → English (kaikki.org / wiktextract)."}),
        ),
        mk(
            json!({"id": "wiktionary-xxyzz", "name": "Wiktionary StarDict (xxyzz)", "type": "github-releases",
            "url": "https://github.com/xxyzz/wiktionary_stardict", "homepage": "https://github.com/xxyzz/wiktionary_stardict",
            "license": "wiktionary", "filter": "\\.(zip|tar\\.gz|tar\\.xz|tar\\.bz2)$",
            "description": "Wiktionary dictionaries for KOReader and e-readers, rebuilt monthly."}),
        ),
        mk(
            json!({"id": "ecdict", "name": "ECDICT (English → Chinese)", "type": "github-releases",
            "url": "https://github.com/skywind3000/ECDICT", "homepage": "https://github.com/skywind3000/ECDICT",
            "license": "MIT", "source_lang": "en", "target_lang": "zh", "filter": "(?i)stardict.*\\.(zip|7z|tar\\.gz)$",
            "description": "Free English–Chinese dictionary database, with a StarDict build."}),
        ),
        mk(
            json!({"id": "huzheng", "name": "StarDict archive (huzheng mirror)", "type": "http-index",
            "url": "https://stardict.uber.space/", "homepage": "https://stardict.uber.space/", "license": "unknown",
            "filter": "\\.(tar\\.bz2|tar\\.gz|tgz|zip)$", "depth": 2,
            "description": "The huzheng.org StarDict archive (the oldest, largest, many languages), recovered from the Internet Archive; download.huzheng.org itself is gone. Licences are mostly not stated."}),
        ),
        mk(
            json!({"id": "archive-org", "name": "Internet Archive", "type": "archive-org", "url": "https://archive.org",
            "homepage": "https://archive.org/", "license": "unknown",
            "filter": "(stardict OR \"babylon bgl\" OR \"lingvo dsl\" OR goldendict) AND mediatype:(data OR software OR texts)",
            "description": "Dictionary packs uploaded by the community (StarDict, Babylon, DSL). Licences vary."}),
        ),
        mk(
            json!({"id": "telegram-goldendict", "name": "Telegram: Dictionaries_GoldenDict", "type": "telegram",
            "url": "https://t.me/s/Dictionaries_GoldenDict", "homepage": "https://t.me/Dictionaries_GoldenDict",
            "license": "unknown",
            "description": "A channel with packs in 40+ languages. Listed here; the files open in Telegram."}),
        ),
    ]
}

/// The type a pasted address most likely is.
pub fn guess_type(url: &str) -> &'static str {
    let u = url.trim().to_lowercase();
    if u.ends_with("freedict-database.json") {
        "freedict"
    } else if u.ends_with(".json") {
        "dictz-catalog"
    } else if u.contains("github.com/") && u.contains("/releases") {
        "github-releases"
    } else if u.contains("github.com/") {
        "github-folder"
    } else if u.contains("/index.php/s/") || (u.contains("/s/") && u.contains("cloud")) {
        "nextcloud-share"
    } else if u.contains("archive.org") {
        "archive-org"
    } else if u.contains("t.me/") {
        "telegram"
    } else if u.starts_with("webdav://")
        || u.starts_with("webdavs://")
        || u.contains("/remote.php/dav")
    {
        "webdav"
    } else if u.starts_with("http://") || u.starts_with("https://") {
        "http-index"
    } else {
        "local"
    }
}

/// `DATA/center`.
pub fn center_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CENTER_DIR)
}

fn empty_file() -> Value {
    json!({"version": 1, "default": DEFAULT_SERVER, "removed": [], "sources": []})
}

fn empty_obj() -> Value {
    json!({})
}

/// `sources.json` + `credentials.json`.
pub struct Sources {
    /// `DATA/center`
    pub dir: PathBuf,
    file: JsonFile,
    secrets: JsonFile,
}

/// Earlier addresses of built-in sources. A stored built-in still on one of
/// them is moved to today's address (and its cached answer dropped); an
/// address the user typed stays. Mirrors `RETIRED_URLS` in `sources.py`.
pub const RETIRED_URLS: &[(&str, &[&str])] = &[(
    "huzheng",
    &[
        "https://download.huzheng.org/",
        "http://download.huzheng.org/",
    ],
)];

/// Built-in sources that were dropped, with their built-in addresses. A
/// stored copy still on that address goes (with its cache); one the user
/// pointed elsewhere stays as the user's own source. Mirrors
/// `RETIRED_BUILTINS` in `sources.py`.
pub const RETIRED_BUILTINS: &[(&str, &[&str])] = &[
    (
        // its server answers HTTP 500 to folder listings (v0.2.0)
        "freemdict",
        &["https://cloud.freemdict.com/index.php/s/pgKcDcbSDTCzXCs"],
    ),
    (
        // dictz's own server: its catalog was empty, and the repository it
        // was read from is private since v0.2.1 (it only showed an error)
        "dictz-data",
        &[
            "https://raw.githubusercontent.com/bayramkotan/dictz/main/docs/dictz-data/catalog.json",
            "https://raw.githubusercontent.com/bayramkotan/dictz-data/main/catalog.json",
        ],
    ),
];

/// A stored built-in that is no longer built in.
fn dropped(s: &Source) -> bool {
    s.builtin
        && RETIRED_BUILTINS
            .iter()
            .any(|(id, urls)| *id == s.id && urls.contains(&s.url.as_str()))
}

/// Today's built-in for a stored built-in still on a retired address.
fn retired(s: &Source) -> Option<Source> {
    if !s.builtin {
        return None;
    }
    let old = RETIRED_URLS.iter().find(|(id, _)| *id == s.id)?.1;
    if !old.contains(&s.url.as_str()) {
        return None;
    }
    builtin().into_iter().find(|b| b.id == s.id)
}

fn complete(data: &mut Value) -> bool {
    if !data.is_object() {
        *data = empty_file();
    }
    let o = data.as_object_mut().expect("object");
    o.entry("version").or_insert(json!(1));
    o.entry("default").or_insert(json!(DEFAULT_SERVER));
    o.entry("removed").or_insert(json!([]));
    let removed: Vec<String> = o["removed"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut rows: Vec<Source> = o
        .get("sources")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter(|x| x.is_object()).map(clean).collect())
        .unwrap_or_default();
    let mut changed = false;
    let before = rows.len();
    rows.retain(|r| !dropped(r));
    if rows.len() != before {
        changed = true;
    }
    let default_ok = o
        .get("default")
        .and_then(|v| v.as_str())
        .map(|d| d == DEFAULT_SERVER || rows.iter().any(|r| r.id == d))
        .unwrap_or(false);
    if !default_ok {
        o.insert("default".into(), json!(DEFAULT_SERVER));
        changed = true;
    }
    for r in rows.iter_mut() {
        if let Some(b) = retired(r) {
            r.name = b.name;
            r.kind = b.kind;
            r.url = b.url;
            r.homepage = b.homepage;
            r.description = b.description;
            r.filter = b.filter;
            r.depth = b.depth;
            r.license = b.license;
            changed = true;
        }
    }
    for (i, b) in builtin().into_iter().enumerate() {
        if !rows.iter().any(|r| r.id == b.id) && !removed.contains(&b.id) {
            let at = i.min(rows.len());
            rows.insert(at, b);
            changed = true;
        }
    }
    o.insert(
        "sources".into(),
        Value::Array(rows.iter().map(Source::to_value).collect()),
    );
    changed
}

impl Sources {
    /// The sources of a data folder.
    pub fn new(data_dir: &Path) -> Self {
        let dir = center_dir(data_dir);
        Self {
            file: JsonFile::new(dir.join("sources.json"), empty_file),
            secrets: JsonFile::new(dir.join("credentials.json"), empty_obj),
            dir,
        }
    }

    /// The whole file (built-ins completed).
    pub fn data(&self) -> Value {
        let mut d = self.file.read();
        if let Some(rows) = d.get("sources").and_then(|v| v.as_array()) {
            for s in rows.iter().filter(|x| x.is_object()).map(clean) {
                if retired(&s).is_some() || dropped(&s) {
                    // its cached answer is the old address's error
                    let _ =
                        std::fs::remove_file(self.dir.join("cache").join(format!("{}.json", s.id)));
                }
            }
        }
        let changed = complete(&mut d);
        if changed || !self.file.path.exists() {
            let _ = self.file.update(|x| {
                complete(x);
            });
        }
        d
    }

    /// Every source, in order.
    pub fn all(&self) -> Vec<Source> {
        self.data()["sources"]
            .as_array()
            .map(|a| a.iter().map(clean).collect())
            .unwrap_or_default()
    }

    /// The switched-on ones.
    pub fn enabled(&self) -> Vec<Source> {
        self.all().into_iter().filter(|s| s.enabled).collect()
    }

    /// By id.
    pub fn get(&self, id: &str) -> Option<Source> {
        self.all().into_iter().find(|s| s.id == id)
    }

    /// By id or (case-insensitive) name.
    pub fn find(&self, key: &str) -> Option<Source> {
        let k = key.trim().to_lowercase();
        self.all()
            .into_iter()
            .find(|s| s.id == k || s.name.to_lowercase() == k)
    }

    /// The default server's id.
    pub fn default_id(&self) -> String {
        self.data()["default"]
            .as_str()
            .unwrap_or(DEFAULT_SERVER)
            .to_string()
    }

    fn change(&self, f: impl FnOnce(&mut Map<String, Value>)) {
        let _ = self.file.update(|d| {
            complete(d);
            if let Some(o) = d.as_object_mut() {
                f(o);
            }
        });
    }

    fn rows(o: &Map<String, Value>) -> Vec<Source> {
        o.get("sources")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(clean).collect())
            .unwrap_or_default()
    }

    fn put(o: &mut Map<String, Value>, rows: &[Source]) {
        o.insert(
            "sources".into(),
            Value::Array(rows.iter().map(Source::to_value).collect()),
        );
    }

    /// Add a user source (a unique id: `x`, `x-2` …).
    pub fn add(&self, v: &Value) -> Source {
        let mut o = v.as_object().cloned().unwrap_or_default();
        o.insert("builtin".into(), Value::Bool(false));
        let mut src = clean(&Value::Object(o));
        let taken: Vec<String> = self.all().into_iter().map(|s| s.id).collect();
        let base = src.id.clone();
        let mut n = 2;
        while taken.contains(&src.id) {
            src.id = format!("{}-{}", base, n);
            n += 1;
        }
        let add = src.clone();
        self.change(|o| {
            let mut rows = Self::rows(o);
            rows.push(add);
            Self::put(o, &rows);
        });
        src
    }

    /// Change fields of a source (`id` and `builtin` stay).
    pub fn update(&self, id: &str, fields: &Value) -> Option<Source> {
        let mut out = None;
        self.change(|o| {
            let mut rows = Self::rows(o);
            for r in rows.iter_mut() {
                if r.id == id {
                    let mut m = r.to_value().as_object().cloned().unwrap_or_default();
                    if let Some(f) = fields.as_object() {
                        for (k, v) in f {
                            if k != "id" && k != "builtin" {
                                m.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    let mut n = clean(&Value::Object(m));
                    n.id = r.id.clone();
                    n.builtin = r.builtin;
                    *r = n.clone();
                    out = Some(n);
                }
            }
            Self::put(o, &rows);
        });
        out
    }

    /// On / off.
    pub fn set_enabled(&self, id: &str, on: bool) {
        self.update(id, &json!({"enabled": on}));
    }

    /// Remove (a built-in one stays removed until reset).
    pub fn remove(&self, id: &str) -> bool {
        let mut found = false;
        self.change(|o| {
            let mut rows = Self::rows(o);
            if let Some(i) = rows.iter().position(|r| r.id == id) {
                let r = rows.remove(i);
                found = true;
                if r.builtin {
                    let rem = o.entry("removed").or_insert(json!([]));
                    if let Some(a) = rem.as_array_mut() {
                        if !a.iter().any(|x| x == id) {
                            a.push(json!(id));
                        }
                    }
                }
            }
            if o.get("default").and_then(|d| d.as_str()) == Some(id) {
                let d = rows
                    .first()
                    .map(|r| r.id.clone())
                    .unwrap_or_else(|| DEFAULT_SERVER.into());
                o.insert("default".into(), json!(d));
            }
            Self::put(o, &rows);
        });
        self.set_secret(id, "", "");
        found
    }

    /// Up (−1) / down (+1).
    pub fn move_by(&self, id: &str, delta: i64) {
        self.change(|o| {
            let mut rows = Self::rows(o);
            if let Some(i) = rows.iter().position(|r| r.id == id) {
                let j = (i as i64 + delta).clamp(0, rows.len() as i64 - 1) as usize;
                let r = rows.remove(i);
                rows.insert(j, r);
            }
            Self::put(o, &rows);
        });
    }

    /// The default server: listed first, read first.
    pub fn set_default(&self, id: &str) {
        self.change(|o| {
            let mut rows = Self::rows(o);
            if let Some(i) = rows.iter().position(|r| r.id == id) {
                o.insert("default".into(), json!(id));
                let r = rows.remove(i);
                rows.insert(0, r);
            }
            Self::put(o, &rows);
        });
    }

    /// Built-in sources back as shipped (the user's own stay).
    pub fn reset(&self) {
        self.change(|o| {
            let mine: Vec<Source> = Self::rows(o).into_iter().filter(|r| !r.builtin).collect();
            let mut rows = builtin();
            rows.extend(mine);
            o.insert("removed".into(), json!([]));
            o.insert("default".into(), json!(DEFAULT_SERVER));
            Self::put(o, &rows);
        });
    }

    /// (password, token): environment first, then `credentials.json`.
    pub fn secret(&self, id: &str) -> (String, String) {
        let env = format!("DICTZ_SOURCE_{}", id.to_uppercase().replace('-', "_"));
        let stored = self.secrets.read();
        let rec = stored.get(id).cloned().unwrap_or(json!({}));
        let pw = std::env::var(format!("{}_PASSWORD", env)).unwrap_or_else(|_| {
            rec.get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        });
        let tok = std::env::var(format!("{}_TOKEN", env)).unwrap_or_else(|_| {
            rec.get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        });
        (pw, tok)
    }

    /// Keep (or, both empty, forget) a source's password / token.
    pub fn set_secret(&self, id: &str, password: &str, token: &str) {
        if password.is_empty() && token.is_empty() && !self.secrets.path.exists() {
            return;
        }
        let _ = self.secrets.update(|d| {
            if !d.is_object() {
                *d = json!({});
            }
            let o = d.as_object_mut().expect("object");
            if password.is_empty() && token.is_empty() {
                o.remove(id);
            } else {
                o.insert(id.into(), json!({"password": password, "token": token}));
            }
        });
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &self.secrets.path,
                std::fs::Permissions::from_mode(0o600),
            );
        }
    }

    /// Write the list (never passwords); returns the count.
    pub fn export(&self, path: &Path, include_builtin: bool) -> std::io::Result<usize> {
        let rows: Vec<Value> = self
            .all()
            .iter()
            .filter(|s| include_builtin || !s.builtin)
            .map(Source::to_value)
            .collect();
        let n = rows.len();
        JsonFile::new(path.to_path_buf(), empty_obj)
            .write(&json!({"dictz-sources": 1, "default": self.default_id(), "sources": rows}))?;
        Ok(n)
    }

    /// Add / update the sources of an exported list; `replace` drops the
    /// user's own first. Returns how many came in.
    pub fn import(&self, path: &Path, replace: bool) -> Result<usize, String> {
        let raw = JsonFile::new(path.to_path_buf(), empty_obj).read();
        let rows = raw
            .get("sources")
            .and_then(|v| v.as_array())
            .ok_or("not a dictz source list")?;
        let incoming: Vec<Source> = rows.iter().filter(|x| x.is_object()).map(clean).collect();
        let n = incoming.len();
        self.change(|o| {
            let mut cur = Self::rows(o);
            if replace {
                cur.retain(|r| r.builtin);
            }
            for mut s in incoming {
                match cur.iter().position(|x| x.id == s.id) {
                    None => cur.push(s),
                    Some(i) => {
                        s.builtin = cur[i].builtin;
                        cur[i] = s;
                    }
                }
            }
            Self::put(o, &cur);
        });
        Ok(n)
    }
}
