//! Main window — the same layout, menus, shortcuts and features as the
//! Python edition's Qt window.

use crate::i18n::{self, tr, tr_args};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontFamily, Key, KeyboardShortcut, Modifiers, RichText};
use serde_json::Value;

use crate::history::HistoryStore;
use crate::html::{self, Line};
use crate::index::IndexStore;
use crate::media::{media_kind, MediaKind};
use crate::paths::{HISTORY_DB, INDEX_DB};
use crate::query::{
    add_folder_source, folder_sources, is_dictionary_file, scan_folder, scan_sources, QueryManager,
    Suggestion,
};
use crate::settings::{state_defaults, Preferences};
use crate::shortcuts::SHORTCUTS;
use crate::{DictionaryEntry, VERSION};

use super::prefs_window::{self, Advanced, PrefsWindow};
use super::render::{self, Click, MediaSource};
use super::services::{Event as SvcEvent, Services, TrayAction};
use super::uiicons;
use super::window::MainWindow;
use super::{audio, fonts, system};
use crate::scantext::{clean_scan_text, DoubleCopy};

const NEW_TAB: &str = "Yeni sekme";

struct Article {
    headword: String,
    definition: String,
    lines: Vec<Line>,
}

struct Group {
    dict: String,
    articles: Vec<Article>,
}

enum Content {
    Welcome,
    Articles(Vec<Group>),
    NotFound {
        query: String,
        suggestions: Vec<(String, f64)>,
    },
}

struct Tab {
    word: String,
    content: Content,
    generation: u64,
}

