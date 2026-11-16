//! The dictionary list: `dictionaries.json` in the data folder.
//!
//! Same file and rules as the Python edition (`dictz/storage/registry.py`):
//!
//! * the list order is the search order;
//! * a dictionary is identified by its **file path**; its `name` is unique —
//!   a second dictionary with the same name gets `"Name (2)"`;
//! * every change is written at once (temp file + rename), after re-reading
//!   the file so a change made by the other edition or the CLI is not lost;
//! * the first start migrates the old SQLite `dictionaries` table
//!   (see [`crate::index::IndexStore`]).

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{json, Map, Value};

/// File name inside the data folder.
pub const REGISTRY_FILE: &str = "dictionaries.json";

/// FNV-1a 64-bit hash.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

/// Absolute path as text (the identity of a dictionary); `~` is expanded,
/// symlinks are not resolved — like Python's `Path.expanduser().absolute()`.
pub fn norm_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    let expanded = if s == "~" || s.starts_with("~/") || s.starts_with("~\\") {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();
        if s.len() > 2 {
            home.join(&s[2..])
        } else {
            home
        }
    } else {
        path.to_path_buf()
    };
    std::path::absolute(&expanded)
        .unwrap_or(expanded)
        .to_string_lossy()
        .into_owned()
}

/// Stable id of a dictionary file: FNV-1a 64 of its absolute path, hex.
pub fn path_id(path: &Path) -> String {
    format!("{:016x}", fnv1a64(norm_path(path).as_bytes()))
}

/// `base`, or `base (2)`, `base (3)`… if it is taken.
pub fn unique_name(base: &str, taken: &HashSet<String>) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "?" } else { base };
    if !taken.contains(base) {
        return base.to_string();
    }
    let mut n = 2;
    while taken.contains(&format!("{} ({})", base, n)) {
        n += 1;
    }
    format!("{} ({})", base, n)
}

/// One entry of `dictionaries.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// [`path_id`] of `path`.
    pub id: String,
    /// Unique name.
    pub name: String,
    /// Absolute file path.
    pub path: String,
    /// "stardict", "dsl" or "babylon".
    pub format: String,
    /// Searched or not.
    pub enabled: bool,
    /// Headword count.
    pub word_count: i64,
    /// Source language.
    pub source_lang: String,
    /// Target language.
    pub target_lang: String,
    /// Description.
    pub description: String,
    /// File modification time when indexed (0 = unknown).
    pub mtime: f64,
    /// Headwords are in the FTS index.
    pub indexed: bool,
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Null) | None => String::new(),
        Some(x) => x.to_string(),
    }
}

impl Entry {
    /// New entry for `path` (enabled, not indexed).
    pub fn new(name: &str, path: &Path) -> Self {
        let p = norm_path(path);
        Self {
            id: format!("{:016x}", fnv1a64(p.as_bytes())),
            name: name.to_string(),
            path: p,
            format: String::new(),
            enabled: true,
            word_count: 0,
            source_lang: String::new(),
            target_lang: String::new(),
            description: String::new(),
            mtime: 0.0,
            indexed: false,
        }
    }

    /// Read one JSON object (missing keys get defaults); `None` without a path.
    pub fn from_value(v: &Value) -> Option<Self> {
        let path = s(v, "path");
        if path.is_empty() {
            return None;
        }
        let id = s(v, "id");
        let name = s(v, "name");
        let stem = Path::new(&path)
            .file_stem()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(Self {
            id: if id.is_empty() {
                path_id(Path::new(&path))
            } else {
                id
            },
            name: if name.is_empty() { stem } else { name },
            format: s(v, "format"),
            enabled: v.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            word_count: v.get("word_count").and_then(Value::as_i64).unwrap_or(0),
            source_lang: s(v, "source_lang"),
            target_lang: s(v, "target_lang"),
            description: s(v, "description"),
            mtime: v.get("mtime").and_then(Value::as_f64).unwrap_or(0.0),
            indexed: v.get("indexed").and_then(Value::as_bool).unwrap_or(false),
            path,
        })
    }

