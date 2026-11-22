//! Import / export — the whole setup (`.dictz-setup.zip`), the dictionary
//! list (CSV / JSON / text) and catalogs (`build_catalog`, `check_catalog`).
//! Same archive layout and rules as the Python edition
//! (`dictz/center/transfer.py`): both editions read each other's archives.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::Digest;

use super::adapters::{group_sets, Listed};
use super::install::{safe_target, Installer};
use super::model::{archive_of, format_of, normalize, Entry};
use super::sources::{center_dir, Sources};
use super::store::{dumps, JsonFile};
use super::util::{hex, now_iso, quote};
use super::{langs, licenses, CATALOG_SCHEMA};
use crate::registry::{norm_path, Registry};

/// The parts of a setup archive.
pub const PARTS: &[&str] = &[
    "dictionaries",
    "settings",
    "sources",
    "installed",
    "history",
    "files",
];

fn zerr<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The files that make one dictionary — what it is read from (a StarDict is
/// three or four files) and its media zips: what "delete with its files"
/// removes. Media folders and anything else in the folder stay.
pub fn own_files(path: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = crate::articles::source_files(path)
        .into_iter()
        .filter(|f| f.is_file())
        .collect();
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ifo"))
    {
        let syn = path.with_extension("syn");
        if syn.is_file() {
            out.push(syn);
        }
    }
    let locs = crate::media::MediaResolver::for_dictionary(path).search_locations();
    for loc in &locs[..locs.len().saturating_sub(1)] {
        if loc.is_file() && !out.contains(loc) {
            out.push(loc.clone());
        }
    }
    out
}

fn media_of(path: &Path) -> Vec<PathBuf> {
    let r = crate::media::MediaResolver::for_dictionary(path);
    let locs = r.search_locations();
    let mut out = Vec::new();
    let parent_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_lowercase());
    for loc in &locs[..locs.len().saturating_sub(1)] {
        if loc.is_file() {
            out.push(loc.clone());
        } else if loc.is_dir()
            && loc.file_name().map(|n| n.to_string_lossy().to_lowercase()) != parent_name
        {
            let mut all = Vec::new();
            fn walk(d: &Path, out: &mut Vec<PathBuf>) {
                if let Ok(rd) = fs::read_dir(d) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            walk(&p, out);
                        } else {
                            out.push(p);
                        }
                    }
                }
            }
            walk(loc, &mut all);
            all.sort();
            out.extend(all);
        }
    }
    out
}

fn registry(data_dir: &Path) -> Registry {
    let mut r = Registry::new(Some(data_dir.join("dictionaries.json")));
    r.reload_with(true);
    r
}

fn history_dump(data_dir: &Path) -> Value {
    let db = data_dir.join("dictz_history.db");
    let mut h = Vec::new();
    let mut f = Vec::new();
    if db.is_file() {
        if let Ok(con) =
            rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        {
            if let Ok(mut st) =
                con.prepare("SELECT word, dictionary, looked_up FROM history ORDER BY id")
            {
                let rows = st.query_map([], |r| {
                    Ok(json!({"word": r.get::<_, String>(0)?, "dictionary": r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                              "looked_up": r.get::<_, f64>(2)?}))
                });
                if let Ok(rows) = rows {
                    h.extend(rows.flatten());
                }
            }
            if let Ok(mut st) =
                con.prepare("SELECT word, note, added_at FROM favorites ORDER BY id")
            {
                let rows = st.query_map([], |r| {
                    Ok(json!({"word": r.get::<_, String>(0)?, "note": r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                              "added_at": r.get::<_, f64>(2)?}))
                });
                if let Ok(rows) = rows {
                    f.extend(rows.flatten());
                }
            }
        }
    }
    json!({"history": h, "favorites": f})
}