impl Tab {
    fn welcome() -> Self {
        Self {
            word: String::new(),
            content: Content::Welcome,
            generation: 0,
        }
    }
    fn title(&self, max: usize) -> String {
        if self.word.is_empty() {
            tr(NEW_TAB)
        } else if matches!(self.content, Content::NotFound { .. }) {
            format!("{} ✗", truncate(&self.word, max))
        } else {
            truncate(&self.word, max)
        }
    }
    fn groups(&self) -> &[Group] {
        match &self.content {
            Content::Articles(g) => g,
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SidebarTab {
    Search,
    Dictionaries,
    Favorites,
    History,
}

#[derive(Clone, PartialEq)]
enum Confirm {
    ClearHistory,
    ClearFavorites,
    RemoveDict(String),
    Quit,
}

/// A dictionary being dragged (dictionary bar, found-in panel, side list).
#[derive(Clone)]
struct DictDrag(String);

enum Action {
    CreateShortcuts,
    SetScan(bool),
    PopupLookup(String),
    ToggleMain,
    OpenWord(String),
    OpenWordIn {
        word: String,
        dicts: Vec<String>,
    },
    Explicit {
        new_tab: bool,
    },
    NavBack,
    NavForward,
    Random,
    ToggleFavorite,
    Zoom(f64),
    ZoomReset,
    SetTheme(&'static str),
    CycleTheme,
    SetLanguage(String),
    AddDictionaryDialog,
    AddFolderDialog,
    AddPaths(Vec<PathBuf>),
    OpenManager,
    /// Go to this dictionary's article.
    Jump(String),
    /// Put a dictionary before another one (`None` = at the end).
    MoveBefore(String, Option<String>),
    /// The new order of the dictionaries.
    SetOrder(Vec<String>),
    /// Select it in "Sözlüklerim" (`true` = and ask to remove it).
    ShowInMine(String, bool),
    /// The Dictionary Center on this tab.
    OpenCenter(usize),
    AddFromLink,
    OpenPreferences,
    About,
    Shortcuts,
    CliHelp,
    CheckUpdates,
    OpenDataDir,
    Quit,
    NewTab,
    CloseTab(usize),
    SelectTab(usize),
    CycleTab(isize),
    TogglePref(&'static str),
    Fullscreen,
    CopyWord,
    CopyArticle(Option<usize>),
    SaveArticle,
    ExportFavorites,
    OpenFind,
    FindStep(isize),
    ExpandAll(bool),
    ScrollToDict(usize),
    Ask(Confirm),
    FocusSearch,
    Click(String, Click),
    SetEnabled(String, bool),
    Solo(String),
    EnableAll(bool),
    RemoveDict(String),
    Reindex(Vec<String>),
    Info(String),
    RemoveHistory(String),
    RemoveFavorite(String),
}

/// Media cache shared by the renderer: (dictionary, file) → bytes.
struct MediaCache<'a> {
    qm: &'a QueryManager,
    dict: &'a str,
    cache: &'a mut HashMap<(String, String), Option<Arc<[u8]>>>,
}

impl MediaSource for MediaCache<'_> {
    fn image(&mut self, name: &str) -> Option<Arc<[u8]>> {
        if media_kind(name) != MediaKind::Image {
            return None;
        }
        let key = (self.dict.to_string(), name.to_string());
        if let Some(v) = self.cache.get(&key) {
            return v.clone();
        }
        let v: Option<Arc<[u8]>> = self
            .qm
            .media(self.dict)
            .and_then(|m| m.read(name))
            .map(Arc::from);
        self.cache.insert(key, v.clone());
        v
    }
}

/// Scan popup state.
struct Popup {
    open: bool,
    word: String,
    edit: String,
    content: Content,
    generation: u64,
    pinned: bool,
    pos: Option<egui::Pos2>,
    entered: bool,
    left_at: Option<Instant>,
    focused: bool,
    size: Option<egui::Vec2>,
}

impl Default for Popup {
    fn default() -> Self {
        Self {
            open: false,
            word: String::new(),
            edit: String::new(),
            content: Content::Welcome,
            generation: 0,
            pinned: false,
            pos: None,
            entered: false,
            left_at: None,
            focused: false,
            size: None,
        }
    }
}

/// The application.
pub struct DictzApp {
    qm: QueryManager,
    history: Option<HistoryStore>,
    prefs: Preferences,
    data_dir: PathBuf,

    loader: Option<Receiver<crate::loading::Event>>,
    /// The running job: more work goes to it (see `queue_work`).
    job: Option<crate::loading::JobHandle>,
    load_progress: Option<crate::loading::LoadState>,
    load_errors: Vec<String>,
    loaded_count: usize,
    /// The running load only adds new files (not the start-up load).
    extra_load: bool,

    search: String,
    edited_at: Option<Instant>,
    suggestions: Vec<Suggestion>,
    show_suggestions: bool,
    sugg_index: Option<usize>,
    focus_search: bool,

    tabs: Vec<Tab>,
    current: usize,
    generation: u64,
    nav: Vec<String>,
    nav_cursor: isize,
    applying_nav: bool,

    sidebar_tab: SidebarTab,
    side_filter: [String; 3],
    favorites: Vec<String>,
    recent: Vec<String>,
    is_favorite: bool,

    find_open: bool,
    find_text: String,
    find_current: usize,
    find_count: usize,
    find_scroll: bool,
    focus_find: bool,
    scroll_to_dict: Option<usize>,
    expand_all: Option<bool>,

    status: Option<(String, Instant, u64)>,
    timing: String,
    error: Option<(String, String)>,
    confirm: Option<Confirm>,
    allow_close: bool,
    fullscreen: bool,
    prefs_win: Option<PrefsWindow>,
    /// The Dictionary Center (kept while dictz runs: downloads go on).
    center: Option<super::center::CenterWindow>,
    center_open: bool,
    show_about: bool,
    show_shortcuts: bool,
    show_cli: bool,
    info: Option<String>,
    update_rx: Option<Receiver<Result<String, String>>>,
    update_silent: bool,
    update_msg: Option<String>,

    fonts_key: Option<(String, String)>,
    media_cache: HashMap<(String, String), Option<Arc<[u8]>>>,
    dict_looks: super::dictbadge::DictLooks,
    flags: super::langflags::Flags,

    win: MainWindow,
    services: Services,
    popup: Popup,
    double_copy: DoubleCopy,
    frame_no: u64,
    start_hidden: bool,
    quitting: bool,
    pending_popup: Option<String>,
    tray_hint_shown: bool,
    started: Instant,
}

impl DictzApp {
    /// Build the window; dictionaries are loaded in the background.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        data_dir: PathBuf,
        prefs: Preferences,
        files: Vec<PathBuf>,
        popup: Option<String>,
    ) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut services = Services::new(&cc.egui_ctx, prefs.bool("scan.enabled"));
        services.start_server(&data_dir);
        let win = MainWindow::new(cc);
        let double_copy = DoubleCopy::new(prefs.int("scan.double_copy_ms") as f64);
        let _ = std::fs::create_dir_all(&data_dir);
        let mut status = None;
        let index = match IndexStore::open(&data_dir.join(INDEX_DB)) {
            Ok(i) => Some(i),
            Err(e) => {
                status = Some((
                    format!("{} {}", tr("İndeks açılamadı:"), e),
                    Instant::now(),
                    8,
                ));
                None
            }
        };
        let history = HistoryStore::open(&data_dir.join(HISTORY_DB)).ok();
        if let Some(h) = &history {
            let _ = h.purge(prefs.int("history.max_entries"));
        }
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let search = if prefs.bool("general.restore_last_search") {
            prefs.str("state.last_search")
        } else {
            String::new()
        };
        let mut app = Self {
            qm: QueryManager::new(index),
            history,
            data_dir,
            loader: None,
            job: None,
            load_progress: None,
            load_errors: Vec::new(),
            loaded_count: 0,
            extra_load: false,
            search,
            edited_at: None,
            suggestions: Vec::new(),
            show_suggestions: false,
            sugg_index: None,
            focus_search: true,
            tabs: vec![Tab::welcome()],
            current: 0,
            generation: 0,
            nav: Vec::new(),
            nav_cursor: -1,
            applying_nav: false,
            sidebar_tab: SidebarTab::Search,
            side_filter: Default::default(),
            favorites: Vec::new(),
            recent: Vec::new(),
            is_favorite: false,
            find_open: false,
            find_text: String::new(),
            find_current: 0,
            find_count: 0,
            find_scroll: false,
            focus_find: false,
            scroll_to_dict: None,
            expand_all: None,
            status,
            timing: String::new(),
            error: None,
            confirm: None,
            allow_close: false,
            fullscreen: false,
            prefs_win: None,
            center: None,
            center_open: false,
            show_about: false,
            show_shortcuts: false,
            show_cli: false,
            info: None,
            update_rx: None,
            update_silent: true,
            update_msg: None,
            fonts_key: None,
            media_cache: HashMap::new(),
            dict_looks: Default::default(),
            flags: Default::default(),
            win,
            services,
            popup: Popup::default(),
            double_copy,
            frame_no: 0,
            start_hidden: false,
            quitting: false,
            pending_popup: popup,
            tray_hint_shown: false,
            started: Instant::now(),
            prefs,
        };
        app.start_hidden = app.prefs.bool("general.start_in_tray");
        app.sync_autostart();
        app.apply_prefs(&cc.egui_ctx);
        app.refresh_lists();
        app.start_loading(&cc.egui_ctx, files);
        if app.prefs.bool("general.check_updates_on_start") {
            app.check_updates(&cc.egui_ctx, true);
        }
        app
    }

    // ------------------------------------------------------------ scan popup, tray

    fn set_scan(&mut self, ctx: &egui::Context, on: bool) {
        if self.prefs.bool("scan.enabled") != on {
            self.set_pref("scan.enabled", Value::Bool(on));
        }
        self.apply_prefs(ctx);
        self.set_status(
            if on {
                tr("Tarama açık")
            } else {
                tr("Tarama kapalı")
            },
            3,
        );
    }

    fn show_main(&mut self, ctx: &egui::Context) {
        self.win.show(ctx);
    }

    fn toggle_main(&mut self, ctx: &egui::Context) {
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.win.is_hidden() || minimized {
            self.win.show(ctx);
        } else if self.services.has_tray() {
            self.win.hide(ctx);
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        self.allow_close = true;
        if self.win.is_hidden() {
            // put the window back where it was so its position is saved
            self.win.show(ctx);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Show `word` (empty = the clipboard text) in the scan popup.
    fn popup_lookup(&mut self, ctx: &egui::Context, word: &str) {
        let mut word = word.trim().to_string();
        if word.is_empty() {
            let max = (self.prefs.int("scan.max_length").max(1) as usize).max(200);
            match Services::clipboard_text().and_then(|t| clean_scan_text(&t, max)) {
                Some(w) => word = w,
                None => {
                    self.set_status(tr("Panoda aranacak kelime yok."), 3);
                    return;
                }
            }
        }
        if self.loader.is_some() && !self.extra_load {
            self.pending_popup = Some(word); // the start-up load is still running
            return;
        }
        let (entries, fz) = self.search_with_fallback(&word, None);
        let found = !entries.is_empty();
        let auto_play = found && self.prefs.bool("audio.auto_play");
        self.generation += 1;
        self.popup.generation = self.generation;
        self.popup.content = if found {
            Content::Articles(Self::groups(entries))
        } else {
            Content::NotFound {
                query: word.clone(),
                suggestions: fz,
            }
        };
        self.popup.word = word.clone();
        self.popup.edit = word.clone();
        if !self.popup.open {
            self.popup.open = true;
            self.popup.entered = false;
            self.popup.left_at = None;
            self.popup.size = None;
            self.popup.pos = None;
            if self.prefs.bool("scan.near_cursor") {
                if let Some((x, y)) = Services::mouse_position() {
                    let ppp = ctx
                        .input(|i| i.viewport().native_pixels_per_point)
                        .unwrap_or(1.0)
                        .max(0.5);
                    self.popup.pos = Some(egui::pos2(x as f32 / ppp + 16.0, y as f32 / ppp + 20.0));
                }
            }
        }
        self.popup.left_at = None;
        if found && self.prefs.bool("history.enabled") && self.prefs.bool("scan.record_history") {
            if let Some(h) = &self.history {
                let _ = h.record(&word, "");
            }
            self.refresh_lists();
        }
        if auto_play {
            let first = match &self.popup.content {
                Content::Articles(groups) => groups.iter().find_map(|g| {
                    g.articles.iter().find_map(|a| {
                        render::first_audio(&a.lines).map(|s| (g.dict.clone(), s.to_string()))
                    })
                }),
                _ => None,
            };
            if let Some((d, s)) = first {
                self.play_audio(&d, &s);
            }
        }
        ctx.request_repaint();
    }

    fn handle_services(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused) || self.popup.focused;
        for ev in self.services.poll() {
            if std::env::var_os("DICTZ_DEBUG").is_some() {
                eprintln!("dictz: event {:?} (focused: {})", ev, focused);
            }
            match ev {
                SvcEvent::Clipboard(text, at) => {
                    if !self.prefs.bool("scan.enabled") || focused {
                        continue;
                    }
                    let Some(word) =
                        clean_scan_text(&text, self.prefs.int("scan.max_length").max(1) as usize)
                    else {
                        continue;
                    };
                    let now = at.duration_since(self.started).as_secs_f64() * 1000.0;
                    let double = self.double_copy.feed(&word, now);
                    if self.prefs.bool("scan.clipboard")
                        || (self.prefs.bool("scan.ctrl_c_c") && double)
                    {
                        self.popup_lookup(ctx, &word);
                    }
                }
                SvcEvent::Selection(text) => {
                    if !self.prefs.bool("scan.enabled")
                        || !self.prefs.bool("scan.selection")
                        || focused
                    {
                        continue;
                    }
                    if let Some(word) =
                        clean_scan_text(&text, self.prefs.int("scan.max_length").max(1) as usize)
                    {
                        self.popup_lookup(ctx, &word);
                    }
                }
                SvcEvent::Hotkey(name) => match name {
                    "show_main" => self.toggle_main(ctx),
                    "toggle_scan" => {
                        let on = !self.prefs.bool("scan.enabled");
                        self.set_scan(ctx, on);
                    }
                    _ => self.popup_lookup(ctx, ""),
                },
                SvcEvent::Tray(a) => match a {
                    TrayAction::Toggle => self.toggle_main(ctx),
                    TrayAction::Scan(on) => self.set_scan(ctx, on),
                    TrayAction::ToggleScan => {
                        let on = !self.prefs.bool("scan.enabled");
                        self.set_scan(ctx, on);
                    }
                    TrayAction::LookupClipboard => self.popup_lookup(ctx, ""),
                    TrayAction::Preferences => {
                        self.show_main(ctx);
                        self.prefs_win = Some(PrefsWindow::new(&self.prefs));
                    }
                    TrayAction::Quit => self.quit(ctx),
                },
                SvcEvent::Remote(req) => {
                    let word = req["word"].as_str().unwrap_or("").trim().to_string();
                    match req["cmd"].as_str().unwrap_or("") {
                        "show" => self.show_main(ctx),
                        "hide" => self.win.hide(ctx),
                        "toggle" => self.toggle_main(ctx),
                        "lookup" => {
                            self.show_main(ctx);
                            self.open_word(&word, None, false);
                        }
                        "popup" => self.popup_lookup(ctx, &word),
                        "scan" => {
                            let on = req["on"].as_bool().unwrap_or(false);
                            self.set_scan(ctx, on);
                        }
                        "open" => {
                            let files: Vec<PathBuf> = req["files"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|f| f.as_str())
                                        .map(PathBuf::from)
                                        .collect()
                                })
                                .unwrap_or_default();
                            self.show_main(ctx);
                            if !files.is_empty() {
                                self.add_paths(ctx, files);
                            }
                        }
                        "quit" => self.quit(ctx),
                        _ => {}
                    }
                }
            }
        }
        for m in std::mem::take(&mut self.services.messages) {
            self.set_status(m, 10);
        }
    }

    fn popup_viewport(&mut self, ctx: &egui::Context) {
        if !self.popup.open {
            self.popup.focused = false;
            return;
        }
        let w = self.prefs.int("scan.width") as f32;
        let h = self.prefs.int("scan.height") as f32;
        let mut builder = egui::ViewportBuilder::default()
            .with_title(tr("dictz — tarama"))
            .with_inner_size([w, h])
            .with_min_inner_size([240.0, 160.0])
            .with_taskbar(false)
            .with_icon(Arc::new(egui::IconData {
                rgba: crate::icon::app_icon_rgba(64),
                width: 64,
                height: 64,
            }));
        if let Some(p) = self.popup.pos {
            builder = builder.with_position(p);
        }
        if self.prefs.bool("scan.always_on_top") {
            builder = builder.with_always_on_top();
        }
        let id = egui::ViewportId::from_hash_of("dictz-scan-popup");
        let mut close = false;
        let mut lookup: Option<String> = None;
        let mut open_main: Option<String> = None;
        let mut clicks: Vec<(String, Click)> = Vec::new();
        ctx.show_viewport_immediate(id, builder, |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(Key::Escape)) {
                close = true;
            }
            self.popup.focused = ctx.input(|i| i.focused);
            if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
                self.popup.size = Some(r.size());
            }
            egui::TopBottomPanel::top("popup-bar").show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::singleline(&mut self.popup.edit)
                        .hint_text(tr("Kelime…"))
                        .desired_width((ui.available_width() - 130.0).max(80.0));
                    let r = ui.add(edit);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        lookup = Some(self.popup.edit.trim().to_string());
                    }
                    let pin = if self.popup.pinned {
                        format!("📌 {}", tr("Sabit"))
                    } else {
                        "📌".to_string()
                    };
                    if ui
                        .selectable_label(self.popup.pinned, pin)
                        .on_hover_text(tr("Sabitle (fare çıkınca kapanmaz)"))
                        .clicked()
                    {
                        self.popup.pinned = !self.popup.pinned;
                    }
                    if ui
                        .button("⤢")
                        .on_hover_text(tr("Ana pencerede aç"))
                        .clicked()
                    {
                        open_main = Some(self.popup.word.clone());
                    }
                    if ui.button("✕").on_hover_text(tr("Kapat (Esc)")).clicked() {
                        close = true;
                    }
                });
                if let Content::Articles(groups) = &self.popup.content {
                    let n: usize = groups.iter().map(|g| g.articles.len()).sum();
                    ui.weak(tr_args(
                        "{n} madde · {m} sözlük",
                        &[("n", &n), ("m", &groups.len())],
                    ));
                }
                ui.add_space(2.0);
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                let ready =
                    ui.fonts(|f| f.families().contains(&FontFamily::Name(fonts::BOLD.into())));
                if !ready {
                    ui.ctx().request_repaint();
                    return;
                }
                let opts = self.article_opts(ui, false);
                let size = opts.size;
                let head_scale = self.prefs.float("article.headword_scale") as f32;
                let accent = ui.visuals().hyperlink_color;
                let mut find = render::Find::default();
                egui::ScrollArea::vertical()
                    .id_salt(("popup", self.popup.generation))
                    .auto_shrink([false, false])
                    .show(ui, |ui| match &self.popup.content {
                        Content::Articles(groups) => {
                            for g in groups {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&g.dict)
                                        .size(size * 0.8)
                                        .color(accent)
                                        .strong(),
                                );
                                for a in &g.articles {
                                    ui.label(
                                        RichText::new(&a.headword)
                                            .font(egui::FontId::new(
                                                size * head_scale,
                                                FontFamily::Name(fonts::BOLD.into()),
                                            ))
                                            .strong(),
                                    );
                                    let mut local = Vec::new();
                                    let mut media = MediaCache {
                                        qm: &self.qm,
                                        dict: &g.dict,
                                        cache: &mut self.media_cache,
                                    };
                                    render::lines(
                                        ui, &a.lines, &opts, &g.dict, &mut media, &mut find,
                                        &mut local,
                                    );
                                    clicks.extend(local.into_iter().map(|c| (g.dict.clone(), c)));
                                    ui.add_space(size * 0.5);
                                }
                                ui.separator();
                            }
                        }
                        Content::NotFound { query, suggestions } => {
                            ui.add_space(12.0);
                            ui.label(RichText::new(tr("Sonuç yok")).size(size * 1.2).strong());
                            ui.label(tr_args(
                                "“{word}” için hiçbir sözlükte eşleşme bulunamadı.",
                                &[("word", query)],
                            ));
                            if !suggestions.is_empty() {
                                ui.add_space(8.0);
                                ui.strong(tr("Bunu mu demek istediniz?"));
                                for (w, score) in suggestions {
                                    ui.horizontal(|ui| {
                                        if ui.link(w).clicked() {
                                            lookup = Some(w.clone());
                                        }
                                        ui.weak(format!("%{:.0}", score));
                                    });
                                }
                            }
                        }
                        Content::Welcome => {}
                    });
            });
            // auto-hide after the pointer has been inside and left
            let inside = ctx.input(|i| i.pointer.has_pointer());
            if inside {
                self.popup.entered = true;
                self.popup.left_at = None;
            } else if self.popup.entered && self.popup.left_at.is_none() {
                self.popup.left_at = Some(Instant::now());
            }
            if let Some(t) = self.popup.left_at {
                let delay =
                    Duration::from_millis(self.prefs.int("scan.hide_delay_ms").max(0) as u64);
                if self.prefs.bool("scan.auto_hide") && !self.popup.pinned && !self.popup.focused {
                    if t.elapsed() >= delay {
                        close = true;
                    } else {
                        ctx.request_repaint_after(delay - t.elapsed());
                    }
                }
            }
        });
        for (d, c) in clicks {
            match c {
                Click::Link(href) => {
                    if let Some(rest) = href
                        .strip_prefix("bword:")
                        .or_else(|| href.strip_prefix("dict:"))
                    {
                        lookup = Some(percent_decode(rest.trim_start_matches('/')));
                    } else {
                        self.handle_link(&d, &href);
                    }
                }
                Click::Audio(name) => self.play_audio(&d, &name),
            }
        }
        if let Some(w) = lookup.filter(|w| !w.is_empty()) {
            self.popup_lookup(ctx, &w);
        }
        if let Some(w) = open_main {
            close = true;
            self.show_main(ctx);
            if !w.is_empty() {
                self.open_word(&w, None, false);
            }
        }
        if close {
            self.close_popup();
        }
    }

    fn close_popup(&mut self) {
        if !self.popup.open {
            return;
        }
        self.popup.open = false;
        self.popup.focused = false;
        if let Some(s) = self.popup.size.take() {
            let (w, h) = (s.x.round() as i64, s.y.round() as i64);
            if w != self.prefs.int("scan.width") || h != self.prefs.int("scan.height") {
                let _ = self.prefs.set("scan.width", Value::from(w));
                let _ = self.prefs.set("scan.height", Value::from(h));
                self.save_prefs();
            }
        }
    }

    /// Article drawing options from the preferences.
    fn article_opts(&self, ui: &egui::Ui, with_find: bool) -> render::Opts {
        let p = &self.prefs;
        let size = p.int("article.font_size") as f32 * p.float("article.zoom") as f32;
        let line_spacing = p.float("article.line_spacing") as f32;
        let family = if self
            .fonts_key
            .as_ref()
            .map(|k| k.1.trim().is_empty())
            .unwrap_or(true)
        {
            FontFamily::Proportional
        } else {
            FontFamily::Name(fonts::ARTICLE.into())
        };
        let _ = ui;
        render::Opts {
            size,
            row_gap: (size * (line_spacing - 1.0) * 0.6).max(0.0),
            family,
            show_images: p.bool("article.show_images"),
            max_image_height: p.int("article.max_image_height") as f32,
            find: if with_find && self.find_open {
                render::lower_chars(self.find_text.trim())
            } else {
                Vec::new()
            },
            highlight: prefs_window::parse_hex(&p.str("article.highlight_color"))
                .unwrap_or(Color32::YELLOW),
        }
    }

    // ------------------------------------------------------------ prefs

    fn accent(&self) -> Color32 {
        prefs_window::parse_hex(&self.prefs.str("appearance.accent_color"))
            .unwrap_or(Color32::from_rgb(0x25, 0x63, 0xeb))
    }

    /// Started at log-in (`--autostart`): hidden in the tray.
    pub fn start_in_tray(&mut self) {
        self.start_hidden = true;
    }

    /// Make the system's log-in start match `general.autostart`.
    fn sync_autostart(&mut self) {
        let on = self.prefs.bool("general.autostart");
        if let Err(e) = crate::autostart::sync(on, Some(&self.data_dir)) {
            self.set_status(
                format!("{} {}", tr("Otomatik başlatma ayarlanamadı:"), e),
                8,
            );
        }
    }

    fn apply_prefs(&mut self, ctx: &egui::Context) {
        i18n::set_language(&self.prefs.str("general.language"));
        let p = &self.prefs;
        ctx.set_theme(match p.str("appearance.theme").as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        });
        let accent = self.accent();
        let base = p.int("appearance.ui_font_size") as f32;
        ctx.all_styles_mut(|style| {
            use egui::{FontId, TextStyle};
            style.text_styles = [
                (
                    TextStyle::Small,
                    FontId::new(base * 0.8, FontFamily::Proportional),
                ),
                (TextStyle::Body, FontId::new(base, FontFamily::Proportional)),
                (
                    TextStyle::Button,
                    FontId::new(base, FontFamily::Proportional),
                ),
                (
                    TextStyle::Heading,
                    FontId::new(base * 1.4, FontFamily::Proportional),
                ),
                (
                    TextStyle::Monospace,
                    FontId::new(base * 0.92, FontFamily::Monospace),
                ),
            ]
            .into();
            style.visuals.hyperlink_color = if style.visuals.dark_mode {
                lighten(accent)
            } else {
                accent
            };
            style.visuals.selection.bg_fill =
                accent.gamma_multiply(if style.visuals.dark_mode { 0.6 } else { 0.35 });
            compact_style(style);
        });
        // interface scale (Preferences → Appearance), on top of the system's own
        let zoom = (p.int("appearance.ui_scale").clamp(50, 250) as f32) / 100.0;
        if (ctx.zoom_factor() - zoom).abs() > 0.001 {
            ctx.set_zoom_factor(zoom);
        }
        let key = (
            p.str("appearance.ui_font_family"),
            p.str("article.font_family"),
        );
        if self.fonts_key.as_ref() != Some(&key) {
            fonts::install(ctx, &key.0, &key.1);
            self.fonts_key = Some(key);
        }
        let level = if p.bool("general.always_on_top") {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        self.services.configure(&self.prefs);
        self.double_copy.interval_ms = self.prefs.int("scan.double_copy_ms") as f64;
        for m in std::mem::take(&mut self.services.messages) {
            self.set_status(m, 10);
        }
    }

    fn save_prefs(&mut self) {
        if let Err(e) = self.prefs.save() {
            self.set_status(format!("Tercihler kaydedilemedi: {}", e), 6);
        }
    }

    fn set_pref(&mut self, key: &str, v: Value) {
        let _ = self.prefs.set(key, v);
        self.save_prefs();
    }

    fn set_status(&mut self, msg: impl Into<String>, secs: u64) {
        self.status = Some((msg.into(), Instant::now(), secs));
    }

    // ------------------------------------------------------------ loading

    fn start_loading(&mut self, ctx: &egui::Context, files: Vec<PathBuf>) {
        let rows = self
            .qm
            .index
            .as_ref()
            .and_then(|i| i.list_dictionaries(false).ok())
            .unwrap_or_default();
        let mut scans: Vec<crate::loading::Scan> = Vec::new();
        if self.prefs.bool("dictionaries.scan_on_startup") {
            // every folder with its own "subfolders" box; listed in the job's
            // thread (unchanged folders come from scan_cache.json)
            let prefs = self.prefs.clone();
            let dir = self.data_dir.clone();
            scans.push(Box::new(move || scan_sources(&prefs, Some(&dir))));
        }
        let files: Vec<PathBuf> = files
            .into_iter()
            .filter(|f| is_dictionary_file(f))
            .collect();
        if self.qm.index.is_none() || (rows.is_empty() && scans.is_empty() && files.is_empty()) {
            self.loading_finished(ctx);
            return;
        }
        let background = self.prefs.bool("general.load_in_background");
        self.start_job(ctx, rows, files, scans, Vec::new(), background);
    }

    /// A background job (worker threads read new or changed files, several
    /// at once); its events arrive in `poll_loader`.
    fn start_job(
        &mut self,
        ctx: &egui::Context,
        rows: Vec<crate::index::DictionaryRow>,
        files: Vec<PathBuf>,
        scans: Vec<crate::loading::Scan>,
        rebuild: Vec<(String, PathBuf, bool)>,
        background: bool,
    ) {
        let job = crate::loading::LoadJob::new(
            &self.data_dir.join(INDEX_DB),
            self.prefs.bool("dictionaries.reindex_on_change"),
            self.prefs.int("dictionaries.build_workers"),
        );
        let h = job.handle();
        job.add_rows(rows);
        h.add_files(files);
        for sc in scans {
            h.add_scan(sc);
        }
        h.add_rebuild(rebuild);
        self.loaded_count = 0;
        self.load_errors.clear();
        let (tx, rx) = channel();
        let c = ctx.clone();
        let work = move || {
            job.run(&mut |e| {
                let _ = tx.send(e);
                c.request_repaint();
            })
        };
        if background {
            std::thread::spawn(work);
        } else {
            work();
        }
        self.job = Some(h);
        self.loader = Some(rx);
        self.poll_loader(ctx);
    }

    /// More work: the running job takes it, else a new job starts.
    fn queue_work(
        &mut self,
        ctx: &egui::Context,
        files: Vec<PathBuf>,
        scans: Vec<crate::loading::Scan>,
        rebuild: Vec<(String, PathBuf, bool)>,
    ) {
        if self.qm.index.is_none() {
            return;
        }
        let (files, scans, rebuild) = match self.job.as_ref().filter(|_| self.loader.is_some()) {
            Some(h) => match h.add_all(files, scans, rebuild) {
                None => return, // the running job takes it
                Some(back) => {
                    // it has just finished: take its last events first
                    self.poll_loader_until_done(ctx);
                    back
                }
            },
            None => (files, scans, rebuild),
        };
        if self.loader.is_none() {
            self.extra_load = true;
        }
        self.start_job(ctx, Vec::new(), files, scans, rebuild, true);
    }

    fn poll_loader(&mut self, ctx: &egui::Context) {
        self.poll_loader_with(ctx, false);
    }

    /// The job has closed its inbox: wait for its last few events.
    fn poll_loader_until_done(&mut self, ctx: &egui::Context) {
        self.poll_loader_with(ctx, true);
    }

    fn poll_loader_with(&mut self, ctx: &egui::Context, block: bool) {
        let Some(rx) = self.loader.take() else { return };
        let mut done = false;
        let mut changed = false;
        loop {
            let ev = if block {
                rx.recv()
                    .map_err(|_| std::sync::mpsc::TryRecvError::Disconnected)
            } else {
                rx.try_recv()
            };
            match ev {
                Ok(ev) => changed |= self.on_load_event(ev),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    done = true; // the job's thread has ended
                    break;
                }
            }
        }
        if changed {
            self.media_cache.clear();
        }
        if done {
            self.job = None;
            self.loading_finished(ctx);
        } else {
            self.loader = Some(rx);
        }
    }

    /// `true` when a dictionary arrived.
    fn on_load_event(&mut self, ev: crate::loading::Event) -> bool {
        use crate::loading::Event;
        match ev {
            Event::Progress(st) => {
                self.load_progress = Some(st);
                false
            }
            Event::Loaded {
                dict,
                path,
                enabled,
                ..
            } => {
                self.qm.attach(dict, &path, enabled);
                self.loaded_count += 1;
                true
            }
            Event::Failed(path, e) => {
                // stays in the list, shown with ⚠ (never dropped)
                self.load_errors.push(format!("{}: {}", path.display(), e));
                let me = crate::registry::norm_path(&path);
                let name = self
                    .qm
                    .index
                    .as_ref()
                    .and_then(|i| i.list_dictionaries(false).ok())
                    .and_then(|rows| {
                        rows.into_iter()
                            .find(|r| crate::registry::norm_path(&r.path) == me)
                            .map(|r| r.name)
                    });
                if let Some(n) = name {
                    self.qm.note_failed(&n, &e);
                }
                false
            }
        }
    }

    fn loading_finished(&mut self, ctx: &egui::Context) {
        // the order is already in dictionaries.json; dictionaries that did
        // not load keep their place there
        self.load_progress = None;
        self.loader = None;
        if std::mem::take(&mut self.extra_load) {
            // new files from the dictionary folders: no tabs to restore
            if self.load_errors.is_empty() {
                let msg = tr_args("{n} sözlük eklendi", &[("n", &self.loaded_count)]);
                self.set_status(msg, 4);
            } else {
                let msg = format!(
                    "{} {}",
                    tr_args("{n} sözlük açılamadı:", &[("n", &self.load_errors.len())]),
                    self.load_errors.join("; ")
                );
                self.set_status(msg, 12);
            }
            self.refresh_lists();
            return;
        }
        if let Some(w) = self.pending_popup.take() {
            self.popup_lookup(ctx, &w);
        }
        if !self.load_errors.is_empty() {
            let msg = format!(
                "{} {}",
                tr_args("{n} sözlük açılamadı:", &[("n", &self.load_errors.len())]),
                self.load_errors.join("; ")
            );
            self.set_status(msg, 12);
        } else if self.loaded_count > 0 {
            let msg = tr_args("{n} sözlük yüklendi", &[("n", &self.loaded_count)]);
            self.set_status(msg, 4);
        }
        if self.prefs.bool("general.restore_tabs") {
            let words: Vec<String> = self
                .prefs
                .get("state.open_tabs")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .filter(|w| !w.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            if !words.is_empty() {
                for (i, w) in words.iter().enumerate() {
                    if i > 0 {
                        self.tabs.push(Tab::welcome());
                    }
                    let (entries, fz) = self.search_with_fallback(w, None);
                    let idx = self.tabs.len() - 1;
                    self.show_in_tab(idx, w, entries, Some(fz), false);
                }
                self.current =
                    (self.prefs.int("state.current_tab").max(0) as usize).min(self.tabs.len() - 1);
                self.update_favorite_flag();
                return;
            }
        }
        if !self.search.trim().is_empty() {
            self.edited_at = Some(Instant::now() - Duration::from_secs(5));
        }
    }

    fn check_updates(&mut self, ctx: &egui::Context, silent: bool) {
        self.update_silent = silent;
        #[cfg(feature = "update")]
        {
            let (tx, rx) = channel();
            let c = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(crate::updater::latest_version());
                c.request_repaint();
            });
            self.update_rx = Some(rx);
            if !silent {
                self.set_status(tr("Güncellemeler denetleniyor…"), 6);
            }
        }
        #[cfg(not(feature = "update"))]
        {
            let _ = ctx;
            if !silent {
                self.update_msg = Some(tr("Bu derleme `update` özelliği olmadan yapılmış.\nGüncellemek için: cargo install dictz --locked --force"));
            }
        }
    }

    fn poll_updates(&mut self) {
        let Some(rx) = &self.update_rx else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.update_rx = None;
        match res {
            Ok(latest) => {
                #[cfg(feature = "update")]
                let newer = crate::updater::is_newer(&latest, VERSION);
                #[cfg(not(feature = "update"))]
                let newer = false;
                if newer {
                    let msg = format!(
                        "{}\n{} dictz update",
                        tr_args(
                            "Yeni sürüm var: {latest} (kurulu: {v}).",
                            &[("latest", &latest), ("v", &VERSION)]
                        ),
                        tr("Güncellemek için terminalde:")
                    );
                    self.set_status(
                        tr_args(
                            "Yeni sürüm: dictz {latest} — terminalde `dictz update`",
                            &[("latest", &latest)],
                        ),
                        15,
                    );
                    if !self.update_silent {
                        self.update_msg = Some(msg);
                    }
                } else if !self.update_silent {
                    self.update_msg = Some(tr_args("dictz {v} güncel.", &[("v", &VERSION)]));
                }
            }
            Err(e) => {
                if !self.update_silent {
                    self.update_msg = Some(format!("Denetlenemedi: {}", e));
                }
            }
        }
    }

    // ------------------------------------------------------------ lists

    fn refresh_lists(&mut self) {
        if let Some(h) = &self.history {
            self.favorites = h
                .favorites(self.prefs.int("history.favorites_limit").max(1) as usize)
                .unwrap_or_default();
            self.recent = h
                .recent(self.prefs.int("history.limit").max(1) as usize)
                .unwrap_or_default();
        }
        self.update_favorite_flag();
    }

    fn current_word(&self) -> String {
        self.tabs
            .get(self.current)
            .map(|t| t.word.clone())
            .unwrap_or_default()
    }

    fn update_favorite_flag(&mut self) {
        let w = self.current_word();
        self.is_favorite = !w.is_empty()
            && self
                .history
                .as_ref()
                .map(|h| h.is_favorite(&w).unwrap_or(false))
                .unwrap_or(false);
    }

    // ------------------------------------------------------------ search

    fn case_filter(&self, q: &str, mut entries: Vec<DictionaryEntry>) -> Vec<DictionaryEntry> {
        if !self.prefs.bool("search.ignore_case") {
            entries.retain(|e| e.headword == q);
        }
        entries
    }

    fn fuzzy(&mut self, q: &str) -> Vec<(String, f64)> {
        if !self.prefs.bool("search.fuzzy") {
            return Vec::new();
        }
        let limit = self.prefs.int("search.fuzzy_limit").max(1) as usize;
        let cutoff = self.prefs.float("search.fuzzy_cutoff");
        self.qm.fuzzy(q, limit, cutoff)
    }

    fn search_with_fallback(
        &mut self,
        q: &str,
        dicts: Option<&[String]>,
    ) -> (Vec<DictionaryEntry>, Vec<(String, f64)>) {
        let limit = Some(self.prefs.int("search.max_articles").max(1) as usize);
        let exact = self.qm.search_in(q, true, dicts, limit);
        let mut entries = self.case_filter(q, exact);
        if entries.is_empty() && self.prefs.bool("search.prefix_fallback") {
            entries = self.qm.search_in(q, false, dicts, limit);
        }
        let fz = if entries.is_empty() {
            self.fuzzy(q)
        } else {
            Vec::new()
        };
        (entries, fz)
    }

    fn groups(entries: Vec<DictionaryEntry>) -> Vec<Group> {
        let mut out: Vec<Group> = Vec::new();
        for e in entries {
            let lines = if html::looks_like_html(&e.definition) {
                html::parse(&e.definition)
            } else {
                html::plain_to_lines(&e.definition)
            };
            let a = Article {
                headword: e.headword,
                definition: e.definition,
                lines,
            };
            match out.iter_mut().find(|g| g.dict == e.dictionary_id) {
                Some(g) => g.articles.push(a),
                None => out.push(Group {
                    dict: e.dictionary_id,
                    articles: vec![a],
                }),
            }
        }
        out
    }

    /// Show `entries` in tab `idx`. `fz` = did-you-mean list (computed when
    /// `None` and nothing was found).
    fn show_in_tab(
        &mut self,
        idx: usize,
        word: &str,
        entries: Vec<DictionaryEntry>,
        fz: Option<Vec<(String, f64)>>,
        record: bool,
    ) {
        self.generation += 1;
        let found = !entries.is_empty();
        let content = if found {
            Content::Articles(Self::groups(entries))
        } else {
            let suggestions = match fz {
                Some(f) => f,
                None => self.fuzzy(word),
            };
            Content::NotFound {
                query: word.to_string(),
                suggestions,
            }
        };
        let tab = &mut self.tabs[idx];
        tab.word = word.to_string();
        tab.content = content;
        tab.generation = self.generation;
        self.find_current = 0;
        if found && record {
            if self.prefs.bool("history.enabled") {
                if let Some(h) = &self.history {
                    let _ = h.record(word, "");
                }
                self.refresh_lists();
            }
            if self.prefs.bool("audio.auto_play") {
                let first = self.tabs[idx].groups().iter().find_map(|g| {
                    g.articles.iter().find_map(|a| {
                        render::first_audio(&a.lines).map(|s| (g.dict.clone(), s.to_string()))
                    })
                });
                if let Some((d, s)) = first {
                    self.play_audio(&d, &s);
                }
            }
        }
        self.update_favorite_flag();
    }

    fn timed<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let t = Instant::now();
        let r = f(self);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let groups = self
            .tabs
            .get(self.current)
            .map(|t| t.groups())
            .unwrap_or(&[]);
        let n: usize = groups.iter().map(|g| g.articles.len()).sum();
        self.timing = format!(
            "{} · {:.0} ms",
            tr_args("{n} madde · {m} sözlük", &[("n", &n), ("m", &groups.len())]),
            ms
        );
        r
    }

    fn push_nav(&mut self, word: &str) {
        if self.applying_nav || word.is_empty() {
            return;
        }
        if self.nav_cursor >= 0
            && self
                .nav
                .get(self.nav_cursor as usize)
                .map(|w| w == word)
                .unwrap_or(false)
        {
            return;
        }
        self.nav.truncate((self.nav_cursor + 1).max(0) as usize);
        self.nav.push(word.to_string());
        self.nav_cursor = self.nav.len() as isize - 1;
    }

    fn open_word(&mut self, word: &str, dicts: Option<Vec<String>>, new_tab: bool) {
        let word = word.trim().to_string();
        if word.is_empty() {
            return;
        }
        self.search = word.clone();
        self.edited_at = None;
        self.show_suggestions = false;
        self.timed(|s| {
            let (entries, fz) = s.search_with_fallback(&word, dicts.as_deref());
            if new_tab {
                s.tabs.push(Tab::welcome());
                s.current = s.tabs.len() - 1;
            }
            s.show_in_tab(s.current, &word, entries, Some(fz), true);
        });
        self.push_nav(&word);
    }

    fn run_live_search(&mut self) {
        let q = self.search.trim().to_string();
        if q.chars().count() < self.prefs.int("search.min_chars").max(1) as usize {
            self.suggestions.clear();
            self.show_suggestions = false;
            return;
        }
        let fill = if self.prefs.bool("search.fuzzy") {
            self.prefs.int("search.fuzzy_fill_below") as usize
        } else {
            0
        };
        self.suggestions = self.qm.suggest_with(
            &q,
            self.prefs.int("search.suggestion_limit").max(1) as usize,
            fill,
            self.prefs.int("search.fuzzy_limit").max(1) as usize,
            self.prefs.float("search.fuzzy_cutoff"),
        );
        self.show_suggestions = !self.suggestions.is_empty();
        self.sugg_index = None;
        if !self.prefs.bool("search.live") {
            return;
        }
        let limit = Some(self.prefs.int("search.max_articles").max(1) as usize);
        let found = self.timed(|s| {
            let exact = s.qm.search_in(&q, true, None, limit);
            let entries = s.case_filter(&q, exact);
            if entries.is_empty() {
                return false;
            }
            s.show_in_tab(s.current, &q, entries, None, true);
            true
        });
        if found {
            self.push_nav(&q);
        }
    }

    fn run_explicit(&mut self, new_tab: bool) {
        let q = self.search.trim().to_string();
        let new_tab = new_tab || self.prefs.bool("search.enter_new_tab");
        self.open_word(&q, None, new_tab);
        self.edited_at = None;
    }

    fn apply_nav(&mut self) {
        let Some(word) = self.nav.get(self.nav_cursor.max(0) as usize).cloned() else {
            return;
        };
        self.applying_nav = true;
        self.open_word(&word, None, false);
        self.applying_nav = false;
    }

    // ------------------------------------------------------------ dictionaries

    /// Add dictionary files (and folders' dictionaries) — in the background.
    fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let rec = self.prefs.bool("dictionaries.recursive");
        let mut files = Vec::new();
        let mut scans: Vec<crate::loading::Scan> = Vec::new();
        for p in paths {
            let p = std::path::absolute(&p).unwrap_or(p);
            if p.is_dir() {
                scans.push(Box::new(move || scan_folder(&p, rec)));
            } else {
                files.push(p);
            }
        }
        if self.qm.index.is_none() {
            // no data folder: read here (rare)
            for p in files {
                if let Err(e) = self.qm.add_dictionary(&p, false) {
                    self.error = Some((tr("Yüklenemedi"), format!("{}\n\n{}", p.display(), e)));
                }
            }
            self.media_cache.clear();
            return;
        }
        if !files.is_empty() {
            self.load_new_files(ctx, files);
        }
        if !scans.is_empty() {
            self.queue_work(ctx, Vec::new(), scans, Vec::new());
        }
    }

    /// Add dictionary files that are not in the list yet, in the background.
    fn load_new_files(&mut self, ctx: &egui::Context, files: Vec<PathBuf>) {
        let known: std::collections::HashSet<String> = self
            .qm
            .index
            .as_ref()
            .and_then(|i| i.list_dictionaries(false).ok())
            .unwrap_or_default()
            .iter()
            .map(|r| crate::registry::norm_path(&r.path))
            .collect();
        let new: Vec<PathBuf> = files
            .into_iter()
            .filter(|f| !known.contains(&crate::registry::norm_path(f)))
            .collect();
        if new.is_empty() {
            self.set_status(tr("Yeni sözlük yok"), 4);
            return;
        }
        self.queue_work(ctx, new, Vec::new(), Vec::new());
    }

    /// Read dictionaries' files again (new article store + headword index) in
    /// the background; each keeps its place.
    fn rebuild_dictionaries(&mut self, ctx: &egui::Context, names: Vec<String>) {
        let mut work = Vec::new();
        for n in names {
            let enabled = self.qm.is_enabled(&n);
            // Windows: the store file must be closed to be replaced
            if let Some(path) = self.qm.suspend(&n) {
                work.push((n, path, enabled));
            }
        }
        self.media_cache.clear();
        if !work.is_empty() {
            self.queue_work(ctx, Vec::new(), Vec::new(), work);
        }
    }

    /// Scan every dictionary folder now (in the background) and add the new files.
    fn rescan_folders(&mut self, ctx: &egui::Context) {
        let prefs = self.prefs.clone();
        let dir = self.data_dir.clone();
        self.queue_work(
            ctx,
            Vec::new(),
            vec![Box::new(move || scan_sources(&prefs, Some(&dir)))],
            Vec::new(),
        );
    }

    /// Keep `folder` in the dictionary folders (Preferences → Dictionaries;
    /// scanned at every start) and add its dictionaries now (listed in the
    /// background: a big folder tree must not stop the window).
    fn add_folder(&mut self, ctx: &egui::Context, folder: &Path) {
        if add_folder_source(&mut self.prefs, folder, None) {
            self.save_prefs();
        }
        let me = crate::registry::norm_path(folder);
        let rec = folder_sources(&self.prefs)
            .into_iter()
            .find(|(f, _)| *f == me)
            .map(|(_, r)| r)
            .unwrap_or(true);
        let folder = folder.to_path_buf();
        self.queue_work(
            ctx,
            Vec::new(),
            vec![Box::new(move || scan_folder(&folder, rec))],
            Vec::new(),
        );
    }

    // ------------------------------------------------------------ center

    /// The Dictionary Center (one window, kept while dictz runs).
    fn open_center(&mut self, ctx: &egui::Context, tab: usize) {
        if self.qm.index.is_none() {
            return;
        }
        if self.center.is_none() {
            self.center = Some(super::center::CenterWindow::new(
                ctx,
                &self.data_dir,
                &self.prefs,
            ));
        }
        if let Some(c) = &mut self.center {
            c.tab = tab;
        }
        self.center_open = true;
    }

    /// Draw the center (when open) and do what it asks; its downloads are
    /// followed also while it is closed.
    fn center_events(&mut self, ctx: &egui::Context) {
        use super::center::{Event, MineRow};
        let Some(mut c) = self.center.take() else {
            return;
        };
        let mut open = self.center_open;
        // "Sözlüklerim": every dictionary of the list, also the failed ones
        let rows: Vec<MineRow> = if open {
            self.qm
                .list_rows()
                .into_iter()
                .map(|r| {
                    let langs = self
                        .qm
                        .dictionary(&r.name)
                        .map(|d| {
                            let m = d.metadata();
                            if m.source_language.is_empty() {
                                String::new()
                            } else {
                                format!("{} → {}", m.source_language, m.target_language)
                            }
                        })
                        .unwrap_or_default();
                    MineRow {
                        name: r.name.clone(),
                        enabled: r.enabled,
                        loaded: r.loaded,
                        error: r.error.clone(),
                        format: r.format.clone(),
                        words: r.word_count as u64,
                        langs,
                        path: r.path.clone(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let events = {
            let looks = &mut self.dict_looks;
            let qm = &self.qm;
            let mut badge = |ui: &mut egui::Ui, name: &str| {
                let look = looks.get(ui.ctx(), qm, name);
                super::dictbadge::badge(ui, &look, 16.0);
            };
            c.update(ctx, &mut open, rows, &mut badge)
        };
        self.center = Some(c);
        self.center_open = open;
        for ev in events {
            match ev {
                Event::InstallFiles(files) => self.load_new_files(ctx, files),
                Event::RemoveFiles(files) => {
                    let want: std::collections::HashSet<String> = files
                        .iter()
                        .map(|f| crate::registry::norm_path(Path::new(f)))
                        .collect();
                    for row in self.dictionary_rows() {
                        if want.contains(&crate::registry::norm_path(&row.path)) {
                            let _ = self.qm.remove_dictionary(&row.name);
                            self.media_cache.retain(|k, _| k.0 != row.name);
                        }
                    }
                }
                Event::RemoveAll => {
                    for row in self.dictionary_rows() {
                        let _ = self.qm.remove_dictionary(&row.name);
                    }
                    self.media_cache.clear();
                }
                Event::Imported(settings) => self.after_import(ctx, settings),
                Event::SetEnabled(n, on) => self.apply(ctx, Action::SetEnabled(n, on)),
                Event::Solo(names) => {
                    for n in self.qm.loaded_names() {
                        let on = names.contains(&n);
                        self.qm.set_enabled(&n, on);
                    }
                    self.rerun_current();
                }
                Event::SetOrder(names) => self.apply(ctx, Action::SetOrder(names)),
                Event::Info(n) => self.info = Some(n),
                Event::Reindex(names) => self.rebuild_dictionaries(ctx, names),
                Event::AddFiles => self.apply(ctx, Action::AddDictionaryDialog),
                Event::AddFolder => self.apply(ctx, Action::AddFolderDialog),
                Event::Rescan => self.rescan_folders(ctx),
                Event::Remove(names, files) => {
                    // closed first: Windows cannot delete an open file
                    for n in &names {
                        let _ = self.qm.remove_dictionary(n);
                        self.media_cache.retain(|k, _| k.0 != *n);
                    }
                    let mut problems = Vec::new();
                    for f in files {
                        if let Err(e) = std::fs::remove_file(&f) {
                            problems.push(format!("{}: {}", f.display(), e));
                        }
                    }
                    if !problems.is_empty() {
                        self.error = Some((tr("Kaldır"), problems.join("\n")));
                    }
                    self.refresh_lists();
                    self.rerun_current();
                }
                Event::PrefsChanged(p) => {
                    let before = folder_sources(&self.prefs);
                    let mut p = p;
                    for (k, _) in state_defaults() {
                        if let Some(v) = self.prefs.get(k) {
                            let _ = p.set(k, v.clone());
                        }
                    }
                    self.prefs = p;
                    self.save_prefs();
                    self.apply_prefs(ctx);
                    let after = folder_sources(&self.prefs);
                    if after.iter().any(|f| !before.contains(f)) {
                        self.rescan_folders(ctx); // new folders: their dictionaries are added now
                    }
                }
            }
        }
    }

    fn dictionary_rows(&self) -> Vec<crate::index::DictionaryRow> {
        self.qm
            .index
            .as_ref()
            .and_then(|i| i.list_dictionaries(false).ok())
            .unwrap_or_default()
    }

    /// Open what an imported setup added (in the background); new settings
    /// apply at once.
    fn after_import(&mut self, ctx: &egui::Context, settings: bool) {
        if settings {
            let path = self.data_dir.join(crate::settings::SETTINGS_FILE);
            self.prefs = Preferences::load(&path);
            self.apply_prefs(ctx);
            if let Some(c) = &mut self.center {
                c.set_prefs(&self.prefs);
            }
        }
        let loaded: std::collections::HashSet<String> =
            self.qm.loaded_names().into_iter().collect();
        let rows: Vec<crate::index::DictionaryRow> = self
            .dictionary_rows()
            .into_iter()
            .filter(|r| !loaded.contains(&r.name))
            .collect();
        if rows.is_empty() {
            self.refresh_lists();
            return;
        }
        if self.loader.is_some() {
            let files = rows.into_iter().map(|r| r.path).collect();
            self.queue_work(ctx, files, Vec::new(), Vec::new());
        } else {
            self.start_job(ctx, rows, Vec::new(), Vec::new(), Vec::new(), true);
        }
        self.refresh_lists();
    }

    fn play_audio(&mut self, dict: &str, name: &str) {
        let Some(bytes) = self.qm.media(dict).and_then(|m| m.read(name)) else {
            self.set_status(
                tr_args("Ses dosyası bulunamadı: {name}", &[("name", &name)]),
                4,
            );
            return;
        };
        self.set_status(tr_args("Çalınıyor: {name}", &[("name", &name)]), 3);
        let vol = self.prefs.int("audio.volume") as f32 / 100.0;
        audio::play(
            bytes,
            name.to_string(),
            self.data_dir.join("media-cache"),
            vol,
            self.prefs.bool("audio.system_player"),
        );
    }

    fn handle_link(&mut self, dict: &str, href: &str) {
        if let Some(rest) = href
            .strip_prefix("bword:")
            .or_else(|| href.strip_prefix("dict:"))
        {
            let word = percent_decode(rest.trim_start_matches('/'));
            self.open_word(&word, None, false);
        } else if let Some(rest) = href.strip_prefix("sound://") {
            self.play_audio(dict, rest);
        } else if href.starts_with("http://")
            || href.starts_with("https://")
            || href.starts_with("mailto:")
        {
            if let Err(e) = system::open_external(href) {
                self.set_status(format!("{} {}", tr("Bağlantı açılamadı:"), e), 5);
            }
        } else if media_kind(href) == MediaKind::Audio {
            self.play_audio(dict, href);
        }
    }

    fn group_text(g: &Group) -> String {
        let mut s = String::new();
        for a in &g.articles {
            s.push_str(&a.headword);
            s.push('\n');
            s.push_str(&html::to_text(&a.definition));
            s.push_str("\n\n");
        }
        s
    }

    fn article_text(&self, only: Option<usize>) -> String {
        let Some(tab) = self.tabs.get(self.current) else {
            return String::new();
        };
        let mut out = String::new();
        for (i, g) in tab.groups().iter().enumerate() {
            if only.map(|o| o != i).unwrap_or(false) {
                continue;
            }
            if only.is_none() {
                out.push_str(&format!("== {} ==\n", g.dict));
            }
            out.push_str(&Self::group_text(g));
        }
        out.trim_end().to_string() + "\n"
    }

    fn article_html(&self) -> String {
        let Some(tab) = self.tabs.get(self.current) else {
            return String::new();
        };
        let esc = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let mut body = String::new();
        for g in tab.groups() {
            body.push_str(&format!("<h2 class=\"dict\">{}</h2>\n", esc(&g.dict)));
            for a in &g.articles {
                body.push_str(&format!(
                    "<h3>{}</h3>\n<div class=\"def\">{}</div>\n",
                    esc(&a.headword),
                    a.definition
                ));
            }
        }
        format!(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title>\n<style>body{{font-family:sans-serif;max-width:52em;margin:2em auto;line-height:{}}} h2.dict{{color:{};font-size:1em;border-bottom:1px solid #ccc}}</style></head>\n<body>\n{}</body></html>\n",
            esc(&tab.word),
            self.prefs.float("article.line_spacing"),
            self.prefs.str("appearance.accent_color"),
            body
        )
    }

    fn info_text(&self, name: &str) -> String {
        let Some(d) = self.qm.dictionary(name) else {
            return String::new();
        };
        let m = d.metadata();
        let p = &m.file_path;
        let size = std::fs::metadata(p).map(|x| x.len()).unwrap_or(0);
        let media = self
            .qm
            .media(name)
            .map(|m| m.search_locations())
            .unwrap_or_default();
        let rows: Vec<(String, String)> = vec![
            (tr("Ad"), m.name.clone()),
            (tr("Biçim"), m.format.clone()),
            (tr("Kelime"), group(m.word_count)),
            (tr("Kaynak dil"), or_dash(&m.source_language)),
            (tr("Hedef dil"), or_dash(&m.target_language)),
            (tr("Yazar"), or_dash(&m.author)),
            (tr("Dosya"), p.display().to_string()),
            (
                tr("Boyut"),
                format!("{} KB", group((size as f64 / 1024.0).round() as usize)),
            ),
            (
                tr("Etkin"),
                if self.qm.is_enabled(name) {
                    tr("evet")
                } else {
                    tr("hayır")
                },
            ),
            (
                tr("Medya klasörleri"),
                media
                    .iter()
                    .map(|x| x.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ];
        let mut s = String::new();
        for (k, v) in rows {
            s.push_str(&format!("{}\t{}\n", k, v));
        }
        if !m.description.is_empty() {
            s.push('\n');
            s.push_str(&m.description);
        }
        s
    }

    // ------------------------------------------------------------ actions

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::OpenWord(w) => self.open_word(&w, None, false),
            Action::OpenWordIn { word, dicts } => self.open_word(&word, Some(dicts), true),
            Action::Explicit { new_tab } => self.run_explicit(new_tab),
            Action::NavBack => {
                if self.nav_cursor > 0 {
                    self.nav_cursor -= 1;
                    self.apply_nav();
                }
            }
            Action::NavForward => {
                if self.nav_cursor + 1 < self.nav.len() as isize {
                    self.nav_cursor += 1;
                    self.apply_nav();
                }
            }
            Action::Random => match self.qm.random_entry(None) {
                Some(e) => self.open_word(&e.headword, None, false),
                None => self.set_status(tr("Önce bir sözlük ekleyin."), 3),
            },
            Action::ToggleFavorite => {
                let w = Some(self.current_word())
                    .filter(|w| !w.is_empty())
                    .unwrap_or_else(|| self.search.trim().to_string());
                if !w.is_empty() {
                    if let Some(h) = &self.history {
                        let _ = h.toggle_favorite(&w);
                    }
                    self.refresh_lists();
                }
            }
            Action::Zoom(d) => {
                let z = ((self.prefs.float("article.zoom") + d) * 10.0).round() / 10.0;
                self.set_pref("article.zoom", Value::from(z));
            }
            Action::ZoomReset => self.set_pref("article.zoom", Value::from(1.0)),
            Action::SetTheme(t) => {
                self.set_pref("appearance.theme", Value::from(t));
                self.apply_prefs(ctx);
            }
            Action::CycleTheme => {
                let order = ["auto", "light", "dark"];
                let cur = self.prefs.str("appearance.theme");
                let i = order.iter().position(|t| *t == cur).unwrap_or(0);
                let next = order[(i + 1) % order.len()];
                self.apply(ctx, Action::SetTheme(next));
                self.set_status(format!("{}: {}", tr("Tema"), theme_label(next)), 3);
            }
            Action::SetLanguage(code) => {
                let _ = self
                    .prefs
                    .set("general.language", serde_json::Value::String(code));
                let _ = self.prefs.save();
                self.apply_prefs(ctx);
                self.dict_looks = Default::default();
            }
            Action::AddDictionaryDialog => {
                let picked = rfd::FileDialog::new()
                    .set_title(tr("Sözlük dosyası seçin"))
                    .add_filter(tr("Tüm sözlükler"), &["ifo", "dsl", "dz", "bgl"])
                    .add_filter("StarDict", &["ifo"])
                    .add_filter(tr("DSL / Lingvo"), &["dsl", "dz"])
                    .add_filter(tr("Babylon"), &["bgl"])
                    .add_filter(tr("Tüm dosyalar"), &["*"])
                    .pick_files();
                if let Some(p) = picked {
                    self.add_paths(ctx, p);
                }
            }
            Action::AddFolderDialog => {
                if let Some(d) = rfd::FileDialog::new()
                    .set_title(tr("Sözlük klasörü seçin"))
                    .pick_folder()
                {
                    self.add_folder(ctx, &d);
                }
            }
            Action::AddPaths(p) => {
                // a folder is kept like "Add folder…" (scanned at every start)
                let (dirs, files): (Vec<PathBuf>, Vec<PathBuf>) =
                    p.into_iter().partition(|x| x.is_dir());
                for d in dirs {
                    self.add_folder(ctx, &d);
                }
                if !files.is_empty() {
                    self.add_paths(ctx, files);
                }
            }
            Action::OpenManager => self.open_center(ctx, super::center::TAB_MINE),
            Action::Jump(name) => self.jump_to_dictionary(&name),
            Action::MoveBefore(name, target) => {
                self.qm.move_before(&name, target.as_deref());
                self.rerun_current();
            }
            Action::SetOrder(names) => {
                self.qm.set_order(&names);
                self.rerun_current();
            }
            Action::ShowInMine(name, remove) => {
                self.open_center(ctx, super::center::TAB_MINE);
                if let Some(c) = &mut self.center {
                    c.select_dictionary(&name, remove);
                }
            }
            Action::OpenCenter(tab) => self.open_center(ctx, tab),
            Action::AddFromLink => {
                self.open_center(ctx, 2);
                if let Some(c) = &mut self.center {
                    c.add_from_link();
                }
            }
            Action::OpenPreferences => self.prefs_win = Some(PrefsWindow::new(&self.prefs)),
            Action::About => self.show_about = true,
            Action::Shortcuts => self.show_shortcuts = true,
            Action::CliHelp => self.show_cli = true,
            Action::CheckUpdates => self.check_updates(ctx, false),
            Action::OpenDataDir => {
                let _ = system::open_external(&self.data_dir.to_string_lossy());
            }
            Action::Quit => self.quit(ctx),
            Action::CreateShortcuts => {
                match crate::desktop::create_shortcuts(&self.data_dir, true, true) {
                    Ok(paths) if !paths.is_empty() => {
                        let list: Vec<String> =
                            paths.iter().map(|p| p.display().to_string()).collect();
                        self.error = Some((
                            tr("Kısayol"),
                            format!("{}\n{}", tr("Oluşturuldu:"), list.join("\n")),
                        ));
                    }
                    Ok(_) => {
                        self.error =
                            Some((tr("Kısayol"), tr("Masaüstü veya menü klasörü bulunamadı.")))
                    }
                    Err(e) => {
                        self.error = Some((
                            tr("Kısayol"),
                            format!("{}\n{}", tr("Kısayol oluşturulamadı:"), e),
                        ))
                    }
                }
            }
            Action::SetScan(on) => self.set_scan(ctx, on),
            Action::PopupLookup(w) => self.popup_lookup(ctx, &w),
            Action::ToggleMain => self.toggle_main(ctx),
            Action::NewTab => {
                self.tabs.push(Tab::welcome());
                self.current = self.tabs.len() - 1;
                self.update_favorite_flag();
                self.focus_search = true;
            }
            Action::CloseTab(i) => {
                if self.tabs.len() <= 1 {
                    self.tabs[0] = Tab::welcome();
                    self.current = 0;
                } else if i < self.tabs.len() {
                    self.tabs.remove(i);
                    if self.current >= self.tabs.len() || self.current > i {
                        self.current = self.current.saturating_sub(1);
                    }
                }
                self.update_favorite_flag();
            }
            Action::SelectTab(i) => {
                self.current = i.min(self.tabs.len() - 1);
                self.find_current = 0;
                self.update_favorite_flag();
            }
            Action::CycleTab(d) => {
                let n = self.tabs.len() as isize;
                self.current = (self.current as isize + d).rem_euclid(n) as usize;
                self.update_favorite_flag();
            }
            Action::TogglePref(key) => {
                let v = !self.prefs.bool(key);
                self.set_pref(key, Value::Bool(v));
                self.apply_prefs(ctx);
            }
            Action::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::CopyWord => {
                let w = self.current_word();
                if !w.is_empty() {
                    ctx.copy_text(w.clone());
                    self.set_status(tr_args("Kopyalandı: {word}", &[("word", &w)]), 3);
                }
            }
            Action::CopyArticle(only) => {
                let t = self.article_text(only);
                if !t.trim().is_empty() {
                    ctx.copy_text(t);
                    self.set_status(tr("Makale panoya kopyalandı"), 3);
                }
            }
            Action::SaveArticle => {
                let word = self.current_word();
                if self
                    .tabs
                    .get(self.current)
                    .map(|t| t.groups().is_empty())
                    .unwrap_or(true)
                {
                    return;
                }
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(tr("Makaleyi kaydet"))
                    .set_file_name(format!("{}.html", word))
                    .add_filter("HTML", &["html", "htm"])
                    .add_filter(tr("Metin"), &["txt"])
                    .save_file()
                {
                    let txt = path
                        .extension()
                        .map(|e| e.eq_ignore_ascii_case("txt"))
                        .unwrap_or(false);
                    let data = if txt {
                        self.article_text(None)
                    } else {
                        self.article_html()
                    };
                    match std::fs::write(&path, data) {
                        Ok(()) => self.set_status(format!("Kaydedildi: {}", path.display()), 4),
                        Err(e) => self.error = Some((tr("Kaydedilemedi"), e.to_string())),
                    }
                }
            }
            Action::ExportFavorites => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(tr("Favorileri dışa aktar"))
                    .set_file_name("dictz-favoriler.txt")
                    .add_filter(tr("Metin"), &["txt"])
                    .save_file()
                {
                    let all = self
                        .history
                        .as_ref()
                        .and_then(|h| h.favorites(1_000_000).ok())
                        .unwrap_or_default();
                    match std::fs::write(&path, all.join("\n") + "\n") {
                        Ok(()) => self.set_status(format!("{} favori kaydedildi", all.len()), 4),
                        Err(e) => self.error = Some((tr("Kaydedilemedi"), e.to_string())),
                    }
                }
            }
            Action::OpenFind => {
                self.find_open = true;
                self.focus_find = true;
            }
            Action::FindStep(d) => {
                if !self.find_open {
                    self.find_open = true;
                    self.focus_find = true;
                } else if self.find_count > 0 {
                    let n = self.find_count as isize;
                    self.find_current = (self.find_current as isize + d).rem_euclid(n) as usize;
                    self.find_scroll = true;
                }
            }
            Action::ExpandAll(on) => self.expand_all = Some(on),
            Action::ScrollToDict(i) => self.scroll_to_dict = Some(i),
            Action::Ask(c) => self.confirm = Some(c),
            Action::FocusSearch => self.focus_search = true,
            Action::Click(dict, Click::Link(href)) => self.handle_link(&dict, &href),
            Action::Click(dict, Click::Audio(name)) => self.play_audio(&dict, &name),
            Action::SetEnabled(name, on) => {
                self.qm.set_enabled(&name, on);
                self.rerun_current(); // the change shows at once
            }
            Action::Solo(name) => {
                for n in self.qm.loaded_names() {
                    self.qm.set_enabled(&n, n == name);
                }
                self.rerun_current();
                self.set_status(
                    tr_args(
                        "Yalnızca “{name}” etkin — Sözlükler → Tümünü etkinleştir ile geri alın",
                        &[("name", &name)],
                    ),
                    5,
                );
            }
            Action::EnableAll(on) => {
                for n in self.qm.loaded_names() {
                    self.qm.set_enabled(&n, on);
                }
                self.rerun_current();
            }
            Action::RemoveDict(name) => {
                if let Err(e) = self.qm.remove_dictionary(&name) {
                    self.error = Some((tr("Kaldırılamadı"), e.to_string()));
                }
                self.media_cache.retain(|k, _| k.0 != name);
            }
            // read again in the background (progress in the status bar)
            Action::Reindex(names) => self.rebuild_dictionaries(ctx, names),
            Action::Info(name) => self.info = Some(name),
            Action::RemoveHistory(w) => {
                if let Some(h) = &self.history {
                    let _ = h.remove_history(&w);
                }
                self.refresh_lists();
            }
            Action::RemoveFavorite(w) => {
                if let Some(h) = &self.history {
                    let _ = h.remove_favorite(&w);
                }
                self.refresh_lists();
            }
        }
    }

    fn advanced(&mut self, ctx: &egui::Context, a: Advanced) {
        match a {
            Advanced::OpenData => self.apply(ctx, Action::OpenDataDir),
            Advanced::DictionariesSettings => {
                self.prefs_win = None;
                self.open_center(ctx, super::center::TAB_SETTINGS);
            }
            Advanced::Rescan => {
                let prefs = match self.prefs_win.as_ref() {
                    Some(w) => w.draft.clone(),
                    None => self.prefs.clone(),
                };
                let dir = self.data_dir.clone();
                self.queue_work(
                    ctx,
                    Vec::new(),
                    vec![Box::new(move || scan_sources(&prefs, Some(&dir)))],
                    Vec::new(),
                );
            }
            Advanced::Reindex => {
                let names = self.qm.loaded_names();
                self.apply(ctx, Action::Reindex(names));
            }
            Advanced::CheckUpdates => self.check_updates(ctx, false),
            Advanced::ClearHistory => self.confirm = Some(Confirm::ClearHistory),
            Advanced::ClearFavorites => self.confirm = Some(Confirm::ClearFavorites),
            Advanced::Export => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("dictz-settings.json")
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    let data = self
                        .prefs_win
                        .as_ref()
                        .map(|w| w.draft.export())
                        .unwrap_or_else(|| self.prefs.export());
                    let text = serde_json::to_string_pretty(&data).unwrap_or_default() + "\n";
                    match std::fs::write(&path, text) {
                        Ok(()) => self
                            .set_status(format!("{} {}", tr("Dışa aktarıldı:"), path.display()), 4),
                        Err(e) => self.error = Some((tr("Dışa aktarılamadı"), e.to_string())),
                    }
                }
            }
            Advanced::Import => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    let parsed = std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()));
                    match parsed {
                        Ok(v) => {
                            if let Some(w) = self.prefs_win.as_mut() {
                                let errors = w.draft.import(&v);
                                if !errors.is_empty() {
                                    self.error =
                                        Some((tr("İçe aktarma uyarıları"), errors.join("\n")));
                                }
                            }
                        }
                        Err(e) => self.error = Some((tr("İçe aktarılamadı"), e)),
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ shortcuts

    fn shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let cmd = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let mut sc = |m: Modifiers, k: Key, a: Action| {
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k))) {
                actions.push(a);
            }
        };
        sc(cmd | shift, Key::Tab, Action::CycleTab(-1));
        sc(cmd, Key::Tab, Action::CycleTab(1));
        sc(cmd | shift, Key::C, Action::CopyWord);
        sc(cmd | shift, Key::A, Action::CopyArticle(None));
        sc(cmd | shift, Key::F, Action::OpenFind);
        sc(cmd | shift, Key::O, Action::AddFolderDialog);
        sc(
            cmd | shift,
            Key::M,
            Action::OpenCenter(super::center::TAB_CATALOG),
        );
        sc(cmd | shift, Key::L, Action::AddFromLink);
        sc(shift, Key::F3, Action::FindStep(-1));
        sc(Modifiers::NONE, Key::F3, Action::FindStep(1));
        sc(Modifiers::NONE, Key::F11, Action::Fullscreen);
        sc(Modifiers::NONE, Key::F1, Action::Shortcuts);
        sc(cmd, Key::F, Action::FocusSearch);
        sc(cmd, Key::T, Action::NewTab);
        sc(cmd, Key::D, Action::ToggleFavorite);
        sc(cmd, Key::R, Action::Random);
        sc(cmd, Key::S, Action::SaveArticle);
        sc(cmd, Key::B, Action::TogglePref("appearance.show_sidebar"));
        sc(cmd, Key::Equals, Action::Zoom(0.1));
        sc(cmd, Key::Plus, Action::Zoom(0.1));
        sc(cmd, Key::Minus, Action::Zoom(-0.1));
        sc(cmd, Key::Num0, Action::ZoomReset);
        sc(cmd, Key::Comma, Action::OpenPreferences);
        sc(cmd, Key::O, Action::AddDictionaryDialog);
        sc(cmd, Key::M, Action::OpenManager);
        sc(cmd, Key::Q, Action::Quit);
        sc(Modifiers::ALT, Key::ArrowLeft, Action::NavBack);
        sc(Modifiers::ALT, Key::ArrowRight, Action::NavForward);
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(cmd, Key::W))) {
            actions.push(Action::CloseTab(self.current));
        }
        if self.fullscreen && ctx.input(|i| i.key_pressed(Key::Escape)) && !self.find_open {
            actions.push(Action::Fullscreen);
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            actions.push(Action::AddPaths(dropped));
        }
    }

    // ------------------------------------------------------------ panels

    fn menu_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let item = |ui: &mut egui::Ui, text: &str, shortcut: &str| -> bool {
            let clicked = ui
                .add(egui::Button::new(text).shortcut_text(shortcut))
                .clicked();
            if clicked {
                ui.close_menu();
            }
            clicked
        };
        let check = |ui: &mut egui::Ui, on: bool, text: &str, shortcut: &str| -> bool {
            let label = format!("{} {}", if on { "☑" } else { "☐" }, text);
            let clicked = ui
                .add(egui::Button::new(label).shortcut_text(shortcut))
                .clicked();
            if clicked {
                ui.close_menu();
            }
            clicked
        };
        egui::menu::bar(ui, |ui| {
            ui.menu_button(tr("Dosya"), |ui| {
                if item(ui, &tr("Makaleyi HTML olarak kaydet…"), "Ctrl+S") {
                    actions.push(Action::SaveArticle);
                }
                if item(ui, &tr("Favorileri dışa aktar…"), "") {
                    actions.push(Action::ExportFavorites);
                }
                ui.separator();
                if item(ui, &tr("Masaüstü kısayolu oluştur"), "") {
                    actions.push(Action::CreateShortcuts);
                }
                if item(ui, &tr("Çıkış"), "Ctrl+Q") {
                    actions.push(Action::Quit);
                }
            });
            ui.menu_button(tr("Düzen"), |ui| {
                if item(ui, &tr("Kelimeyi kopyala"), "Ctrl+Shift+C") {
                    actions.push(Action::CopyWord);
                }
                if item(ui, &tr("Makaleyi metin olarak kopyala"), "Ctrl+Shift+A") {
                    actions.push(Action::CopyArticle(None));
                }
                ui.separator();
                if item(ui, &tr("Arama kutusuna odaklan"), "Ctrl+F") {
                    actions.push(Action::FocusSearch);
                }
                if item(ui, &tr("Sayfada bul…"), "Ctrl+Shift+F") {
                    actions.push(Action::OpenFind);
                }
                if item(ui, &tr("Sonraki eşleşme"), "F3") {
                    actions.push(Action::FindStep(1));
                }
                if item(ui, &tr("Önceki eşleşme"), "Shift+F3") {
                    actions.push(Action::FindStep(-1));
                }
                ui.separator();
                if item(ui, &tr("Tercihler…"), "Ctrl+,") {
                    actions.push(Action::OpenPreferences);
                }
            });
            ui.menu_button(tr("Görünüm"), |ui| {
                if item(ui, &tr("Yeni sekme"), "Ctrl+T") {
                    actions.push(Action::NewTab);
                }
                if item(ui, &tr("Sekmeyi kapat"), "Ctrl+W") {
                    actions.push(Action::CloseTab(self.current));
                }
                if item(ui, &tr("Tüm makaleleri aç"), "") {
                    actions.push(Action::ExpandAll(true));
                }
                if item(ui, &tr("Tüm makaleleri kapat"), "") {
                    actions.push(Action::ExpandAll(false));
                }
                ui.separator();
                for (key, label, sc) in [
                    ("appearance.show_toolbar", tr("Araç çubuğu"), ""),
                    ("appearance.show_dictionary_bar", tr("Sözlük çubuğu"), ""),
                    ("appearance.show_sidebar", tr("Yan panel"), "Ctrl+B"),
                    ("appearance.show_found_in", tr("Bulunduğu sözlükler"), ""),
                    ("appearance.show_statusbar", tr("Durum çubuğu"), ""),
                ] {
                    if check(ui, self.prefs.bool(key), &label, sc) {
                        actions.push(Action::TogglePref(key));
                    }
                }
                ui.separator();
                if check(ui, self.fullscreen, &tr("Tam ekran"), "F11") {
                    actions.push(Action::Fullscreen);
                }
                if check(
                    ui,
                    self.prefs.bool("general.always_on_top"),
                    &tr("Her zaman üstte"),
                    "",
                ) {
                    actions.push(Action::TogglePref("general.always_on_top"));
                }
                let scan = self.prefs.bool("scan.enabled");
                if check(ui, scan, &tr("Tarama"), "") {
                    actions.push(Action::SetScan(!scan));
                }
                if item(ui, &tr("Panodaki kelimeyi tarama penceresinde ara"), "") {
                    actions.push(Action::PopupLookup(String::new()));
                }
                if self.services.has_tray() && item(ui, &tr("Tepsiye gizle"), "") {
                    actions.push(Action::ToggleMain);
                }
                ui.separator();
                if item(ui, &tr("Büyüt"), "Ctrl+=") {
                    actions.push(Action::Zoom(0.1));
                }
                if item(ui, &tr("Küçült"), "Ctrl+-") {
                    actions.push(Action::Zoom(-0.1));
                }
                if item(ui, &tr("Yakınlaştırmayı sıfırla"), "Ctrl+0") {
                    actions.push(Action::ZoomReset);
                }
                ui.menu_button(tr("Tema"), |ui| {
                    let cur = self.prefs.str("appearance.theme");
                    for (v, l) in [
                        ("auto", tr("Otomatik")),
                        ("light", tr("Açık")),
                        ("dark", tr("Koyu")),
                    ] {
                        if ui.radio(cur == v, l).clicked() {
                            actions.push(Action::SetTheme(v));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.menu_button(tr("Git"), |ui| {
                if item(ui, &tr("Geri"), "Alt+←") {
                    actions.push(Action::NavBack);
                }
                if item(ui, &tr("İleri"), "Alt+→") {
                    actions.push(Action::NavForward);
                }
                if item(ui, &tr("Rastgele kelime"), "Ctrl+R") {
                    actions.push(Action::Random);
                }
                ui.separator();
                if item(ui, &tr("Sonraki sekme"), "Ctrl+Tab") {
                    actions.push(Action::CycleTab(1));
                }
                if item(ui, &tr("Önceki sekme"), "Ctrl+Shift+Tab") {
                    actions.push(Action::CycleTab(-1));
                }
            });
            // everything about dictionaries; the Dictionaries window (Ctrl+M) has the rest
            ui.menu_button(tr("Sözlükler"), |ui| {
                use super::center::{
                    TAB_CATALOG, TAB_FOLDERS, TAB_MINE, TAB_SETTINGS, TAB_SOURCES, TAB_TRANSFER,
                };
                if item(ui, &tr("Sözlükler…"), "Ctrl+M") {
                    actions.push(Action::OpenCenter(TAB_MINE));
                }
                ui.separator();
                if item(ui, &tr("Sözlük Ekle…"), "Ctrl+O") {
                    actions.push(Action::AddDictionaryDialog);
                }
                if item(ui, &tr("Klasör Ekle…"), "Ctrl+Shift+O") {
                    actions.push(Action::AddFolderDialog);
                }
                if item(ui, &tr("Bağlantıdan sözlük ekle…"), "Ctrl+Shift+L") {
                    actions.push(Action::AddFromLink);
                }
                if item(ui, &tr("Sözlük klasörleri…"), "") {
                    actions.push(Action::OpenCenter(TAB_FOLDERS));
                }
                ui.separator();
                if item(ui, &tr("Sözlük indir (katalog)…"), "Ctrl+Shift+M") {
                    actions.push(Action::OpenCenter(TAB_CATALOG));
                }
                if item(ui, &tr("Kaynaklar (sunucular)…"), "") {
                    actions.push(Action::OpenCenter(TAB_SOURCES));
                }
                if item(ui, &tr("Kurulumu dışa / içe aktar…"), "") {
                    actions.push(Action::OpenCenter(TAB_TRANSFER));
                }
                ui.separator();
                if item(ui, &tr("Tümünü etkinleştir"), "") {
                    actions.push(Action::EnableAll(true));
                }
                if item(ui, &tr("Tümünü devre dışı bırak"), "") {
                    actions.push(Action::EnableAll(false));
                }
                ui.separator();
                if item(ui, &tr("Sözlük ayarları…"), "") {
                    actions.push(Action::OpenCenter(TAB_SETTINGS));
                }
            });
            ui.menu_button(tr("Yardım"), |ui| {
                if item(ui, &tr("Kısayollar"), "F1") {
                    actions.push(Action::Shortcuts);
                }
                if item(ui, &tr("Komut satırı (CLI)"), "") {
                    actions.push(Action::CliHelp);
                }
                if item(ui, &tr("Güncellemeleri denetle…"), "") {
                    actions.push(Action::CheckUpdates);
                }
                if item(ui, &tr("Veri klasörünü aç"), "") {
                    actions.push(Action::OpenDataDir);
                }
                ui.separator();
                if item(ui, &tr("Hakkında"), "") {
                    actions.push(Action::About);
                }
            });
        });
    }

    /// One compact row like GoldenDict: navigation · the dictionary bar
    /// (icons only) · tools on the right. The search box is in the side pane.
    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let show_buttons = self.prefs.bool("appearance.show_toolbar");
        let show_dicts = self.prefs.bool("appearance.show_dictionary_bar");
        uiicons::flat(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            if show_buttons {
                let can_back = self.nav_cursor > 0;
                let can_fwd = self.nav_cursor + 1 < self.nav.len() as isize;
                if uiicons::button(ui, "back", &tr("Geri (Alt+←)"), false, can_back).clicked() {
                    actions.push(Action::NavBack);
                }
                if uiicons::button(ui, "forward", &tr("İleri (Alt+→)"), false, can_fwd).clicked()
                {
                    actions.push(Action::NavForward);
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }
            // room for the tools on the right
            let right_w = if show_buttons { 300.0 } else { 70.0 };
            if show_dicts {
                let width = (ui.available_width() - right_w).max(60.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(width, ui.available_height()),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("dictbar")
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                ui.horizontal(|ui| self.dictionary_bar(ui, actions))
                            });
                    },
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                self.language_button(ui, actions);
                if !show_buttons {
                    return;
                }
                if uiicons::button(ui, "settings", &tr("Tercihler (Ctrl+,)"), false, true).clicked()
                {
                    actions.push(Action::OpenPreferences);
                }
                if uiicons::button(
                    ui,
                    "center",
                    &tr("Sözlükler: listem, klasörler, sözlük indir, kaynaklar, içe / dışa aktar, ayarlar (Ctrl+M)"),
                    self.center_open,
                    true,
                )
                .clicked()
                {
                    actions.push(Action::OpenCenter(0));
                }
                let scan = self.prefs.bool("scan.enabled");
                if uiicons::button(
                    ui,
                    "scan",
                    &tr("Tarama penceresi: başka uygulamada kopyalanan / seçilen kelimeyi çevirir"),
                    scan,
                    true,
                )
                .clicked()
                {
                    actions.push(Action::SetScan(!scan));
                }
                ui.separator();
                let theme = self.prefs.str("appearance.theme");
                let tip = format!(
                    "{} · {}",
                    tr("Tema: Otomatik → Açık → Koyu"),
                    theme_label(&theme)
                );
                if uiicons::button(ui, "theme", &tip, false, true).clicked() {
                    actions.push(Action::CycleTheme);
                }
                if uiicons::button(ui, "zoom-in", &tr("Büyüt (Ctrl+=)"), false, true).clicked() {
                    actions.push(Action::Zoom(0.1));
                }
                if uiicons::button(ui, "zoom-reset", &tr("Normal boyut (Ctrl+0)"), false, true)
                    .clicked()
                {
                    actions.push(Action::ZoomReset);
                }
                if uiicons::button(ui, "zoom-out", &tr("Küçült (Ctrl+-)"), false, true).clicked()
                {
                    actions.push(Action::Zoom(-0.1));
                }
                ui.separator();
                let star = if self.is_favorite {
                    "star-filled"
                } else {
                    "star"
                };
                if uiicons::button(ui, star, &tr("Favori (Ctrl+D)"), self.is_favorite, true)
                    .clicked()
                {
                    actions.push(Action::ToggleFavorite);
                }
                if uiicons::button(ui, "random", &tr("Rastgele kelime (Ctrl+R)"), false, true)
                    .clicked()
                {
                    actions.push(Action::Random);
                }
            });
        });
    }

    /// The search box and the word list (GoldenDict's search pane).
    fn search_pane(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let id = egui::Id::new("search-box");
        let edit = egui::TextEdit::singleline(&mut self.search)
            .hint_text(tr("Bir kelime ara…   (Ctrl+F)"))
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(6, 4))
            .id(id);
        let mut out = edit.show(ui);
        let resp = out.response.clone();
        if self.focus_search {
            resp.request_focus();
            self.focus_search = false;
        }
        if resp.gained_focus() && self.prefs.bool("search.select_all_on_focus") {
            let n = self.search.chars().count();
            out.state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(n),
                )));
            out.state.store(ui.ctx(), id);
        }
        if resp.changed() {
            self.edited_at = Some(Instant::now());
            self.show_suggestions = true;
        }
        let has_list = !self.search.trim().is_empty() && !self.suggestions.is_empty();
        if resp.has_focus() && has_list {
            if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                self.sugg_index = Some(
                    self.sugg_index
                        .map(|i| (i + 1).min(self.suggestions.len() - 1))
                        .unwrap_or(0),
                );
            }
            if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                self.sugg_index = self.sugg_index.and_then(|i| i.checked_sub(1));
            }
        }
        if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.sugg_index = None;
        }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            match self.sugg_index.and_then(|i| self.suggestions.get(i)) {
                Some(s) if has_list => actions.push(Action::OpenWord(s.word.clone())),
                _ => {
                    let new_tab = ui.input(|i| i.modifiers.command);
                    actions.push(Action::Explicit { new_tab });
                }
            }
            self.focus_search = true;
        }
        ui.add_space(3.0);
        egui::Frame::NONE
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .inner_margin(egui::Margin::symmetric(2, 2))
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                egui::ScrollArea::vertical()
                    .id_salt("word-list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.set_width(ui.available_width());
                        if self.search.trim().is_empty() {
                            // an empty box: the last searched words
                            for w in self.recent.iter().take(30) {
                                let r = ui.add(
                                    egui::Button::new(RichText::new(w).weak())
                                        .frame(false)
                                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                                );
                                if r.clicked() {
                                    actions.push(Action::OpenWord(w.clone()));
                                }
                            }
                            return;
                        }
                        for (i, s) in self.suggestions.iter().enumerate() {
                            let mut rt = RichText::new(&s.word);
                            if s.fuzzy {
                                rt = rt.italics().weak();
                            }
                            let sel = self.sugg_index == Some(i);
                            let r = ui.add(
                                egui::Button::new(rt)
                                    .selected(sel)
                                    .frame(sel)
                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                            );
                            if sel {
                                r.scroll_to_me(None);
                            }
                            let r = if s.fuzzy {
                                r.on_hover_text(tr("Benzer kelime (bulanık eşleşme)"))
                            } else {
                                r
                            };
                            if r.clicked() {
                                actions.push(Action::OpenWord(s.word.clone()));
                            }
                        }
                    });
            });
    }

    /// One icon per dictionary: click = go to its article, Ctrl+click =
    /// on / off, right click = menu, drag = change the order. A switched-off
    /// dictionary is drawn faded.
    fn dictionary_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let names = self.qm.loaded_names();
        if names.is_empty() {
            ui.weak(if self.loader.is_some() {
                tr("Sözlükler yükleniyor…")
            } else {
                tr("Sözlük yok — Ctrl+O ile ekleyin")
            });
            return;
        }
        let tip = tr(
            "Tıkla: makalesine git · Ctrl+tık: aç/kapat · Sağ tık: menü · Sürükle: sırayı değiştir",
        );
        for (i, n) in names.iter().enumerate() {
            let on = self.qm.is_enabled(n);
            let look = self.dict_looks.get(ui.ctx(), &self.qm, n);
            let size = egui::vec2(26.0, 26.0);
            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            if r.hovered() {
                ui.painter().rect_filled(
                    rect.shrink(1.0),
                    3.0,
                    ui.visuals().widgets.hovered.weak_bg_fill,
                );
            }
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(4.0))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let b = super::dictbadge::badge(&mut child, &look, 18.0);
            if !on {
                ui.painter()
                    .rect_filled(b.rect, 3.0, ui.visuals().panel_fill.gamma_multiply(0.7));
            }
            if r.drag_started() {
                r.dnd_set_drag_payload(DictDrag(n.clone()));
            }
            if let Some(p) = r.dnd_hover_payload::<DictDrag>() {
                if p.0 != *n {
                    let x = if ui
                        .input(|i| i.pointer.hover_pos())
                        .is_some_and(|p| p.x > rect.center().x)
                    {
                        rect.right()
                    } else {
                        rect.left()
                    };
                    ui.painter().vline(
                        x,
                        rect.y_range(),
                        egui::Stroke::new(2.0_f32, ui.visuals().selection.stroke.color),
                    );
                }
            }
            if let Some(p) = r.dnd_release_payload::<DictDrag>() {
                let right = ui
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|p| p.x > rect.center().x);
                let target = if right {
                    names.get(i + 1).cloned()
                } else {
                    Some(n.clone())
                };
                if target.as_deref() != Some(p.0.as_str()) {
                    actions.push(Action::MoveBefore(p.0.clone(), target));
                }
            }
            let off = if on {
                String::new()
            } else {
                format!("\n{}", tr("(kapalı)"))
            };
            let r = r.on_hover_text(format!("{}{}\n\n{}", look.tooltip(), off, tip));
            if r.clicked() {
                if ui.input(|i| i.modifiers.command) {
                    actions.push(Action::SetEnabled(n.clone(), !on));
                } else {
                    actions.push(Action::Jump(n.clone()));
                }
            }
            r.context_menu(|ui| self.dictionary_menu(ui, n, actions));
        }
        if uiicons::button(ui, "all", &tr("Tüm sözlükleri etkinleştir"), false, true).clicked()
        {
            actions.push(Action::EnableAll(true));
        }
    }

    /// Right click on a dictionary (dictionary bar, found-in panel, side list).
    fn dictionary_menu(&self, ui: &mut egui::Ui, name: &str, actions: &mut Vec<Action>) {
        let on = self.qm.is_enabled(name);
        let mut b = |ui: &mut egui::Ui, text: String, a: Action| {
            if ui.button(text).clicked() {
                actions.push(a);
                ui.close_menu();
            }
        };
        b(
            ui,
            format!("{} {}", if on { "☑" } else { "☐" }, tr("Etkin")),
            Action::SetEnabled(name.to_string(), !on),
        );
        b(ui, tr("Makalesine git"), Action::Jump(name.to_string()));
        b(
            ui,
            tr("Yalnızca bunu etkinleştir"),
            Action::Solo(name.to_string()),
        );
        b(ui, tr("Tümünü etkinleştir"), Action::EnableAll(true));
        ui.separator();
        b(ui, tr("Bilgi"), Action::Info(name.to_string()));
        b(
            ui,
            tr("Yeniden indeksle"),
            Action::Reindex(vec![name.to_string()]),
        );
        ui.separator();
        b(
            ui,
            tr("Sözlüklerimde göster"),
            Action::ShowInMine(name.to_string(), false),
        );
        b(
            ui,
            tr("Kaldır…"),
            Action::ShowInMine(name.to_string(), true),
        );
    }

    /// Show the current word again (the dictionaries or their order changed).
    fn rerun_current(&mut self) {
        let w = self.current_word();
        if !w.is_empty() {
            self.open_word(&w, None, false);
        }
    }

    fn jump_to_dictionary(&mut self, name: &str) {
        if !self.qm.is_enabled(name) {
            self.set_status(
                tr_args("“{name}” kapalı — açmak için Ctrl+tık", &[("name", &name)]),
                5,
            );
            return;
        }
        let i = self
            .tabs
            .get(self.current)
            .and_then(|t| t.groups().iter().position(|g| g.dict == name));
        match i {
            Some(i) => self.scroll_to_dict = Some(i),
            None => self.set_status(
                tr_args("“{name}” bu kelimede sonuç vermedi", &[("name", &name)]),
                5,
            ),
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.focus_search {
            self.sidebar_tab = SidebarTab::Search;
        }
        // icon tabs (names in the tooltips), like a compact GoldenDict pane
        ui.horizontal(|ui| {
            uiicons::flat(ui);
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = (ui.available_width() / 4.0).floor();
            for (tab, icon, tip) in [
                (SidebarTab::Search, "search", tr("Ara")),
                (SidebarTab::Dictionaries, "dict", tr("Sözlükler")),
                (SidebarTab::Favorites, "star", tr("Favoriler")),
                (SidebarTab::History, "history", tr("Geçmiş")),
            ] {
                let on = self.sidebar_tab == tab;
                let tint = if on {
                    ui.visuals().strong_text_color()
                } else {
                    ui.visuals().weak_text_color()
                };
                let Some(img) = uiicons::image(ui.ctx(), icon, 16.0, tint) else {
                    continue;
                };
                let r = ui
                    .add(egui::Button::image(img).min_size(egui::vec2(w, 24.0)))
                    .on_hover_text(tip);
                if on {
                    let rr = r.rect;
                    ui.painter().line_segment(
                        [rr.left_bottom(), rr.right_bottom()],
                        egui::Stroke::new(2.0_f32, ui.visuals().hyperlink_color),
                    );
                }
                if r.clicked() {
                    self.sidebar_tab = tab;
                }
            }
        });
        ui.add_space(3.0);
        let slot = match self.sidebar_tab {
            SidebarTab::Search => {
                self.search_pane(ui, actions);
                return;
            }
            SidebarTab::Dictionaries => 0,
            SidebarTab::Favorites => 1,
            SidebarTab::History => 2,
        };
        ui.add(
            egui::TextEdit::singleline(&mut self.side_filter[slot])
                .hint_text(tr("Süz…"))
                .desired_width(f32::INFINITY),
        );
        let filter = self.side_filter[slot].to_lowercase();
        let keep = |s: &str| filter.is_empty() || s.to_lowercase().contains(&filter);
        match self.sidebar_tab {
            SidebarTab::Search => {}
            SidebarTab::Dictionaries => {
                let names = self.qm.loaded_names();
                let enabled = names.iter().filter(|n| self.qm.is_enabled(n)).count();
                ui.horizontal(|ui| {
                    ui.strong(format!("{} ({}/{})", tr("Sözlükler"), enabled, names.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(tr("Sözlükler…")).clicked() {
                            actions.push(Action::OpenManager);
                        }
                    });
                });
                egui::ScrollArea::vertical().id_salt("side-dicts").show(ui, |ui| {
                    if names.is_empty() {
                        ui.weak(tr("Henüz sözlük yok.\nDosya → Sözlük Ekle (Ctrl+O)\nveya dosyayı pencereye sürükleyin."));
                    }
                    for name in names.into_iter().filter(|n| keep(n)) {
                        let mut on = self.qm.is_enabled(&name);
                        let count = self.qm.dictionary(&name).map(|d| d.metadata().word_count).unwrap_or(0);
                        let _ = count;
                        let look = self.dict_looks.get(ui.ctx(), &self.qm, &name);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut on, "").changed() {
                                actions.push(Action::SetEnabled(name.clone(), on));
                            }
                            super::dictbadge::badge(ui, &look, 18.0);
                            let r = ui
                                .add(egui::Label::new(&name).sense(egui::Sense::click_and_drag()).truncate())
                                .on_hover_text(format!(
                                    "{}\n\n{}",
                                    look.tooltip(),
                                    tr("Çift tık: bilgiler · Sürükle: sırayı değiştir · Sağ tık: menü")
                                ));
                            if r.double_clicked() {
                                actions.push(Action::Info(name.clone()));
                            }
                            if r.drag_started() {
                                r.dnd_set_drag_payload(DictDrag(name.clone()));
                            }
                            if let Some(p) = r.dnd_hover_payload::<DictDrag>() {
                                if p.0 != name {
                                    ui.painter().hline(
                                        r.rect.x_range(),
                                        r.rect.top() - 1.0,
                                        egui::Stroke::new(2.0_f32, ui.visuals().selection.stroke.color),
                                    );
                                }
                            }
                            if let Some(p) = r.dnd_release_payload::<DictDrag>() {
                                if p.0 != name {
                                    actions.push(Action::MoveBefore(p.0.clone(), Some(name.clone())));
                                }
                            }
                            r.context_menu(|ui| self.dictionary_menu(ui, &name, actions));
                        });
                    }
                    // dictionaries of the list that could not be opened: kept, marked ⚠
                    let failed: Vec<(String, String)> = self
                        .qm
                        .list_rows()
                        .into_iter()
                        .filter(|r| !r.loaded && keep(&r.name))
                        .map(|r| (r.name, r.error))
                        .collect();
                    for (name, error) in failed {
                        let red = ui.visuals().error_fg_color;
                        let r = ui
                            .add(
                                egui::Label::new(RichText::new(format!("⚠ {}", name)).color(red))
                                    .sense(egui::Sense::click())
                                    .truncate(),
                            )
                            .on_hover_text(format!("{}: {}", tr("açılamadı"), error));
                        r.context_menu(|ui| {
                            if ui.button(tr("Kaldır…")).clicked() {
                                actions.push(Action::Ask(Confirm::RemoveDict(name.clone())));
                                ui.close_menu();
                            }
                        });
                    }
                });
            }
            SidebarTab::Favorites => {
                ui.horizontal(|ui| {
                    ui.strong(format!("Favoriler ({})", self.favorites.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(tr("Dışa aktar…")).clicked() {
                            actions.push(Action::ExportFavorites);
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("side-favs")
                    .show(ui, |ui| {
                        if self.favorites.is_empty() {
                            ui.weak(tr("Favori yok. Bir kelimeyi Ctrl+D ile ekleyin."));
                        }
                        for w in self.favorites.iter().filter(|w| keep(w)) {
                            let r = ui.selectable_label(false, w);
                            if r.clicked() {
                                actions.push(Action::OpenWord(w.clone()));
                            }
                            r.context_menu(|ui| {
                                if ui.button(tr("Yeni sekmede aç")).clicked() {
                                    actions.push(Action::OpenWordIn {
                                        word: w.clone(),
                                        dicts: self.qm.loaded_names(),
                                    });
                                    ui.close_menu();
                                }
                                if ui.button(tr("Favorilerden çıkar")).clicked() {
                                    actions.push(Action::RemoveFavorite(w.clone()));
                                    ui.close_menu();
                                }
                            });
                        }
                    });
            }
            SidebarTab::History => {
                ui.horizontal(|ui| {
                    ui.strong(format!("{} ({})", tr("Geçmiş"), self.recent.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(tr("Temizle")).clicked() {
                            actions.push(Action::Ask(Confirm::ClearHistory));
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("side-hist")
                    .show(ui, |ui| {
                        if !self.prefs.bool("history.enabled") {
                            ui.weak(tr("Geçmiş kaydı kapalı (Tercihler → Geçmiş)."));
                        }
                        for w in self.recent.iter().filter(|w| keep(w)) {
                            let r = ui.selectable_label(false, w);
                            if r.clicked() {
                                actions.push(Action::OpenWord(w.clone()));
                            }
                            r.context_menu(|ui| {
                                if ui.button(tr("Geçmişten sil")).clicked() {
                                    actions.push(Action::RemoveHistory(w.clone()));
                                    ui.close_menu();
                                }
                            });
                        }
                    });
            }
        }
    }

    fn language_button(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let cur = i18n::current();
        let setting = self.prefs.str("general.language");
        let flag_size = egui::vec2(24.0, 18.0);
        let button = match self.flags.get(ui.ctx(), &cur) {
            Some(tex) => egui::Button::image_and_text(
                egui::Image::new(&tex).fit_to_exact_size(flag_size),
                cur.to_uppercase(),
            ),
            None => egui::Button::new(cur.to_uppercase()),
        };
        let mut textures = Vec::new();
        for (code, ..) in i18n::LANGUAGES {
            textures.push(self.flags.get(ui.ctx(), code));
        }
        let r = egui::menu::menu_custom_button(ui, button, |ui| {
            ui.set_min_width(250.0);
            let auto = format!(
                "{} — {}",
                tr("Otomatik (sistem)"),
                i18n::native_name(&i18n::system_language())
            );
            if ui.selectable_label(setting == "auto", auto).clicked() {
                actions.push(Action::SetLanguage("auto".into()));
                ui.close_menu();
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(480.0)
                .show(ui, |ui| {
                    for (i, (code, native, english, _)) in i18n::LANGUAGES.iter().enumerate() {
                        let text = format!("{}   ({})", native, english);
                        let text = if i18n::is_rtl(code) {
                            format!("{}   ({})", i18n::visual(native), english)
                        } else {
                            text
                        };
                        let clicked = match &textures[i] {
                            Some(tex) => ui
                                .add(
                                    egui::Button::image_and_text(
                                        egui::Image::new(tex).fit_to_exact_size(flag_size),
                                        text,
                                    )
                                    .selected(setting == *code)
                                    .frame(false),
                                )
                                .clicked(),
                            None => ui.selectable_label(setting == *code, text).clicked(),
                        };
                        if clicked {
                            actions.push(Action::SetLanguage(code.to_string()));
                            ui.close_menu();
                        }
                    }
                });
        });
        r.response.on_hover_text(format!(
            "{}: {}",
            tr("Arayüz dili"),
            i18n::native_name(&cur)
        ));
    }

    fn found_in(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.strong(tr("Bulunduğu sözlükler"));
        ui.separator();
        let Some(tab) = self.tabs.get(self.current) else {
            return;
        };
        let groups = tab.groups();
        if groups.is_empty() {
            ui.weak("—");
        }
        let looks: Vec<_> = groups
            .iter()
            .map(|g| self.dict_looks.get(ui.ctx(), &self.qm, &g.dict))
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("found-in")
            .show(ui, |ui| {
                for (i, g) in groups.iter().enumerate() {
                    let label = if g.articles.len() > 1 {
                        format!("{}  ({})", g.dict, g.articles.len())
                    } else {
                        g.dict.clone()
                    };
                    let row = ui.horizontal(|ui| {
                        super::dictbadge::badge(ui, &looks[i], 18.0);
                        ui.add(
                            egui::Button::new(label)
                                .frame(false)
                                .truncate()
                                .sense(egui::Sense::click_and_drag()),
                        )
                    });
                    let r = row.inner.on_hover_text(format!(
                        "{}\n\n{}",
                        looks[i].tooltip(),
                        tr("Tıkla: makaleye git · Sürükle: sırayı değiştir · Sağ tık: menü")
                    ));
                    if r.clicked() {
                        actions.push(Action::ScrollToDict(i));
                    }
                    if r.drag_started() {
                        r.dnd_set_drag_payload(DictDrag(g.dict.clone()));
                    }
                    let whole = row.response.union(r.clone());
                    if let Some(p) = whole.dnd_hover_payload::<DictDrag>() {
                        if p.0 != g.dict {
                            ui.painter().hline(
                                whole.rect.x_range(),
                                whole.rect.top(),
                                egui::Stroke::new(2.0_f32, ui.visuals().selection.stroke.color),
                            );
                        }
                    }
                    if let Some(p) = whole.dnd_release_payload::<DictDrag>() {
                        if p.0 != g.dict {
                            actions.push(Action::MoveBefore(p.0.clone(), Some(g.dict.clone())));
                        }
                    }
                    r.context_menu(|ui| self.dictionary_menu(ui, &g.dict, actions));
                }
            });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.tabs.len() <= 1 {
            return; // one tab: no tab row (like GoldenDict); Ctrl+T opens another
        }
        let max = self.prefs.int("appearance.tab_title_length").max(4) as usize;
        egui::ScrollArea::horizontal()
            .id_salt("tabbar")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let selected = i == self.current;
                        egui::Frame::NONE
                            .fill(if selected {
                                ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let r = ui
                                        .selectable_label(selected, tab.title(max))
                                        .on_hover_text(&tab.word);
                                    if r.clicked() {
                                        actions.push(Action::SelectTab(i));
                                    }
                                    if r.middle_clicked() {
                                        actions.push(Action::CloseTab(i));
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text(tr("Sekmeyi kapat (Ctrl+W)"))
                                        .clicked()
                                    {
                                        actions.push(Action::CloseTab(i));
                                    }
                                });
                            });
                    }
                    if ui
                        .button("+")
                        .on_hover_text(tr("Yeni sekme (Ctrl+T)"))
                        .clicked()
                    {
                        actions.push(Action::NewTab);
                    }
                });
            });
        ui.separator();
    }

    fn find_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.label(tr("Sayfada bul:"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.find_text)
                    .desired_width(220.0)
                    .id(egui::Id::new("find-box")),
            );
            if self.focus_find {
                r.request_focus();
                self.focus_find = false;
            }
            if r.changed() {
                self.find_current = 0;
                self.find_scroll = true;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let back = ui.input(|i| i.modifiers.shift);
                actions.push(Action::FindStep(if back { -1 } else { 1 }));
                self.focus_find = true;
            }
            if self.find_text.is_empty() {
                ui.weak("");
            } else if self.find_count == 0 {
                ui.colored_label(ui.visuals().error_fg_color, tr("bulunamadı"));
            } else {
                ui.label(format!("{}/{}", self.find_current + 1, self.find_count));
            }
            if ui
                .button("▲")
                .on_hover_text(tr("Önceki (Shift+F3)"))
                .clicked()
            {
                actions.push(Action::FindStep(-1));
            }
            if ui.button("▼").on_hover_text(tr("Sonraki (F3)")).clicked() {
                actions.push(Action::FindStep(1));
            }
            if ui.button("✕").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                self.find_open = false;
                self.find_text.clear();
            }
        });
        ui.separator();
    }

    fn content(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // fonts registered with set_fonts become usable on the next frame
        let ready = ui.fonts(|f| {
            let fam = f.families();
            fam.contains(&FontFamily::Name(fonts::BOLD.into()))
                && fam.contains(&FontFamily::Name(fonts::ARTICLE.into()))
        });
        if !ready {
            ui.ctx().request_repaint();
            return;
        }
        let p = &self.prefs;
        let size = p.int("article.font_size") as f32 * p.float("article.zoom") as f32;
        let line_spacing = p.float("article.line_spacing") as f32;
        let family = if self
            .fonts_key
            .as_ref()
            .map(|k| k.1.trim().is_empty())
            .unwrap_or(true)
        {
            FontFamily::Proportional
        } else {
            FontFamily::Name(fonts::ARTICLE.into())
        };
        let opts = render::Opts {
            size,
            row_gap: (size * (line_spacing - 1.0) * 0.6).max(0.0),
            family,
            show_images: p.bool("article.show_images"),
            max_image_height: p.int("article.max_image_height") as f32,
            find: if self.find_open {
                render::lower_chars(self.find_text.trim())
            } else {
                Vec::new()
            },
            highlight: prefs_window::parse_hex(&p.str("article.highlight_color"))
                .unwrap_or(Color32::YELLOW),
        };
        let head_scale = p.float("article.headword_scale") as f32;
        let show_headers = p.bool("article.show_dict_headers");
        let collapsed = p.bool("article.collapsed");
        let max_width = p.int("article.max_width") as f32;
        let accent = ui.visuals().hyperlink_color;
        let mut find = render::Find {
            counter: 0,
            current: self.find_current,
            scroll: self.find_scroll,
        };
        let expand_all = self.expand_all.take();
        let scroll_to = self.scroll_to_dict.take();
        let dark = ui.visuals().dark_mode;
        let looks: Vec<_> = match self.tabs.get(self.current).map(|t| &t.content) {
            Some(Content::Articles(groups)) => groups
                .iter()
                .map(|g| self.dict_looks.get(ui.ctx(), &self.qm, &g.dict))
                .collect(),
            _ => Vec::new(),
        };
        let Some(tab) = self.tabs.get(self.current) else {
            return;
        };
        let tab_gen = tab.generation;
        egui::ScrollArea::vertical()
            .id_salt(("content", self.current, tab_gen))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail = ui.available_width();
                let (pad, width) = if max_width > 0.0 && avail > max_width { ((avail - max_width) / 2.0, max_width) } else { (0.0, avail) };
                ui.horizontal_top(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_max_width(width);
                        ui.set_min_width(width);
                        match &tab.content {
                            Content::Welcome => {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(80.0);
                                    ui.label(RichText::new("dictz").size(size * 2.3).strong());
                                    ui.label(RichText::new(tr("çok formatlı çevrimdışı sözlük")).size(size).weak());
                                    ui.add_space(24.0);
                                    let msg = if self.loader.is_some() {
                                        tr("Sözlükler yükleniyor…").to_string()
                                    } else if self.qm.loaded_names().is_empty() {
                                        tr("Başlamak için Dosya → Sözlük Ekle menüsünden bir StarDict (.ifo),\nDSL (.dsl) veya Babylon (.bgl) dosyası yükleyin ya da dosyayı pencereye sürükleyin.").to_string()
                                    } else {
                                        tr_args("{n} sözlük hazır. Aramak için yazmaya başlayın.", &[("n", &self.qm.loaded_names().len())])
                                    };
                                    ui.label(RichText::new(msg).size(size * 0.9).weak());
                                    ui.add_space(36.0);
                                    ui.label(RichText::new(tr("Ctrl+F ara · Ctrl+T yeni sekme · Ctrl+R rastgele · Ctrl+D favori · F1 kısayollar")).size(size * 0.8).weak());
                                });
                            }
                            Content::NotFound { query, suggestions } => {
                                ui.add_space(24.0);
                                ui.label(RichText::new(tr("Sonuç yok")).size(size * 1.5).strong());
                                ui.label(RichText::new(tr_args("“{word}” için hiçbir sözlükte eşleşme bulunamadı.", &[("word", query)])).size(size));
                                if !suggestions.is_empty() {
                                    ui.add_space(16.0);
                                    ui.label(RichText::new(tr("Bunu mu demek istediniz?")).size(size * 1.1).strong());
                                    ui.add_space(4.0);
                                    for (w, score) in suggestions {
                                        ui.horizontal(|ui| {
                                            let r = ui
                                                .add(egui::Label::new(RichText::new(w).size(size).color(accent).underline()).sense(egui::Sense::click()))
                                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                                            ui.label(RichText::new(format!("  %{:.0}", score)).size(size * 0.8).weak());
                                            if r.clicked() {
                                                actions.push(Action::OpenWord(w.clone()));
                                            }
                                        });
                                    }
                                }
                            }
                            Content::Articles(groups) => {
                                let mut clicks: Vec<(String, Click)> = Vec::new();
                                for (gi, g) in groups.iter().enumerate() {
                                    let id = ui.make_persistent_id(("dict-group", tab_gen, gi));
                                    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, !collapsed);
                                    if let Some(on) = expand_all {
                                        state.set_open(on);
                                    }
                                    if scroll_to == Some(gi) {
                                        state.set_open(true);
                                    }
                                    let mut body = |ui: &mut egui::Ui, find: &mut render::Find| {
                                        for a in &g.articles {
                                            let hw_size = size * head_scale;
                                            let hw_family = FontFamily::Name(fonts::BOLD.into());
                                            render::highlighted(ui, &a.headword, |rt| rt.font(egui::FontId::new(hw_size, hw_family.clone())).strong(), &opts, find, None);
                                            ui.add_space(2.0);
                                            let mut local = Vec::new();
                                            let mut media = MediaCache { qm: &self.qm, dict: &g.dict, cache: &mut self.media_cache };
                                            render::lines(ui, &a.lines, &opts, &g.dict, &mut media, find, &mut local);
                                            clicks.extend(local.into_iter().map(|c| (g.dict.clone(), c)));
                                            ui.add_space(size * 0.35);
                                        }
                                    };
                                    if show_headers {
                                        // GoldenDict-like: a thin strip per dictionary, then the text
                                        let look = looks[gi].clone();
                                        let border = ui.visuals().widgets.noninteractive.bg_stroke.color;
                                        let open = state.is_open();
                                        let hdr = egui::Frame::NONE
                                            .fill(look.tint(dark))
                                            .inner_margin(egui::Margin { left: 9, right: 6, top: 2, bottom: 2 })
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;
                                                    super::dictbadge::badge(ui, &look, size * 1.05);
                                                    let name = ui
                                                        .add(egui::Label::new(RichText::new(&g.dict).size(size * 0.82).strong().color(ui.visuals().strong_text_color())).sense(egui::Sense::click()).truncate())
                                                        .on_hover_text(look.tooltip())
                                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                                    if name.clicked() {
                                                        state.toggle(ui);
                                                    }
                                                    if scroll_to == Some(gi) {
                                                        name.scroll_to_me(Some(egui::Align::TOP));
                                                    }
                                                    if let Some((a, b)) = &look.langs {
                                                        ui.label(RichText::new(format!("{} → {}", a, b)).size(size * 0.72).color(look.ink(dark)));
                                                    }
                                                    if g.articles.len() > 1 {
                                                        ui.label(RichText::new(tr_args("{n} madde", &[("n", &g.articles.len())])).size(size * 0.72).weak());
                                                    }
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        uiicons::flat(ui);
                                                        ui.spacing_mut().item_spacing.x = 0.0;
                                                        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                                                        let tint = ui.visuals().weak_text_color();
                                                        let small = size * 0.85;
                                                        if let Some(img) = uiicons::image(ui.ctx(), "search", small, tint) {
                                                            if ui.add(egui::Button::image(img)).on_hover_text(tr("Bu sözlükte yeni sekmede ara")).clicked() {
                                                                actions.push(Action::OpenWordIn { word: tab.word.clone(), dicts: vec![g.dict.clone()] });
                                                            }
                                                        }
                                                        if let Some(img) = uiicons::image(ui.ctx(), "copy", small, tint) {
                                                            if ui.add(egui::Button::image(img)).on_hover_text(tr("Bu makaleyi metin olarak kopyala")).clicked() {
                                                                actions.push(Action::CopyArticle(Some(gi)));
                                                            }
                                                        }
                                                    });
                                                });
                                            });
                                        let r = hdr.response.rect;
                                        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(3.0, r.height())), 0.0, look.color);
                                        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], egui::Stroke::new(1.0_f32, border));
                                        ui.painter().line_segment([r.left_top(), r.right_top()], egui::Stroke::new(1.0_f32, border));
                                        state.show_body_unindented(ui, |ui| {
                                            egui::Frame::NONE
                                                .inner_margin(egui::Margin { left: 14, right: 12, top: 4, bottom: 6 })
                                                .show(ui, |ui| {
                                                    ui.set_width(ui.available_width());
                                                    body(ui, &mut find);
                                                });
                                        });
                                        if !open && !opts.find.is_empty() {
                                            // still count hidden matches so n/m stays right
                                            find.counter += g.articles.iter().map(|a| render::count_matches(&a.lines, &opts.find) + render::find_ranges(&a.headword, &opts.find).len()).sum::<usize>();
                                        }
                                    } else {
                                        body(ui, &mut find);
                                    }
                                }
                                for (d, c) in clicks {
                                    actions.push(Action::Click(d, c));
                                }
                            }
                        }
                        ui.add_space(24.0);
                    });
                });
            });
        self.find_count = find.counter;
        self.find_scroll = false;
        if self.find_current >= self.find_count.max(1) {
            self.find_current = 0;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(st) = self.load_progress.clone().filter(|_| self.loader.is_some()) {
                // the load: text, a bar with the share done (the running
                // dictionaries' parts counted) and a stop button
                use crate::loading::Phase;
                let scanning = st.phase == Phase::Scan && st.total == 0;
                let mut text = if scanning {
                    tr("Klasörler taranıyor…").to_string()
                } else {
                    let names = if st.running.is_empty() {
                        "…".to_string()
                    } else {
                        st.names()
                    };
                    tr_args(
                        "Sözlükler yükleniyor ({i}/{n}): {name}",
                        &[("i", &st.done), ("n", &st.total), ("name", &names)],
                    )
                };
                if st.cancelling {
                    text = format!("{} {}", tr("Durduruluyor…"), text);
                }
                let bar = if scanning {
                    egui::ProgressBar::new(0.0).animate(true)
                } else {
                    egui::ProgressBar::new(st.fraction()).show_percentage()
                };
                ui.add(bar.desired_width(220.0).desired_height(14.0));
                if super::uiicons::button(ui, "close", &tr("Durdur"), false, !st.cancelling)
                    .clicked()
                {
                    if let Some(h) = &self.job {
                        h.cancel();
                    }
                }
                let tip: Vec<String> = st
                    .running
                    .iter()
                    .map(|(n, f)| match f {
                        Some(f) => format!("{} — {}%", n, (f * 100.0) as u32),
                        None => format!("{} — {}", n, tr("okunuyor")),
                    })
                    .collect();
                let label = ui.add(egui::Label::new(&text).truncate());
                if !tip.is_empty() {
                    label.on_hover_text(format!("{}\n\n{}", text, tip.join("\n")));
                }
            } else if let Some((msg, at, secs)) = self
                .status
                .as_ref()
                .filter(|(_, at, secs)| at.elapsed() < Duration::from_secs(*secs))
            {
                let _ = (at, secs);
                ui.label(msg);
            } else if !self.timing.is_empty() {
                // left: the last search
                ui.weak(&self.timing);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.weak(format!(
                    "{}%",
                    (self.prefs.float("article.zoom") * 100.0).round() as i32
                ));
                let names: Vec<String> = self
                    .qm
                    .loaded_names()
                    .into_iter()
                    .filter(|n| self.qm.is_enabled(n))
                    .collect();
                if names.is_empty() {
                    ui.weak(tr("0 sözlük etkin"));
                } else {
                    ui.weak(tr_args(
                        "{n} sözlük · {w} kelime",
                        &[("n", &names.len()), ("w", &group(self.qm.word_count()))],
                    ));
                }
            });
        });
    }

    // ------------------------------------------------------------ windows

    fn windows(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(mut w) = self.prefs_win.take() {
            let mut open = true;
            let events = w.show(ctx, &mut open);
            let mut keep = open;
            for e in events {
                match e {
                    prefs_window::Event::Apply | prefs_window::Event::Ok => {
                        let before = folder_sources(&self.prefs);
                        let autostart_before = self.prefs.bool("general.autostart");
                        let mut p = w.draft.clone();
                        for (k, _) in state_defaults() {
                            if let Some(v) = self.prefs.get(k) {
                                let _ = p.set(k, v.clone());
                            }
                        }
                        self.prefs = p;
                        self.save_prefs();
                        if let Some(c) = &mut self.center {
                            c.set_prefs(&self.prefs);
                        }
                        self.apply_prefs(ctx);
                        self.refresh_lists();
                        if self.prefs.bool("general.autostart") != autostart_before {
                            self.sync_autostart();
                        }
                        if folder_sources(&self.prefs) != before {
                            // new folders: their dictionaries are added now
                            self.rescan_folders(ctx);
                        }
                        if matches!(e, prefs_window::Event::Ok) {
                            keep = false;
                        }
                    }
                    prefs_window::Event::Cancel => keep = false,
                    prefs_window::Event::Action(a) => {
                        self.prefs_win = Some(w);
                        self.advanced(ctx, a);
                        w = self.prefs_win.take().unwrap();
                    }
                }
            }
            if keep {
                self.prefs_win = Some(w);
            }
        }

        let simple = |ctx: &egui::Context,
                      title: &str,
                      open: &mut bool,
                      add: &mut dyn FnMut(&mut egui::Ui)| {
            egui::Window::new(title)
                .open(open)
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| add(ui));
        };

        if self.show_about {
            let mut open = true;
            let data_dir = self.data_dir.display().to_string();
            simple(ctx, &tr("dictz hakkında"), &mut open, &mut |ui| {
                ui.heading(format!("dictz {}", VERSION));
                ui.label(tr("Çok formatlı çevrimdışı sözlük — Rust sürümü."));
                ui.label(tr(
                    "Python sürümüyle aynı özellikler, aynı ayar ve indeks dosyaları.",
                ));
                ui.label("© 2026 Bayram Kotan · GPL-3.0+");
                if ui.link("github.com/bayramkotan/dictz").clicked() {
                    let _ = system::open_external("https://github.com/bayramkotan/dictz");
                }
                ui.weak(format!("{} {}", tr("Veri klasörü:"), data_dir));
            });
            self.show_about = open;
        }

        if self.show_shortcuts {
            let mut open = true;
            simple(ctx, &tr("Kısayollar"), &mut open, &mut |ui| {
                egui::Grid::new("sc-help")
                    .striped(true)
                    .spacing([24.0, 6.0])
                    .show(ui, |ui| {
                        for (k, d) in SHORTCUTS {
                            ui.monospace(*k);
                            ui.label(tr(d));
                            ui.end_row();
                        }
                    });
            });
            self.show_shortcuts = open;
        }

        if self.show_cli {
            let mut open = true;
            simple(ctx, &tr("Komut satırı"), &mut open, &mut |ui| {
                ui.label(tr("dictz komut satırından da tam olarak kullanılabilir (Python sürümünde aynı komutlar):"));
                ui.add_space(4.0);
                #[cfg(feature = "cli")]
                egui::Grid::new("cli-help")
                    .striped(true)
                    .spacing([24.0, 4.0])
                    .show(ui, |ui| {
                        for (c, d) in crate::cli::COMMANDS {
                            ui.monospace(format!("dictz {}", c));
                            ui.label(*d);
                            ui.end_row();
                        }
                    });
                ui.add_space(4.0);
                ui.label(tr("Ayrıntı: dictz help, dictz <komut> --help"));
            });
            self.show_cli = open;
        }

        if let Some(name) = self.info.clone() {
            let mut open = true;
            let text = self.info_text(&name);
            simple(
                ctx,
                &format!("{} — {}", tr("Sözlük bilgisi"), name),
                &mut open,
                &mut |ui| {
                    let mut parts = text.splitn(2, "\n\n");
                    let table = parts.next().unwrap_or("");
                    egui::Grid::new("info-grid")
                        .striped(true)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            for line in table.lines() {
                                if let Some((k, v)) = line.split_once('\t') {
                                    ui.strong(k);
                                    ui.label(v);
                                    ui.end_row();
                                } else {
                                    ui.label("");
                                    ui.label(line);
                                    ui.end_row();
                                }
                            }
                        });
                    if let Some(desc) = parts.next() {
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                ui.label(desc);
                            });
                    }
                },
            );
            if !open {
                self.info = None;
            }
        }

        if let Some(msg) = self.update_msg.clone() {
            let mut close = false;
            egui::Window::new(tr("Güncelleme"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button(tr("Tamam")).clicked() {
                        close = true;
                    }
                });
            if close {
                self.update_msg = None;
            }
        }

        if let Some(c) = self.confirm.clone() {
            let (title, text) = match &c {
                Confirm::ClearHistory => (
                    tr("Geçmişi temizle"),
                    tr("Tüm arama geçmişi silinsin mi?").to_string(),
                ),
                Confirm::ClearFavorites => (
                    tr("Favorileri temizle"),
                    tr("Tüm favoriler silinsin mi?").to_string(),
                ),
                Confirm::RemoveDict(n) => (
                    tr("Sözlüğü kaldır"),
                    tr_args(
                        "“{name}” dictz'den kaldırılsın mı?\n(Dosyası silinmez.)",
                        &[("name", n)],
                    ),
                ),
                Confirm::Quit => (tr("Çıkış"), tr("dictz kapatılsın mı?").to_string()),
            };
            let mut answer = None;
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(text);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("Evet")).clicked() {
                            answer = Some(true);
                        }
                        if ui.button(tr("Hayır")).clicked() {
                            answer = Some(false);
                        }
                    });
                });
            if let Some(yes) = answer {
                self.confirm = None;
                if yes {
                    match c {
                        Confirm::ClearHistory => {
                            if let Some(h) = &self.history {
                                let _ = h.clear_history();
                            }
                            self.refresh_lists();
                        }
                        Confirm::ClearFavorites => {
                            if let Some(h) = &self.history {
                                let _ = h.clear_favorites();
                            }
                            self.refresh_lists();
                        }
                        Confirm::RemoveDict(n) => actions.push(Action::RemoveDict(n)),
                        Confirm::Quit => {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
        }

        if let Some((title, msg)) = self.error.clone() {
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button(tr("Tamam")).clicked() {
                        self.error = None;
                    }
                });
        }
    }

    fn save_state(&mut self) {
        let words: Vec<Value> = self
            .tabs
            .iter()
            .filter(|t| !t.word.is_empty())
            .map(|t| Value::from(t.word.clone()))
            .collect();
        let _ = self.prefs.set("state.open_tabs", Value::Array(words));
        let _ = self
            .prefs
            .set("state.current_tab", Value::from(self.current as i64));
        let _ = self
            .prefs
            .set("state.last_search", Value::from(self.search.trim()));
        self.save_prefs();
    }
}

