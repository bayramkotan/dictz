//! The catalog: every enabled source's entries, kept in `center/cache` —
//! same as the Python edition (`dictz/center/catalog.py`).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::adapters;
use super::model::{matches, sort_key, Entry, Filter};
use super::net::Fetch;
use super::sources::{center_dir, Source, Sources};
use super::store::JsonFile;
use super::util::{age_days, now_iso};

/// A source's last read.
#[derive(Debug, Clone, Default)]
pub struct Cached {
    /// Source id.
    pub source: String,
    /// When (ISO, '' = never).
    pub fetched: String,
    /// Read without error.
    pub ok: bool,
    /// The last error.
    pub error: String,
    /// Its entries.
    pub entries: Vec<Entry>,
}

fn empty_obj() -> Value {
    json!({})
}

/// The catalog of a data folder.
pub struct Catalog {
    data_dir: PathBuf,
    /// The sources.
    pub sources: Sources,
}

impl Catalog {
    /// For `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            sources: Sources::new(data_dir),
        }
    }

    fn cache_file(&self, id: &str) -> JsonFile {
        JsonFile::new(
            center_dir(&self.data_dir)
                .join("cache")
                .join(format!("{}.json", id)),
            empty_obj,
        )
    }

    /// The last read of a source.
    pub fn cached(&self, id: &str) -> Cached {
        let d = self.cache_file(id).read();
        Cached {
            source: id.to_string(),
            fetched: d
                .get("fetched")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ok: d.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
            error: d
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            entries: d
                .get("entries")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(Entry::from_value).collect())
                .unwrap_or_default(),
        }
    }

    /// Older than `days` (or never read).
    pub fn stale(&self, id: &str, days: f64) -> bool {
        let c = self.cached(id);
        c.fetched.is_empty() || (days > 0.0 && age_days(&c.fetched) >= days)
    }

    /// Read one source now; the cache keeps the old entries when it fails.
    pub fn refresh_one(&self, src: &Source, http: &dyn Fetch) -> Cached {
        let old = self.cached(&src.id);
        let (pw, tok) = self.sources.secret(&src.id);
        let res = match adapters::read(src, http, &pw, &tok) {
            Ok(entries) => Cached {
                source: src.id.clone(),
                fetched: now_iso(),
                ok: true,
                error: String::new(),
                entries,
            },
            Err(e) => Cached {
                ok: false,
                error: if e.0.is_empty() { "error".into() } else { e.0 },
                ..old
            },
        };
        let v = json!({
            "source": src.id, "fetched": res.fetched, "ok": res.ok, "error": res.error,
            "entries": res.entries.iter().map(Entry::to_value).collect::<Vec<_>>(),
        });
        let _ = self.cache_file(&src.id).write(&v);
        res
    }

    /// Read the enabled sources (or `ids`), several at once; only stale ones
    /// when `stale_days` > 0. `progress(id, result)` after each.
    pub fn refresh(
        &self,
        ids: Option<&[String]>,
        stale_days: f64,
        http: &dyn Fetch,
        progress: &(dyn Fn(&str, &Cached) + Sync),
        workers: usize,
    ) -> Vec<(String, Cached)> {
        let todo: Vec<Source> = self
            .sources
            .all()
            .into_iter()
            .filter(|s| match ids {
                Some(ids) => ids.contains(&s.id),
                None => s.enabled,
            })
            .filter(|s| stale_days <= 0.0 || self.stale(&s.id, stale_days))
            .collect();
        let out = std::sync::Mutex::new(Vec::new());
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..workers.max(1).min(todo.len().max(1)) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(s) = todo.get(i) else { break };
                    let r = self.refresh_one(s, http);
                    progress(&s.id, &r);
                    out.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push((i, s.id.clone(), r));
                });
            }
        });
        let mut v = out.into_inner().unwrap_or_default();
        v.sort_by_key(|x| x.0);
        v.into_iter().map(|(_, id, r)| (id, r)).collect()
    }

    /// Every entry of the (enabled) sources, in source order; the first
    /// source wins a duplicate id.
    pub fn entries(&self, include_disabled: bool) -> Vec<Entry> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for s in self.sources.all() {
            if !s.enabled && !include_disabled {
                continue;
            }
            for e in self.cached(&s.id).entries {
                if seen.insert(e.id.clone()) {
                    out.push(e);
                }
            }
        }
        out
    }

    /// By id, or by name (exact, then case-insensitive substring).
    pub fn find(&self, key: &str) -> Vec<Entry> {
        let es = self.entries(true);
        let hit: Vec<Entry> = es.iter().filter(|e| e.id == key).cloned().collect();
        if !hit.is_empty() {
            return hit;
        }
        let low = key.to_lowercase();
        let exact: Vec<Entry> = es
            .iter()
            .filter(|e| e.name.to_lowercase() == low)
            .cloned()
            .collect();
        if !exact.is_empty() {
            return exact;
        }
        es.into_iter()
            .filter(|e| e.name.to_lowercase().contains(&low))
            .collect()
    }

    /// Filtered, sorted.
    pub fn search(&self, f: &Filter) -> Vec<Entry> {
        let mut v: Vec<Entry> = self
            .entries(false)
            .into_iter()
            .filter(|e| matches(e, f))
            .collect();
        v.sort_by_key(sort_key);
        v
    }

    /// Source and target languages in the catalog.
    pub fn languages(&self) -> (Vec<String>, Vec<String>) {
        let mut s = std::collections::BTreeSet::new();
        let mut t = std::collections::BTreeSet::new();
        for e in self.entries(false) {
            if !e.source_lang.is_empty() {
                s.insert(e.source_lang.clone());
            }
            t.extend(e.target_lang.iter().cloned());
        }
        (s.into_iter().collect(), t.into_iter().collect())
    }
}