/// Write a setup archive; its manifest.
pub fn export_setup(zip_path: &Path, data_dir: &Path, parts: &[&str]) -> Result<Value, String> {
    let mut reg = registry(data_dir);
    let rows: Vec<crate::registry::Entry> = reg.rows(false).into_iter().map(|(_, e)| e).collect();
    if let Some(d) = zip_path.parent() {
        let _ = fs::create_dir_all(d);
    }
    let file = fs::File::create(zip_path).map_err(zerr)?;
    let mut z = zip::ZipWriter::new(file);
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let put = |z: &mut zip::ZipWriter<fs::File>, name: &str, data: &[u8]| -> Result<(), String> {
        z.start_file(name, opt).map_err(zerr)?;
        z.write_all(data).map_err(zerr)
    };
    let has = |p: &str| parts.contains(&p);
    if has("dictionaries") {
        let list: Vec<Value> = rows.iter().map(|e| e.to_value()).collect();
        put(
            &mut z,
            "dictionaries.json",
            dumps(&json!({"version": 1, "dictionaries": list})).as_bytes(),
        )?;
    }
    if has("settings") && data_dir.join("settings.json").is_file() {
        put(
            &mut z,
            "settings.json",
            &fs::read(data_dir.join("settings.json")).map_err(zerr)?,
        )?;
    }
    if has("sources") {
        let src = Sources::new(data_dir);
        let all: Vec<Value> = src.all().iter().map(|s| s.to_value()).collect();
        put(
            &mut z,
            "center/sources.json",
            dumps(&json!({"dictz-sources": 1, "default": src.default_id(), "sources": all}))
                .as_bytes(),
        )?;
    }
    let inst = center_dir(data_dir).join("installed.json");
    if has("installed") && inst.is_file() {
        put(
            &mut z,
            "center/installed.json",
            &fs::read(&inst).map_err(zerr)?,
        )?;
    }
    if has("history") {
        put(
            &mut z,
            "history.json",
            dumps(&history_dump(data_dir)).as_bytes(),
        )?;
    }
    let mut items = Vec::new();
    for row in &rows {
        let mut files: Vec<String> = Vec::new();
        if has("files") {
            let p = PathBuf::from(&row.path);
            let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
            let mut all = crate::articles::source_files(&p);
            all.extend(media_of(&p));
            for f in all {
                if !f.is_file() {
                    continue;
                }
                let rel = match f.strip_prefix(&parent) {
                    Ok(r) if f.starts_with(&parent) => r.to_string_lossy().replace('\\', "/"),
                    _ => f
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                };
                let arc = format!("files/{}/{}", row.id, rel);
                if !files.contains(&arc) {
                    put(&mut z, &arc, &fs::read(&f).map_err(zerr)?)?;
                    files.push(arc);
                }
            }
        }
        items.push(json!({"id": row.id, "name": row.name, "path": row.path, "files": files}));
    }
    let manifest = json!({
        "dictz-setup": 1, "created": now_iso(), "app": format!("dictz {} (rust)", crate::VERSION),
        "parts": PARTS.iter().filter(|p| parts.contains(p)).collect::<Vec<_>>(), "dictionaries": items,
    });
    put(&mut z, "dictz-setup.json", dumps(&manifest).as_bytes())?;
    z.finish().map_err(zerr)?;
    Ok(manifest)
}

