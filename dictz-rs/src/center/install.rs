//! Downloading, checking, unpacking and installing catalog entries — same
//! steps, folders and records as the Python edition
//! (`dictz/center/install.py`).

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::{json, Value};
use sha2::Digest;

use super::adapters;
use super::model::{archive_of, norm_file, Entry, FileRef};
use super::net::{auth_headers, DownloadError, Http};
use super::sources::{center_dir, slug, Sources};
use super::store::JsonFile;
use super::util::{base64, hex, now_iso};

/// History entries kept.
pub const HISTORY_LIMIT: usize = 500;

/// An install failed (or was stopped: `cancelled`).
#[derive(Debug, Clone)]
pub struct InstallError {
    /// What went wrong.
    pub message: String,
    /// The user stopped it.
    pub cancelled: bool,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn err(m: impl Into<String>) -> InstallError {
    InstallError {
        message: m.into(),
        cancelled: false,
    }
}

fn fnv(text: &str) -> String {
    format!("{:016x}", crate::registry::fnv1a64(text.as_bytes()))
}

/// The partial download's file name for an address.
pub fn part_name(url: &str) -> String {
    format!("{}.part", fnv(url))
}

/// SHA-256, SHA-512, MD5 of a file.
pub fn file_hashes(path: &Path) -> std::io::Result<(String, String, String)> {
    let mut f = fs::File::open(path)?;
    let mut a = sha2::Sha256::new();
    let mut b = sha2::Sha512::new();
    let mut c = md5::Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        a.update(&buf[..n]);
        b.update(&buf[..n]);
        c.update(&buf[..n]);
    }
    Ok((hex(&a.finalize()), hex(&b.finalize()), hex(&c.finalize())))
}

/// Size and checksums the catalog gives; an error on a mismatch.
pub fn verify(path: &Path, f: &FileRef) -> Result<(), InstallError> {
    let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if f.size > 0 && len != f.size {
        return Err(err(format!("{}: size {} ≠ {}", f.name, len, f.size)));
    }
    if f.sha256.is_empty() && f.sha512.is_empty() && f.md5.is_empty() {
        return Ok(());
    }
    let (s256, s512, m5) = file_hashes(path).map_err(|e| err(e.to_string()))?;
    for (k, want, got) in [
        ("SHA256", &f.sha256, s256),
        ("SHA512", &f.sha512, s512),
        ("MD5", &f.md5, m5),
    ] {
        if !want.is_empty() && want.to_lowercase() != got {
            return Err(err(format!("{}: {} does not match", f.name, k)));
        }
    }
    Ok(())
}

