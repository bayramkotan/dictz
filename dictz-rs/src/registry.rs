//! The dictionary list: `dictionaries.json` in the data folder.
//!
//! Same file and rules as the Python edition (`dictz/storage/registry.py`):
//!
//! * the list order is the search order;
//! * a dictionary is identified by its **file path**; its `name` is unique —
//!   a second dictionary with the same name gets `"Name (2)"`;
//! * every change re-reads the file first (always, not only when it looks
//!   changed) and is written at once (temp file + rename), so a change made by
//!   the other edition, the CLI or another window is never overwritten;
//! * a change holds `dictionaries.json.lock` (threads, windows, the CLI and
//!   both editions take turns), so two changes at once never lose one;
//! * the previous file is kept as `dictionaries.json.bak`; when the file
//!   cannot be read, the backup is used;
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
    /// Files' `name:size:mtime_ns` when the article store was built: a start
    /// that finds the same stamp opens the dictionary from this list alone.
    pub stamp: String,
    /// Author.
    pub author: String,
    /// Cached icon in `DATA/icons` (`""` = not looked for yet, `"-"` = none).
    pub icon: String,
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
            stamp: String::new(),
            author: String::new(),
            icon: String::new(),
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
            stamp: s(v, "stamp"),
            author: s(v, "author"),
            icon: s(v, "icon"),
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
        m.insert("stamp".into(), json!(self.stamp));
        m.insert("author".into(), json!(self.author));
        m.insert("icon".into(), json!(self.icon));
        Value::Object(m)
    }
}

/// Held while the list is read, changed and written (see `Registry::lock`).
struct ListLock {
    _guard: std::sync::MutexGuard<'static, ()>,
    file: Option<PathBuf>,
}

impl Drop for ListLock {
    fn drop(&mut self) {
        if let Some(f) = self.file.take() {
            let _ = std::fs::remove_file(f);
        }
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
        self.reload_with(false);
    }

