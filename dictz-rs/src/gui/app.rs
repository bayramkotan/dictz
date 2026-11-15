//! Main window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};

use crate::fuzzy;
use crate::history::HistoryStore;
use crate::html::{self, Line};
use crate::index::IndexStore;
use crate::media::{media_kind, MediaKind};
use crate::paths::{HISTORY_DB, INDEX_DB, PREFS_FILE};
use crate::query::{QueryManager, Suggestion};
use crate::settings::Preferences;
use crate::{DictionaryEntry, VERSION};

use super::render::{self, Click, MediaSource};
use super::{audio, fonts, system};

const NEW_TAB: &str = "Yeni sekme";

struct Article {
    dict: String,
    headword: String,
    lines: Vec<Line>,
}

enum Content {
    Welcome,
    Articles(Vec<Article>),
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
    fn title(&self) -> String {
        if self.word.is_empty() {
            NEW_TAB.to_string()
        } else if matches!(self.content, Content::NotFound { .. }) {
            format!("{} ✗", self.word)
        } else {
            self.word.clone()
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SidebarTab {
    Dictionaries,
    Favorites,
    History,
}

enum Action {
    OpenWord(String),
    Explicit { new_tab: bool },
    PickSuggestion(String),
    NavBack,
    NavForward,
    ToggleFavorite,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CycleTheme,
    AddDictionaryDialog,
    AddPaths(Vec<PathBuf>),
    OpenManager,
    OpenPreferences,
    About,
    Quit,
    NewTab,
    CloseTab(usize),
    SelectTab(usize),
    CycleTab(isize),
    ToggleSidebar,
    CopyWord,
    AskClearHistory,
    FocusSearch,
    Click(String, Click),
    SetEnabled(String, bool),
    MoveDict(String, isize),
    RemoveDict(String),
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

pub struct DictzApp {
    qm: QueryManager,
    history: Option<HistoryStore>,
    prefs: Preferences,
    data_dir: PathBuf,

    search: String,
    edited_at: Option<Instant>,
    suggestions: Vec<Suggestion>,
    show_suggestions: bool,
    focus_search: bool,

    tabs: Vec<Tab>,
    current: usize,
    generation: u64,
    nav: Vec<String>,
    nav_cursor: isize,
    applying_nav: bool,

    sidebar_tab: SidebarTab,
    favorites: Vec<String>,
    recent: Vec<String>,
    is_favorite: bool,

    status: Option<(String, Instant)>,
    error: Option<(String, String)>,
    show_manager: bool,
    manager_selected: Option<String>,
    manager_path: String,
    show_prefs: bool,
    prefs_draft: Preferences,
    show_about: bool,
    confirm_clear: bool,
    fonts_dirty: bool,

    media_cache: HashMap<(String, String), Option<Arc<[u8]>>>,
}

impl DictzApp {
    pub fn new(cc: &eframe::CreationContext<'_>, data_dir: PathBuf) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let _ = std::fs::create_dir_all(&data_dir);
        let prefs = Preferences::load(&data_dir.join(PREFS_FILE));
        let mut status = None;
        let index = match IndexStore::open(&data_dir.join(INDEX_DB)) {
            Ok(i) => Some(i),
            Err(e) => {
                status = Some((format!("İndeks açılamadı: {}", e), Instant::now()));
                None
            }
        };
        let history = HistoryStore::open(&data_dir.join(HISTORY_DB)).ok();
        let mut qm = QueryManager::new(index);
        let (loaded, errors) = qm.restore_from_index();
        if !errors.is_empty() {
            status = Some((
                format!("{} sözlük açılamadı: {}", errors.len(), errors.join("; ")),
                Instant::now(),
            ));
        } else if !loaded.is_empty() {
            status = Some((format!("{} sözlük yüklendi", loaded.len()), Instant::now()));
        }
        fonts::install(&cc.egui_ctx, &prefs.font_family);
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let start_search = !prefs.last_search.trim().is_empty();
        let mut app = Self {
            qm,
            history,
            prefs_draft: prefs.clone(),
            search: prefs.last_search.clone(),
            prefs,
            data_dir,
            edited_at: start_search.then(Instant::now),
            suggestions: Vec::new(),
            show_suggestions: false,
            focus_search: true,
            tabs: vec![Tab::welcome()],
            current: 0,
            generation: 0,
            nav: Vec::new(),
            nav_cursor: -1,
            applying_nav: false,
            sidebar_tab: SidebarTab::Dictionaries,
            favorites: Vec::new(),
            recent: Vec::new(),
            is_favorite: false,
            status,
            error: None,
            show_manager: false,
            manager_selected: None,
            manager_path: String::new(),
            show_prefs: false,
            show_about: false,
            confirm_clear: false,
            fonts_dirty: false,
            media_cache: HashMap::new(),
        };
        app.apply_theme(&cc.egui_ctx);
        app.apply_text_size(&cc.egui_ctx);
        app.refresh_lists();
        app
    }

    // ------------------------------------------------------------ helpers

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let pref = match self.prefs.theme.as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        };
        ctx.set_theme(pref);
    }

    fn apply_text_size(&self, ctx: &egui::Context) {
        let base = self.prefs.font_size as f32;
        ctx.all_styles_mut(|style| {
            use egui::{FontFamily, FontId, TextStyle};
            style.text_styles = [
                (
                    TextStyle::Small,
                    FontId::new(base * 0.75, FontFamily::Proportional),
                ),
                (
                    TextStyle::Body,
                    FontId::new(base * 0.95, FontFamily::Proportional),
                ),
                (
                    TextStyle::Button,
                    FontId::new(base * 0.95, FontFamily::Proportional),
                ),
                (
                    TextStyle::Heading,
                    FontId::new(base * 1.45, FontFamily::Proportional),
                ),
                (
                    TextStyle::Monospace,
                    FontId::new(base * 0.9, FontFamily::Monospace),
                ),
            ]
            .into();
        });
    }

    fn save_prefs(&mut self) {
        self.prefs.last_search = self.search.clone();
        if let Err(e) = self.prefs.save() {
            self.set_status(format!("Tercihler kaydedilemedi: {}", e));
        }
    }

    fn refresh_lists(&mut self) {
        if let Some(h) = &self.history {
            self.favorites = h.favorites(500).unwrap_or_default();
            self.recent = h.recent(200).unwrap_or_default();
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

    fn articles(entries: Vec<DictionaryEntry>) -> Vec<Article> {
        entries
            .into_iter()
            .map(|e| Article {
                lines: if html::looks_like_html(&e.definition) {
                    html::parse(&e.definition)
                } else {
                    html::plain_to_lines(&e.definition)
                },
                dict: e.dictionary_id,
                headword: e.headword,
            })
            .collect()
    }

    fn show_in_tab(
        &mut self,
        idx: usize,
        word: &str,
        entries: Vec<DictionaryEntry>,
        fuzzy_if_empty: bool,
    ) {
        self.generation += 1;
        let found = !entries.is_empty();
        let content = if found {
            Content::Articles(Self::articles(entries))
        } else {
            let suggestions = if fuzzy_if_empty {
                self.qm
                    .fuzzy(word, fuzzy::DEFAULT_LIMIT, fuzzy::DEFAULT_CUTOFF)
            } else {
                Vec::new()
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
        if found {
            if let Some(h) = &self.history {
                let _ = h.record(word, "");
            }
            self.recent = self
                .history
                .as_ref()
                .and_then(|h| h.recent(200).ok())
                .unwrap_or_default();
        }
        self.update_favorite_flag();
    }

    fn push_nav(&mut self, word: &str) {
        if self.applying_nav || word.is_empty() {
            return;
        }
        self.nav.truncate((self.nav_cursor + 1).max(0) as usize);
        self.nav.push(word.to_string());
        self.nav_cursor = self.nav.len() as isize - 1;
    }

    fn open_word(&mut self, word: &str) {
        let word = word.trim().to_string();
        if word.is_empty() {
            return;
        }
        self.search = word.clone();
        self.show_suggestions = false;
        let entries = self.qm.search(&word, true);
        self.show_in_tab(self.current, &word, entries, true);
        self.push_nav(&word);
    }

    fn run_live_search(&mut self) {
        let q = self.search.trim().to_string();
        if q.is_empty() {
            self.suggestions.clear();
            self.show_suggestions = false;
            return;
        }
        self.suggestions = self.qm.suggest(&q, 100);
        self.show_suggestions = !self.suggestions.is_empty();
        let entries = self.qm.search(&q, true);
        if !entries.is_empty() {
            self.show_in_tab(self.current, &q, entries, false);
            self.push_nav(&q);
        }
    }

    fn run_explicit(&mut self, new_tab: bool) {
        let q = self.search.trim().to_string();
        if q.is_empty() {
            return;
        }
        let mut entries = self.qm.search(&q, true);
        if entries.is_empty() {
            entries = self.qm.search(&q, false);
        }
        if new_tab {
            self.tabs.push(Tab::welcome());
            self.current = self.tabs.len() - 1;
        }
        self.show_in_tab(self.current, &q, entries, true);
        self.push_nav(&q);
        self.show_suggestions = false;
        self.edited_at = None;
    }

    fn apply_nav(&mut self) {
        let Some(word) = self.nav.get(self.nav_cursor.max(0) as usize).cloned() else {
            return;
        };
        self.applying_nav = true;
        self.search = word.clone();
        let entries = self.qm.search(&word, true);
        self.show_in_tab(self.current, &word, entries, true);
        self.applying_nav = false;
        self.show_suggestions = false;
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            match self.qm.add_dictionary(&p, false) {
                Ok(name) => {
                    let n = self
                        .qm
                        .dictionary(&name)
                        .map(|d| d.metadata().word_count)
                        .unwrap_or(0);
                    self.set_status(format!("Yüklendi: {} ({} kelime)", name, group(n)));
                }
                Err(e) => {
                    self.error = Some(("Yüklenemedi".into(), format!("{}\n\n{}", p.display(), e)))
                }
            }
        }
        self.media_cache.clear();
    }

    fn play_audio(&mut self, dict: &str, name: &str) {
        let Some(bytes) = self.qm.media(dict).and_then(|m| m.read(name)) else {
            self.set_status(format!("Ses dosyası bulunamadı: {}", name));
            return;
        };
        self.set_status(format!("Çalınıyor: {}", name));
        audio::play(bytes, name.to_string(), self.data_dir.join("media-cache"));
    }

    fn handle_link(&mut self, dict: &str, href: &str) {
        if let Some(rest) = href
            .strip_prefix("bword:")
            .or_else(|| href.strip_prefix("dict:"))
        {
            let word = rest.trim_start_matches('/');
            let word = percent_decode(word);
            self.open_word(&word);
        } else if let Some(rest) = href.strip_prefix("sound://") {
            self.play_audio(dict, rest);
        } else if href.starts_with("http://")
            || href.starts_with("https://")
            || href.starts_with("mailto:")
        {
            if let Err(e) = system::open_external(href) {
                self.set_status(format!("Bağlantı açılamadı: {}", e));
            }
        } else if media_kind(href) == MediaKind::Audio {
            self.play_audio(dict, href);
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::OpenWord(w) => self.open_word(&w),
            Action::Explicit { new_tab } => self.run_explicit(new_tab),
            Action::PickSuggestion(w) => self.open_word(&w),
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
            Action::ToggleFavorite => {
                let w = {
                    let cw = self.current_word();
                    if cw.is_empty() {
                        self.search.trim().to_string()
                    } else {
                        cw
                    }
                };
                if !w.is_empty() {
                    if let Some(h) = &self.history {
                        let _ = h.toggle_favorite(&w);
                    }
                    self.refresh_lists();
                }
            }
            Action::ZoomIn => {
                self.prefs.zoom_factor = (self.prefs.zoom_factor + 0.1).min(3.0);
                self.save_prefs();
            }
            Action::ZoomOut => {
                self.prefs.zoom_factor = (self.prefs.zoom_factor - 0.1).max(0.5);
                self.save_prefs();
            }
            Action::ZoomReset => {
                self.prefs.zoom_factor = 1.0;
                self.save_prefs();
            }
            Action::CycleTheme => {
                let order = ["auto", "light", "dark"];
                let i = order
                    .iter()
                    .position(|t| *t == self.prefs.theme)
                    .unwrap_or(0);
                self.prefs.theme = order[(i + 1) % order.len()].to_string();
                self.apply_theme(ctx);
                self.save_prefs();
                let label = theme_label(&self.prefs.theme);
                self.set_status(format!("Tema: {}", label));
            }
            Action::AddDictionaryDialog => {
                let picked = rfd::FileDialog::new()
                    .set_title("Sözlük dosyası seçin")
                    .add_filter("Tüm sözlükler", &["ifo", "dsl", "dz", "bgl"])
                    .add_filter("StarDict", &["ifo"])
                    .add_filter("DSL / Lingvo", &["dsl", "dz"])
                    .add_filter("Babylon", &["bgl"])
                    .add_filter("Tüm dosyalar", &["*"])
                    .pick_files();
                if let Some(p) = picked {
                    self.add_paths(p);
                }
            }
            Action::AddPaths(p) => self.add_paths(p),
            Action::OpenManager => self.show_manager = true,
            Action::OpenPreferences => {
                self.prefs_draft = self.prefs.clone();
                self.show_prefs = true;
            }
            Action::About => self.show_about = true,
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::NewTab => {
                self.tabs.push(Tab::welcome());
                self.current = self.tabs.len() - 1;
                self.update_favorite_flag();
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
                self.update_favorite_flag();
            }
            Action::CycleTab(d) => {
                let n = self.tabs.len() as isize;
                self.current = ((self.current as isize + d).rem_euclid(n)) as usize;
                self.update_favorite_flag();
            }
            Action::ToggleSidebar => {
                self.prefs.show_sidebar = !self.prefs.show_sidebar;
                self.save_prefs();
            }
            Action::CopyWord => {
                let w = self.current_word();
                if !w.is_empty() {
                    ctx.copy_text(w.clone());
                    self.set_status(format!("Kopyalandı: {}", w));
                }
            }
            Action::AskClearHistory => self.confirm_clear = true,
            Action::FocusSearch => self.focus_search = true,
            Action::Click(dict, Click::Link(href)) => self.handle_link(&dict, &href),
            Action::Click(dict, Click::Audio(name)) => self.play_audio(&dict, &name),
            Action::SetEnabled(name, on) => {
                self.qm.set_enabled(&name, on);
            }
            Action::MoveDict(name, d) => self.qm.move_dictionary(&name, d),
            Action::RemoveDict(name) => {
                if let Err(e) = self.qm.remove_dictionary(&name) {
                    self.error = Some(("Kaldırılamadı".into(), e.to_string()));
                }
                if self.manager_selected.as_deref() == Some(name.as_str()) {
                    self.manager_selected = None;
                }
                self.media_cache.retain(|k, _| k.0 != name);
            }
        }
    }

    // ------------------------------------------------------------ shortcuts

    fn shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let cmd = Modifiers::COMMAND;
        let sc = |m: Modifiers, k: Key, a: Action, actions: &mut Vec<Action>| {
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k))) {
                actions.push(a);
            }
        };
        sc(
            cmd | Modifiers::SHIFT,
            Key::Tab,
            Action::CycleTab(-1),
            actions,
        );
        sc(cmd, Key::Tab, Action::CycleTab(1), actions);
        sc(cmd | Modifiers::SHIFT, Key::C, Action::CopyWord, actions);
        sc(cmd, Key::F, Action::FocusSearch, actions);
        sc(cmd, Key::T, Action::NewTab, actions);
        sc(cmd, Key::D, Action::ToggleFavorite, actions);
        sc(cmd, Key::B, Action::ToggleSidebar, actions);
        sc(cmd, Key::Equals, Action::ZoomIn, actions);
        sc(cmd, Key::Plus, Action::ZoomIn, actions);
        sc(cmd, Key::Minus, Action::ZoomOut, actions);
        sc(cmd, Key::Num0, Action::ZoomReset, actions);
        sc(cmd, Key::Comma, Action::OpenPreferences, actions);
        sc(cmd, Key::O, Action::AddDictionaryDialog, actions);
        sc(cmd, Key::M, Action::OpenManager, actions);
        sc(cmd, Key::Q, Action::Quit, actions);
        sc(Modifiers::ALT, Key::ArrowLeft, Action::NavBack, actions);
        sc(Modifiers::ALT, Key::ArrowRight, Action::NavForward, actions);
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(cmd, Key::W))) {
            actions.push(Action::CloseTab(self.current));
        }
        // files dropped onto the window
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
            let b = egui::Button::new(text).shortcut_text(shortcut);
            let clicked = ui.add(b).clicked();
            if clicked {
                ui.close_menu();
            }
            clicked
        };
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Dosya", |ui| {
                if item(ui, "Sözlük Ekle…", "Ctrl+O") {
                    actions.push(Action::AddDictionaryDialog);
                }
                if item(ui, "Sözlük Yöneticisi…", "Ctrl+M") {
                    actions.push(Action::OpenManager);
                }
                ui.separator();
                if item(ui, "Çıkış", "Ctrl+Q") {
                    actions.push(Action::Quit);
                }
            });
            ui.menu_button("Düzen", |ui| {
                if item(ui, "Kelimeyi kopyala", "Ctrl+Shift+C") {
                    actions.push(Action::CopyWord);
                }
                if item(ui, "Arama kutusuna odaklan", "Ctrl+F") {
                    actions.push(Action::FocusSearch);
                }
                ui.separator();
                if item(ui, "Tercihler…", "Ctrl+,") {
                    actions.push(Action::OpenPreferences);
                }
            });
            ui.menu_button("Görünüm", |ui| {
                if item(ui, "Yeni sekme", "Ctrl+T") {
                    actions.push(Action::NewTab);
                }
                if item(ui, "Sekmeyi kapat", "Ctrl+W") {
                    actions.push(Action::CloseTab(self.current));
                }
                ui.separator();
                if item(ui, "Yan paneli göster/gizle", "Ctrl+B") {
                    actions.push(Action::ToggleSidebar);
                }
                ui.separator();
                if item(ui, "Büyüt", "Ctrl++") {
                    actions.push(Action::ZoomIn);
                }
                if item(ui, "Küçült", "Ctrl+-") {
                    actions.push(Action::ZoomOut);
                }
                if item(ui, "Yakınlaştırmayı sıfırla", "Ctrl+0") {
                    actions.push(Action::ZoomReset);
                }
                ui.separator();
                if item(ui, &format!("Tema: {}", theme_label(&self.prefs.theme)), "") {
                    actions.push(Action::CycleTheme);
                }
            });
            ui.menu_button("Yardım", |ui| {
                if item(ui, "Hakkında", "") {
                    actions.push(Action::About);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let can_back = self.nav_cursor > 0;
            let can_fwd = self.nav_cursor + 1 < self.nav.len() as isize;
            if ui
                .add_enabled(can_back, egui::Button::new("◀"))
                .on_hover_text("Geri (Alt+←)")
                .clicked()
            {
                actions.push(Action::NavBack);
            }
            if ui
                .add_enabled(can_fwd, egui::Button::new("▶"))
                .on_hover_text("İleri (Alt+→)")
                .clicked()
            {
                actions.push(Action::NavForward);
            }
            let right_w = 230.0;
            let edit = egui::TextEdit::singleline(&mut self.search)
                .hint_text("Bir kelime ara…   (Ctrl+F odaklan)")
                .desired_width((ui.available_width() - right_w).max(160.0))
                .id(egui::Id::new("search-box"));
            let resp = ui.add(edit);
            if self.focus_search {
                resp.request_focus();
                self.focus_search = false;
            }
            if resp.changed() {
                self.edited_at = Some(Instant::now());
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let new_tab = ui.input(|i| i.modifiers.command);
                actions.push(Action::Explicit { new_tab });
                self.focus_search = true;
            }
            if resp.has_focus()
                && ui.input(|i| i.key_pressed(Key::ArrowDown))
                && !self.suggestions.is_empty()
            {
                self.show_suggestions = true;
            }
            if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                self.show_suggestions = false;
            }
            let star = if self.is_favorite { "★" } else { "☆" };
            if ui.button(star).on_hover_text("Favori (Ctrl+D)").clicked() {
                actions.push(Action::ToggleFavorite);
            }
            if ui.button("−").on_hover_text("Küçült (Ctrl+-)").clicked() {
                actions.push(Action::ZoomOut);
            }
            if ui.button("+").on_hover_text("Büyüt (Ctrl++)").clicked() {
                actions.push(Action::ZoomIn);
            }
            if ui
                .button("◐")
                .on_hover_text(format!("Tema: {}", theme_label(&self.prefs.theme)))
                .clicked()
            {
                actions.push(Action::CycleTheme);
            }
            if ui.button("⚙").on_hover_text("Tercihler (Ctrl+,)").clicked() {
                actions.push(Action::OpenPreferences);
            }
        });
        if self.show_suggestions && !self.suggestions.is_empty() {
            ui.add_space(2.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("suggestions")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for s in &self.suggestions {
                            let label = if s.fuzzy {
                                format!("≈ {}", s.word)
                            } else {
                                s.word.clone()
                            };
                            let mut rt = RichText::new(label);
                            if s.fuzzy {
                                rt = rt.italics().weak();
                            }
                            let r = ui.selectable_label(false, rt);
                            let r = if s.fuzzy {
                                r.on_hover_text("Benzer kelime (bulanık eşleşme)")
                            } else {
                                r
                            };
                            if r.clicked() {
                                actions.push(Action::PickSuggestion(s.word.clone()));
                            }
                        }
                    });
            });
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.sidebar_tab,
                SidebarTab::Dictionaries,
                "📚 Sözlükler",
            );
            ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Favorites, "★ Favoriler");
            ui.selectable_value(&mut self.sidebar_tab, SidebarTab::History, "🕘 Geçmiş");
        });
        ui.separator();
        match self.sidebar_tab {
            SidebarTab::Dictionaries => {
                ui.horizontal(|ui| {
                    ui.strong("Sözlükler");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Yönet…").clicked() {
                            actions.push(Action::OpenManager);
                        }
                    });
                });
                egui::ScrollArea::vertical().id_salt("side-dicts").show(ui, |ui| {
                    let names = self.qm.loaded_names();
                    if names.is_empty() {
                        ui.weak("Henüz sözlük yok.\nDosya → Sözlük Ekle (Ctrl+O)\nveya dosyayı pencereye sürükleyin.");
                    }
                    for name in names {
                        let mut on = self.qm.is_enabled(&name);
                        let count = self.qm.dictionary(&name).map(|d| d.metadata().word_count).unwrap_or(0);
                        if ui.checkbox(&mut on, &name).on_hover_text(format!("{} kelime", group(count))).changed() {
                            actions.push(Action::SetEnabled(name.clone(), on));
                        }
                    }
                });
            }
            SidebarTab::Favorites => {
                ui.strong("Favoriler");
                egui::ScrollArea::vertical()
                    .id_salt("side-favs")
                    .show(ui, |ui| {
                        if self.favorites.is_empty() {
                            ui.weak("Favori yok. Bir kelimeyi Ctrl+D ile ekleyin.");
                        }
                        for w in &self.favorites {
                            if ui.selectable_label(false, w).clicked() {
                                actions.push(Action::OpenWord(w.clone()));
                            }
                        }
                    });
            }
            SidebarTab::History => {
                ui.horizontal(|ui| {
                    ui.strong("Geçmiş");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Temizle").clicked() {
                            actions.push(Action::AskClearHistory);
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("side-hist")
                    .show(ui, |ui| {
                        for w in &self.recent {
                            if ui.selectable_label(false, w).clicked() {
                                actions.push(Action::OpenWord(w.clone()));
                            }
                        }
                    });
            }
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
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
                                egui::Color32::TRANSPARENT
                            })
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let t = truncate(&tab.title(), 28);
                                    if ui.selectable_label(selected, t).clicked() {
                                        actions.push(Action::SelectTab(i));
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Sekmeyi kapat (Ctrl+W)")
                                        .clicked()
                                    {
                                        actions.push(Action::CloseTab(i));
                                    }
                                });
                            });
                    }
                    if ui
                        .button("+")
                        .on_hover_text("Yeni sekme (Ctrl+T)")
                        .clicked()
                    {
                        actions.push(Action::NewTab);
                    }
                });
            });
        ui.separator();
    }

    fn content(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let size = self.prefs.font_size as f32 * self.prefs.zoom_factor;
        let Some(tab) = self.tabs.get(self.current) else {
            return;
        };
        let accent = ui.visuals().hyperlink_color;
        egui::ScrollArea::vertical()
            .id_salt(("content", self.current, tab.generation))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(6.0);
                match &tab.content {
                    Content::Welcome => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.label(RichText::new("dictz").size(size * 2.3).strong());
                            ui.label(RichText::new("çok formatlı çevrimdışı sözlük").size(size).weak());
                            ui.add_space(24.0);
                            ui.label(RichText::new("Başlamak için Dosya → Sözlük Ekle menüsünden bir StarDict (.ifo),\nDSL (.dsl) veya Babylon (.bgl) dosyası yükleyin ya da dosyayı pencereye sürükleyin.").size(size * 0.9).weak());
                            ui.add_space(36.0);
                            ui.label(RichText::new("Ctrl+F ara · Ctrl+T yeni sekme · Ctrl+D favoriye ekle · Ctrl++/Ctrl+- yakınlaştır").size(size * 0.8).weak());
                        });
                    }
                    Content::NotFound { query, suggestions } => {
                        ui.add_space(24.0);
                        ui.label(RichText::new("Sonuç yok").size(size * 1.5).strong());
                        ui.label(RichText::new(format!("“{}” için hiçbir sözlükte eşleşme bulunamadı.", query)).size(size));
                        if !suggestions.is_empty() {
                            ui.add_space(16.0);
                            ui.label(RichText::new("Bunu mu demek istediniz?").size(size * 1.1).strong());
                            ui.add_space(4.0);
                            for (w, score) in suggestions {
                                ui.horizontal(|ui| {
                                    let r = ui.add(egui::Label::new(RichText::new(w).size(size).color(accent).underline()).sense(egui::Sense::click()))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                    ui.label(RichText::new(format!("  %{:.0}", score)).size(size * 0.8).weak());
                                    if r.clicked() {
                                        actions.push(Action::OpenWord(w.clone()));
                                    }
                                });
                            }
                        }
                    }
                    Content::Articles(list) => {
                        let mut last_dict: Option<&str> = None;
                        let mut clicks: Vec<(String, Click)> = Vec::new();
                        for a in list {
                            if last_dict != Some(a.dict.as_str()) {
                                last_dict = Some(a.dict.as_str());
                                ui.add_space(10.0);
                                ui.label(RichText::new(&a.dict).size(size * 0.85).color(accent).strong());
                                ui.separator();
                            }
                            ui.label(RichText::new(&a.headword).size(size * 1.35).family(egui::FontFamily::Name(fonts::BOLD.into())).strong());
                            ui.add_space(2.0);
                            let mut local = Vec::new();
                            let mut media = MediaCache { qm: &self.qm, dict: &a.dict, cache: &mut self.media_cache };
                            render::lines(ui, &a.lines, size, &a.dict, &mut media, &mut local);
                            clicks.extend(local.into_iter().map(|c| (a.dict.clone(), c)));
                            ui.add_space(size * 0.6);
                        }
                        for (d, c) in clicks {
                            actions.push(Action::Click(d, c));
                        }
                    }
                }
                ui.add_space(24.0);
            });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((msg, at)) = &self.status {
                if at.elapsed() < Duration::from_secs(6) {
                    ui.label(msg);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!(
                    "{}%",
                    (self.prefs.zoom_factor * 100.0).round() as i32
                ));
                ui.label("·");
                let names: Vec<String> = self
                    .qm
                    .loaded_names()
                    .into_iter()
                    .filter(|n| self.qm.is_enabled(n))
                    .collect();
                if names.is_empty() {
                    ui.label("0 sözlük yüklü");
                } else {
                    ui.label(format!(
                        "{} sözlük · {} kelime",
                        names.len(),
                        group(self.qm.word_count())
                    ));
                }
            });
        });
    }

    // ------------------------------------------------------------ windows

    fn windows(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.show_manager {
            let mut open = true;
            egui::Window::new("Sözlük yöneticisi")
                .open(&mut open)
                .default_size([560.0, 380.0])
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Yüklü sözlükler (etkinleştirmek için kutuyu işaretleyin):");
                    egui::ScrollArea::vertical()
                        .id_salt("mgr-list")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for name in self.qm.loaded_names() {
                                let Some(d) = self.qm.dictionary(&name) else {
                                    continue;
                                };
                                let m = d.metadata();
                                let mut on = self.qm.is_enabled(&name);
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut on, "").changed() {
                                        actions.push(Action::SetEnabled(name.clone(), on));
                                    }
                                    let text = format!(
                                        "{}  —  {}  ·  {} kelime",
                                        name,
                                        m.format,
                                        group(m.word_count)
                                    );
                                    let sel =
                                        self.manager_selected.as_deref() == Some(name.as_str());
                                    if ui
                                        .selectable_label(sel, text)
                                        .on_hover_text(m.file_path.display().to_string())
                                        .clicked()
                                    {
                                        self.manager_selected = Some(name.clone());
                                    }
                                });
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Ekle…").clicked() {
                            actions.push(Action::AddDictionaryDialog);
                        }
                        let sel = self.manager_selected.clone();
                        if ui
                            .add_enabled(sel.is_some(), egui::Button::new("Kaldır"))
                            .clicked()
                        {
                            actions.push(Action::RemoveDict(sel.clone().unwrap()));
                        }
                        if ui
                            .add_enabled(sel.is_some(), egui::Button::new("Yukarı"))
                            .clicked()
                        {
                            actions.push(Action::MoveDict(sel.clone().unwrap(), -1));
                        }
                        if ui
                            .add_enabled(sel.is_some(), egui::Button::new("Aşağı"))
                            .clicked()
                        {
                            actions.push(Action::MoveDict(sel.clone().unwrap(), 1));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Dosya yolu:");
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.manager_path)
                                .hint_text("/yol/sozluk.ifo")
                                .desired_width(300.0),
                        );
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if (ui.button("Yolu ekle").clicked() || enter)
                            && !self.manager_path.trim().is_empty()
                        {
                            actions.push(Action::AddPaths(vec![PathBuf::from(
                                self.manager_path.trim(),
                            )]));
                            self.manager_path.clear();
                        }
                    });
                });
            self.show_manager = open;
        }

        if self.show_prefs {
            let mut open = true;
            let mut result: Option<bool> = None;
            egui::Window::new("Tercihler")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("prefs-grid")
                        .num_columns(2)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Tema");
                            egui::ComboBox::from_id_salt("theme")
                                .selected_text(theme_label(&self.prefs_draft.theme))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.prefs_draft.theme,
                                        "auto".to_string(),
                                        "Otomatik",
                                    );
                                    ui.selectable_value(
                                        &mut self.prefs_draft.theme,
                                        "light".to_string(),
                                        "Açık",
                                    );
                                    ui.selectable_value(
                                        &mut self.prefs_draft.theme,
                                        "dark".to_string(),
                                        "Koyu",
                                    );
                                });
                            ui.end_row();
                            ui.label("Yazı tipi dosyası");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.prefs_draft.font_family)
                                        .hint_text("varsayılan")
                                        .desired_width(220.0),
                                );
                                if ui.button("Seç…").clicked() {
                                    if let Some(p) = rfd::FileDialog::new()
                                        .add_filter("Yazı tipi", &["ttf", "otf", "ttc"])
                                        .pick_file()
                                    {
                                        self.prefs_draft.font_family = p.display().to_string();
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label("Yazı boyutu");
                            ui.add(
                                egui::DragValue::new(&mut self.prefs_draft.font_size)
                                    .range(8..=32)
                                    .suffix(" px"),
                            );
                            ui.end_row();
                            ui.label("Arama gecikmesi");
                            ui.add(
                                egui::DragValue::new(&mut self.prefs_draft.search_debounce_ms)
                                    .range(0..=1000)
                                    .suffix(" ms"),
                            );
                            ui.end_row();
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Tamam").clicked() {
                            result = Some(true);
                        }
                        if ui.button("İptal").clicked() {
                            result = Some(false);
                        }
                    });
                });
            if result == Some(true) {
                let font_changed = self.prefs_draft.font_family != self.prefs.font_family;
                let mut p = self.prefs_draft.clone();
                p.zoom_factor = self.prefs.zoom_factor;
                p.show_sidebar = self.prefs.show_sidebar;
                p.sidebar_width = self.prefs.sidebar_width;
                self.prefs = p;
                self.apply_theme(ctx);
                self.apply_text_size(ctx);
                if font_changed {
                    self.fonts_dirty = true;
                }
                self.save_prefs();
            }
            self.show_prefs = open && result.is_none();
        }

        if self.show_about {
            let mut open = true;
            egui::Window::new("dictz hakkında")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading(format!("dictz {}", VERSION));
                    ui.label("Çok formatlı çevrimdışı sözlük uygulaması — Rust sürümü.");
                    ui.label("© 2026 Bayram Kotan · GPL-3.0+");
                    if ui.link("github.com/bayramkotan/dictz").clicked() {
                        let _ = system::open_external("https://github.com/bayramkotan/dictz");
                    }
                    ui.weak(format!("Veri klasörü: {}", self.data_dir.display()));
                });
            self.show_about = open;
        }

        if self.confirm_clear {
            egui::Window::new("Geçmişi temizle")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Tüm arama geçmişini silmek istediğinizden emin misiniz?");
                    ui.horizontal(|ui| {
                        if ui.button("Evet").clicked() {
                            if let Some(h) = &self.history {
                                let _ = h.clear_history();
                            }
                            self.refresh_lists();
                            self.confirm_clear = false;
                        }
                        if ui.button("Hayır").clicked() {
                            self.confirm_clear = false;
                        }
                    });
                });
        }

        if let Some((title, msg)) = self.error.clone() {
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button("Tamam").clicked() {
                        self.error = None;
                    }
                });
        }
    }
}