    /// The JSON object (same key order as the Python edition).
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("name".into(), json!(self.name));
        m.insert("path".into(), json!(self.path));
        m.insert("format".into(), json!(self.format));
        m.insert("enabled".into(), json!(self.enabled));
        m.insert("word_count".into(), json!(self.word_count));
        m.insert("source_lang".into(), json!(self.source_lang));
        m.insert("target_lang".into(), json!(self.target_lang));
        m.insert("description".into(), json!(self.description));
        m.insert("mtime".into(), json!(self.mtime));
        m.insert("indexed".into(), json!(self.indexed));
        Value::Object(m)
    }
}

/// `dictionaries.json` (`path == None` keeps it in memory only).
#[derive(Debug, Default)]
pub struct Registry {
    /// File (None = in memory).
    pub path: Option<PathBuf>,
    /// Entries in list order.
    pub items: Vec<Entry>,
    stamp: Option<(SystemTime, u64)>,
}

impl Registry {
    /// Open (and read) the file at `path`.
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut r = Self {
            path,
            items: Vec::new(),
            stamp: None,
        };
        r.reload();
        r
    }

    /// `true` when the file exists.
    pub fn exists(&self) -> bool {
        self.path.as_ref().map(|p| p.is_file()).unwrap_or(false)
    }

    fn file_stamp(&self) -> Option<(SystemTime, u64)> {
        let m = std::fs::metadata(self.path.as_ref()?).ok()?;
        Some((m.modified().ok()?, m.len()))
    }

    /// Re-read the file if it changed on disk.
    pub fn reload(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let stamp = self.file_stamp();
        if stamp.is_none() || stamp == self.stamp {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(data) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        self.items = data
            .get("dictionaries")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Entry::from_value).collect())
            .unwrap_or_default();
        self.stamp = stamp;
    }

    /// The file's text — byte for byte what the Python edition writes
    /// (`json.dumps(..., ensure_ascii=False, indent=1)` + newline).
    pub fn to_text(&self) -> String {
        let doc = json!({
            "version": 1,
            "dictionaries": self.items.iter().map(Entry::to_value).collect::<Vec<_>>(),
        });
        let mut buf = Vec::new();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
        let _ = serde::Serialize::serialize(&doc, &mut ser);
        buf.push(b'\n');
        String::from_utf8(buf).unwrap_or_default()
    }

    /// Read the file's text (what [`Registry::reload`] does).
    pub fn from_text(text: &str) -> Self {
        let items = serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|d| d.get("dictionaries").and_then(Value::as_array).cloned())
            .map(|a| a.iter().filter_map(Entry::from_value).collect())
            .unwrap_or_default();
        Self {
            path: None,
            items,
            stamp: None,
        }
    }

    /// Write the file (temp file + rename).
    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        std::fs::create_dir_all(&dir)?;
        let buf = self.to_text().into_bytes();
        let tmp = dir.join(format!(".dictionaries-{}.json", std::process::id()));
        let res = (|| {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&buf)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, &path)
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        res?;
        self.stamp = self.file_stamp();
        Ok(())
    }

    /// Entries (optionally only enabled ones) with their list index.
    pub fn rows(&mut self, enabled_only: bool) -> Vec<(usize, Entry)> {
        self.reload();
        self.items
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, e)| !enabled_only || e.enabled)
            .collect()
    }

    /// Entry by name.
    pub fn get(&mut self, name: &str) -> Option<Entry> {
        self.reload();
        self.items.iter().find(|e| e.name == name).cloned()
    }

    /// Entry by file path.
    pub fn by_path(&self, path: &Path) -> Option<&Entry> {
        let p = norm_path(path);
        self.items
            .iter()
            .find(|e| norm_path(Path::new(&e.path)) == p)
    }

    /// Add or update the dictionary at `path`; returns its (unique) name.
    /// An existing entry keeps its name, place and enabled flag; `update`
    /// sets the other fields.
    pub fn register(&mut self, name: &str, path: &Path, update: impl FnOnce(&mut Entry)) -> String {
        self.reload();
        let p = norm_path(path);
        let i = match self
            .items
            .iter()
            .position(|e| norm_path(Path::new(&e.path)) == p)
        {
            Some(i) => i,
            None => {
                let taken: HashSet<String> = self.items.iter().map(|e| e.name.clone()).collect();
                self.items
                    .push(Entry::new(&unique_name(name, &taken), path));
                self.items.len() - 1
            }
        };
        let e = &mut self.items[i];
        let (keep_name, keep_path, keep_enabled) = (e.name.clone(), e.path.clone(), e.enabled);
        update(e);
        e.name = keep_name;
        e.path = keep_path;
        e.enabled = keep_enabled;
        e.id = path_id(Path::new(&e.path));
        let out = e.name.clone();
        let _ = self.save();
        out
    }

    fn edit(&mut self, name: &str, f: impl FnOnce(&mut Entry)) {
        self.reload();
        if let Some(e) = self.items.iter_mut().find(|e| e.name == name) {
            f(e);
            let _ = self.save();
        }
    }

    /// Enable / disable.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.edit(name, |e| e.enabled = enabled);
    }

    /// Mark as indexed (or not).
    pub fn set_indexed(&mut self, name: &str, indexed: bool) {
        self.edit(name, |e| e.indexed = indexed);
    }

    /// Put `names` first in this order; the rest keep their order after them.
    pub fn set_order(&mut self, names: &[String]) {
        self.reload();
        let pos = |n: &str| names.iter().position(|x| x == n);
        let mut first: Vec<Entry> = self
            .items
            .iter()
            .filter(|e| pos(&e.name).is_some())
            .cloned()
            .collect();
        first.sort_by_key(|e| pos(&e.name));
        let rest = self
            .items
            .iter()
            .filter(|e| pos(&e.name).is_none())
            .cloned();
        first.extend(rest);
        self.items = first;
        let _ = self.save();
    }

    /// Move `name` to position `index`.
    pub fn move_to(&mut self, name: &str, index: usize) {
        self.reload();
        let Some(i) = self.items.iter().position(|e| e.name == name) else {
            return;
        };
        let e = self.items.remove(i);
        let j = index.min(self.items.len());
        self.items.insert(j, e);
        let _ = self.save();
    }

    /// Drop an entry.
    pub fn remove(&mut self, name: &str) {
        self.reload();
        self.items.retain(|e| e.name != name);
        let _ = self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_and_ids() {
        let mut taken = HashSet::new();
        assert_eq!(unique_name("A", &taken), "A");
        taken.insert("A".to_string());
        assert_eq!(unique_name("A", &taken), "A (2)");
        taken.insert("A (2)".to_string());
        assert_eq!(unique_name("A", &taken), "A (3)");
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn file_round_trip_and_order() {
        let d = std::env::temp_dir().join(format!("dictz-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join(REGISTRY_FILE);
        let mut r = Registry::new(Some(f.clone()));
        let a = r.register("Same", &d.join("a/x.ifo"), |e| e.format = "stardict".into());
        let b = r.register("Same", &d.join("b/x.ifo"), |_| {});
        assert_eq!((a.as_str(), b.as_str()), ("Same", "Same (2)"));
        // the same file again keeps its name and place
        assert_eq!(r.register("Other", &d.join("a/x.ifo"), |_| {}), "Same");
        r.set_enabled("Same", false);
        r.move_to("Same (2)", 0);
        let again = Registry::new(Some(f));
        let names: Vec<_> = again.items.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Same (2)", "Same"]);
        assert!(!again.items[1].enabled);
        assert_eq!(again.items[1].format, "stardict");
        let _ = std::fs::remove_dir_all(&d);
    }
}
