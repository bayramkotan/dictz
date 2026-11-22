//! The Dictionaries window — the same tabs, buttons and texts as the Python
//! edition's `dictz/ui/dictionary_center.py`: your dictionaries (order by
//! drag and drop, on / off, remove, files, updates), dictionary folders, the
//! catalog of downloadable dictionaries, sources (servers), downloads,
//! import / export and the dictionary settings.
//!
//! Everything slow (reading sources, downloading, unpacking, building a
//! catalog, a setup archive) runs in threads; results come back through a
//! channel that [`CenterWindow::update`] drains every frame, also while the
//! window is closed (downloads go on in the background).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText};
use egui_extras::{Column, TableBuilder};
use serde_json::{json, Value};

use crate::center::catalog::{Cached, Catalog};
use crate::center::install::{DownloadQueue, Installer, QueueEvent, Task};
use crate::center::model::{human_size, matches, pair_text, Entry, Filter};
use crate::center::net::Http;
use crate::center::sources::{clean, guess_type, Source, Sources, AUTH, TYPES};
use crate::center::transfer::{self, ImportResult};
use crate::center::{adapters, langs, licenses, store};
use crate::i18n::{tr, tr_args};
use crate::settings::Preferences;

use super::prefs_window::{PrefsWindow, PAGES_IN_DICTIONARIES_WINDOW};

/// The tabs, in order (the main window opens one by number).
pub const TAB_MINE: usize = 0;
/// Dictionary folders.
pub const TAB_FOLDERS: usize = 1;
/// Downloadable dictionaries.
pub const TAB_CATALOG: usize = 2;
/// Servers.
pub const TAB_SOURCES: usize = 3;
/// The download queue.
pub const TAB_DOWNLOADS: usize = 4;
/// Setup / list export and import, catalog builder.
pub const TAB_TRANSFER: usize = 5;
/// Dictionary and Dictionary Center settings.
pub const TAB_SETTINGS: usize = 6;

/// One row of "Sözlüklerim" (from the main window's list).
#[derive(Clone)]
pub struct MineRow {
    /// Unique name.
    pub name: String,
    /// On / off.
    pub enabled: bool,
    /// Opened (false: the file is missing or failed).
    pub loaded: bool,
    /// Why it failed.
    pub error: String,
    /// stardict / dsl / babylon.
    pub format: String,
    /// Headwords.
    pub words: u64,
    /// `en → tr`.
    pub langs: String,
    /// Its file.
    pub path: PathBuf,
}

/// A dictionary dragged in "Sözlüklerim".
#[derive(Clone)]
struct MineDrag(String);

use super::system::open_external;

const RED: Color32 = Color32::from_rgb(0xc2, 0x45, 0x3a);
const GREEN: Color32 = Color32::from_rgb(0x2e, 0x9d, 0x5b);

fn status_color(status: &str) -> Color32 {
    match status {
        "open" => GREEN,
        "free" => Color32::from_rgb(0x3b, 0x82, 0xc4),
        "unknown" => Color32::from_rgb(0xc2, 0x8a, 0x1e),
        "nonfree" => RED,
        _ => Color32::GRAY,
    }
}

/// A licence status in words.
pub fn status_text(status: &str) -> String {
    match status {
        "open" => tr("açık lisans"),
        "free" => tr("serbest"),
        "unknown" => tr("lisans bilinmiyor"),
        "nonfree" => tr("kişisel kullanım"),
        s => s.to_string(),
    }
}

/// The licence's name; the few that are not names are translated.
pub fn license_name(lic: &licenses::License) -> String {
    match lic.id.as_str() {
        "unknown" => tr("Bilinmiyor — kaynak belirtmiyor"),
        "free" => tr("Serbest (lisans adı verilmemiş)"),
        "personal" => tr("Yalnızca kişisel kullanım için serbest"),
        "public-domain" => tr("Kamu malı"),
        "freedict" => tr("FreeDict (GPL, CC BY-SA … — her sözlüğün kendi lisansı)"),
        _ => lic.name.clone(),
    }
}

fn kind_text(kind: &str) -> String {
    match kind {
        "bilingual" => tr("iki dilli"),
        "monolingual" => tr("tek dilli"),
        "multilingual" => tr("çok dilli"),
        "unknown" => "–".into(),
        k => k.to_string(),
    }
}

/// A source type in words.
pub fn type_text(tp: &str) -> String {
    match tp {
        "dictz-catalog" => tr("dictz kataloğu (sunucu)"),
        "freedict" => "FreeDict".into(),
        "github-releases" => tr("GitHub sürümleri"),
        "github-folder" => tr("GitHub klasörü"),
        "http-index" => tr("Web klasör listesi"),
        "webdav" => "WebDAV".into(),
        "nextcloud-share" => tr("Nextcloud paylaşımı"),
        "archive-org" => "Internet Archive".into(),
        "local" => tr("Yerel klasör"),
        "telegram" => tr("Telegram kanalı"),
        t => t.to_string(),
    }
}

fn auth_text(a: &str) -> String {
    match a {
        "none" => tr("yok"),
        "basic" => tr("kullanıcı + parola"),
        "bearer" => tr("belirteç (token)"),
        "header" => tr("özel başlık"),
        x => x.to_string(),
    }
}

fn yn(b: bool) -> String {
    if b {
        tr("evet")
    } else {
        tr("hayır")
    }
}

fn dash(s: &str) -> String {
    if s.is_empty() {
        "–".into()
    } else {
        s.to_string()
    }
}

fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn vs(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// What the window asks the main window to do.
pub enum Event {
    /// Add these dictionary files (an install finished).
    InstallFiles(Vec<PathBuf>),
    /// Drop these files from the list (before they are deleted).
    RemoveFiles(Vec<String>),
    /// Drop every dictionary (an import replaces the list).
    RemoveAll,
    /// A setup came in: open its dictionaries; `true` = settings too.
    Imported(bool),
    /// Switch a dictionary on / off.
    SetEnabled(String, bool),
    /// Only these on.
    Solo(Vec<String>),
    /// The new order of the dictionaries.
    SetOrder(Vec<String>),
    /// Show a dictionary's information.
    Info(String),
    /// Read these again.
    Reindex(Vec<String>),
    /// "Dosya ekle…".
    AddFiles,
    /// "Klasör ekle…".
    AddFolder,
    /// Scan the dictionary folders now.
    Rescan,
    /// Drop these dictionaries from the list, then delete these files.
    Remove(Vec<String>, Vec<PathBuf>),
    /// The settings changed in the window (folders or the settings tab).
    PrefsChanged(Preferences),
}

enum Work {
    Export(usize),
    Import(ImportResult),
    Catalog(PathBuf, usize),
}

#[allow(clippy::large_enum_variant)] // rare messages
enum Msg {
    Refreshed(String, Cached),
    RefreshDone,
    Queue(QueueEvent),
    Work(Result<Work, String>),
    Tested(Result<usize, String>),
}

/// What a "Yes" / "No" leads to.
#[derive(Clone)]
enum Next {
    Nothing,
    Install(Vec<Entry>, u8),
    RemoveSource(String),
    ResetSources,
    Uninstall(Vec<String>),
    Import(PathBuf, bool),
    AddSource(String),
    DownloadLink(String),
    /// Remove from "Sözlüklerim"; `true` = with the files.
    RemoveMine(Vec<String>, bool),
    Close,
}

struct Ask {
    title: String,
    text: String,
    yes: Next,
    no: Next,
    /// Only an OK button.
    info: bool,
    /// A third button (text, what it does) and the yes / no texts.
    extra: Option<(String, Next)>,
    texts: Option<(String, String)>,
}

/// The source editor (add / edit).
struct SourceDialog {
    id: Option<String>,
    base: Source,
    name: String,
    url: String,
    kind: String,
    filter: String,
    pattern: String,
    src: String,
    dst: String,
    license: String,
    depth: i64,
    auth: String,
    user: String,
    password: String,
    token: String,
    description: String,
    notes: String,
    enabled: bool,
    test: String,
}

impl SourceDialog {
    fn new(sources: &Sources, s: Option<&Source>, url: &str) -> Self {
        let base = s.cloned().unwrap_or_else(|| {
            clean(&json!({"url": url, "type": if url.is_empty() { "dictz-catalog" } else { guess_type(url) }}))
        });
        let (pw, tok) = match s {
            Some(s) => sources.secret(&s.id),
            None => (String::new(), String::new()),
        };
        Self {
            id: s.map(|s| s.id.clone()),
            name: base.name.clone(),
            url: base.url.clone(),
            kind: if TYPES.contains(&base.kind.as_str()) {
                base.kind.clone()
            } else {
                TYPES[0].into()
            },
            filter: base.filter.clone(),
            pattern: base.name_pattern.clone(),
            src: base.source_lang.clone(),
            dst: base.target_lang.clone(),
            license: base.license.clone(),
            depth: if base.depth > 0 { base.depth } else { 2 },
            auth: if AUTH.contains(&base.auth.as_str()) {
                base.auth.clone()
            } else {
                "none".into()
            },
            user: base.user.clone(),
            password: pw,
            token: tok,
            description: base.description.clone(),
            notes: base.notes.clone(),
            enabled: base.enabled,
            test: String::new(),
            base,
        }
    }

    fn values(&self) -> Value {
        json!({"name": self.name.trim(), "url": self.url.trim(), "type": self.kind,
               "filter": self.filter.trim(), "name_pattern": self.pattern.trim(),
               "source_lang": self.src.trim(), "target_lang": self.dst.trim(),
               "license": self.license.split(" — ").next().unwrap_or("").trim(), "depth": self.depth,
               "auth": self.auth, "user": self.user.trim(), "description": self.description.trim(),
               "notes": self.notes.trim(), "enabled": self.enabled})
    }
}

/// The Dictionary Center: kept while dictz runs (the downloads go on when
/// the window is closed).
pub struct CenterWindow {
    data_dir: PathBuf,
    prefs: Preferences,
    sources: Sources,
    catalog: Catalog,
    installer: Arc<Installer>,
    queue: DownloadQueue,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// The tab shown.
    pub tab: usize,
    status: String,
    refreshing: bool,
    pending: usize,
    auto_refreshed: bool,
    events: Vec<Event>,
    ask: Option<Ask>,
    // catalog
    entries: Vec<Entry>,
    rows: Vec<usize>,
    installed: BTreeMap<String, Value>,
    updates: HashSet<String>,
    search: String,
    from: String,
    to: String,
    source: String,
    license: String,
    fmt: String,
    supported_only: bool,
    hide_installed: bool,
    sort: Option<(usize, bool)>,
    selected: BTreeSet<String>,
    anchor: Option<usize>,
    from_list: Vec<(String, String)>,
    to_list: Vec<(String, String)>,
    formats: Vec<String>,
    dirty: bool,
    // sources
    cached: HashMap<String, Cached>,
    src_sel: Option<String>,
    editor: Option<SourceDialog>,
    // downloads
    tasks: Vec<Task>,
    dl_sel: BTreeSet<usize>,
    history: Vec<Value>,
    // my dictionaries
    mine_rows: Vec<MineRow>,
    mine_sel: BTreeSet<String>,
    rec_by_path: HashMap<String, (String, Value)>,
    update_by_id: HashMap<String, Entry>,
    pending_uninstall: Vec<String>,
    uninstall_delete: bool,
    remove_next: bool,
    // folders and settings (edited on a copy, sent with `PrefsChanged`)
    settings_ed: PrefsWindow,
    // import / export
    ex_parts: [bool; 6],
    im_replace: bool,
    cb_folder: String,
    cb_url: String,
    cb_name: String,
    cb_license: String,
    // small windows
    link: Option<String>,
    show_licenses: bool,
}

impl CenterWindow {
    /// Open on the data folder; `prefs` gives the center settings.
    pub fn new(ctx: &egui::Context, data_dir: &Path, prefs: &Preferences) -> Self {
        let http = Http::new(prefs.float("center.timeout"), &prefs.str("center.proxy"));
        let root = prefs.str("center.install_folder").trim().to_string();
        let installer = Arc::new(Installer::new(
            data_dir,
            http,
            &root,
            prefs.bool("center.verify_checksums"),
            prefs.bool("center.keep_archives"),
        ));
        let (tx, rx) = channel();
        let sink_tx = tx.clone();
        let repaint = ctx.clone();
        let sink: Arc<dyn Fn(QueueEvent) + Send + Sync> = Arc::new(move |ev| {
            let _ = sink_tx.send(Msg::Queue(ev));
            repaint.request_repaint();
        });
        let queue = DownloadQueue::new(
            Arc::clone(&installer),
            prefs.int("center.parallel_downloads").max(1) as usize,
            sink,
        );
        let mut w = Self {
            data_dir: data_dir.to_path_buf(),
            prefs: prefs.clone(),
            sources: Sources::new(data_dir),
            catalog: Catalog::new(data_dir),
            installer,
            queue,
            tx,
            rx,
            tab: 0,
            status: String::new(),
            refreshing: false,
            pending: 0,
            auto_refreshed: false,
            events: Vec::new(),
            ask: None,
            entries: Vec::new(),
            rows: Vec::new(),
            installed: BTreeMap::new(),
            updates: HashSet::new(),
            search: String::new(),
            from: String::new(),
            to: String::new(),
            source: String::new(),
            license: if prefs.bool("center.show_unknown_license") {
                "all".into()
            } else {
                "free".into()
            },
            fmt: String::new(),
            supported_only: false,
            hide_installed: false,
            sort: None,
            selected: BTreeSet::new(),
            anchor: None,
            from_list: Vec::new(),
            to_list: Vec::new(),
            formats: Vec::new(),
            dirty: true,
            cached: HashMap::new(),
            src_sel: None,
            editor: None,
            tasks: Vec::new(),
            dl_sel: BTreeSet::new(),
            history: Vec::new(),
            mine_rows: Vec::new(),
            mine_sel: BTreeSet::new(),
            rec_by_path: HashMap::new(),
            update_by_id: HashMap::new(),
            pending_uninstall: Vec::new(),
            uninstall_delete: true,
            remove_next: false,
            settings_ed: PrefsWindow::new(prefs),
            ex_parts: [true, true, true, true, true, false],
            im_replace: false,
            cb_folder: String::new(),
            cb_url: "https://raw.githubusercontent.com/bayramkotan/dictz-data/main/".into(),
            cb_name: "dictz-data".into(),
            cb_license: String::new(),
            link: None,
            show_licenses: false,
        };
        w.reload_all();
        w
    }

    /// New settings from the preferences window.
    pub fn set_prefs(&mut self, prefs: &Preferences) {
        self.prefs = prefs.clone();
        self.settings_ed.draft = prefs.clone();
    }

    /// Select `name` in "Sözlüklerim" (`remove`: and ask to remove it).
    pub fn select_dictionary(&mut self, name: &str, remove: bool) {
        self.tab = TAB_MINE;
        self.mine_sel = [name.to_string()].into_iter().collect();
        if remove {
            // the rows come with the next frame; ask then
            self.remove_next = true;
        }
    }

    /// Stop the downloads (dictz quits).
    pub fn stop_all(&self) {
        self.queue.stop_all();
    }

    /// Ask for a link (File → Add dictionary from link…).
    pub fn add_from_link(&mut self) {
        self.tab = TAB_DOWNLOADS;
        self.link = Some(String::new());
    }

    // ------------------------------------------------------------ data

    fn reload_all(&mut self) {
        self.cached = self
            .sources
            .all()
            .iter()
            .map(|s| (s.id.clone(), self.catalog.cached(&s.id)))
            .collect();
        self.entries = self.catalog.entries(false);
        self.history = self.installer.history().into_iter().take(300).collect();
        self.fill_filters();
        self.dirty = true;
        if !self.refreshing {
            self.status = self.summary();
        }
    }

    fn summary(&self) -> String {
        format!("{}: ", tr("Katalog"))
            + &tr_args(
                "{n} sözlük, {k} kaynak",
                &[
                    ("n", &self.entries.len()),
                    ("k", &self.sources.enabled().len()),
                ],
            )
    }

    fn fill_filters(&mut self) {
        let (src, dst) = self.catalog.languages();
        let mut cs: HashMap<String, usize> = HashMap::new();
        let mut ct: HashMap<String, usize> = HashMap::new();
        for e in &self.entries {
            *cs.entry(e.source_lang.clone()).or_default() += 1;
            for t in &e.target_lang {
                *ct.entry(t.clone()).or_default() += 1;
            }
        }
        let list = |codes: Vec<String>, counts: &HashMap<String, usize>| {
            let mut v: Vec<(String, String)> = codes
                .into_iter()
                .map(|c| {
                    let label = format!(
                        "{} · {}",
                        langs::label(&c),
                        counts.get(&c).copied().unwrap_or(0)
                    );
                    (c, label)
                })
                .collect();
            v.sort_by_key(|(c, _)| {
                let n = langs::name(c);
                if n.is_empty() {
                    c.clone()
                } else {
                    n
                }
            });
            v
        };
        self.from_list = list(src, &cs);
        self.to_list = list(dst, &ct);
        let mut f: Vec<String> = self.entries.iter().map(|e| e.format.clone()).collect();
        f.sort();
        f.dedup();
        self.formats = f;
    }

    fn apply_filter(&mut self) {
        self.installed = self.installer.installed();
        self.rec_by_path.clear();
        for (rid, rec) in &self.installed {
            for f in rec["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                self.rec_by_path.insert(
                    crate::registry::norm_path(Path::new(f)),
                    (rid.clone(), rec.clone()),
                );
            }
        }
        let all = self.catalog.entries(true);
        self.update_by_id = self
            .installer
            .updates(&all)
            .into_iter()
            .map(|(r, e)| (vs(&r, "id"), e))
            .collect();
        self.updates = self
            .installer
            .updates(&self.entries)
            .into_iter()
            .map(|(_, e)| e.id)
            .collect();
        let f = Filter {
            text: self.search.trim().to_string(),
            src: self.from.clone(),
            dst: self.to.clone(),
            source: self.source.clone(),
            license: self.license.clone(),
            fmt: self.fmt.clone(),
            supported_only: self.supported_only,
        };
        self.rows = (0..self.entries.len())
            .filter(|&i| {
                let e = &self.entries[i];
                matches(e, &f) && !(self.hide_installed && self.installed.contains_key(&e.id))
            })
            .collect();
        if let Some((col, asc)) = self.sort {
            let es = &self.entries;
            let inst = &self.installed;
            self.rows.sort_by(|&a, &b| {
                let (x, y) = (&es[a], &es[b]);
                let o = match col {
                    0 => (!inst.contains_key(&x.id)).cmp(&!inst.contains_key(&y.id)),
                    1 => x.name.to_lowercase().cmp(&y.name.to_lowercase()),
                    2 => pair_text(x).cmp(&pair_text(y)),
                    3 => x.kind.cmp(&y.kind),
                    4 => x.format.cmp(&y.format),
                    5 => x.headwords.cmp(&y.headwords),
                    6 => x.size.cmp(&y.size),
                    7 => x.license.cmp(&y.license),
                    8 => x.source.cmp(&y.source),
                    _ => x.date.cmp(&y.date),
                };
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        let shown: HashSet<&str> = self
            .rows
            .iter()
            .map(|&i| self.entries[i].id.as_str())
            .collect();
        self.selected.retain(|id| shown.contains(id.as_str()));
        self.dirty = false;
    }

    fn selected_entries(&self) -> Vec<Entry> {
        self.rows
            .iter()
            .map(|&i| &self.entries[i])
            .filter(|e| self.selected.contains(&e.id))
            .cloned()
            .collect()
    }

    fn online(&self) -> bool {
        self.prefs.bool("center.online")
    }

    fn offline_text() -> String {
        tr("İnternet kaynakları kapalı (Sözlükler → Ayarlar).")
    }

    fn info(&mut self, title: String, text: String) {
        self.ask = Some(Ask {
            title,
            text,
            yes: Next::Nothing,
            no: Next::Nothing,
            info: true,
            extra: None,
            texts: None,
        });
    }

    fn question(&mut self, title: String, text: String, yes: Next, no: Next) {
        self.ask = Some(Ask {
            title,
            text,
            yes,
            no,
            info: false,
            extra: None,
            texts: None,
        });
    }

    // ------------------------------------------------------------ refresh

    fn refresh(&mut self, ids: Option<Vec<String>>) {
        if self.refreshing {
            return;
        }
        if !self.online() {
            self.status = Self::offline_text();
            return;
        }
        let todo: Vec<String> =
            ids.unwrap_or_else(|| self.sources.enabled().into_iter().map(|s| s.id).collect());
        if todo.is_empty() {
            return;
        }
        self.refreshing = true;
        self.pending = todo.len();
        self.status = tr_args("Kataloglar okunuyor ({n})…", &[("n", &todo.len())]);
        let tx = self.tx.clone();
        let dir = self.data_dir.clone();
        let (timeout, proxy) = (
            self.prefs.float("center.timeout"),
            self.prefs.str("center.proxy"),
        );
        std::thread::Builder::new()
            .name("dictz-catalog-refresh".into())
            .spawn(move || {
                let cat = Catalog::new(&dir);
                let http = Http::new(timeout, &proxy);
                let tx2 = std::sync::Mutex::new(tx.clone());
                cat.refresh(
                    Some(&todo),
                    0.0,
                    &http,
                    &|sid, r| {
                        if let Ok(t) = tx2.lock() {
                            let _ = t.send(Msg::Refreshed(sid.to_string(), r.clone()));
                        }
                    },
                    4,
                );
                let _ = tx.send(Msg::RefreshDone);
            })
            .ok();
    }

    fn auto_refresh(&mut self) {
        let days = self.prefs.int("center.refresh_days");
        if days > 0 && self.online() {
            let stale: Vec<String> = self
                .sources
                .enabled()
                .into_iter()
                .filter(|s| self.catalog.stale(&s.id, days as f64))
                .map(|s| s.id)
                .collect();
            if !stale.is_empty() {
                self.refresh(Some(stale));
            }
        }
    }

    // ------------------------------------------------------------ messages

    fn poll(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Refreshed(sid, res) => {
                    self.pending = self.pending.saturating_sub(1);
                    let name = self
                        .sources
                        .get(&sid)
                        .map(|s| s.name)
                        .unwrap_or(sid.clone());
                    let state = if res.ok {
                        tr_args("{n} sözlük", &[("n", &res.entries.len())])
                    } else {
                        tr("hata")
                    };
                    self.status = format!(
                        "{}  {}: {}",
                        tr_args("Kataloglar okunuyor ({n})…", &[("n", &self.pending)]),
                        name,
                        state
                    );
                    self.cached.insert(sid, res);
                }
                Msg::RefreshDone => {
                    self.refreshing = false;
                    self.reload_all();
                    let errors: Vec<String> = self
                        .sources
                        .enabled()
                        .into_iter()
                        .filter(|s| self.cached.get(&s.id).is_some_and(|c| !c.error.is_empty()))
                        .map(|s| s.name)
                        .collect();
                    self.status = self.summary();
                    if !errors.is_empty() {
                        self.status += &format!(
                            " · {}",
                            tr_args("okunamayan: {names}", &[("names", &errors.join(", "))])
                        );
                    }
                }
                Msg::Queue(QueueEvent::Changed(t)) => {
                    let done = t.state == "done" || t.state == "failed";
                    match self.tasks.iter_mut().find(|x| x.id == t.id) {
                        Some(x) => *x = t,
                        None => self.tasks.push(t),
                    }
                    if done {
                        self.history = self.installer.history().into_iter().take(300).collect();
                    }
                }
                Msg::Queue(QueueEvent::Installed(rec, entry)) => {
                    let files: Vec<PathBuf> = rec["files"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(PathBuf::from)
                                .collect()
                        })
                        .unwrap_or_default();
                    let mut s = tr_args("Kuruldu: {name}", &[("name", &entry.name)]);
                    if files.is_empty() {
                        s += &format!(
                            " — {}",
                            tr("dictz bu biçimi henüz açamıyor; dosyalar klasörde saklandı")
                        );
                    } else if self.prefs.bool("center.add_after_install") {
                        self.events.push(Event::InstallFiles(files));
                    }
                    self.status = s;
                    self.dirty = true;
                }
                Msg::Work(Err(e)) => {
                    self.status.clear();
                    self.info(tr("Olmadı"), e);
                }
                Msg::Work(Ok(Work::Export(n))) => {
                    self.status = tr_args("{n} sözlük dışa aktarıldı", &[("n", &n)]);
                }
                Msg::Work(Ok(Work::Catalog(path, n))) => {
                    self.status = tr_args(
                        "{n} sözlük → {path}",
                        &[("n", &n), ("path", &path.display())],
                    );
                }
                Msg::Work(Ok(Work::Import(res))) => {
                    self.status = tr_args(
                        "İçe aktarıldı: {n} sözlük, {f} dosya, {s} kaynak",
                        &[
                            ("n", &res.added.len()),
                            ("f", &res.files),
                            ("s", &res.sources),
                        ],
                    );
                    let mut text = self.status.clone();
                    if !res.missing.is_empty() {
                        text += &format!(
                            "\n\n{}\n{}",
                            tr("Bulunamayan dosyalar:"),
                            res.missing
                                .iter()
                                .take(20)
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                    }
                    self.info(tr("İçe aktar"), text);
                    self.events.push(Event::Imported(res.settings));
                    self.sources = Sources::new(&self.data_dir);
                    self.reload_all();
                }
                Msg::Tested(r) => {
                    if let Some(ed) = &mut self.editor {
                        ed.test = match r {
                            Ok(n) => tr_args("Tamam: {n} sözlük bulundu", &[("n", &n)]),
                            Err(e) => format!("{} {}", tr("Olmadı:"), e),
                        };
                    }
                }
            }
        }
    }

    fn run(&mut self, f: impl FnOnce() -> Result<Work, String> + Send + 'static) {
        self.status = tr("Çalışıyor…");
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Work(f()));
        });
    }

    fn follow(&mut self, next: Next, open: &mut bool) {
        match next {
            Next::Nothing => {}
            Next::Install(todo, stage) => self.install_flow(todo, stage),
            Next::RemoveSource(id) => {
                self.sources.remove(&id);
                self.src_sel = None;
                self.reload_all();
            }
            Next::ResetSources => {
                self.sources.reset();
                self.reload_all();
            }
            Next::Uninstall(ids) => self.do_uninstall(ids),
            Next::Import(path, replace) => {
                if replace {
                    self.events.push(Event::RemoveAll);
                }
                let dir = self.data_dir.clone();
                self.run(move || {
                    transfer::import_setup(&path, &dir, replace, transfer::PARTS).map(Work::Import)
                });
            }
            Next::AddSource(url) => {
                self.tab = TAB_SOURCES;
                self.editor = Some(SourceDialog::new(&self.sources, None, &url));
            }
            Next::DownloadLink(url) => {
                if !self.online() && !url.starts_with("file:") {
                    self.info(tr("Bağlantıdan ekle"), Self::offline_text());
                    return;
                }
                self.queue.add(adapters::entry_from_url(&url));
                self.tab = TAB_DOWNLOADS;
            }
            Next::RemoveMine(names, with_files) => self.do_remove_mine(names, with_files),
            Next::Close => *open = false,
        }
    }

    // ------------------------------------------------------------ install

    /// The questions before downloading: no files → open the page; unknown
    /// licence; a format dictz cannot open yet; offline.
    fn install_flow(&mut self, mut todo: Vec<Entry>, stage: u8) {
        if stage == 0 {
            let no_files: Vec<Entry> = todo
                .iter()
                .filter(|e| e.files.is_empty() && e.resolve.is_empty())
                .cloned()
                .collect();
            todo.retain(|e| !e.files.is_empty() || !e.resolve.is_empty());
            if !no_files.is_empty() {
                for e in no_files.iter().take(5) {
                    if !e.page.is_empty() {
                        let _ = open_external(&e.page);
                    }
                }
                let names: Vec<String> = no_files.iter().take(10).map(|e| e.name.clone()).collect();
                self.info(
                    tr("Kur"),
                    format!(
                        "{}\n{}",
                        tr("Bunların indirilecek dosyası yok, sayfaları açılıyor:"),
                        names.join("\n")
                    ),
                );
                if todo.is_empty() {
                    return;
                }
                // the questions follow after the note
                if let Some(a) = &mut self.ask {
                    a.yes = Next::Install(todo.clone(), 1);
                    a.no = Next::Install(todo, 1);
                }
                return;
            }
        }
        if stage <= 1 {
            let unknown: Vec<Entry> = todo
                .iter()
                .filter(|e| {
                    matches!(
                        licenses::get(&e.license).status.as_str(),
                        "unknown" | "nonfree"
                    )
                })
                .cloned()
                .collect();
            if !unknown.is_empty() {
                let rest: Vec<Entry> = todo
                    .iter()
                    .filter(|e| !unknown.iter().any(|u| u.id == e.id))
                    .cloned()
                    .collect();
                self.question(
                    tr("Lisans"),
                    tr_args(
                        "{n} sözlüğün lisansı bilinmiyor ya da yalnızca kişisel kullanım için. Kullanım hakkı sizde mi? Yine de indirilsin mi?",
                        &[("n", &unknown.len())],
                    ),
                    Next::Install(todo, 2),
                    Next::Install(rest, 2),
                );
                return;
            }
        }
        if stage <= 2 {
            let unsupported: Vec<Entry> = todo
                .iter()
                .filter(|e| e.supported == Some(false))
                .cloned()
                .collect();
            if !unsupported.is_empty() {
                let mut fmts: Vec<String> = unsupported.iter().map(|e| e.format.clone()).collect();
                fmts.sort();
                fmts.dedup();
                let rest: Vec<Entry> = todo
                    .iter()
                    .filter(|e| e.supported != Some(false))
                    .cloned()
                    .collect();
                self.question(
                    tr("Biçim"),
                    tr_args(
                        "{n} sözlüğün biçimini dictz henüz açamıyor ({f}). Dosyalar indirilip saklansın mı?",
                        &[("n", &unsupported.len()), ("f", &fmts.join(", "))],
                    ),
                    Next::Install(todo, 3),
                    Next::Install(rest, 3),
                );
                return;
            }
        }
        if todo.is_empty() {
            return;
        }
        if !self.online()
            && todo.iter().any(|e| {
                e.files.iter().any(|f| !f.url.starts_with("file:")) || !e.resolve.is_empty()
            })
        {
            self.info(tr("Kur"), Self::offline_text());
            return;
        }
        for e in todo {
            self.queue.add(e);
        }
        self.tab = TAB_DOWNLOADS;
    }

    fn uninstall(&mut self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        let recs = self.installer.installed();
        let names: Vec<String> = ids
            .iter()
            .filter_map(|i| recs.get(i))
            .map(|r| vs(r, "name"))
            .collect();
        let tail = tr("İndirilen dosyalar da silinecek.");
        self.question(
            tr("Kaldır"),
            format!(
                "{}\n\n{}\n\n{}",
                tr("Bu sözlükler dictz'ten kaldırılsın mı?"),
                names.join("\n"),
                tail
            ),
            Next::Uninstall(ids),
            Next::Nothing,
        );
    }

    fn do_uninstall(&mut self, ids: Vec<String>) {
        let recs = self.installer.installed();
        let mut files = Vec::new();
        for i in &ids {
            if let Some(r) = recs.get(i) {
                files.extend(
                    r["files"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(String::from),
                );
            }
        }
        // the main window drops them from the list first (Windows: closes them)
        self.events.push(Event::RemoveFiles(files));
        self.pending_uninstall = ids;
        self.uninstall_delete = true;
    }

    /// "Kaldır…" in "Sözlüklerim": a catalog install loses its folder (or
    /// only leaves the list), an own dictionary only the files it is made of.
    fn remove_mine(&mut self) {
        let names: Vec<String> = self.mine_sel.iter().cloned().collect();
        if names.is_empty() {
            return;
        }
        let mut files = Vec::new();
        for r in self
            .mine_rows
            .iter()
            .filter(|r| self.mine_sel.contains(&r.name))
        {
            if !self
                .rec_by_path
                .contains_key(&crate::registry::norm_path(&r.path))
            {
                files.extend(
                    transfer::own_files(&r.path)
                        .into_iter()
                        .map(|f| f.display().to_string()),
                );
            }
        }
        let mut text = format!(
            "{}\n\n{}",
            tr("Bu sözlükler dictz'ten kaldırılsın mı?"),
            names
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
        if !files.is_empty() {
            text += &format!(
                "\n\n{}\n{}",
                tr("“Dosyalarıyla sil” şunları da siler:"),
                files
                    .iter()
                    .take(20)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        self.ask = Some(Ask {
            title: tr("Kaldır"),
            text,
            yes: Next::RemoveMine(names.clone(), false),
            no: Next::Nothing,
            info: false,
            extra: Some((tr("Dosyalarıyla sil"), Next::RemoveMine(names, true))),
            texts: Some((tr("Listeden kaldır"), tr("İptal"))),
        });
    }

    fn do_remove_mine(&mut self, names: Vec<String>, with_files: bool) {
        let mut own = Vec::new();
        let mut rids = Vec::new();
        for r in self.mine_rows.iter().filter(|r| names.contains(&r.name)) {
            match self.rec_by_path.get(&crate::registry::norm_path(&r.path)) {
                Some((rid, _)) => rids.push(rid.clone()),
                None if with_files => own.extend(transfer::own_files(&r.path)),
                None => {}
            }
        }
        rids.sort();
        rids.dedup();
        // the main window drops them from the list (and closes them) first
        self.events.push(Event::Remove(names, own));
        self.pending_uninstall = rids;
        self.uninstall_delete = with_files;
        self.mine_sel.clear();
    }
}

// The files are deleted one frame later, after the main window has closed
// them (see `update`).
impl CenterWindow {
    fn finish_uninstall(&mut self) {
        let ids = std::mem::take(&mut self.pending_uninstall);
        if ids.is_empty() {
            return;
        }
        for i in &ids {
            self.installer.uninstall(i, self.uninstall_delete);
        }
        self.dirty = true;
    }
}

// ---------------------------------------------------------------- drawing

const COLUMNS: [&str; 10] = [
    "", "Ad", "Diller", "Tür", "Biçim", "Kelime", "Boyut", "Lisans", "Kaynak", "Tarih",
];

fn combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    items: &[(String, String)],
    width: f32,
) -> bool {
    let shown = items
        .iter()
        .find(|(k, _)| k == value)
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| items.first().map(|i| i.1.clone()).unwrap_or_default());
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(width)
        .height(400.0)
        .show_ui(ui, |ui| {
            for (k, l) in items {
                if ui.selectable_label(value == k, l).clicked() && value != k {
                    *value = k.clone();
                    changed = true;
                }
            }
        });
    changed
}

fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).weak());
}