impl eframe::App for DictzApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.fonts_dirty {
            fonts::install(ctx, &self.prefs.font_family);
            self.fonts_dirty = false;
        }
        let mut actions: Vec<Action> = Vec::new();
        self.shortcuts(ctx, &mut actions);

        // debounced live search
        if let Some(t) = self.edited_at {
            let wait = Duration::from_millis(self.prefs.search_debounce_ms);
            if t.elapsed() >= wait {
                self.edited_at = None;
                self.run_live_search();
            } else {
                ctx.request_repaint_after(wait - t.elapsed());
            }
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui, &mut actions));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui, &mut actions);
            ui.add_space(4.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        if self.prefs.show_sidebar {
            let panel = egui::SidePanel::left("sidebar")
                .resizable(true)
                .default_width(self.prefs.sidebar_width)
                .width_range(200.0..=520.0)
                .show(ctx, |ui| self.sidebar(ui, &mut actions));
            let w = panel.response.rect.width();
            if (w - self.prefs.sidebar_width).abs() > 1.0 {
                self.prefs.sidebar_width = w;
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.tab_bar(ui, &mut actions);
            self.content(ui, &mut actions);
        });
        self.windows(ctx, &mut actions);

        for a in actions {
            self.apply(ctx, a);
        }
        if let Some((_, at)) = &self.status {
            if at.elapsed() < Duration::from_secs(6) {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_prefs();
    }
}

fn theme_label(theme: &str) -> &'static str {
    match theme {
        "light" => "Açık",
        "dark" => "Koyu",
        _ => "Otomatik",
    }
}

/// 12345 → "12,345" (same grouping as the Python edition's `{:,}`).
fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
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
}
