//! Adding, opening and rebuilding dictionaries in the background, several at
//! once. Same rules as the Python edition (`dictz/core/loading.py`).
//!
//! Reading a dictionary file and writing its article store is the slow part.
//! It is done by **worker threads**, several dictionaries at the same time.
//! One coordinating thread ([`LoadJob::run`]) hands the work out, then opens
//! the finished stores and writes the list (`dictionaries.json`) and the
//! headword index — the only writer of both.
//!
//! A job keeps an inbox: files, folder scans and rebuilds added while it runs
//! ([`JobHandle`]) are taken up by the same job; nothing runs in the window's
//! thread. Progress is a [`LoadState`], sent at most every 0.1 s.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::index::{DictionaryRow, IndexStore};
use crate::query::QueryManager;
use crate::{articles, registry, Dictionary, DictzError};

/// The most worker threads chosen automatically.
pub const MAX_AUTO_WORKERS: usize = 4;

/// Workers for `jobs` dictionaries: the setting (`dictionaries.build_workers`)
/// or, at 0, the processors minus one, at most [`MAX_AUTO_WORKERS`]; never
/// more than there is work for.
pub fn worker_count(setting: i64, jobs: usize) -> usize {
    let n = if setting > 0 {
        setting as usize
    } else {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        cpus.saturating_sub(1).clamp(1, MAX_AUTO_WORKERS)
    };
    n.min(jobs.max(1)).max(1)
}

/// What the job is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Listing dictionary folders.
    Scan,
    /// Opening the list's dictionaries.
    Open,
    /// Workers are reading files.
    Prepare,
}

/// What the progress bar shows.
#[derive(Debug, Clone)]
pub struct LoadState {
    /// Current phase.
    pub phase: Phase,
    /// Dictionaries finished.
    pub done: usize,
    /// Dictionaries in the job.
    pub total: usize,
    /// Being prepared: file name and share written (`None` = still reading).
    pub running: Vec<(String, Option<f32>)>,
    /// Stop asked for.
    pub cancelling: bool,
}

impl Default for LoadState {
    fn default() -> Self {
        Self {
            phase: Phase::Open,
            done: 0,
            total: 0,
            running: Vec::new(),
            cancelling: false,
        }
    }
}

impl LoadState {
    /// Share of the whole job done, counting the running dictionaries' parts.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let part: f32 = self.running.iter().filter_map(|(_, f)| *f).sum();
        ((self.done as f32 + part) / self.total as f32).min(1.0)
    }

    /// The running dictionaries' file names.
    pub fn names(&self) -> String {
        self.running
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// What the job reports.
pub enum Event {
    /// A dictionary is ready. `replaced` is the list name of a rebuilt
    /// dictionary (it keeps its place), else empty.
    Loaded {
        /// The dictionary.
        dict: Box<dyn Dictionary>,
        /// Its file.
        path: PathBuf,
        /// On in searches.
        enabled: bool,
        /// Rebuilt dictionary's name.
        replaced: String,
    },
    /// It could not be opened (it stays in the list, marked ⚠).
    Failed(PathBuf, String),
    /// Progress.
    Progress(LoadState),
}

/// What a worker did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prepared {
    /// The store was fresh already.
    Fresh,
    /// The file was read and the store written.
    Built,
    /// The file was read but the store could not be written (read-only data
    /// folder): the coordinator opens the file itself.
    NoStore,
}

/// Runs in a worker: make the article store of `path` fresh. `report(None)`
/// = reading the file, `report(Some((done, total)))` = writing articles.
pub fn prepare(
    path: &Path,
    data_dir: &Path,
    rebuild: bool,
    report: &mut dyn FnMut(Option<(usize, usize)>),
) -> Result<Prepared, DictzError> {
    let store = articles::store_path(data_dir, &registry::path_id(path));
    if !rebuild && articles::is_fresh(&store, path) {
        return Ok(Prepared::Fresh);
    }
    report(None);
    let mut parsed = QueryManager::open_file(path)?;
    let res = articles::build_with(parsed.as_ref(), &store, &mut |d, t| report(Some((d, t))));
    parsed.close();
    Ok(if res.is_ok() {
        Prepared::Built
    } else {
        Prepared::NoStore
    })
}