fn button(ui: &mut egui::Ui, icon: &str, text: &str) -> egui::Response {
    match super::uiicons::image(ui.ctx(), icon, 14.0, ui.visuals().text_color()) {
        Some(img) => ui.add(egui::Button::image_and_text(img, text)),
        None => ui.button(text),
    }
}

/// `files` of the open dialog as text paths.
fn pick_save(title: &str, name: &str, filter: (&str, &[&str])) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(name)
        .add_filter(filter.0, filter.1)
        .save_file()
}

fn pick_open(title: &str, filter: (&str, &[&str])) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter.0, filter.1)
        .pick_file()
}

impl CenterWindow {
    /// Drain the threads' messages and, when `open`, draw the window.
    /// Returns what the main window should do.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        rows: Vec<MineRow>,
        badge: &mut dyn FnMut(&mut egui::Ui, &str),
    ) -> Vec<Event> {
        self.finish_uninstall();
        self.poll();
        if *open || !rows.is_empty() {
            self.mine_rows = rows;
        }
        if self.remove_next && !self.mine_rows.is_empty() {
            self.remove_next = false;
            self.remove_mine();
        }
        if *open {
            if !self.auto_refreshed {
                self.auto_refreshed = true;
                self.auto_refresh();
            }
            if self.dirty {
                self.apply_filter();
            }
            let mut keep = true;
            let screen = ctx.screen_rect();
            let size = egui::vec2(
                (screen.width() - 40.0).min(1180.0),
                (screen.height() - 90.0).min(720.0),
            );
            egui::Window::new(tr("Sözlükler"))
                .id(egui::Id::new("dictz-center"))
                .open(&mut keep)
                .collapsible(false)
                .default_size(size)
                .default_pos(screen.center() - size / 2.0)
                .constrain(true)
                .min_width(760.0)
                .min_height(420.0)
                .resizable(true)
                .show(ctx, |ui| self.body(ui, badge));
            if !keep {
                if self.queue.active() > 0 {
                    self.question(
                        tr("Sözlükler"),
                        tr("İndirmeler sürüyor. Pencere kapanınca arka planda devam etsinler mi?"),
                        Next::Close,
                        Next::Nothing,
                    );
                } else {
                    *open = false;
                }
            }
            self.dialogs(ctx, open);
            if self.refreshing || self.queue.active() > 0 {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        std::mem::take(&mut self.events)
    }

    fn body(&mut self, ui: &mut egui::Ui, badge: &mut dyn FnMut(&mut egui::Ui, &str)) {
        // rows are picked by clicking anywhere on them
        ui.style_mut().interaction.selectable_labels = false;
        // keep the window's size when a tab has less to show
        ui.set_min_size(ui.available_size());
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui.add_enabled_ui(!self.refreshing, |ui| {
                    button(ui, "refresh", &tr("Katalogları yenile"))
                });
                if r.inner.clicked() {
                    self.refresh(None);
                }
                if button(ui, "link", &tr("Bağlantıdan ekle…")).clicked() {
                    self.link = Some(String::new());
                }
                if self.refreshing {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(&self.status).weak()).truncate());
                });
            });
        });
        let active = self.queue.active();
        let tabs = [
            (
                "dict",
                format!("{} ({})", tr("Sözlüklerim"), self.mine_rows.len()),
            ),
            ("folder", tr("Klasörler")),
            ("center", tr("Katalog")),
            ("server", tr("Kaynaklar (sunucular)")),
            (
                "download",
                if active > 0 {
                    format!("{} ({})", tr("İndirmeler"), active)
                } else {
                    tr("İndirmeler")
                },
            ),
            ("upload", tr("İçe / Dışa aktar")),
            ("settings", tr("Ayarlar")),
        ];
        ui.horizontal(|ui| {
            for (i, (icon, label)) in tabs.iter().enumerate() {
                let img = super::uiicons::image(ui.ctx(), icon, 14.0, ui.visuals().text_color());
                let b = match img {
                    Some(img) => egui::Button::image_and_text(img, label.as_str()),
                    None => egui::Button::new(label.as_str()),
                };
                if ui.add(b.selected(self.tab == i)).clicked() {
                    self.tab = i;
                }
            }
        });
        ui.separator();
        match self.tab {
            TAB_MINE => self.mine_tab(ui, badge),
            TAB_FOLDERS => self.folders_tab(ui),
            TAB_CATALOG => self.catalog_tab(ui),
            TAB_SOURCES => self.sources_tab(ui),
            TAB_DOWNLOADS => self.downloads_tab(ui),
            TAB_TRANSFER => self.transfer_tab(ui),
            _ => self.settings_tab(ui),
        }
    }

    // ------------------------------------------------------------ catalog tab

    fn catalog_tab(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr("Ara: ad, açıklama, yazar, etiket…"))
                    .desired_width(300.0),
            );
            changed |= r.changed();
            if !self.search.is_empty() && ui.small_button("✕").clicked() {
                self.search.clear();
                changed = true;
            }
            ui.label(tr("Kaynak dil"));
            let mut from = vec![(String::new(), tr("Herhangi"))];
            from.extend(self.from_list.iter().cloned());
            changed |= combo(ui, "c-from", &mut self.from, &from, 170.0);
            if ui
                .button("⇄")
                .on_hover_text(tr("Dilleri değiştir"))
                .clicked()
            {
                std::mem::swap(&mut self.from, &mut self.to);
                changed = true;
            }
            ui.label(tr("Hedef dil"));
            let mut to = vec![(String::new(), tr("Herhangi"))];
            to.extend(self.to_list.iter().cloned());
            changed |= combo(ui, "c-to", &mut self.to, &to, 170.0);
        });
        ui.horizontal(|ui| {
            let mut srcs = vec![(String::new(), tr("Tüm kaynaklar"))];
            srcs.extend(self.sources.enabled().into_iter().map(|s| (s.id, s.name)));
            changed |= combo(ui, "c-src", &mut self.source, &srcs, 200.0);
            let lics = vec![
                ("all".to_string(), tr("Tüm lisanslar")),
                ("open".into(), tr("Açık lisans")),
                ("free".into(), tr("Açık + serbest")),
                ("unknown".into(), tr("Lisansı bilinmeyen")),
                ("nonfree".into(), tr("Kişisel kullanım")),
            ];
            changed |= combo(ui, "c-lic", &mut self.license, &lics, 160.0);
            let mut fmts = vec![(String::new(), tr("Tüm biçimler"))];
            fmts.extend(self.formats.iter().map(|f| (f.clone(), f.clone())));
            changed |= combo(ui, "c-fmt", &mut self.fmt, &fmts, 120.0);
            changed |= ui
                .checkbox(&mut self.supported_only, tr("Yalnızca açılabilenler"))
                .changed();
            changed |= ui
                .checkbox(&mut self.hide_installed, tr("Kurulu olanları gizle"))
                .changed();
        });
        if changed {
            self.apply_filter();
        }
        let bottom = 30.0;
        let avail = ui.available_size();
        let detail_w = 340.0_f32.min(avail.x * 0.4);
        let table_w = avail.x - detail_w - 10.0;
        let h = (avail.y - bottom).max(120.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(table_w, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(table_w);
                    self.catalog_table(ui, h);
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(detail_w, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(detail_w);
                    self.detail(ui, h);
                },
            );
        });
        muted(
            ui,
            tr_args(
                "{n} / {m} sözlük gösteriliyor · {k} kurulu",
                &[
                    ("n", &self.rows.len()),
                    ("m", &self.entries.len()),
                    ("k", &self.installed.len()),
                ],
            ),
        );
    }

    fn catalog_table(&mut self, ui: &mut egui::Ui, h: f32) {
        let widths = [22.0, 0.0, 76.0, 66.0, 70.0, 62.0, 64.0, 112.0, 100.0, 78.0];
        let mut tb = TableBuilder::new(ui)
            .id_salt("center-catalog")
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .max_scroll_height(h - 24.0)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        for (i, w) in widths.iter().enumerate() {
            tb = tb.column(if i == 1 {
                Column::initial(260.0).at_least(120.0).clip(true)
            } else if i == widths.len() - 1 {
                Column::remainder().at_least(*w).clip(true)
            } else {
                Column::initial(*w).at_least(20.0).clip(true)
            });
        }
        let mut sort_click = None;
        let mut clicked: Option<(usize, egui::Modifiers)> = None;
        let mut double = false;
        let rows = self.rows.clone();
        let selected = self.selected.clone();
        tb.header(20.0, |mut head| {
            for (i, c) in COLUMNS.iter().enumerate() {
                head.col(|ui| {
                    let mut label = if c.is_empty() { String::new() } else { tr(c) };
                    if let Some((col, asc)) = self.sort {
                        if col == i {
                            label += if asc { " ▲" } else { " ▼" };
                        }
                    }
                    if ui.add(egui::Label::new(RichText::new(label).strong()).sense(egui::Sense::click())).clicked() {
                        sort_click = Some(i);
                    }
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows.len(), |mut row| {
                let ri = row.index();
                let e = &self.entries[rows[ri]];
                row.set_selected(selected.contains(&e.id));
                let lic = licenses::get(&e.license);
                let mark = if self.updates.contains(&e.id) {
                    "↑"
                } else if self.installed.contains_key(&e.id) {
                    "✓"
                } else {
                    ""
                };
                row.col(|ui| {
                    let r = ui.label(mark);
                    if !mark.is_empty() {
                        r.on_hover_text(if mark == "↑" { tr("güncelleme var") } else { tr("kurulu") });
                    }
                });
                row.col(|ui| {
                    ui.add(egui::Label::new(&e.name).truncate()).on_hover_text(&e.name);
                });
                row.col(|ui| {
                    ui.label(dash(&pair_text(e)));
                });
                row.col(|ui| {
                    ui.label(kind_text(&e.kind));
                });
                row.col(|ui| {
                    let f = format!("{}{}", e.format, if e.supported == Some(false) { " *" } else { "" });
                    let r = ui.label(f);
                    if e.supported == Some(false) {
                        r.on_hover_text(tr("dictz bu biçimi henüz açamıyor; indirilebilir, sonraki sürümler açacak"));
                    }
                });
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(if e.headwords > 0 { group(e.headwords) } else { "–".into() });
                    });
                });
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(human_size(e.size));
                    });
                });
                row.col(|ui| {
                    ui.label(RichText::new(&lic.id).color(status_color(&lic.status)))
                        .on_hover_text(format!("{} — {}", license_name(&lic), status_text(&lic.status)));
                });
                row.col(|ui| {
                    ui.label(&e.source);
                });
                row.col(|ui| {
                    ui.label(dash(&e.date));
                });
                let r = row.response();
                if r.clicked() {
                    clicked = Some((ri, ui_modifiers(&r)));
                }
                if r.double_clicked() {
                    double = true;
                    clicked = Some((ri, egui::Modifiers::NONE));
                }
            });
        });
        if let Some(i) = sort_click {
            self.sort = match self.sort {
                Some((c, asc)) if c == i => Some((i, !asc)),
                _ => Some((i, true)),
            };
            self.apply_filter();
        }
        if let Some((ri, m)) = clicked {
            let id = self.entries[self.rows[ri]].id.clone();
            if m.command {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.anchor = Some(ri);
            } else if m.shift && self.anchor.is_some() {
                let a = self.anchor.unwrap_or(ri);
                let (lo, hi) = (a.min(ri), a.max(ri));
                self.selected = (lo..=hi)
                    .map(|k| self.entries[self.rows[k]].id.clone())
                    .collect();
            } else {
                self.selected = [id].into_iter().collect();
                self.anchor = Some(ri);
            }
        }
        if double {
            self.install_selected();
        }
    }

    fn install_selected(&mut self) {
        let sel = self.selected_entries();
        if !sel.is_empty() {
            self.install_flow(sel, 0);
        }
    }

    fn detail(&mut self, ui: &mut egui::Ui, h: f32) {
        let sel = self.selected_entries();
        egui::ScrollArea::vertical()
            .id_salt("center-detail")
            .max_height(h - 34.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if sel.is_empty() {
                    muted(ui, tr("Bir sözlük seçin."));
                } else if sel.len() > 1 {
                    ui.heading(tr_args("{n} sözlük seçili", &[("n", &sel.len())]));
                    let total: u64 = sel.iter().map(|e| e.size).sum();
                    ui.label(format!("{}: {}", tr("Toplam boyut"), human_size(total)));
                } else {
                    entry_view(ui, &sel[0], self.installed.get(&sel[0].id));
                }
            });
        ui.horizontal_wrapped(|ui| {
            if button(ui, "download", &tr("Kur")).clicked() {
                self.install_selected();
            }
            if button(ui, "remove", &tr("Kaldır")).clicked() {
                let ids: Vec<String> = sel
                    .iter()
                    .filter(|e| self.installed.contains_key(&e.id))
                    .map(|e| e.id.clone())
                    .collect();
                self.uninstall(ids);
            }
            if button(ui, "open", &tr("Sayfası")).clicked() {
                for e in sel.iter().take(5) {
                    let url = [
                        e.page.clone(),
                        e.homepage.clone(),
                        e.files.first().map(|f| f.url.clone()).unwrap_or_default(),
                    ]
                    .into_iter()
                    .find(|u| !u.is_empty());
                    if let Some(u) = url {
                        let _ = open_external(&u);
                    }
                }
            }
            if button(ui, "copy", &tr("Bağlantıyı kopyala")).clicked() {
                let mut links: Vec<String> = sel
                    .iter()
                    .flat_map(|e| e.files.iter().map(|f| f.url.clone()))
                    .collect();
                if links.is_empty() {
                    links = sel.iter().map(|e| e.page.clone()).collect();
                }
                links.retain(|l| !l.is_empty());
                ui.ctx().copy_text(links.join("\n"));
            }
        });
    }

    // ------------------------------------------------------------ sources tab

    fn sources_tab(&mut self, ui: &mut egui::Ui) {
        muted(ui, tr("Sözlükler bu kaynaklardan okunur; sıra katalogdaki sırayı belirler. Varsayılan sunucu (★) dictz'in kendi kataloğudur. Her adres değiştirilebilir; kendi sunucunuzu, bir klasörü ya da bir paylaşım bağlantısını ekleyebilirsiniz."));
        let rows = self.sources.all();
        let default = self.sources.default_id();
        let h = (ui.available_height() - 40.0).max(120.0);
        let mut toggled: Option<(String, bool)> = None;
        let mut edit = false;
        TableBuilder::new(ui)
            .id_salt("center-sources")
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .max_scroll_height(h)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(44.0))
            .column(Column::initial(230.0).clip(true))
            .column(Column::initial(150.0).clip(true))
            .column(Column::initial(64.0))
            .column(Column::initial(90.0))
            .column(Column::initial(220.0).clip(true))
            .column(Column::remainder().clip(true))
            .header(20.0, |mut head| {
                for c in ["Açık", "Ad", "Tür", "Sözlük", "Son okuma", "Durum", "Adres"] {
                    head.col(|ui| {
                        ui.strong(tr(c));
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, rows.len(), |mut row| {
                    let s = &rows[row.index()];
                    let c = self
                        .cached
                        .get(&s.id)
                        .cloned()
                        .unwrap_or_else(|| self.catalog.cached(&s.id));
                    row.set_selected(self.src_sel.as_deref() == Some(s.id.as_str()));
                    let lic = if s.license.is_empty() {
                        String::new()
                    } else {
                        licenses::get(&s.license).id
                    };
                    let mut tip = s.description.clone();
                    if !lic.is_empty() {
                        tip += &format!("\n{}: {}", tr("Lisans"), lic);
                    }
                    if !s.homepage.is_empty() {
                        tip += &format!("\n{}", s.homepage);
                    }
                    let tip = tip.trim().to_string();
                    row.col(|ui| {
                        let mut on = s.enabled;
                        if ui.checkbox(&mut on, "").changed() {
                            toggled = Some((s.id.clone(), on));
                        }
                    });
                    row.col(|ui| {
                        let star = if s.id == default { "★ " } else { "" };
                        ui.add(egui::Label::new(format!("{}{}", star, s.name)).truncate())
                            .on_hover_text(&tip);
                    });
                    row.col(|ui| {
                        ui.label(type_text(&s.kind)).on_hover_text(&tip);
                    });
                    row.col(|ui| {
                        ui.label(c.entries.len().to_string());
                    });
                    row.col(|ui| {
                        ui.label(dash(&take(&c.fetched, 10)));
                    });
                    row.col(|ui| {
                        let state = if c.ok {
                            tr("tamam")
                        } else if !c.error.is_empty() {
                            format!("{}: {}", tr("hata"), c.error)
                        } else {
                            tr("hiç okunmadı")
                        };
                        let t = if c.error.is_empty() {
                            RichText::new(&state)
                        } else {
                            RichText::new(&state).color(RED)
                        };
                        ui.add(egui::Label::new(t).truncate()).on_hover_text(&state);
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(&s.url).truncate())
                            .on_hover_text(&tip);
                    });
                    let r = row.response();
                    if r.clicked() {
                        self.src_sel = Some(s.id.clone());
                    }
                    if r.double_clicked() {
                        self.src_sel = Some(s.id.clone());
                        edit = true;
                    }
                });
            });
        if let Some((id, on)) = toggled {
            self.sources.set_enabled(&id, on);
            self.reload_all();
        }
        let cur = self.src_sel.clone().and_then(|id| self.sources.get(&id));
        ui.horizontal_wrapped(|ui| {
            if button(ui, "new", &tr("Ekle…")).clicked() {
                self.editor = Some(SourceDialog::new(&self.sources, None, ""));
            }
            if button(ui, "settings", &tr("Düzenle…")).clicked() {
                edit = true;
            }
            if button(ui, "remove", &tr("Sil")).clicked() {
                if let Some(s) = &cur {
                    self.question(
                        tr("Sil"),
                        tr_args("“{name}” kaynağı silinsin mi? (Kurulu sözlükler kalır.)", &[("name", &s.name)]),
                        Next::RemoveSource(s.id.clone()),
                        Next::Nothing,
                    );
                }
            }
            for (icon, text, d) in [("up", tr("Yukarı"), -1), ("down", tr("Aşağı"), 1)] {
                if button(ui, icon, &text).clicked() {
                    if let Some(s) = &cur {
                        self.sources.move_by(&s.id, d);
                        self.reload_all();
                    }
                }
            }
            if button(ui, "star", &tr("Varsayılan sunucu yap")).clicked() {
                if let Some(s) = &cur {
                    self.sources.set_default(&s.id);
                    self.reload_all();
                }
            }
            if button(ui, "refresh", &tr("Yenile")).clicked() {
                if let Some(s) = &cur {
                    self.refresh(Some(vec![s.id.clone()]));
                }
            }
            ui.separator();
            if button(ui, "upload", &tr("Dışa aktar…")).clicked() {
                if let Some(p) = pick_save(&tr("Kaynak listesini dışa aktar"), "dictz-sources.json", ("JSON", &["json"])) {
                    match self.sources.export(&p, true) {
                        Ok(n) => self.status = tr_args("{n} kaynak dışa aktarıldı", &[("n", &n)]),
                        Err(e) => self.info(tr("Olmadı"), e.to_string()),
                    }
                }
            }
            if button(ui, "download", &tr("İçe aktar…")).clicked() {
                if let Some(p) = pick_open(&tr("Kaynak listesini içe aktar"), ("JSON", &["json"])) {
                    match self.sources.import(&p, false) {
                        Ok(n) => {
                            self.reload_all();
                            self.status = tr_args("{n} kaynak içe aktarıldı", &[("n", &n)]);
                        }
                        Err(e) => self.info(tr("İçe aktar"), e),
                    }
                }
            }
            if button(ui, "refresh", &tr("Varsayılanlara dön")).clicked() {
                self.question(
                    tr("Varsayılanlara dön"),
                    tr("Yerleşik kaynaklar ilk hallerine dönsün mü? (Sizin eklediğiniz kaynaklar kalır.)"),
                    Next::ResetSources,
                    Next::Nothing,
                );
            }
        });
        if edit {
            if let Some(s) = cur {
                self.editor = Some(SourceDialog::new(&self.sources, Some(&s), ""));
            }
        }
    }

    // ------------------------------------------------------------ downloads tab

    fn state_text(t: &Task) -> String {
        if t.state == "running" {
            return match t.stage.as_str() {
                "check" => tr("denetleniyor"),
                "unpack" => tr("açılıyor"),
                _ => tr("iniyor"),
            };
        }
        match t.state.as_str() {
            "queued" => tr("sırada"),
            "paused" => tr("duraklatıldı"),
            "done" => tr("kuruldu"),
            "failed" => tr("olmadı"),
            "cancelled" => tr("iptal edildi"),
            s => s.to_string(),
        }
    }

    fn downloads_tab(&mut self, ui: &mut egui::Ui) {
        let tasks = self.queue.snapshot();
        let h = ((ui.available_height() - 80.0) * 0.6).max(100.0);
        TableBuilder::new(ui)
            .id_salt("center-downloads")
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .max_scroll_height(h)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(300.0).clip(true))
            .column(Column::initial(100.0))
            .column(Column::initial(180.0))
            .column(Column::initial(90.0))
            .column(Column::initial(90.0))
            .column(Column::remainder().clip(true))
            .header(20.0, |mut head| {
                for c in ["Ad", "Durum", "İlerleme", "Boyut", "Hız", "Hata"] {
                    head.col(|ui| {
                        ui.strong(tr(c));
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, tasks.len(), |mut row| {
                    let t = &tasks[row.index()];
                    row.set_selected(self.dl_sel.contains(&t.id));
                    row.col(|ui| {
                        ui.add(egui::Label::new(&t.entry.name).truncate());
                    });
                    row.col(|ui| {
                        ui.label(Self::state_text(t));
                    });
                    row.col(|ui| {
                        let frac = if t.state == "done" {
                            1.0
                        } else if t.total > 0 {
                            t.done as f32 / t.total as f32
                        } else {
                            0.0
                        };
                        let mut bar = egui::ProgressBar::new(frac)
                            .desired_height(14.0)
                            .show_percentage();
                        if t.state == "running" && t.total == 0 {
                            bar = egui::ProgressBar::new(0.0)
                                .desired_height(14.0)
                                .animate(true);
                        }
                        ui.add(bar);
                    });
                    row.col(|ui| {
                        ui.label(human_size(if t.total > 0 { t.total } else { t.done }));
                    });
                    row.col(|ui| {
                        if t.state == "running" && t.speed > 0.0 {
                            ui.label(format!("{}/s", human_size(t.speed as u64)));
                        }
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(RichText::new(&t.error).color(RED)).truncate())
                            .on_hover_text(&t.error);
                    });
                    if row.response().clicked() {
                        let m = ui_modifiers(&row.response());
                        if m.command {
                            if !self.dl_sel.remove(&t.id) {
                                self.dl_sel.insert(t.id);
                            }
                        } else {
                            self.dl_sel = [t.id].into_iter().collect();
                        }
                    }
                });
            });
        ui.horizontal(|ui| {
            let sel: Vec<usize> = self.dl_sel.iter().copied().collect();
            if button(ui, "close", &tr("Duraklat")).clicked() {
                sel.iter().for_each(|&i| self.queue.pause(i));
            }
            if button(ui, "download", &tr("Sürdür")).clicked() {
                sel.iter().for_each(|&i| self.queue.resume(i));
            }
            if button(ui, "remove", &tr("İptal")).clicked() {
                sel.iter().for_each(|&i| self.queue.cancel(i));
            }
            if button(ui, "refresh", &tr("Yeniden dene")).clicked() {
                sel.iter().for_each(|&i| self.queue.resume(i));
            }
            if ui.button(tr("Bitenleri temizle")).clicked() {
                self.queue.clear_finished();
                self.dl_sel.clear();
            }
        });
        ui.add_space(6.0);
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong(tr("Geçmiş"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Geçmişi temizle")).clicked() {
                        self.installer.clear_history();
                        self.history.clear();
                    }
                });
            });
            let h = (ui.available_height() - 8.0).max(80.0);
            let hist = self.history.clone();
            TableBuilder::new(ui)
                .id_salt("center-history")
                .striped(true)
                .resizable(true)
                .max_scroll_height(h)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(140.0))
                .column(Column::initial(80.0))
                .column(Column::initial(300.0).clip(true))
                .column(Column::initial(130.0).clip(true))
                .column(Column::remainder().clip(true))
                .header(20.0, |mut head| {
                    for c in ["Zaman", "Durum", "Ad", "Lisans", "Hata"] {
                        head.col(|ui| {
                            ui.strong(tr(c));
                        });
                    }
                })
                .body(|body| {
                    body.rows(20.0, hist.len(), |mut row| {
                        let h = &hist[row.index()];
                        let status = vs(h, "status");
                        let vals = [
                            take(&vs(h, "finished"), 16).replace('T', " "),
                            match status.as_str() {
                                "done" => tr("kuruldu"),
                                "failed" => tr("olmadı"),
                                _ => status.clone(),
                            },
                            vs(h, "name"),
                            vs(h, "license"),
                            vs(h, "error"),
                        ];
                        for v in vals {
                            row.col(|ui| {
                                ui.add(egui::Label::new(&v).truncate()).on_hover_text(&v);
                            });
                        }
                    });
                });
        });
    }

    // ------------------------------------------------------------ installed tab

    fn mine_tab(&mut self, ui: &mut egui::Ui, badge: &mut dyn FnMut(&mut egui::Ui, &str)) {
        muted(
            ui,
            tr("Arama sonuçlarındaki sıra budur: satırları sürükleyip bırakarak sıralayın. Kutu = etkin. Çift tık: bilgiler."),
        );
        let rows = self.mine_rows.clone();
        let src_names: HashMap<String, String> = self
            .sources
            .all()
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect();
        let h = (ui.available_height() - 64.0).max(120.0);
        let mut drop: Option<(String, usize)> = None;
        let mut clicked: Option<(usize, egui::Modifiers)> = None;
        let mut double: Option<String> = None;
        let mut menu_events: Vec<Event> = Vec::new();
        let mut menu_remove = false;
        let mut menu_move: Option<bool> = None;
        TableBuilder::new(ui)
            .id_salt("center-mine")
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .max_scroll_height(h)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(28.0))
            .column(Column::initial(290.0).clip(true))
            .column(Column::initial(70.0))
            .column(Column::initial(80.0))
            .column(Column::initial(90.0).clip(true))
            .column(Column::initial(110.0).clip(true))
            .column(Column::initial(110.0).clip(true))
            .column(Column::initial(80.0).clip(true))
            .column(Column::remainder().clip(true))
            .header(20.0, |mut head| {
                for c in [
                    "",
                    "Ad",
                    "Biçim",
                    "Kelime",
                    "Diller",
                    "Kaynak",
                    "Lisans",
                    "Güncelleme",
                    "Dosya",
                ] {
                    head.col(|ui| {
                        ui.strong(if c.is_empty() { String::new() } else { tr(c) });
                    });
                }
            })
            .body(|body| {
                body.rows(24.0, rows.len(), |mut row| {
                    let ri = row.index();
                    let r = &rows[ri];
                    let rec = self.rec_by_path.get(&crate::registry::norm_path(&r.path));
                    let up = rec.and_then(|(rid, _)| self.update_by_id.get(rid));
                    row.set_selected(self.mine_sel.contains(&r.name));
                    row.col(|ui| {
                        let mut on = r.enabled;
                        if ui.checkbox(&mut on, "").changed() {
                            menu_events.push(Event::SetEnabled(r.name.clone(), on));
                        }
                    });
                    row.col(|ui| {
                        if r.loaded {
                            let id = egui::Id::new(("mine-dnd", &r.name));
                            ui.dnd_drag_source(id, MineDrag(r.name.clone()), |ui| {
                                ui.horizontal(|ui| {
                                    badge(ui, &r.name);
                                    ui.add(egui::Label::new(&r.name).truncate());
                                });
                            });
                        } else {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("⚠ {} — {}", r.name, tr("açılamadı")))
                                        .color(RED),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&r.error);
                        }
                    });
                    row.col(|ui| {
                        ui.label(&r.format);
                    });
                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(group(r.words));
                        });
                    });
                    row.col(|ui| {
                        ui.label(&r.langs);
                    });
                    row.col(|ui| {
                        let src = match rec {
                            Some((_, v)) => {
                                let id = vs(v, "source");
                                src_names.get(&id).cloned().unwrap_or(id)
                            }
                            None => tr("yerel"),
                        };
                        ui.label(src);
                    });
                    row.col(|ui| {
                        ui.label(rec.map(|(_, v)| vs(v, "license")).unwrap_or_default());
                    });
                    row.col(|ui| {
                        if let Some(e) = up {
                            let v = if e.version.is_empty() {
                                e.date.clone()
                            } else {
                                e.version.clone()
                            };
                            ui.label(RichText::new(v).color(GREEN));
                        }
                    });
                    row.col(|ui| {
                        let p = r.path.display().to_string();
                        ui.add(egui::Label::new(RichText::new(&p).weak()).truncate())
                            .on_hover_text(&p);
                    });
                    let resp = row.response();
                    if let Some(p) = resp.dnd_hover_payload::<MineDrag>() {
                        if p.0 != r.name {
                            let y = resp.rect.top();
                            ui_line(&resp, y);
                        }
                    }
                    if let Some(p) = resp.dnd_release_payload::<MineDrag>() {
                        drop = Some((p.0.clone(), ri));
                    }
                    if resp.clicked() {
                        clicked = Some((ri, resp.ctx.input(|i| i.modifiers)));
                    }
                    if resp.double_clicked() {
                        double = Some(r.name.clone());
                    }
                    resp.context_menu(|ui| {
                        if !self.mine_sel.contains(&r.name) {
                            self.mine_sel = [r.name.clone()].into_iter().collect();
                        }
                        if ui.button(tr("Bilgi")).clicked() {
                            menu_events.push(Event::Info(r.name.clone()));
                            ui.close_menu();
                        }
                        if ui.button(tr("Yalnızca bunu etkinleştir")).clicked() {
                            menu_events.push(Event::Solo(self.mine_sel.iter().cloned().collect()));
                            ui.close_menu();
                        }
                        if ui.button(tr("Yeniden indeksle")).clicked() {
                            menu_events
                                .push(Event::Reindex(self.mine_sel.iter().cloned().collect()));
                            ui.close_menu();
                        }
                        if ui.button(tr("Klasörü aç")).clicked() {
                            if let Some(d) = r.path.parent() {
                                let _ = open_external(&d.display().to_string());
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(tr("En üste")).clicked() {
                            menu_move = Some(true);
                            ui.close_menu();
                        }
                        if ui.button(tr("En alta")).clicked() {
                            menu_move = Some(false);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(tr("Kaldır…")).clicked() {
                            menu_remove = true;
                            ui.close_menu();
                        }
                    });
                });
            });
        self.events.append(&mut menu_events);
        if let Some((name, at)) = drop {
            let mut order: Vec<String> = rows
                .iter()
                .filter(|r| r.loaded)
                .map(|r| r.name.clone())
                .collect();
            let target = rows.get(at).map(|r| r.name.clone());
            order.retain(|n| *n != name);
            let j = target
                .and_then(|t| order.iter().position(|n| *n == t))
                .unwrap_or(order.len());
            order.insert(j, name);
            self.events.push(Event::SetOrder(order));
        }
        if let Some((ri, m)) = clicked {
            let name = rows[ri].name.clone();
            if m.command {
                if !self.mine_sel.remove(&name) {
                    self.mine_sel.insert(name);
                }
            } else if m.shift && !self.mine_sel.is_empty() {
                let first = rows
                    .iter()
                    .position(|r| self.mine_sel.contains(&r.name))
                    .unwrap_or(ri);
                let (lo, hi) = (first.min(ri), first.max(ri));
                self.mine_sel = rows[lo..=hi].iter().map(|r| r.name.clone()).collect();
            } else {
                self.mine_sel = [name].into_iter().collect();
            }
        }
        if let Some(n) = double {
            self.events.push(Event::Info(n));
        }
        if let Some(top) = menu_move {
            self.move_mine(if top { "top" } else { "bottom" });
        }
        if menu_remove {
            self.remove_mine();
        }
        ui.horizontal_wrapped(|ui| {
            if button(ui, "open", &tr("Dosya ekle…")).clicked() {
                self.events.push(Event::AddFiles);
            }
            if button(ui, "folder", &tr("Klasör ekle…")).clicked() {
                self.events.push(Event::AddFolder);
            }
            if button(ui, "link", &tr("Bağlantıdan ekle…")).clicked() {
                self.link = Some(String::new());
            }
            if button(ui, "remove", &tr("Kaldır…")).clicked() {
                self.remove_mine();
            }
            ui.separator();
            for (icon, text, how) in [
                ("up", tr("En üste"), "top"),
                ("up", tr("Yukarı"), "up"),
                ("down", tr("Aşağı"), "down"),
                ("down", tr("En alta"), "bottom"),
            ] {
                if button(ui, icon, &text).clicked() {
                    self.move_mine(how);
                }
            }
            ui.separator();
            let sel: Vec<String> = self.mine_sel.iter().cloned().collect();
            if button(ui, "help", &tr("Bilgi")).clicked() {
                if let Some(n) = sel.first() {
                    self.events.push(Event::Info(n.clone()));
                }
            }
            if button(ui, "refresh", &tr("Yeniden indeksle")).clicked() && !sel.is_empty() {
                self.events.push(Event::Reindex(sel.clone()));
            }
            if button(ui, "folder", &tr("Klasörü aç")).clicked() {
                for r in rows
                    .iter()
                    .filter(|r| self.mine_sel.contains(&r.name))
                    .take(3)
                {
                    if let Some(d) = r.path.parent() {
                        let _ = open_external(&d.display().to_string());
                    }
                }
            }
            if button(ui, "download", &tr("Güncelle")).clicked() {
                let mut todo = Vec::new();
                for r in rows.iter().filter(|r| self.mine_sel.contains(&r.name)) {
                    if let Some((rid, _)) =
                        self.rec_by_path.get(&crate::registry::norm_path(&r.path))
                    {
                        if let Some(e) = self.update_by_id.get(rid) {
                            todo.push(e.clone());
                        }
                    }
                }
                self.queue_updates(todo);
            }
            if button(ui, "refresh", &tr("Hepsini güncelle")).clicked() {
                let todo: Vec<Entry> = self.update_by_id.values().cloned().collect();
                self.queue_updates(todo);
            }
        });
        let enabled: Vec<&MineRow> = rows.iter().filter(|r| r.enabled && r.loaded).collect();
        let words: u64 = enabled.iter().map(|r| r.words).sum();
        muted(
            ui,
            tr_args(
                "{n} / {m} sözlük etkin · {w} kelime",
                &[
                    ("n", &enabled.len()),
                    ("m", &rows.len()),
                    ("w", &group(words)),
                ],
            ),
        );
    }

    fn queue_updates(&mut self, todo: Vec<Entry>) {
        if todo.is_empty() {
            self.info(tr("Güncelle"), tr("Kurulu sözlüklerin hepsi güncel."));
            return;
        }
        for e in todo {
            self.queue.add(e);
        }
        self.tab = TAB_DOWNLOADS;
    }

    fn move_mine(&mut self, how: &str) {
        let mut order: Vec<String> = self
            .mine_rows
            .iter()
            .filter(|r| r.loaded)
            .map(|r| r.name.clone())
            .collect();
        let sel: Vec<String> = order
            .iter()
            .filter(|n| self.mine_sel.contains(*n))
            .cloned()
            .collect();
        if sel.is_empty() {
            return;
        }
        match how {
            "top" => {
                order.retain(|n| !sel.contains(n));
                order.splice(0..0, sel);
            }
            "bottom" => {
                order.retain(|n| !sel.contains(n));
                order.extend(sel);
            }
            _ => {
                let d: isize = if how == "up" { -1 } else { 1 };
                let names: Vec<String> = if d < 0 {
                    sel
                } else {
                    sel.into_iter().rev().collect()
                };
                for n in names {
                    let i = order.iter().position(|x| *x == n).unwrap_or(0);
                    let j = (i as isize + d).clamp(0, order.len() as isize - 1) as usize;
                    let x = order.remove(i);
                    order.insert(j, x);
                }
            }
        }
        self.events.push(Event::SetOrder(order));
    }

    fn folders_tab(&mut self, ui: &mut egui::Ui) {
        muted(
            ui,
            format!(
                "{} {}",
                tr("Açılışta bu klasörlerdeki .ifo / .dsl / .dsl.dz / .bgl dosyaları otomatik eklenir."),
                tr("Kutu = alt klasörleri de tara. Bir klasörü kaldırmak sözlüklerini listeden çıkarmaz.")
            ),
        );
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("center-folders")
            .show(ui, |ui| {
                self.settings_ed.folder_sources(ui);
            });
        self.settings_changed();
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let h = (ui.available_height() - 34.0).max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("center-settings")
            .max_height(h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.settings_ed
                    .form(ui, PAGES_IN_DICTIONARIES_WINDOW, &["dictionaries.folders"]);
            });
        if ui.button(tr("Bu sayfayı varsayılana döndür")).clicked() {
            self.settings_ed
                .reset_sections(PAGES_IN_DICTIONARIES_WINDOW, &["dictionaries.folders"]);
        }
        self.settings_changed();
    }

    /// Send an edit of the folders / settings to the main window at once.
    fn settings_changed(&mut self) {
        for e in self.settings_ed.take_events() {
            if let super::prefs_window::Event::Action(super::prefs_window::Advanced::Rescan) = e {
                self.events.push(Event::Rescan);
            }
        }
        if self.settings_ed.draft.export() != self.prefs.export() {
            self.prefs = self.settings_ed.draft.clone();
            self.events.push(Event::PrefsChanged(self.prefs.clone()));
        }
    }

    // ------------------------------------------------------------ import / export tab

    fn transfer_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("center-transfer").show(ui, |ui| {
            ui.group(|ui| {
                ui.strong(tr("Kurulumu dışa aktar (başka bilgisayara taşımak için)"));
                ui.horizontal_wrapped(|ui| {
                    let labels = [
                        tr("Sözlük listesi"),
                        tr("Ayarlar"),
                        tr("Kaynaklar (parolasız)"),
                        tr("Kurulum kayıtları"),
                        tr("Geçmiş ve favoriler"),
                        tr("Sözlük dosyaları"),
                    ];
                    for (i, l) in labels.iter().enumerate() {
                        ui.checkbox(&mut self.ex_parts[i], l);
                    }
                });
                if button(ui, "upload", &tr("Dışa aktar…")).clicked() {
                    if let Some(p) = pick_save(&tr("Kurulumu dışa aktar"), "dictz-setup.zip", ("ZIP", &["zip"])) {
                        let parts: Vec<&'static str> = transfer::PARTS
                            .iter()
                            .zip(self.ex_parts)
                            .filter(|(_, on)| *on)
                            .map(|(p, _)| *p)
                            .collect();
                        let dir = self.data_dir.clone();
                        self.run(move || {
                            transfer::export_setup(&p, &dir, &parts).map(|m| {
                                Work::Export(m["dictionaries"].as_array().map_or(0, Vec::len))
                            })
                        });
                    }
                }
            });
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong(tr("Kurulumu içe aktar"));
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut self.im_replace, false, tr("Birleştir (var olanlar kalır)"));
                    ui.radio_value(
                        &mut self.im_replace,
                        true,
                        tr("Değiştir (liste, kaynaklar ve geçmiş arşivdeki olur)"),
                    );
                    if button(ui, "download", &tr("İçe aktar…")).clicked() {
                        self.import_setup();
                    }
                });
            });
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong(tr("Sözlük listesini dışa aktar (lisanslarıyla)"));
                ui.horizontal(|ui| {
                    for fmt in ["csv", "json", "txt"] {
                        if ui.button(format!("{}…", fmt.to_uppercase())).clicked() {
                            let up = fmt.to_uppercase();
                            if let Some(p) = pick_save(
                                &tr("Sözlük listesini dışa aktar"),
                                &format!("dictz-list.{}", fmt),
                                (&up, &[fmt]),
                            ) {
                                match transfer::export_list(&p, &self.data_dir, fmt) {
                                    Ok(n) => self.status = tr_args("{n} sözlük dışa aktarıldı", &[("n", &n)]),
                                    Err(e) => self.info(tr("Olmadı"), e),
                                }
                            }
                        }
                    }
                });
            });
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong(tr("Kendi sunucun için katalog oluştur"));
                egui::Grid::new("center-catalog-build").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(tr("Klasör"));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.cb_folder).desired_width(420.0));
                        if button(ui, "folder", "").clicked() {
                            if let Some(d) = rfd::FileDialog::new().set_title(tr("Sözlüklerin klasörü")).pick_folder() {
                                self.cb_folder = d.to_string_lossy().into_owned();
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Dosyaların adresi"));
                    ui.add(egui::TextEdit::singleline(&mut self.cb_url).desired_width(460.0));
                    ui.end_row();
                    ui.label(tr("Katalog adı"));
                    ui.add(egui::TextEdit::singleline(&mut self.cb_name).desired_width(260.0));
                    ui.end_row();
                    ui.label(tr("Varsayılan lisans"));
                    let mut items = vec![(String::new(), tr("(her sözlüğün kendi lisansı)"))];
                    items.extend(licenses::all().into_iter().map(|l| (l.id.clone(), format!("{} — {}", l.id, l.name))));
                    combo(ui, "cb-license", &mut self.cb_license, &items, 360.0);
                    ui.end_row();
                    ui.label("");
                    ui.horizontal(|ui| {
                        if button(ui, "save", &tr("catalog.json oluştur")).clicked() {
                            self.build_catalog_file();
                        }
                        if button(ui, "license", &tr("Kataloğu denetle…")).clicked() {
                            self.check_catalog_file();
                        }
                    });
                    ui.end_row();
                });
                muted(ui, tr("Klasördeki her sözlük için boyut, SHA-256, diller, kelime sayısı ve lisans yazılır. Bir sözlüğün yanına <dosya>.json koyarak lisans, açıklama, ana sayfa gibi alanları değiştirebilirsiniz. Ayrıntılar: docs/CATALOG.md"));
            });
            ui.add_space(4.0);
            if button(ui, "license", &tr("Lisanslar ve anlamları…")).clicked() {
                self.show_licenses = true;
            }
        });
    }

    fn import_setup(&mut self) {
        let Some(path) = pick_open(&tr("Kurulumu içe aktar"), ("ZIP", &["zip"])) else {
            return;
        };
        let m = match transfer::read_manifest(&path) {
            Ok(m) => m,
            Err(e) => {
                self.info(tr("İçe aktar"), e);
                return;
            }
        };
        let parts: Vec<String> = m["parts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mut msg = tr_args(
            "{app} ile {date} tarihinde oluşturulmuş: {n} sözlük; içinde: {parts}.",
            &[
                ("app", &vs(&m, "app")),
                ("date", &take(&vs(&m, "created"), 10)),
                ("n", &m["dictionaries"].as_array().map_or(0, Vec::len)),
                ("parts", &parts.join(", ")),
            ],
        );
        msg += "\n\n";
        msg += &if self.im_replace {
            tr("Var olan liste, kaynaklar ve geçmiş DEĞİŞTİRİLECEK.")
        } else {
            tr("Var olanlara eklenecek.")
        };
        self.question(
            tr("İçe aktar"),
            msg,
            Next::Import(path, self.im_replace),
            Next::Nothing,
        );
    }

    fn build_catalog_file(&mut self) {
        let folder = PathBuf::from(self.cb_folder.trim());
        if self.cb_folder.trim().is_empty() || !folder.is_dir() {
            self.info(tr("Katalog"), tr("Bir klasör seçin."));
            return;
        }
        let (url, name, lic) = (
            self.cb_url.trim().to_string(),
            self.cb_name.trim().to_string(),
            self.cb_license.clone(),
        );
        self.run(move || {
            let cat = transfer::build_catalog(&folder, &url, &name, &lic, true);
            let out = folder.join("catalog.json");
            std::fs::write(&out, store::dumps(&cat))
                .map_err(|e| format!("{}: {}", out.display(), e))?;
            Ok(Work::Catalog(
                out,
                cat["dictionaries"].as_array().map_or(0, Vec::len),
            ))
        });
    }

    fn check_catalog_file(&mut self) {
        let Some(path) = pick_open(&tr("Kataloğu denetle"), ("JSON", &["json"])) else {
            return;
        };
        let data: Result<Value, String> =
            std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    let b = b.strip_prefix(b"\xef\xbb\xbf".as_slice()).unwrap_or(&b);
                    serde_json::from_slice(b).map_err(|e| e.to_string())
                });
        match data {
            Err(e) => self.info(tr("Kataloğu denetle"), e),
            Ok(d) => {
                let (n, problems) = transfer::check_catalog(&d);
                let errors = problems.iter().filter(|p| p.starts_with("error")).count();
                let mut text = tr_args(
                    "{n} sözlük · {e} hata · {w} uyarı",
                    &[("n", &n), ("e", &errors), ("w", &(problems.len() - errors))],
                );
                if !problems.is_empty() {
                    text += &format!(
                        "\n\n{}",
                        problems
                            .iter()
                            .take(40)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                }
                self.info(tr("Kataloğu denetle"), text);
            }
        }
    }

    // ------------------------------------------------------------ small windows

    fn dialogs(&mut self, ctx: &egui::Context, open: &mut bool) {
        if let Some(mut url) = self.link.take() {
            let mut keep = true;
            let mut ok = false;
            egui::Window::new(tr("Bağlantıdan ekle"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr("Bir sözlük dosyasının, arşivin ya da kataloğun adresi:"));
                    let r = ui.add(egui::TextEdit::singleline(&mut url).desired_width(520.0));
                    r.request_focus();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ok = true;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr("Tamam")).clicked() {
                            ok = true;
                        }
                        if ui.button(tr("İptal")).clicked() {
                            keep = false;
                        }
                    });
                });
            if ok {
                let url = url.trim().to_string();
                if !url.is_empty() {
                    let tp = guess_type(&url);
                    if tp != "http-index" && tp != "local" {
                        self.question(
                            tr("Bağlantıdan ekle"),
                            tr_args(
                                "Bu adres bir kaynak gibi görünüyor ({type}). Kaynak olarak eklensin mi?",
                                &[("type", &type_text(tp))],
                            ),
                            Next::AddSource(url.clone()),
                            Next::DownloadLink(url),
                        );
                    } else {
                        self.follow(Next::DownloadLink(url), open);
                    }
                }
            } else if keep {
                self.link = Some(url);
            }
        }

        if let Some(mut ed) = self.editor.take() {
            let mut keep = true;
            let mut save = false;
            let title = if ed.id.is_some() {
                tr("Kaynağı düzenle")
            } else {
                tr("Kaynak ekle")
            };
            egui::Window::new(title.clone())
                .id(egui::Id::new("center-source-editor"))
                .collapsible(false)
                .resizable(true)
                .default_width(620.0)
                .show(ctx, |ui| {
                    egui::Grid::new("src-editor").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        let line = |ui: &mut egui::Ui, label: &str, v: &mut String, hint: &str, pw: bool| {
                            ui.label(tr(label));
                            ui.add(egui::TextEdit::singleline(v).hint_text(hint).password(pw).desired_width(440.0));
                            ui.end_row();
                        };
                        line(ui, "Ad", &mut ed.name, "", false);
                        line(ui, "Adres", &mut ed.url, "https://…/catalog.json · https://github.com/owner/repo · /path/to/folder", false);
                        ui.label(tr("Tür"));
                        ui.horizontal(|ui| {
                            let items: Vec<(String, String)> = TYPES.iter().map(|t| (t.to_string(), type_text(t))).collect();
                            combo(ui, "src-type", &mut ed.kind, &items, 260.0);
                            if ui.button(tr("Tahmin et")).clicked() {
                                ed.kind = guess_type(&ed.url).to_string();
                            }
                        });
                        ui.end_row();
                        line(ui, "Dosya süzgeci", &mut ed.filter, r"\.(tar\.gz|zip)$", false);
                        line(ui, "Dil kalıbı", &mut ed.pattern, r"^(?P<src>.+?)-(?P<dst>.+?) ", false);
                        ui.label(tr("Diller (kaynak söylemezse)"));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut ed.src).desired_width(120.0));
                            ui.label("→");
                            ui.add(egui::TextEdit::singleline(&mut ed.dst).desired_width(120.0));
                        });
                        ui.end_row();
                        ui.label(tr("Lisans (kaynak söylemezse)"));
                        let mut items = vec![(String::new(), String::new())];
                        items.extend(licenses::all().into_iter().map(|l| (l.id.clone(), format!("{} — {}", l.id, l.name))));
                        if !ed.license.is_empty() && !items.iter().any(|(k, _)| *k == ed.license) {
                            items.push((ed.license.clone(), ed.license.clone()));
                        }
                        combo(ui, "src-license", &mut ed.license, &items, 360.0);
                        ui.end_row();
                        ui.label(tr("Derinlik"));
                        ui.add(egui::DragValue::new(&mut ed.depth).range(0..=8));
                        ui.end_row();
                        ui.label(tr("Oturum açma"));
                        let items: Vec<(String, String)> = AUTH.iter().map(|a| (a.to_string(), auth_text(a))).collect();
                        combo(ui, "src-auth", &mut ed.auth, &items, 200.0);
                        ui.end_row();
                        line(ui, "Kullanıcı / başlık adı", &mut ed.user, "", false);
                        line(ui, "Parola", &mut ed.password, "", true);
                        line(ui, "Belirteç (token)", &mut ed.token, "", true);
                        line(ui, "Açıklama", &mut ed.description, "", false);
                        ui.label(tr("Notlar"));
                        ui.add(egui::TextEdit::multiline(&mut ed.notes).desired_rows(3).desired_width(440.0));
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut ed.enabled, tr("Açık"));
                        ui.end_row();
                    });
                    muted(ui, tr("Parola ve belirteç yalnızca bu bilgisayarda credentials.json içinde tutulur; dışa aktarılan listelere asla girmez."));
                    ui.horizontal(|ui| {
                        if button(ui, "refresh", &tr("Dene")).clicked() {
                            ed.test = tr("Okunuyor…");
                            let mut v = ed.base.to_value();
                            if let (Some(m), Some(n)) = (v.as_object_mut(), ed.values().as_object()) {
                                for (k, x) in n {
                                    m.insert(k.clone(), x.clone());
                                }
                                if ed.id.is_none() {
                                    m.insert("id".into(), Value::from("test"));
                                }
                            }
                            let src = clean(&v);
                            let (pw, tok) = (ed.password.clone(), ed.token.clone());
                            let (timeout, proxy) = (self.prefs.float("center.timeout"), self.prefs.str("center.proxy"));
                            let tx = self.tx.clone();
                            let repaint = ctx.clone();
                            std::thread::spawn(move || {
                                let http = Http::new(timeout, &proxy);
                                let r = adapters::read(&src, &http, &pw, &tok).map(|v| v.len()).map_err(|e| e.0);
                                let _ = tx.send(Msg::Tested(r));
                                repaint.request_repaint();
                            });
                        }
                        ui.add(egui::Label::new(&ed.test).wrap());
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr("Tamam")).clicked() {
                            save = true;
                        }
                        if ui.button(tr("İptal")).clicked() {
                            keep = false;
                        }
                    });
                });
            if save {
                if ed.url.trim().is_empty() {
                    self.info(title, tr("Adres gerekli."));
                    self.editor = Some(ed);
                } else {
                    let v = ed.values();
                    let sid = match &ed.id {
                        Some(id) => {
                            self.sources.update(id, &v);
                            id.clone()
                        }
                        None => self.sources.add(&v).id,
                    };
                    self.sources.set_secret(&sid, &ed.password, &ed.token);
                    self.src_sel = Some(sid.clone());
                    self.reload_all();
                    self.refresh(Some(vec![sid]));
                }
            } else if keep {
                self.editor = Some(ed);
            }
        }

        if self.show_licenses {
            let mut open_l = true;
            egui::Window::new(tr("Lisanslar"))
                .open(&mut open_l)
                .collapsible(false)
                .resizable(true)
                .default_size([820.0, 520.0])
                .show(ctx, |ui| {
                    let all = licenses::all();
                    let h = (ui.available_height() - 40.0).max(200.0);
                    TableBuilder::new(ui)
                        .id_salt("center-licenses")
                        .striped(true)
                        .resizable(true)
                        .sense(egui::Sense::click())
                        .max_scroll_height(h)
                        .column(Column::initial(120.0))
                        .column(Column::remainder().at_least(200.0).clip(true))
                        .column(Column::initial(110.0))
                        .column(Column::initial(60.0))
                        .column(Column::initial(90.0))
                        .column(Column::initial(60.0))
                        .header(20.0, |mut head| {
                            for c in ["Kimlik", "Ad", "Durum", "Atıf", "Aynı lisans", "Ticari"] {
                                head.col(|ui| {
                                    ui.strong(tr(c));
                                });
                            }
                        })
                        .body(|body| {
                            body.rows(20.0, all.len(), |mut row| {
                                let l = &all[row.index()];
                                let vals = [
                                    l.id.clone(),
                                    license_name(l),
                                    status_text(&l.status),
                                    yn(l.attribution),
                                    yn(l.share_alike),
                                    yn(l.commercial),
                                ];
                                for (j, v) in vals.iter().enumerate() {
                                    row.col(|ui| {
                                        let t = if j == 2 { RichText::new(v).color(status_color(&l.status)) } else { RichText::new(v) };
                                        ui.add(egui::Label::new(t).truncate()).on_hover_text(&l.url);
                                    });
                                }
                                if row.response().double_clicked() && !l.url.is_empty() {
                                    let _ = open_external(&l.url);
                                }
                            });
                        });
                    muted(ui, tr("Bu tablo bir özet, hukuki tavsiye değildir; bağlayıcı olan lisansın kendi metnidir (çift tıklayınca açılır)."));
                });
            self.show_licenses = open_l;
        }

        if let Some(a) = self.ask.take() {
            let mut answer: Option<bool> = None;
            let mut extra = false;
            egui::Window::new(a.title.clone())
                .id(egui::Id::new("center-ask"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_max_width(520.0);
                    ui.label(&a.text);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if a.info {
                            if ui.button(tr("Tamam")).clicked() {
                                answer = Some(true);
                            }
                        } else {
                            let (yes, no) =
                                a.texts.clone().unwrap_or_else(|| (tr("Evet"), tr("Hayır")));
                            if ui.button(yes).clicked() {
                                answer = Some(true);
                            }
                            if let Some((text, _)) = &a.extra {
                                if ui.button(RichText::new(text).color(RED)).clicked() {
                                    extra = true;
                                }
                            }
                            if ui.button(no).clicked() {
                                answer = Some(false);
                            }
                        }
                    });
                });
            if extra {
                if let Some((_, next)) = a.extra {
                    self.follow(next, open);
                }
                return;
            }
            match answer {
                Some(true) => self.follow(a.yes, open),
                Some(false) => self.follow(a.no, open),
                None => self.ask = Some(a),
            }
        }
    }
}