impl eframe::App for DictzApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_no += 1;
        self.poll_loader(ctx);
        self.poll_updates();
        self.handle_services(ctx);
        if self.frame_no == 3 && self.start_hidden && self.services.has_tray() {
            self.win.hide(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.prefs.bool("general.close_to_tray")
            && self.services.has_tray()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.win.hide(ctx);
            if !self.tray_hint_shown {
                self.tray_hint_shown = true;
                self.set_status(tr("dictz tepside çalışmaya devam ediyor"), 5);
            }
        } else if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.prefs.bool("general.confirm_quit")
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }
        let mut actions: Vec<Action> = Vec::new();
        self.shortcuts(ctx, &mut actions);

        // debounced live search
        if let Some(t) = self.edited_at {
            let wait = Duration::from_millis(self.prefs.int("search.debounce_ms").max(0) as u64);
            if t.elapsed() >= wait {
                self.edited_at = None;
                self.run_live_search();
            } else {
                ctx.request_repaint_after(wait - t.elapsed());
            }
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui, &mut actions));
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .inner_margin(egui::Margin::symmetric(4, 1)),
            )
            .show(ctx, |ui| self.toolbar(ui, &mut actions));
        if self.prefs.bool("appearance.show_statusbar") || self.load_progress.is_some() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        }
        let right_sidebar = self.prefs.str("appearance.sidebar_side") == "right";
        if self.focus_search && !self.prefs.bool("appearance.show_sidebar") {
            // Ctrl+F brings the search pane back
            let _ = self.prefs.set("appearance.show_sidebar", Value::from(true));
        }
        if self.prefs.bool("appearance.show_sidebar") {
            let width = self.prefs.float("state.sidebar_width") as f32;
            let panel = if right_sidebar {
                egui::SidePanel::right("sidebar")
            } else {
                egui::SidePanel::left("sidebar")
            };
            let r = panel
                .resizable(true)
                .default_width(width)
                .width_range(150.0..=560.0)
                .show(ctx, |ui| self.sidebar(ui, &mut actions));
            let w = r.response.rect.width().round();
            if (w as f64 - self.prefs.float("state.sidebar_width")).abs() > 1.0 {
                let _ = self.prefs.set("state.sidebar_width", Value::from(w as i64));
            }
        }
        if self.prefs.bool("appearance.show_found_in") {
            let panel = if right_sidebar {
                egui::SidePanel::left("found-in")
            } else {
                egui::SidePanel::right("found-in")
            };
            panel
                .resizable(true)
                .default_width(190.0)
                .width_range(120.0..=400.0)
                .show(ctx, |ui| self.found_in(ui, &mut actions));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .inner_margin(0)
                    .fill(ctx.style().visuals.extreme_bg_color),
            )
            .show(ctx, |ui| {
                self.tab_bar(ui, &mut actions);
                if self.find_open {
                    self.find_bar(ui, &mut actions);
                }
                self.content(ui, &mut actions);
            });
        self.windows(ctx, &mut actions);
        self.center_events(ctx);
        self.popup_viewport(ctx);

        for a in actions {
            self.apply(ctx, a);
        }
        if self.loader.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some((_, at, secs)) = &self.status {
            if at.elapsed() < Duration::from_secs(*secs) {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(h) = &self.job {
            h.stop(); // workers end with the program; their temp files go later
        }
        if let Some(c) = &self.center {
            c.stop_all(); // a partial download is continued next time (Range)
        }
        self.close_popup();
        self.services.stop();
        self.save_state();
        if self.prefs.bool("history.clear_on_exit") {
            if let Some(h) = &self.history {
                let _ = h.clear_history();
            }
        }
    }
}