/// A folder scan run inside the job.
pub type Scan = Box<dyn FnOnce() -> Vec<PathBuf> + Send>;

enum Work {
    Row(Box<DictionaryRow>),
    File(PathBuf),
    Scan(Scan),
    Rebuild(String, PathBuf, bool),
}

struct Shared {
    inbox: Mutex<(Vec<Work>, bool)>, // (work, closed)
    cancel: AtomicBool,
    stop: AtomicBool,
}

/// Talks to a running [`LoadJob`] from another thread.
#[derive(Clone)]
pub struct JobHandle(Arc<Shared>);

impl JobHandle {
    fn push(&self, work: Vec<Work>) -> bool {
        let mut g = self.0.inbox.lock().unwrap_or_else(|e| e.into_inner());
        if g.1 {
            return false;
        }
        g.0.extend(work);
        true
    }

    /// Files, scans and rebuilds at once. When the job has already finished
    /// the work comes back (`Some`): start a new job with it.
    #[allow(clippy::type_complexity)]
    pub fn add_all(
        &self,
        files: Vec<PathBuf>,
        scans: Vec<Scan>,
        rebuild: Vec<(String, PathBuf, bool)>,
    ) -> Option<(Vec<PathBuf>, Vec<Scan>, Vec<(String, PathBuf, bool)>)> {
        let mut g = self.0.inbox.lock().unwrap_or_else(|e| e.into_inner());
        if g.1 {
            return Some((files, scans, rebuild));
        }
        g.0.extend(scans.into_iter().map(Work::Scan));
        g.0.extend(files.into_iter().map(Work::File));
        g.0.extend(rebuild.into_iter().map(|(n, p, e)| Work::Rebuild(n, p, e)));
        None
    }

    /// More files; `false` when the job has finished (start a new one).
    pub fn add_files(&self, files: Vec<PathBuf>) -> bool {
        self.push(files.into_iter().map(Work::File).collect())
    }

    /// A folder scan; its files are added.
    pub fn add_scan(&self, scan: Scan) -> bool {
        self.push(vec![Work::Scan(scan)])
    }

    /// Rebuild dictionaries: (list name, file, enabled).
    pub fn add_rebuild(&self, items: Vec<(String, PathBuf, bool)>) -> bool {
        self.push(
            items
                .into_iter()
                .map(|(n, p, e)| Work::Rebuild(n, p, e))
                .collect(),
        )
    }

    /// Add no more new dictionaries (the ones being prepared now finish); the
    /// list's own dictionaries and rebuilds still complete.
    pub fn cancel(&self) {
        self.0.cancel.store(true, Ordering::SeqCst);
    }

    /// The program quits: stop at once (running workers are left to end with it).
    pub fn stop(&self) {
        self.0.cancel.store(true, Ordering::SeqCst);
        self.0.stop.store(true, Ordering::SeqCst);
    }

    /// `true` after [`JobHandle::cancel`].
    pub fn is_cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::SeqCst)
    }
}

enum Kind {
    Row(Box<DictionaryRow>, bool), // row, changed
    File,
    Rebuild(String, bool), // name, enabled
}

struct Item {
    kind: Kind,
    path: PathBuf,
}

struct Task {
    id: usize,
    path: PathBuf,
    rebuild: bool,
}

enum WorkerMsg {
    Progress(usize, Option<(usize, usize)>),
    Done(usize, Result<Prepared, String>),
}

struct Pool {
    size: usize,
    tasks: Sender<Task>,
    results: Receiver<WorkerMsg>,
}