/// The drop line above a row.
fn ui_line(r: &egui::Response, y: f32) {
    let stroke = egui::Stroke::new(2.0_f32, r.ctx.style().visuals.selection.stroke.color);
    r.ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("dnd-line"),
        ))
        .hline(r.rect.x_range(), y, stroke);
}

fn ui_modifiers(r: &egui::Response) -> egui::Modifiers {
    r.ctx.input(|i| i.modifiers)
}

/// The detail pane of one catalog entry.
fn entry_view(ui: &mut egui::Ui, e: &Entry, installed: Option<&Value>) {
    let lic = licenses::get(&e.license);
    let color = status_color(&lic.status);
    ui.label(RichText::new(&e.name).heading());
    if !pair_text(e).is_empty() {
        let mut l = langs::label(&e.source_lang);
        if e.kind != "monolingual" {
            l += &format!(
                " → {}",
                e.target_lang
                    .iter()
                    .map(|x| langs::label(x))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong(l);
            ui.label(format!("· {}", kind_text(&e.kind)));
        });
    }
    if !e.description.is_empty() {
        ui.add(egui::Label::new(&e.description).wrap());
    }
    ui.add_space(4.0);
    let mut rows = vec![
        (
            tr("Biçim"),
            format!(
                "{}{}",
                e.format,
                if e.supported == Some(false) {
                    format!(" — {}", tr("dictz henüz açamıyor"))
                } else {
                    String::new()
                }
            ),
        ),
        (
            tr("Kelime"),
            if e.headwords > 0 {
                group(e.headwords)
            } else {
                "–".into()
            },
        ),
        (tr("Boyut"), human_size(e.size)),
        (tr("Sürüm"), dash(&e.version)),
        (tr("Tarih"), dash(&e.date)),
        (tr("Yazar"), dash(&e.author)),
        (tr("Kaynak"), e.source.clone()),
    ];
    if let Some(r) = installed {
        rows.push((
            tr("Kurulu"),
            format!("{} · {}", take(&vs(r, "installed"), 10), vs(r, "folder")),
        ));
    }
    egui::Grid::new("center-entry")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            for (k, v) in rows {
                ui.label(RichText::new(k).weak());
                ui.add(egui::Label::new(v).wrap());
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{}:", tr("Lisans")));
        let name = license_name(&lic);
        if e.license_url.is_empty() {
            ui.label(name);
        } else if ui.link(name).clicked() {
            let _ = open_external(&e.license_url);
        }
        ui.label(RichText::new(format!("({})", status_text(&lic.status))).color(color));
    });
    ui.label(
        RichText::new(format!(
            "{}: {} · {}: {} · {}: {}",
            tr("Atıf gerekli"),
            yn(lic.attribution),
            tr("Aynı lisansla paylaş"),
            yn(lic.share_alike),
            tr("Ticari kullanım"),
            yn(lic.commercial)
        ))
        .small(),
    );
    if matches!(lic.status.as_str(), "unknown" | "nonfree") {
        ui.add(
            egui::Label::new(
                RichText::new(tr("Kaynak lisansı belirtmiyor ya da kullanımı kısıtlı. İndirmeden önce kullanım hakkınız olduğundan emin olun."))
                    .small()
                    .color(color),
            )
            .wrap(),
        );
    }
    if !e.sample.is_empty() {
        ui.label(
            RichText::new(format!("{}: {}", tr("Örnek kelimeler"), e.sample))
                .small()
                .weak(),
        );
    }
    ui.horizontal_wrapped(|ui| {
        for (k, v) in [(tr("Sayfası"), &e.page), (tr("Ana sayfa"), &e.homepage)] {
            if !v.is_empty() && ui.link(k).on_hover_text(v.as_str()).clicked() {
                let _ = open_external(v);
            }
        }
    });
    if !e.files.is_empty() {
        ui.strong(tr("Dosyalar"));
        for f in e.files.iter().take(20) {
            let sums: Vec<&str> = [
                ("SHA256", &f.sha256),
                ("SHA512", &f.sha512),
                ("MD5", &f.md5),
            ]
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| *k)
            .collect();
            ui.horizontal_wrapped(|ui| {
                ui.label("•");
                if ui.link(&f.name).on_hover_text(&f.url).clicked() {
                    let _ = open_external(&f.url);
                }
                let mut t = human_size(f.size);
                if !sums.is_empty() {
                    t += &format!(" · {}", sums.join(", "));
                }
                ui.label(RichText::new(t).small());
            });
        }
    } else if !e.resolve.is_empty() {
        ui.label(RichText::new(tr("Dosyalar kurulurken kaynaktan okunur.")).small());
    } else {
        ui.label(RichText::new(tr("İndirilecek dosya yok; sözlük sayfasından alınır.")).small());
    }
}