    /// `dictionaries.json.bak` next to the file.
    pub fn backup_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            let mut n = p.file_name().unwrap_or_default().to_os_string();
            n.push(".bak");
            p.with_file_name(n)
        })
    }

    /// Entries of a list file, `None` when it is missing or unreadable.
    fn read_file(path: &Path) -> Option<Vec<Entry>> {
        let text = std::fs::read_to_string(path).ok()?;
        let data = serde_json::from_str::<Value>(&text).ok()?;
        let arr = data.get("dictionaries")?.as_array()?;
        Some(arr.iter().filter_map(Entry::from_value).collect())
    }

    /// Re-read the file (only when it changed on disk, unless `force`).
    /// A half-written or broken file falls back to the backup.
    pub fn reload_with(&mut self, force: bool) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let stamp = self.file_stamp();
        if stamp.is_none() || (stamp == self.stamp && !force) {
            return;
        }
        let items =
            Self::read_file(&path).or_else(|| self.backup_path().and_then(|b| Self::read_file(&b)));
        if let Some(items) = items {
            self.items = items;
            self.stamp = stamp;
        }
    }

    /// Hold the list for one read-change-write: no other thread, window,
    /// CLI or edition changes it in between (`dictionaries.json.lock`, the
    /// same lock file as the Python edition; a lock older than 10 s is stale).
    fn lock(&self) -> ListLock {
        static THREADS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = THREADS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = self.path.as_ref() else {
            return ListLock {
                _guard: guard,
                file: None,
            };
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut n = path.file_name().unwrap_or_default().to_os_string();
        n.push(".lock");
        let lock = path.with_file_name(n);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock)
            {
                Ok(_) => {
                    return ListLock {
                        _guard: guard,
                        file: Some(lock),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(&lock)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|age| age > std::time::Duration::from_secs(10))
                        .unwrap_or(false);
                    if stale {
                        let _ = std::fs::remove_file(&lock);
                        continue;
                    }
                    if std::time::Instant::now() > deadline {
                        // do the change anyway rather than lose it
                        return ListLock {
                            _guard: guard,
                            file: None,
                        };
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                // read-only folder: no lock
                Err(_) => {
                    return ListLock {
                        _guard: guard,
                        file: None,
                    }
                }
            }
        }
    }

    /// Before a change: always read what is on disk now.
    fn fresh(&mut self) {
        self.reload_with(true);
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

    /// Write the file (temp file + rename); the previous file is kept as
    /// `dictionaries.json.bak`.
    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        std::fs::create_dir_all(&dir)?;
        let buf = self.to_text().into_bytes();
        if path.is_file() && Self::read_file(&path).is_some() {
            if let Some(b) = self.backup_path() {
                let _ = std::fs::copy(&path, b);
            }
        }
        // unique per process *and* call: the GUI's loader thread writes too
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!(".dictionaries-{}-{}.json", std::process::id(), seq));
        let res = (|| {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&buf)?;
            f.sync_all()?;
            drop(f);
            // Windows: the file may be open in the other edition for a moment
            let mut tries = 0;
            loop {
                match std::fs::rename(&tmp, &path) {
                    Ok(()) => return Ok(()),
                    Err(e) if tries < 9 && e.kind() == std::io::ErrorKind::PermissionDenied => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => return Err(e),
                }
            }
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
        let _lock = self.lock();
        self.fresh();
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
        let _lock = self.lock();
        self.fresh();
        if let Some(e) = self.items.iter_mut().find(|e| e.name == name) {
            f(e);
            let _ = self.save();
        }
    }

    /// Enable / disable.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.edit(name, |e| e.enabled = enabled);
    }

    /// Remember the cached icon (`"-"` = none).
    pub fn set_icon(&mut self, name: &str, icon: &str) {
        self.edit(name, |e| e.icon = icon.to_string());
    }

    /// Mark as indexed (or not).
    pub fn set_indexed(&mut self, name: &str, indexed: bool) {
        self.edit(name, |e| e.indexed = indexed);
    }

    /// Put `names` first in this order; the rest keep their order after them.
    pub fn set_order(&mut self, names: &[String]) {
        let _lock = self.lock();
        self.fresh();
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
        let _lock = self.lock();
        self.fresh();
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
        let _lock = self.lock();
        self.fresh();
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

    #[test]
    fn two_threads_writing_at_once_lose_nothing() {
        // the GUI's loader thread and the window write the same file
        let d = std::env::temp_dir().join(format!("dictz-reg-thr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join(REGISTRY_FILE);
        let hs: Vec<_> = (0..4)
            .map(|t| {
                let (f, d) = (f.clone(), d.clone());
                std::thread::spawn(move || {
                    let mut r = Registry::new(Some(f));
                    for i in 0..15 {
                        r.register(
                            &format!("D{t}-{i}"),
                            &d.join(format!("{t}-{i}.ifo")),
                            |_| {},
                        );
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        // nothing lost, no lock left behind
        let r = Registry::new(Some(f));
        assert_eq!(r.items.len(), 60);
        assert!(!d.join("dictionaries.json.lock").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_stale_instance_never_overwrites_and_a_broken_file_uses_the_backup() {
        let d = std::env::temp_dir().join(format!("dictz-reg-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join(REGISTRY_FILE);
        let mut a = Registry::new(Some(f.clone()));
        let mut b = Registry::new(Some(f.clone()));
        a.register("A", &d.join("a.ifo"), |_| {});
        b.reload();
        a.register("B", &d.join("b.ifo"), |_| {});
        b.stamp = b.file_stamp(); // as if both writes fell in one timestamp tick
        b.set_enabled("A", false);
        let names: Vec<_> = Registry::new(Some(f.clone()))
            .items
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["A", "B"]);
        assert!(d.join("dictionaries.json.bak").is_file());
        std::fs::write(&f, "{\"version\": 1, \"dictionaries\": [").unwrap();
        let again = Registry::new(Some(f));
        assert!(!again.items.is_empty(), "the backup is used");
        let _ = std::fs::remove_dir_all(&d);
    }
}
