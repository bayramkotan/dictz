//! `scan_cache.json`: what the dictionary folders held at the last scan.
//!
//! Same file and rules as the Python edition (`dictz/storage/scancache.py`).
//! A folder's listing is kept with the folder's modification time; a folder
//! whose time has not changed is not listed again (adding, removing or
//! renaming a file changes it). A start only looks at the folders' times. A
//! folder that changed less than 2 s before it was listed is listed again
//! next time (its time may not move within one clock tick).
//!
//! ```json
//! {"version": 1, "dirs": {"/abs/dir": [mtime_ns, ["a.ifo"], ["sub"]]}}
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// File name in the data folder.
pub const SCAN_CACHE_FILE: &str = "scan_cache.json";
/// A folder changed this recently before it was listed is listed again.
const RACY_NS: u128 = 2_000_000_000;

type Listing = (u128, Vec<String>, Vec<String>);

fn is_dictionary_name(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".ifo") || l.ends_with(".dsl") || l.ends_with(".dsl.dz") || l.ends_with(".bgl")
}

fn mtime_ns(p: &Path) -> Option<u128> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
}

/// The cache (`path == None`: in memory only).
#[derive(Debug, Default)]
pub struct ScanCache {
    path: Option<PathBuf>,
    dirs: BTreeMap<String, Listing>,
    changed: bool,
}

impl ScanCache {
    /// Read the cache file (a missing or broken file is an empty cache).
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut dirs = BTreeMap::new();
        if let Some(p) = &path {
            if let Some(Value::Object(m)) = std::fs::read_to_string(p)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("dirs").cloned())
            {
                for (k, v) in m {
                    let Some(a) = v.as_array().filter(|a| a.len() == 3) else {
                        continue;
                    };
                    let strs = |x: &Value| -> Vec<String> {
                        x.as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|s| s.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    let m = a[0]
                        .as_u64()
                        .map(|n| n as u128)
                        .or_else(|| a[0].as_f64().map(|f| f as u128));
                    if let Some(m) = m {
                        dirs.insert(k, (m, strs(&a[1]), strs(&a[2])));
                    }
                }
            }
        }
        Self {
            path,
            dirs,
            changed: false,
        }
    }

    fn listing(&mut self, d: &Path) -> Option<(Vec<String>, Vec<String>)> {
        let m = mtime_ns(d)?;
        let key = d.to_string_lossy().into_owned();
        if let Some((cm, f, s)) = self.dirs.get(&key) {
            if *cm == m {
                return Some((f.clone(), s.clone()));
            }
        }
        let mut entries: Vec<_> = std::fs::read_dir(d).ok()?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        let (mut files, mut subs) = (Vec::new(), Vec::new());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.path().is_dir();
            if is_dir {
                if !name.starts_with('.') && !name.to_lowercase().ends_with(".files") {
                    subs.push(name);
                }
            } else if is_dictionary_name(&name) {
                files.push(name);
            }
        }
        // A folder changed less than RACY_NS before it was listed may change
        // again within the same timestamp tick (coarse file-system clocks:
        // Linux ~4 ms, FAT 2 s): its time could stay the same while a file is
        // added. Such a listing is kept with time 0, so the next scan lists it
        // again.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let racy = now < m || now - m < RACY_NS;
        self.dirs
            .insert(key, (if racy { 0 } else { m }, files.clone(), subs.clone()));
        self.changed = true;
        Some((files, subs))
    }

    /// Same result as `query::scan_folder` (sorted dictionary files).
    pub fn scan(&mut self, folder: &Path, recursive: bool) -> Vec<PathBuf> {
        if folder.is_file() {
            let n = folder
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return if is_dictionary_name(&n) {
                vec![folder.to_path_buf()]
            } else {
                Vec::new()
            };
        }
        let mut out = Vec::new();
        let mut stack = vec![folder.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Some((files, subs)) = self.listing(&d) else {
                continue;
            };
            out.extend(files.iter().map(|f| d.join(f)));
            if recursive {
                stack.extend(subs.iter().map(|s| d.join(s)));
            }
        }
        out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        out
    }

    /// Write the file when something was listed again.
    pub fn save(&mut self) {
        let Some(path) = self.path.clone().filter(|_| self.changed) else {
            return;
        };
        let dirs: serde_json::Map<String, Value> = self
            .dirs
            .iter()
            .map(|(k, (m, f, s))| (k.clone(), json!([*m as u64, f, s])))
            .collect();
        let doc = json!({ "dirs": Value::Object(dirs), "version": 1 });
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        if let Ok(text) = serde_json::to_string_pretty(&doc) {
            if std::fs::write(&tmp, format!("{}\n", text)).is_ok()
                && std::fs::rename(&tmp, &path).is_err()
            {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        self.changed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_folders_are_not_listed_again() {
        let d = std::env::temp_dir().join(format!("dictz-scache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a/sub")).unwrap();
        std::fs::write(d.join("a/x.ifo"), "x").unwrap();
        std::fs::write(d.join("a/sub/y.dsl"), "y").unwrap();
        let f = d.join(SCAN_CACHE_FILE);
        // just changed (same clock tick as the listing): listed again next time
        let mut c = ScanCache::new(Some(f.clone()));
        let first = c.scan(&d.join("a"), true);
        assert_eq!(first, crate::query::scan_folder(&d.join("a"), true));
        c.save();
        std::fs::write(d.join("a/sub/z.bgl"), "z").unwrap();
        assert_eq!(
            ScanCache::new(Some(f.clone()))
                .scan(&d.join("a"), true)
                .len(),
            3
        );
        std::fs::remove_file(d.join("a/sub/z.bgl")).unwrap();
        // folders that changed a while ago: kept, not listed again (Unix: a
        // directory's time can be set through std)
        #[cfg(unix)]
        {
            let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
            for p in [d.join("a"), d.join("a/sub")] {
                std::fs::File::open(&p).unwrap().set_modified(old).unwrap();
            }
            let mut c = ScanCache::new(Some(f.clone()));
            assert_eq!(c.scan(&d.join("a"), true), first);
            c.save();
            let mut again = ScanCache::new(Some(f.clone()));
            assert_eq!(again.scan(&d.join("a"), true), first);
            assert!(!again.changed, "nothing listed again");
            std::fs::write(d.join("a/sub/z.bgl"), "z").unwrap();
            assert_eq!(ScanCache::new(Some(f)).scan(&d.join("a"), true).len(), 3);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