/// The manifest of a setup archive.
pub fn read_manifest(zip_path: &Path) -> Result<Value, String> {
    let bad = |e: String| format!("not a dictz setup archive: {} ({})", zip_path.display(), e);
    let f = fs::File::open(zip_path).map_err(|e| bad(e.to_string()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| bad(e.to_string()))?;
    let mut text = String::new();
    z.by_name("dictz-setup.json")
        .map_err(|e| bad(e.to_string()))?
        .read_to_string(&mut text)
        .map_err(|e| bad(e.to_string()))?;
    let m: Value = serde_json::from_str(&text).map_err(|e| bad(e.to_string()))?;
    if m.get("dictz-setup").is_none() {
        return Err(format!("not a dictz setup archive: {}", zip_path.display()));
    }
    Ok(m)
}

fn read_member(z: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut f = z.by_name(name).ok()?;
    let mut b = Vec::new();
    f.read_to_end(&mut b).ok()?;
    Some(b)
}

/// What an import did.
#[derive(Debug, Default, Clone)]
pub struct ImportResult {
    /// New list rows: (name, path).
    pub added: Vec<(String, String)>,
    /// Settings came in.
    pub settings: bool,
    /// Sources.
    pub sources: usize,
    /// History rows.
    pub history: usize,
    /// Favourites.
    pub favorites: usize,
    /// Dictionary files unpacked.
    pub files: usize,
    /// Files that are not there.
    pub missing: Vec<String>,
}

/// Bring a setup archive in (`replace` or merge).
pub fn import_setup(
    zip_path: &Path,
    data_dir: &Path,
    replace: bool,
    parts: &[&str],
) -> Result<ImportResult, String> {
    let manifest = read_manifest(zip_path)?;
    let have: Vec<String> = manifest["parts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let want = |p: &str| parts.contains(&p) && have.iter().any(|h| h == p);
    let mut res = ImportResult::default();
    let mut moved: HashMap<String, String> = HashMap::new();
    let mut z = zip::ZipArchive::new(fs::File::open(zip_path).map_err(zerr)?).map_err(zerr)?;
    let names: std::collections::HashSet<String> = z.file_names().map(String::from).collect();
    if want("files") {
        let dest_root = data_dir.join("dictionaries").join("imported");
        for item in manifest["dictionaries"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let id = item["id"].as_str().unwrap_or("").to_string();
            for arc in item["files"].as_array().cloned().unwrap_or_default() {
                let arc = arc.as_str().unwrap_or("").to_string();
                if !names.contains(&arc) {
                    continue;
                }
                let rel = if arc.matches('/').count() >= 2 {
                    arc.splitn(3, '/').nth(2).unwrap_or("").to_string()
                } else {
                    arc.rsplit('/').next().unwrap_or("").to_string()
                };
                let Some(t) = safe_target(&dest_root.join(&id), &rel) else {
                    continue;
                };
                if let Some(d) = t.parent() {
                    let _ = fs::create_dir_all(d);
                }
                if let Some(b) = read_member(&mut z, &arc) {
                    fs::write(&t, b).map_err(zerr)?;
                    res.files += 1;
                }
            }
            let path = item["path"].as_str().unwrap_or("");
            let main = dest_root
                .join(&id)
                .join(Path::new(path).file_name().unwrap_or_default());
            if main.is_file() {
                moved.insert(
                    norm_path(Path::new(path)),
                    main.to_string_lossy().into_owned(),
                );
            }
        }
    }
    if want("dictionaries") && names.contains("dictionaries.json") {
        let data: Value =
            serde_json::from_slice(&read_member(&mut z, "dictionaries.json").unwrap_or_default())
                .unwrap_or(json!({}));
        let mut reg = registry(data_dir);
        if replace {
            let all: Vec<String> = reg.rows(false).into_iter().map(|(_, e)| e.name).collect();
            for n in all {
                reg.remove(&n);
            }
        }
        let mut known: std::collections::HashSet<String> = reg
            .rows(false)
            .into_iter()
            .map(|(_, e)| norm_path(Path::new(&e.path)))
            .collect();
        for r in data["dictionaries"].as_array().cloned().unwrap_or_default() {
            let orig = r["path"].as_str().unwrap_or("").to_string();
            let path = moved
                .get(&norm_path(Path::new(&orig)))
                .cloned()
                .unwrap_or(orig);
            if !known.insert(norm_path(Path::new(&path))) {
                continue;
            }
            let base = r["name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| {
                    Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            let rv = r.clone();
            let name = reg.register(&base, Path::new(&path), |e| {
                let g = |k: &str| rv.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                e.format = g("format");
                e.word_count = rv.get("word_count").and_then(|v| v.as_i64()).unwrap_or(0);
                e.source_lang = g("source_lang");
                e.target_lang = g("target_lang");
                e.description = g("description");
                e.author = g("author");
            });
            if !r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                reg.set_enabled(&name, false);
            }
            if !Path::new(&path).exists() {
                res.missing.push(path.clone());
            }
            res.added.push((name, path));
        }
    }
    if want("settings") && names.contains("settings.json") {
        let data: Value =
            serde_json::from_slice(&read_member(&mut z, "settings.json").unwrap_or_default())
                .unwrap_or(json!({}));
        let mut prefs =
            crate::settings::Preferences::load(&data_dir.join(crate::settings::SETTINGS_FILE));
        prefs.import(&data);
        let _ = prefs.save();
        res.settings = true;
    }
    if want("sources") && names.contains("center/sources.json") {
        let tmp = center_dir(data_dir).join(".import-sources.json");
        let _ = fs::create_dir_all(center_dir(data_dir));
        fs::write(
            &tmp,
            read_member(&mut z, "center/sources.json").unwrap_or_default(),
        )
        .map_err(zerr)?;
        let r = Sources::new(data_dir).import(&tmp, replace);
        let _ = fs::remove_file(&tmp);
        res.sources = r?;
    }
    if want("installed") && names.contains("center/installed.json") {
        let data: Value = serde_json::from_slice(
            &read_member(&mut z, "center/installed.json").unwrap_or_default(),
        )
        .unwrap_or(json!({}));
        let mut recs = data["installed"].as_object().cloned().unwrap_or_default();
        for rec in recs.values_mut() {
            let files: Vec<String> = rec["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f.as_str())
                        .map(|f| {
                            moved
                                .get(&norm_path(Path::new(f)))
                                .cloned()
                                .unwrap_or(f.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(first) = files.first() {
                let folder = Path::new(first)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                rec["folder"] = json!(folder);
            }
            rec["files"] = json!(files);
        }
        let f = JsonFile::new(
            center_dir(data_dir).join("installed.json"),
            || json!({"version": 1, "installed": {}}),
        );
        let _ = f.update(|d| {
            if !d.is_object() {
                *d = json!({"version": 1, "installed": {}});
            }
            let o = d.as_object_mut().expect("object");
            if replace {
                o.insert("installed".into(), json!({}));
            }
            let m = o.entry("installed").or_insert(json!({}));
            if let Some(m) = m.as_object_mut() {
                for (k, v) in &recs {
                    m.entry(k.clone()).or_insert(v.clone());
                }
            }
        });
    }
    if want("history") && names.contains("history.json") {
        let h: Value =
            serde_json::from_slice(&read_member(&mut z, "history.json").unwrap_or_default())
                .unwrap_or(json!({}));
        let (a, b) = history_load(data_dir, &h, replace).map_err(zerr)?;
        res.history = a;
        res.favorites = b;
    }
    Ok(res)
}

fn history_load(
    data_dir: &Path,
    h: &Value,
    replace: bool,
) -> Result<(usize, usize), crate::DictzError> {
    let store = crate::history::HistoryStore::open(&data_dir.join("dictz_history.db"))?;
    drop(store); // the tables exist now
    let con = rusqlite::Connection::open(data_dir.join("dictz_history.db"))
        .map_err(crate::DictzError::from)?;
    let tx = con
        .unchecked_transaction()
        .map_err(crate::DictzError::from)?;
    if replace {
        tx.execute("DELETE FROM history", [])
            .map_err(crate::DictzError::from)?;
        tx.execute("DELETE FROM favorites", [])
            .map_err(crate::DictzError::from)?;
    }
    let mut have: std::collections::HashSet<(String, u64)> = std::collections::HashSet::new();
    {
        let mut st = tx
            .prepare("SELECT word, looked_up FROM history")
            .map_err(crate::DictzError::from)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
            .map_err(crate::DictzError::from)?;
        for (w, t) in rows.flatten() {
            have.insert((w, t.to_bits()));
        }
    }
    let (mut nh, mut nf) = (0, 0);
    for r in h["history"].as_array().cloned().unwrap_or_default() {
        let w = r["word"].as_str().unwrap_or("").to_string();
        let t = r["looked_up"].as_f64().unwrap_or(0.0);
        if !w.is_empty() && have.insert((w.clone(), t.to_bits())) {
            tx.execute(
                "INSERT INTO history (word, dictionary, looked_up) VALUES (?1, ?2, ?3)",
                rusqlite::params![w, r["dictionary"].as_str().unwrap_or(""), t],
            )
            .map_err(crate::DictzError::from)?;
            nh += 1;
        }
    }
    for r in h["favorites"].as_array().cloned().unwrap_or_default() {
        let w = r["word"].as_str().unwrap_or("");
        if !w.is_empty() {
            nf += tx
                .execute(
                    "INSERT OR IGNORE INTO favorites (word, note, added_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![
                        w,
                        r["note"].as_str().unwrap_or(""),
                        r["added_at"].as_f64().unwrap_or(0.0)
                    ],
                )
                .map_err(crate::DictzError::from)?;
        }
    }
    tx.commit().map_err(crate::DictzError::from)?;
    Ok((nh, nf))
}

/// Columns of the exported list.
pub const LIST_COLUMNS: &[&str] = &[
    "name",
    "format",
    "source_lang",
    "target_lang",
    "words",
    "enabled",
    "license",
    "source",
    "homepage",
    "path",
];

/// The list with licences (from the install records).
pub fn list_rows(data_dir: &Path) -> Vec<Map<String, Value>> {
    let mut reg = registry(data_dir);
    let inst = Installer::new(data_dir, super::net::Http::new(30.0, ""), "", true, false);
    let mut out = Vec::new();
    for (_, r) in reg.rows(false) {
        let rec = inst
            .record_for_path(Path::new(&r.path))
            .unwrap_or(json!({}));
        let g = |k: &str| {
            rec.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let tl = match rec.get("target_lang") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        let mut m = Map::new();
        m.insert("name".into(), json!(r.name));
        m.insert("format".into(), json!(r.format));
        m.insert(
            "source_lang".into(),
            json!(if r.source_lang.is_empty() {
                g("source_lang")
            } else {
                r.source_lang.clone()
            }),
        );
        m.insert(
            "target_lang".into(),
            json!(if r.target_lang.is_empty() {
                tl
            } else {
                r.target_lang.clone()
            }),
        );
        m.insert("words".into(), json!(r.word_count));
        m.insert("enabled".into(), json!(r.enabled));
        let lic = g("license");
        m.insert(
            "license".into(),
            json!(if lic.is_empty() {
                "unknown".to_string()
            } else {
                lic
            }),
        );
        m.insert("source".into(), json!(g("source")));
        m.insert("homepage".into(), json!(g("homepage")));
        m.insert("path".into(), json!(r.path));
        out.push(m);
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The list as `csv`, `json` or `txt` (by extension when `fmt` is empty).
pub fn export_list(path: &Path, data_dir: &Path, fmt: &str) -> Result<usize, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let fmt = if fmt.is_empty() {
        if ext.is_empty() {
            "csv".to_string()
        } else {
            ext
        }
    } else {
        fmt.to_lowercase()
    };
    let rows = list_rows(data_dir);
    let text = match fmt.as_str() {
        "json" => dumps(&json!({"dictz-list": 1, "dictionaries": rows})),
        "csv" => {
            let mut t = LIST_COLUMNS.join(",") + "\n";
            for r in &rows {
                let cells: Vec<String> = LIST_COLUMNS
                    .iter()
                    .map(|k| match (*k, &r[*k]) {
                        ("enabled", Value::Bool(b)) => {
                            if *b {
                                "yes".into()
                            } else {
                                "no".into()
                            }
                        }
                        (_, Value::String(s)) => csv_field(s),
                        (_, v) => v.to_string(),
                    })
                    .collect();
                t.push_str(&cells.join(","));
                t.push('\n');
            }
            t
        }
        _ => {
            let mut t = String::new();
            for r in &rows {
                let sl = r["source_lang"].as_str().unwrap_or("");
                let pair = if sl.is_empty() {
                    String::new()
                } else {
                    format!("{} → {}", sl, r["target_lang"].as_str().unwrap_or(""))
                };
                t.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    r["name"].as_str().unwrap_or(""),
                    r["format"].as_str().unwrap_or(""),
                    pair,
                    r["words"],
                    licenses::get(r["license"].as_str().unwrap_or("")).name
                ));
            }
            t
        }
    };
    if let Some(d) = path.parent() {
        let _ = fs::create_dir_all(d);
    }
    fs::write(path, text).map_err(zerr)?;
    Ok(rows.len())
}

fn sha256_file(p: &Path) -> String {
    let mut h = sha2::Sha256::new();
    if let Ok(mut f) = fs::File::open(p) {
        let mut buf = vec![0u8; 1 << 20];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
    }
    hex(&h.finalize())
}

/// A `dictz-catalog.json` for every dictionary (and archive) in `folder`.
pub fn build_catalog(
    folder: &Path,
    base_url: &str,
    name: &str,
    license_id: &str,
    read_dicts: bool,
) -> Value {
    let folder = fs::canonicalize(folder).unwrap_or(folder.to_path_buf());
    let base = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{}/", base_url)
    };
    let mut found: Vec<(Vec<String>, PathBuf)> = Vec::new();
    fn walk(d: &Path, root: &Path, out: &mut Vec<(Vec<String>, PathBuf)>) {
        if let Ok(rd) = fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    let rel: Vec<String> = p
                        .strip_prefix(root)
                        .map(|r| {
                            r.components()
                                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push((rel, p));
                }
            }
        }
    }
    walk(&folder, &folder, &mut found);
    found.sort();
    let listing: Vec<Listed> = found
        .iter()
        .filter(|(rel, p)| {
            let n = rel.last().cloned().unwrap_or_default();
            p.is_file() && !n.ends_with(".json") && !n.starts_with('.')
        })
        .map(|(rel, p)| {
            let r = rel.join("/");
            (
                r.clone(),
                format!("{}{}", base, quote(&r)),
                fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                String::new(),
            )
        })
        .collect();
    let src = super::sources::Source {
        license: license_id.to_string(),
        ..Default::default()
    };
    let groups = group_sets(&src, &listing);
    let mut out = Vec::new();
    for mut e in groups {
        let files: Vec<Value> = e["files"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|f| {
                let url = f["url"].as_str().unwrap_or("");
                let rel = listing.iter().find(|l| l.1 == url).map(|l| l.0.clone()).unwrap_or_default();
                let p = folder.join(&rel);
                json!({"name": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                       "url": url, "size": fs::metadata(&p).map(|m| m.len()).unwrap_or(0), "sha256": sha256_file(&p)})
            })
            .collect();
        let first_url = files
            .first()
            .and_then(|f| f["url"].as_str())
            .unwrap_or("")
            .to_string();
        let first = folder.join(
            listing
                .iter()
                .find(|l| l.1 == first_url)
                .map(|l| l.0.clone())
                .unwrap_or_default(),
        );
        let o = e.as_object_mut().expect("object");
        o.insert("files".into(), Value::Array(files));
        let id = o["id"].as_str().unwrap_or("").replace('/', "-");
        o.insert("id".into(), json!(id));
        let fname = first
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if read_dicts
            && archive_of(&fname).is_empty()
            && ["stardict", "dsl", "babylon"].contains(&format_of(&fname).as_str())
        {
            if let Ok(mut d) = crate::query::QueryManager::open_file(&first) {
                let m = d.metadata().clone();
                let words = d.words();
                let sample: Vec<String> = words.sorted().iter().take(5).cloned().collect();
                if !m.name.is_empty() {
                    o.insert("name".into(), json!(m.name));
                }
                o.insert("description".into(), json!(m.description));
                o.insert("author".into(), json!(m.author));
                o.insert("headwords".into(), json!(words.len()));
                o.insert("sample".into(), json!(sample.join(", ")));
                if !m.source_language.is_empty() {
                    o.insert("source_lang".into(), json!(langs::norm(&m.source_language)));
                }
                if !m.target_language.is_empty() {
                    o.insert(
                        "target_lang".into(),
                        json!([langs::norm(&m.target_language)]),
                    );
                }
                d.close();
            }
        }
        let side = PathBuf::from(format!("{}.json", first.display()));
        if side.is_file() {
            if let Ok(Value::Object(extra)) =
                serde_json::from_str::<Value>(&fs::read_to_string(&side).unwrap_or_default())
            {
                for (k, v) in extra {
                    if super::model::ENTRY_KEYS.contains(&k.as_str()) && k != "files" && k != "id" {
                        o.insert(k, v);
                    }
                }
            }
        }
        let mut n = normalize(&e, "", license_id).to_value();
        if let Some(m) = n.as_object_mut() {
            m.remove("source");
        }
        out.push(n);
    }
    json!({"catalog": {"schema": CATALOG_SCHEMA, "name": if name.is_empty() {
                folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else { name.to_string() }, "updated": now_iso(), "base_url": base,
            "license": if license_id.is_empty() { String::new() } else { licenses::norm(license_id) }},
           "dictionaries": out})
}

/// (entries, problems) of a catalog; problems start with `error:` or `warning:`.
pub fn check_catalog(data: &Value) -> (usize, Vec<String>) {
    let mut problems = Vec::new();
    let rows = match data {
        Value::Array(a) => a.clone(),
        Value::Object(o) => match o.get("dictionaries") {
            Some(Value::Array(a)) => a.clone(),
            _ => return (0, vec!["error: no \"dictionaries\" list".into()]),
        },
        _ => return (0, vec!["error: no \"dictionaries\" list".into()]),
    };
    if let Some(schema) = data.get("catalog").and_then(|c| c.get("schema")) {
        if schema.as_i64() != Some(CATALOG_SCHEMA) {
            problems.push(format!(
                "warning: catalog schema {} (this dictz reads {})",
                schema, CATALOG_SCHEMA
            ));
        }
    }
    let mut ids = std::collections::HashSet::new();
    for (i, r) in rows.iter().enumerate() {
        if !r.is_object() {
            problems.push(format!("error: #{}: not an object", i + 1));
            continue;
        }
        let e: Entry = normalize(r, "", "");
        let label = if e.name.is_empty() {
            e.id.clone()
        } else {
            e.name.clone()
        };
        let w = format!("#{} {}", i + 1, label);
        if e.id.is_empty() {
            problems.push(format!("error: {}: no id", w));
        } else if !ids.insert(e.id.clone()) {
            problems.push(format!("error: {}: id {} used twice", w, e.id));
        }
        if e.files.is_empty() && e.resolve.is_empty() {
            problems.push(format!("error: {}: no files", w));
        }
        for f in &e.files {
            if f.url.is_empty() {
                problems.push(format!("error: {}: a file without url", w));
            }
            if f.sha256.is_empty() && f.sha512.is_empty() && f.md5.is_empty() {
                problems.push(format!("warning: {}: {}: no checksum", w, f.name));
            }
            if f.size == 0 {
                problems.push(format!("warning: {}: {}: no size", w, f.name));
            }
        }
        if licenses::get(&e.license).status == "unknown" {
            problems.push(format!("warning: {}: licence unknown", w));
        }
        if e.source_lang.is_empty() {
            problems.push(format!("warning: {}: no source language", w));
        }
    }
    (rows.len(), problems)
}