/// The compact, GoldenDict-like look: small paddings, square-ish corners,
/// the same colours as the Python edition (`dictz/ui/themes.py: palette`).
fn compact_style(style: &mut egui::Style) {
    let dark = style.visuals.dark_mode;
    let hex = |s: &str| Color32::from_hex(s).unwrap_or(Color32::GRAY);
    let (bg, panel, border, text, muted, hover, header) = if dark {
        (
            "#1e2127", "#25282f", "#363a43", "#e3e5e9", "#9aa1ac", "#2f333b", "#2a2e36",
        )
    } else {
        (
            "#f5f6f8", "#ffffff", "#d7dbe0", "#1f2328", "#5f6670", "#e8ecf1", "#eef1f4",
        )
    };
    let v = &mut style.visuals;
    v.panel_fill = hex(bg);
    v.window_fill = hex(panel);
    v.extreme_bg_color = hex(panel);
    v.faint_bg_color = hex(header);
    v.code_bg_color = hex(header);
    v.window_stroke = egui::Stroke::new(1.0_f32, hex(border));
    v.override_text_color = None;
    v.window_corner_radius = egui::CornerRadius::same(4);
    v.menu_corner_radius = egui::CornerRadius::same(3);
    v.window_shadow = egui::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    v.popup_shadow = v.window_shadow;
    let w = &mut v.widgets;
    for wv in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        wv.corner_radius = egui::CornerRadius::same(3);
        wv.expansion = 0.0;
    }
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, hex(border));
    w.noninteractive.bg_fill = hex(bg);
    w.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, hex(text));
    w.inactive.fg_stroke = egui::Stroke::new(1.0_f32, hex(text));
    w.inactive.weak_bg_fill = hex(header);
    w.inactive.bg_fill = hex(header);
    w.inactive.bg_stroke = egui::Stroke::new(1.0_f32, hex(border));
    w.hovered.weak_bg_fill = hex(hover);
    w.hovered.bg_fill = hex(hover);
    w.hovered.bg_stroke = egui::Stroke::new(1.0_f32, hex(border));
    w.hovered.fg_stroke = egui::Stroke::new(1.0_f32, hex(text));
    w.active.fg_stroke = egui::Stroke::new(1.0_f32, hex(text));
    let _ = muted;
    let sp = &mut style.spacing;
    sp.item_spacing = egui::vec2(6.0, 3.0);
    sp.button_padding = egui::vec2(5.0, 2.0);
    sp.interact_size.y = 20.0;
    sp.menu_margin = egui::Margin::symmetric(4, 3);
    sp.window_margin = egui::Margin::same(8);
    sp.indent = 14.0;
    sp.scroll = egui::style::ScrollStyle::thin();
    sp.scroll.bar_width = 8.0;
}

fn lighten(c: Color32) -> Color32 {
    let m = |v: u8| (v as f32 + (255.0 - v as f32) * 0.35) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn theme_label(theme: &str) -> String {
    match theme {
        "light" => tr("Açık"),
        "dark" => tr("Koyu"),
        _ => tr("Otomatik"),
    }
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        "—".into()
    } else {
        s.to_string()
    }
}

/// 12345 → "12,345" (same grouping as the Python edition's `{:,}`).
pub(crate) fn group(n: usize) -> String {
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(group(0), "0");
        assert_eq!(group(1234), "1,234");
        assert_eq!(group(1234567), "1,234,567");
    }

    #[test]
    fn percent() {
        assert_eq!(percent_decode("go%20on"), "go on");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn titles() {
        let t = Tab {
            word: "abcdefghij".into(),
            content: Content::Welcome,
            generation: 0,
        };
        assert_eq!(t.title(4), "abcd…");
        assert_eq!(Tab::welcome().title(4), NEW_TAB);
    }
}