/// Where an archive member goes, or `None` when it would leave `root`.
pub fn safe_target(root: &Path, name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') {
        return None;
    }
    let p = Path::new(&name);
    let mut out = root.to_path_buf();
    for c in p.components() {
        match c {
            Component::Normal(x) => {
                if x.to_string_lossy().contains(':') {
                    return None;
                }
                out.push(x);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out == root {
        return None;
    }
    Some(out)
}

fn copy_out(mut src: impl Read, target: &Path) -> std::io::Result<()> {
    if let Some(d) = target.parent() {
        fs::create_dir_all(d)?;
    }
    let mut dst = fs::File::create(target)?;
    std::io::copy(&mut src, &mut dst)?;
    dst.flush()
}

fn untar(reader: impl Read, dest: &Path, name: &str) -> Result<Vec<PathBuf>, InstallError> {
    let mut out = Vec::new();
    let mut ar = tar::Archive::new(reader);
    let entries = ar
        .entries()
        .map_err(|e| err(format!("{}: cannot unpack ({})", name, e)))?;
    for e in entries {
        let mut e = e.map_err(|x| err(format!("{}: cannot unpack ({})", name, x)))?;
        if !e.header().entry_type().is_file() {
            continue; // folders are made as needed; links and devices are skipped
        }
        let path = e
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(t) = safe_target(dest, &path) else {
            continue;
        };
        copy_out(&mut e, &t).map_err(|x| err(format!("{}: cannot unpack ({})", name, x)))?;
        out.push(t);
    }
    Ok(out)
}

/// Extract `archive` into `dest`; returns the files written. Members that
/// would land outside `dest` and links are skipped.
pub fn unpack(archive: &Path, kind: &str, dest: &Path) -> Result<Vec<PathBuf>, InstallError> {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    fs::create_dir_all(dest).map_err(|e| err(e.to_string()))?;
    let open = || fs::File::open(archive).map_err(|e| err(format!("{}: {}", name, e)));
    match kind {
        "zip" => {
            let mut z = zip::ZipArchive::new(open()?)
                .map_err(|e| err(format!("{}: broken zip ({})", name, e)))?;
            let mut out = Vec::new();
            for i in 0..z.len() {
                let mut f = z
                    .by_index(i)
                    .map_err(|e| err(format!("{}: broken zip ({})", name, e)))?;
                if f.is_dir() {
                    continue;
                }
                let Some(t) = safe_target(dest, f.name()) else {
                    continue;
                };
                copy_out(&mut f, &t).map_err(|e| err(e.to_string()))?;
                out.push(t);
            }
            Ok(out)
        }
        "tar" => untar(std::io::BufReader::new(open()?), dest, &name),
        "tar.gz" => untar(
            flate2::read::GzDecoder::new(std::io::BufReader::new(open()?)),
            dest,
            &name,
        ),
        "tar.bz2" => untar(
            bzip2::read::BzDecoder::new(std::io::BufReader::new(open()?)),
            dest,
            &name,
        ),
        "tar.xz" => {
            // lzma-rs writes to a stream: through a temporary file
            let tmp = dest.join(".unpack.tar");
            {
                let mut input = std::io::BufReader::new(open()?);
                let mut output = std::io::BufWriter::new(
                    fs::File::create(&tmp).map_err(|e| err(e.to_string()))?,
                );
                lzma_rs::xz_decompress(&mut input, &mut output)
                    .map_err(|e| err(format!("{}: cannot unpack ({:?})", name, e)))?;
                output.flush().map_err(|e| err(e.to_string()))?;
            }
            let res = untar(
                std::io::BufReader::new(fs::File::open(&tmp).map_err(|e| err(e.to_string()))?),
                dest,
                &name,
            );
            let _ = fs::remove_file(&tmp);
            res
        }
        k => Err(err(format!(
            "{}: {} archives are not supported — unpack it yourself and add the folder",
            name, k
        ))),
    }
}

/// Dictionary files dictz opens inside `folder` (sorted).
pub fn dictionary_files(folder: &Path) -> Vec<PathBuf> {
    let mut v = crate::query::scan_folder(folder, true);
    v.sort();
    v
}

fn installed_default() -> Value {
    json!({"version": 1, "installed": {}})
}

fn history_default() -> Value {
    json!({"version": 1, "downloads": []})
}

/// `progress(stage, done, total)`: `download` (bytes), `check`, `unpack`.
pub type Progress<'a> = &'a mut dyn FnMut(&str, u64, u64);

/// Installs and their records.
pub struct Installer {
    data_dir: PathBuf,
    /// The sources (for sign-in).
    pub sources: Sources,
    /// HTTP.
    pub http: Http,
    /// `DATA/dictionaries` or the setting's folder.
    pub install_root: PathBuf,
    /// Check checksums.
    pub verify: bool,
    /// Keep downloaded archives.
    pub keep_archives: bool,
    installed_file: JsonFile,
    history_file: JsonFile,
    /// Partial downloads.
    pub part_dir: PathBuf,
}

impl Installer {
    /// For `data_dir`; `install_root` '' = `DATA/dictionaries`.
    pub fn new(
        data_dir: &Path,
        http: Http,
        install_root: &str,
        verify: bool,
        keep_archives: bool,
    ) -> Self {
        let c = center_dir(data_dir);
        let root = if install_root.trim().is_empty() {
            data_dir.join("dictionaries")
        } else {
            super::util::expand_user(install_root)
        };
        Self {
            data_dir: data_dir.to_path_buf(),
            sources: Sources::new(data_dir),
            http,
            install_root: root,
            verify,
            keep_archives,
            installed_file: JsonFile::new(c.join("installed.json"), installed_default),
            history_file: JsonFile::new(c.join("downloads.json"), history_default),
            part_dir: c.join("downloads"),
        }
    }

    /// The data folder.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// `installed.json` (id → record), sorted by id.
    pub fn installed(&self) -> std::collections::BTreeMap<String, Value> {
        self.installed_file
            .read()
            .get("installed")
            .and_then(|v| v.as_object())
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// The download history (newest first).
    pub fn history(&self) -> Vec<Value> {
        self.history_file
            .read()
            .get("downloads")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    }

    /// Add to the history.
    pub fn log(&self, item: Value) {
        let _ = self.history_file.update(|d| {
            if !d.is_object() {
                *d = history_default();
            }
            let o = d.as_object_mut().expect("object");
            let rows = o.entry("downloads").or_insert(json!([]));
            if let Some(a) = rows.as_array_mut() {
                a.insert(0, item);
                a.truncate(HISTORY_LIMIT);
            }
        });
    }

    /// Forget the history.
    pub fn clear_history(&self) {
        let _ = self.history_file.write(&history_default());
    }

    /// Where an entry is installed.
    pub fn folder_for(&self, e: &Entry) -> PathBuf {
        let tail = e.id.split_once(':').map(|x| x.1).unwrap_or(&e.id);
        let src = if e.source.is_empty() {
            "link"
        } else {
            &e.source
        };
        self.install_root
            .join(slug(src))
            .join(format!("{}-{}", slug(tail), &fnv(&e.id)[..6]))
    }

    /// The files to fetch (an archive.org item's are looked up now).
    pub fn files_of(&self, e: &Entry) -> Result<Vec<FileRef>, InstallError> {
        if !e.files.is_empty() {
            return Ok(e.files.clone());
        }
        if let Some(id) = e.resolve.strip_prefix("archive-org:") {
            let v = adapters::resolve_archive_org(id, &self.http).map_err(|x| err(x.0))?;
            return Ok(v.iter().map(norm_file).collect());
        }
        Ok(vec![])
    }

    fn headers(&self, e: &Entry) -> Vec<(String, String)> {
        let Some(src) = (if e.source.is_empty() {
            None
        } else {
            self.sources.get(&e.source)
        }) else {
            return vec![];
        };
        let (pw, tok) = self.sources.secret(&src.id);
        if src.kind == "nextcloud-share" {
            if let Ok((_root, token)) = adapters::nextcloud_endpoint(&src.url) {
                return vec![(
                    "Authorization".into(),
                    format!("Basic {}", base64(format!("{}:{}", token, pw).as_bytes())),
                )];
            }
        }
        auth_headers(&src, &pw, &tok)
    }

    fn fetch(
        &self,
        f: &FileRef,
        headers: &[(String, String)],
        progress: &mut dyn FnMut(u64, u64),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PathBuf, InstallError> {
        if let Some(p) = crate::center::net::local_path(&f.url) {
            if !p.is_file() {
                return Err(err(format!("no such file: {}", p.display())));
            }
            return Ok(p);
        }
        let part = self.part_dir.join(part_name(&f.url));
        match self
            .http
            .download(&f.url, &part, headers, progress, cancelled)
        {
            Ok(_) => Ok(part),
            Err(DownloadError::Cancelled) => Err(InstallError {
                message: "cancelled".into(),
                cancelled: true,
            }),
            Err(DownloadError::Net(e)) => Err(err(e.message)),
        }
    }

    /// Download and install `e`; its `installed.json` record (`files` = the
    /// dictionary files to add to the list).
    pub fn install(
        &self,
        e: &Entry,
        progress: Progress,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Value, InstallError> {
        let started = now_iso();
        let files = self.files_of(e)?;
        if files.is_empty() {
            return Err(err(format!(
                "{}: nothing to download here — open its page",
                e.name
            )));
        }
        let headers = self.headers(e);
        let folder = self.folder_for(e);
        let staging = PathBuf::from(format!("{}.new", folder.display()));
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).map_err(|x| err(x.to_string()))?;
        let total: u64 = files.iter().map(|f| f.size).sum();
        let mut done_before = 0u64;
        let mut hashes = serde_json::Map::new();
        let res: Result<Vec<PathBuf>, InstallError> = (|| {
            for f in &files {
                let base = done_before;
                let path = self.fetch(
                    f,
                    &headers,
                    &mut |d, t| progress("download", base + d, total.max(base + t)),
                    cancelled,
                )?;
                progress("check", 0, 0);
                if self.verify {
                    if let Err(x) = verify(&path, f) {
                        if path.parent() == Some(self.part_dir.as_path()) {
                            let _ = fs::remove_file(&path); // a broken download is not resumed
                        }
                        return Err(x);
                    }
                }
                let (s256, _, _) = file_hashes(&path).map_err(|x| err(x.to_string()))?;
                hashes.insert(f.name.clone(), json!(s256));
                done_before += fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let kind = if f.archive.is_empty() {
                    archive_of(&f.name)
                } else {
                    f.archive.clone()
                };
                progress("unpack", 0, 0);
                let safe: String = f
                    .name
                    .chars()
                    .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
                    .collect();
                let safe = if safe.is_empty() {
                    "download".to_string()
                } else {
                    safe
                };
                let from_part = path.parent() == Some(self.part_dir.as_path());
                if !kind.is_empty() {
                    unpack(&path, &kind, &staging)?;
                    if self.keep_archives {
                        let _ = fs::copy(&path, staging.join(&safe));
                    }
                } else if from_part {
                    fs::rename(&path, staging.join(&safe))
                        .or_else(|_| fs::copy(&path, staging.join(&safe)).map(|_| ()))
                        .map_err(|x| err(x.to_string()))?;
                } else {
                    fs::copy(&path, staging.join(&safe)).map_err(|x| err(x.to_string()))?;
                }
                if from_part {
                    let _ = fs::remove_file(&path);
                }
            }
            let found = dictionary_files(&staging);
            if found.is_empty() && e.supported != Some(false) {
                return Err(err(format!(
                    "{}: no dictionary dictz can open was found in the download",
                    e.name
                )));
            }
            if folder.exists() {
                fs::remove_dir_all(&folder).map_err(|x| err(x.to_string()))?;
            }
            fs::rename(&staging, &folder).map_err(|x| err(x.to_string()))?;
            Ok(found)
        })();
        let found = match res {
            Ok(f) => f,
            Err(x) => {
                let _ = fs::remove_dir_all(&staging);
                if !x.cancelled {
                    self.log(json!({"id": e.id, "name": e.name, "url": files.first().map(|f| f.url.clone()).unwrap_or_default(),
                                    "status": "failed", "error": x.message, "started": started, "finished": now_iso(),
                                    "license": e.license}));
                }
                return Err(x);
            }
        };
        let paths: Vec<String> = found
            .iter()
            .map(|p| {
                folder
                    .join(p.strip_prefix(&staging).unwrap_or(p))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let record = json!({
            "id": e.id, "name": e.name, "source": e.source, "version": e.version, "date": e.date,
            "installed": now_iso(), "folder": folder.to_string_lossy(), "files": paths, "license": e.license,
            "license_url": e.license_url, "homepage": e.homepage, "page": e.page,
            "urls": files.iter().map(|f| f.url.clone()).collect::<Vec<_>>(), "sha256": hashes,
            "size": done_before, "format": e.format, "source_lang": e.source_lang, "target_lang": e.target_lang,
            "supported": !found.is_empty(),
        });
        let rec2 = record.clone();
        let id = e.id.clone();
        let _ = self.installed_file.update(move |d| {
            if !d.is_object() {
                *d = installed_default();
            }
            let o = d.as_object_mut().expect("object");
            let inst = o.entry("installed").or_insert(json!({}));
            if let Some(m) = inst.as_object_mut() {
                m.insert(id, rec2);
            }
        });
        self.log(json!({"id": e.id, "name": e.name, "url": files[0].url, "status": "done", "error": "",
                        "started": started, "finished": record["installed"], "license": e.license, "size": done_before}));
        Ok(record)
    }

    /// Forget an installed entry (and delete its folder); its record.
    pub fn uninstall(&self, id: &str, delete_files: bool) -> Option<Value> {
        let rec = self.installed().get(id).cloned()?;
        let id2 = id.to_string();
        let _ = self.installed_file.update(move |d| {
            if let Some(m) = d.get_mut("installed").and_then(|v| v.as_object_mut()) {
                m.remove(&id2);
            }
        });
        if delete_files {
            if let Some(f) = rec.get("folder").and_then(|v| v.as_str()) {
                let folder = PathBuf::from(f);
                if folder.starts_with(&self.install_root) && folder != self.install_root {
                    let _ = fs::remove_dir_all(&folder); // only what dictz put there
                }
            }
        }
        Some(rec)
    }

    /// The installed entry a dictionary file belongs to.
    pub fn record_for_path(&self, path: &Path) -> Option<Value> {
        let p = crate::registry::norm_path(path);
        self.installed().into_values().find(|r| {
            r.get("files").and_then(|v| v.as_array()).is_some_and(|a| {
                a.iter().any(|f| {
                    f.as_str()
                        .is_some_and(|s| crate::registry::norm_path(Path::new(s)) == p)
                })
            })
        })
    }

    /// (record, catalog entry) where the catalog has something newer.
    pub fn updates(&self, entries: &[Entry]) -> Vec<(Value, Entry)> {
        let mut out = Vec::new();
        for (id, rec) in self.installed() {
            if let Some(e) = entries.iter().find(|e| e.id == id) {
                let rv = rec.get("version").and_then(|v| v.as_str()).unwrap_or("");
                let rd = rec.get("date").and_then(|v| v.as_str()).unwrap_or("");
                if (e.version.as_str(), e.date.as_str()) != (rv, rd)
                    && (!e.version.is_empty() || !e.date.is_empty())
                {
                    out.push((rec.clone(), e.clone()));
                }
            }
        }
        out
    }
}

/// A download in the window's queue.
#[derive(Debug, Clone)]
pub struct Task {
    /// Its number.
    pub id: usize,
    /// What.
    pub entry: Entry,
    /// queued | running | paused | done | failed | cancelled
    pub state: String,
    /// download | check | unpack
    pub stage: String,
    /// Bytes done.
    pub done: u64,
    /// Bytes in all.
    pub total: u64,
    /// Bytes per second.
    pub speed: f64,
    /// The error.
    pub error: String,
    stop: Arc<AtomicU8>, // 0 go, 1 pause, 2 cancel
}

/// What the queue reports.
pub enum QueueEvent {
    /// A task changed.
    Changed(Task),
    /// Installed: (record, entry).
    Installed(Value, Entry),
}

/// Downloads for the window: `parallel` at a time, pause / resume / cancel / retry.
pub struct DownloadQueue {
    installer: Arc<Installer>,
    /// Tasks.
    pub tasks: Arc<Mutex<Vec<Task>>>,
    parallel: usize,
    running: Arc<Mutex<usize>>,
    sink: Arc<dyn Fn(QueueEvent) + Send + Sync>,
}

impl DownloadQueue {
    /// `sink` is called from worker threads.
    pub fn new(
        installer: Arc<Installer>,
        parallel: usize,
        sink: Arc<dyn Fn(QueueEvent) + Send + Sync>,
    ) -> Self {
        Self {
            installer,
            tasks: Arc::new(Mutex::new(Vec::new())),
            parallel: parallel.max(1),
            running: Arc::new(Mutex::new(0)),
            sink,
        }
    }

    /// The installer.
    pub fn installer(&self) -> &Arc<Installer> {
        &self.installer
    }

    /// A copy of the tasks.
    pub fn snapshot(&self) -> Vec<Task> {
        self.tasks.lock().map(|t| t.clone()).unwrap_or_default()
    }

    /// Queue an entry (once).
    pub fn add(&self, entry: Entry) {
        {
            let mut ts = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if ts.iter().any(|t| {
                t.entry.id == entry.id
                    && ["queued", "running", "paused"].contains(&t.state.as_str())
            }) {
                return;
            }
            let id = ts.len() + 1;
            let total = entry.size;
            ts.push(Task {
                id,
                entry,
                state: "queued".into(),
                stage: String::new(),
                done: 0,
                total,
                speed: 0.0,
                error: String::new(),
                stop: Arc::new(AtomicU8::new(0)),
            });
            (self.sink)(QueueEvent::Changed(ts[id - 1].clone()));
        }
        self.pump();
    }

    fn pump(&self) {
        loop {
            let task = {
                let mut run = self.running.lock().unwrap_or_else(|e| e.into_inner());
                if *run >= self.parallel {
                    return;
                }
                let mut ts = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
                let Some(t) = ts.iter_mut().find(|t| t.state == "queued") else {
                    return;
                };
                t.state = "running".into();
                t.stop.store(0, Ordering::SeqCst);
                *run += 1;
                t.clone()
            };
            let inst = Arc::clone(&self.installer);
            let tasks = Arc::clone(&self.tasks);
            let running = Arc::clone(&self.running);
            let sink = Arc::clone(&self.sink);
            let me = Self {
                installer: Arc::clone(&inst),
                tasks: Arc::clone(&tasks),
                parallel: self.parallel,
                running: Arc::clone(&running),
                sink: Arc::clone(&sink),
            };
            std::thread::spawn(move || {
                let start = Instant::now();
                let mut last = Instant::now() - std::time::Duration::from_secs(1);
                let stop = Arc::clone(&task.stop);
                let id = task.id;
                let update = |f: &mut dyn FnMut(&mut Task)| {
                    let mut ts = tasks.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(t) = ts.iter_mut().find(|t| t.id == id) {
                        f(t);
                        sink(QueueEvent::Changed(t.clone()));
                    }
                };
                let res = inst.install(
                    &task.entry,
                    &mut |stage, done, total| {
                        if stage != "download" || last.elapsed().as_millis() > 200 {
                            last = Instant::now();
                            let el = start.elapsed().as_secs_f64();
                            update(&mut |t| {
                                t.stage = stage.to_string();
                                if stage == "download" {
                                    t.done = done;
                                    t.total = t.total.max(total);
                                    t.speed = if el > 0.0 { done as f64 / el } else { 0.0 };
                                }
                            });
                        }
                    },
                    &|| stop.load(Ordering::SeqCst) != 0,
                );
                match res {
                    Ok(rec) => {
                        update(&mut |t| {
                            t.state = "done".into();
                            t.stage.clear();
                        });
                        sink(QueueEvent::Installed(rec, task.entry.clone()));
                    }
                    Err(x) if x.cancelled => {
                        let cancel = stop.load(Ordering::SeqCst) == 2;
                        if cancel {
                            for f in &task.entry.files {
                                let _ = fs::remove_file(inst.part_dir.join(part_name(&f.url)));
                            }
                        }
                        update(&mut |t| {
                            t.state = if cancel {
                                "cancelled".into()
                            } else {
                                "paused".into()
                            }
                        });
                    }
                    Err(x) => update(&mut |t| {
                        t.state = "failed".into();
                        t.error = x.message.clone();
                    }),
                }
                *running.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
                me.pump();
            });
        }
    }

    fn with(&self, id: usize, f: impl FnOnce(&mut Task)) {
        let mut ts = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = ts.iter_mut().find(|t| t.id == id) {
            f(t);
            (self.sink)(QueueEvent::Changed(t.clone()));
        }
    }

    /// Pause (a running one keeps its partial file).
    pub fn pause(&self, id: usize) {
        self.with(id, |t| match t.state.as_str() {
            "running" => t.stop.store(1, Ordering::SeqCst),
            "queued" => t.state = "paused".into(),
            _ => {}
        });
    }

    /// Resume / retry.
    pub fn resume(&self, id: usize) {
        self.with(id, |t| {
            if ["paused", "failed", "cancelled"].contains(&t.state.as_str()) {
                t.state = "queued".into();
                t.error.clear();
            }
        });
        self.pump();
    }

    /// Cancel (the partial file is deleted).
    pub fn cancel(&self, id: usize) {
        self.with(id, |t| match t.state.as_str() {
            "running" => t.stop.store(2, Ordering::SeqCst),
            "queued" | "paused" => t.state = "cancelled".into(),
            _ => {}
        });
    }

    /// Drop finished ones from the list.
    pub fn clear_finished(&self) {
        let mut ts = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        ts.retain(|t| ["queued", "running", "paused"].contains(&t.state.as_str()));
    }

    /// Queued + running.
    pub fn active(&self) -> usize {
        self.snapshot()
            .iter()
            .filter(|t| t.state == "queued" || t.state == "running")
            .count()
    }

    /// The program quits: running ones pause (resumed next time).
    pub fn stop_all(&self) {
        for t in self.snapshot() {
            if t.state == "running" {
                t.stop.store(1, Ordering::SeqCst);
            }
        }
    }
}