impl Pool {
    fn start(size: usize, data_dir: PathBuf) -> Self {
        let (task_tx, task_rx) = channel::<Task>();
        let (res_tx, res_rx) = channel::<WorkerMsg>();
        let task_rx = Arc::new(Mutex::new(task_rx));
        for i in 0..size {
            let rx = Arc::clone(&task_rx);
            let tx = res_tx.clone();
            let dir = data_dir.clone();
            let _ = std::thread::Builder::new()
                .name(format!("dictz-prepare-{i}"))
                .spawn(move || loop {
                    let task = {
                        let g = rx.lock().unwrap_or_else(|e| e.into_inner());
                        g.recv()
                    };
                    let Ok(task) = task else { break };
                    let id = task.id;
                    let ptx = tx.clone();
                    let mut last = Instant::now() - Duration::from_secs(1);
                    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        prepare(&task.path, &dir, task.rebuild, &mut |p| {
                            if p.is_none() || last.elapsed() >= Duration::from_millis(100) {
                                last = Instant::now();
                                let _ = ptx.send(WorkerMsg::Progress(id, p));
                            }
                        })
                    }));
                    let res = match res {
                        Ok(r) => r.map_err(|e| e.to_string()),
                        Err(_) => Err("the reader stopped unexpectedly".to_string()),
                    };
                    if tx.send(WorkerMsg::Done(id, res)).is_err() {
                        break;
                    }
                });
        }
        Self {
            size,
            tasks: task_tx,
            results: res_rx,
        }
    }
}

/// One background job; [`LoadJob::run`] in a thread.
pub struct LoadJob {
    db: PathBuf,
    data_dir: PathBuf,
    reindex_changed: bool,
    workers: i64,
    shared: Arc<Shared>,
}

impl LoadJob {
    /// A job on the index `db` (the data folder is its folder).
    pub fn new(db: &Path, reindex_changed: bool, workers: i64) -> Self {
        Self {
            db: db.to_path_buf(),
            data_dir: db.parent().map(Path::to_path_buf).unwrap_or_default(),
            reindex_changed,
            workers,
            shared: Arc::new(Shared {
                inbox: Mutex::new((Vec::new(), false)),
                cancel: AtomicBool::new(false),
                stop: AtomicBool::new(false),
            }),
        }
    }

    /// A handle to add work or cancel.
    pub fn handle(&self) -> JobHandle {
        JobHandle(Arc::clone(&self.shared))
    }

    /// The list's dictionaries to open (at start).
    pub fn add_rows(&self, rows: Vec<DictionaryRow>) {
        self.handle()
            .push(rows.into_iter().map(|r| Work::Row(Box::new(r))).collect());
    }

    fn take(&self, finishing: bool) -> (Vec<Work>, bool) {
        let mut g = self.shared.inbox.lock().unwrap_or_else(|e| e.into_inner());
        let items = std::mem::take(&mut g.0);
        if items.is_empty() && finishing {
            g.1 = true;
        }
        (items, g.1)
    }

    fn close(&self) {
        let mut g = self.shared.inbox.lock().unwrap_or_else(|e| e.into_inner());
        g.1 = true;
    }

    /// Do the work; `sink` gets every [`Event`] (in this thread).
    pub fn run(self, sink: &mut dyn FnMut(Event)) {
        articles::remove_stale_temp(&self.data_dir);
        let mut store = IndexStore::open(&self.db).ok();
        let mut known: HashSet<String> = store
            .as_ref()
            .and_then(|s| s.list_dictionaries(false).ok())
            .unwrap_or_default()
            .iter()
            .map(|r| registry::norm_path(&r.path))
            .collect();
        let mut st = LoadState::default();
        let mut last_sent = Instant::now() - Duration::from_secs(1);
        let mut pool: Option<Pool> = None;
        let mut waiting: VecDeque<Item> = VecDeque::new();
        let mut running: HashMap<usize, (Item, Option<f32>)> = HashMap::new();
        let mut next_id = 0usize;
        let mut send = |st: &LoadState, force: bool, sink: &mut dyn FnMut(Event)| {
            if force || last_sent.elapsed() >= Duration::from_millis(100) {
                last_sent = Instant::now();
                sink(Event::Progress(st.clone()));
            }
        };
        loop {
            if self.shared.stop.load(Ordering::SeqCst) {
                break;
            }
            let cancelled = self.shared.cancel.load(Ordering::SeqCst);
            st.cancelling = cancelled;
            let (items, closed) = self.take(running.is_empty() && waiting.is_empty());
            if items.is_empty() && closed {
                break;
            }
            let mut items: VecDeque<Work> = items.into();
            while let Some(w) = items.pop_front() {
                let cancelled = self.shared.cancel.load(Ordering::SeqCst);
                match w {
                    Work::Scan(scan) => {
                        if cancelled {
                            continue;
                        }
                        st.phase = Phase::Scan;
                        send(&st, true, sink);
                        for f in scan() {
                            items.push_back(Work::File(f));
                        }
                    }
                    Work::File(f) => {
                        if cancelled {
                            continue;
                        }
                        if known.insert(registry::norm_path(&f)) {
                            waiting.push_back(Item {
                                kind: Kind::File,
                                path: f,
                            });
                            st.total += 1;
                        }
                    }
                    Work::Row(row) => {
                        known.insert(registry::norm_path(&row.path));
                        st.total += 1;
                        if let Some(item) = self.open_row(row, store.as_mut(), sink) {
                            waiting.push_back(item);
                        } else {
                            st.done += 1;
                            send(&st, false, sink);
                        }
                    }
                    Work::Rebuild(name, path, enabled) => {
                        st.total += 1;
                        waiting.push_back(Item {
                            kind: Kind::Rebuild(name, enabled),
                            path,
                        });
                    }
                }
            }
            if self.shared.cancel.load(Ordering::SeqCst) {
                st.cancelling = true;
                let before = waiting.len();
                waiting.retain(|it| !matches!(it.kind, Kind::File));
                st.total -= before - waiting.len();
            }
            if !waiting.is_empty() && pool.is_none() {
                pool = Some(Pool::start(
                    worker_count(self.workers, waiting.len() + running.len()),
                    self.data_dir.clone(),
                ));
            }
            if let Some(p) = pool.as_ref() {
                while running.len() < p.size {
                    let Some(it) = waiting.pop_front() else { break };
                    let id = next_id;
                    next_id += 1;
                    let rebuild = matches!(it.kind, Kind::Rebuild(..));
                    let _ = p.tasks.send(Task {
                        id,
                        path: it.path.clone(),
                        rebuild,
                    });
                    running.insert(id, (it, None));
                }
            }
            if running.is_empty() {
                st.running.clear();
                send(&st, false, sink);
                continue;
            }
            st.phase = Phase::Prepare;
            let Some(p) = pool.as_ref() else { break };
            let mut msgs = Vec::new();
            match p.results.recv_timeout(Duration::from_millis(100)) {
                Ok(m) => msgs.push(m),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(m) = p.results.try_recv() {
                msgs.push(m);
            }
            if self.shared.stop.load(Ordering::SeqCst) {
                break;
            }
            let mut finished = false;
            for m in msgs {
                match m {
                    WorkerMsg::Progress(id, prog) => {
                        if let Some(r) = running.get_mut(&id) {
                            r.1 = prog.map(|(d, t)| d as f32 / t.max(1) as f32);
                        }
                    }
                    WorkerMsg::Done(id, res) => {
                        if let Some((it, _)) = running.remove(&id) {
                            self.finish(it, res, store.as_mut(), sink);
                            st.done += 1;
                            finished = true;
                        }
                    }
                }
            }
            st.running = running
                .values()
                .map(|(it, f)| (file_name(&it.path), *f))
                .collect();
            send(&st, finished, sink);
        }
        self.close();
        st.running.clear();
        send(&st, true, sink);
    }

    /// A dictionary of the list: from `dictionaries.json` alone when its
    /// files are unchanged (`None`: done); otherwise a worker prepares it.
    fn open_row(
        &self,
        row: Box<DictionaryRow>,
        store: Option<&mut IndexStore>,
        sink: &mut dyn FnMut(Event),
    ) -> Option<Item> {
        let path = row.path.clone();
        let changed = row.changed();
        let st = articles::store_path(&self.data_dir, &registry::path_id(&path));
        let fast =
            !row.stamp.is_empty() && st.is_file() && row.stamp == articles::source_stamp(&path);
        if fast || !path.exists() || articles::is_fresh(&st, &path) {
            match QueryManager::open_row(&row, &self.data_dir, store.as_deref()) {
                Ok((d, built)) => {
                    if changed && (built || self.reindex_changed) {
                        if let Some(s) = store {
                            let _ = s.index_dictionary(d.as_ref());
                        }
                    }
                    sink(Event::Loaded {
                        dict: d,
                        path,
                        enabled: row.enabled,
                        replaced: String::new(),
                    });
                }
                Err(e) => sink(Event::Failed(path, e.to_string())),
            }
            return None;
        }
        Some(Item {
            kind: Kind::Row(row, changed),
            path,
        })
    }

    fn finish(
        &self,
        it: Item,
        res: Result<Prepared, String>,
        mut store: Option<&mut IndexStore>,
        sink: &mut dyn FnMut(Event),
    ) {
        let result = match res {
            Ok(r) => r,
            Err(e) => {
                sink(Event::Failed(it.path, e));
                return;
            }
        };
        // the store is fresh now (or could not be written: open_cached reads the file)
        let out: Result<Event, DictzError> = (|| match it.kind {
            Kind::Row(row, changed) => {
                let (d, _) = QueryManager::open_row(&row, &self.data_dir, store.as_deref())?;
                if changed && (result == Prepared::Built || self.reindex_changed) {
                    if let Some(s) = store.as_deref_mut() {
                        let _ = s.index_dictionary(d.as_ref());
                    }
                }
                Ok(Event::Loaded {
                    dict: d,
                    path: it.path.clone(),
                    enabled: row.enabled,
                    replaced: String::new(),
                })
            }
            Kind::Rebuild(name, enabled) => {
                let (mut d, _) = articles::open_cached(&it.path, &self.data_dir, false)?;
                d.metadata_mut().name = name.clone();
                if let Some(s) = store.as_deref_mut() {
                    let _ = s.index_dictionary(d.as_ref());
                }
                Ok(Event::Loaded {
                    dict: d,
                    path: it.path.clone(),
                    enabled,
                    replaced: name,
                })
            }
            Kind::File => {
                let (mut d, _) = articles::open_cached(&it.path, &self.data_dir, false)?;
                if let Some(s) = store {
                    // a unique name in the list ("Name (2)")
                    let name = s.register_dictionary(d.as_ref())?;
                    d.metadata_mut().name = name.clone();
                    if !s.is_indexed(&name).unwrap_or(false) {
                        let _ = s.index_dictionary(d.as_ref());
                    }
                }
                Ok(Event::Loaded {
                    dict: d,
                    path: it.path.clone(),
                    enabled: true,
                    replaced: String::new(),
                })
            }
        })();
        match out {
            Ok(ev) => sink(ev),
            Err(e) => sink(Event::Failed(it.path, e.to_string())),
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(dir: &Path, n: usize) -> Vec<PathBuf> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/data/parity/sample.dsl");
        (0..n)
            .map(|i| {
                let d = dir.join(format!("src{i}"));
                std::fs::create_dir_all(&d).unwrap();
                let f = d.join(format!("s{i}.dsl"));
                std::fs::copy(&src, &f).unwrap();
                f
            })
            .collect()
    }

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let d = std::env::temp_dir().join(format!("dictz-load-{}-{}", tag, std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[allow(clippy::type_complexity)]
    fn run(
        job: LoadJob,
    ) -> (
        Vec<(Box<dyn Dictionary>, PathBuf, String)>,
        Vec<PathBuf>,
        Vec<LoadState>,
    ) {
        let (mut got, mut failed, mut states) = (Vec::new(), Vec::new(), Vec::new());
        job.run(&mut |e| match e {
            Event::Loaded {
                dict,
                path,
                replaced,
                ..
            } => got.push((dict, path, replaced)),
            Event::Failed(p, _) => failed.push(p),
            Event::Progress(s) => states.push(s),
        });
        (got, failed, states)
    }

    #[test]
    fn workers() {
        assert_eq!(worker_count(3, 10), 3);
        assert_eq!(worker_count(8, 2), 2);
        assert!((1..=MAX_AUTO_WORKERS).contains(&worker_count(0, 100)));
        assert_eq!(worker_count(0, 0), 1);
    }

    #[test]
    fn new_files_are_prepared_in_parallel_and_listed() {
        let tmp = Tmp::new("par");
        let files = tiny(tmp.path(), 4);
        let db = tmp.path().join("data/dictz_index.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let job = LoadJob::new(&db, true, 2);
        assert!(job.handle().add_files(files.clone()));
        let h = job.handle();
        let (got, failed, states) = run(job);
        assert!(failed.is_empty());
        assert_eq!(got.len(), 4);
        assert!(!h.add_files(files.clone())); // finished: start a new job
        let s = IndexStore::open(&db).unwrap();
        let names: Vec<String> = s
            .list_dictionaries(false)
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names.len(), 4);
        assert_eq!(names.iter().collect::<HashSet<_>>().len(), 4); // "X (2)" …
        let last = states.last().unwrap();
        assert!(last.running.is_empty() && last.done == 4 && last.total == 4);
        assert!(states.iter().any(|s| s.phase == Phase::Prepare));
    }

    #[test]
    fn rows_open_from_the_list_and_rebuild_keeps_the_name() {
        let tmp = Tmp::new("rows");
        let files = tiny(tmp.path(), 1);
        let db = tmp.path().join("data/dictz_index.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let job = LoadJob::new(&db, true, 1);
        job.handle().add_files(files.clone());
        drop(run(job));
        let rows = IndexStore::open(&db)
            .unwrap()
            .list_dictionaries(false)
            .unwrap();
        let name = rows[0].name.clone();
        let job = LoadJob::new(&db, true, 1);
        job.add_rows(rows);
        let (got, failed, _) = run(job);
        assert!(failed.is_empty() && got.len() == 1);
        let job = LoadJob::new(&db, true, 1);
        job.handle()
            .add_rebuild(vec![(name.clone(), files[0].clone(), true)]);
        let (got, _, _) = run(job);
        assert_eq!(got[0].2, name);
        assert_eq!(got[0].0.metadata().name, name);
    }

    #[test]
    fn scans_run_in_the_job_and_cancel_adds_nothing_new() {
        let tmp = Tmp::new("scan");
        let files = tiny(tmp.path(), 2);
        let db = tmp.path().join("data/dictz_index.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let job = LoadJob::new(&db, true, 1);
        let list = files.clone();
        job.handle().add_scan(Box::new(move || list));
        let (got, _, states) = run(job);
        assert_eq!(got.len(), 2);
        assert!(states.iter().any(|s| s.phase == Phase::Scan));
        let more = tiny(&tmp.path().join("more"), 2);
        let job = LoadJob::new(&db, true, 1);
        job.handle().add_files(more);
        job.handle().cancel();
        let (got, _, _) = run(job);
        assert!(got.is_empty());
    }
}
