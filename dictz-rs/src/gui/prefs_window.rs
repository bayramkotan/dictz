//! Preferences window — built from the shared settings schema, the same
//! pages and settings as the Python edition's dialog.

use crate::i18n::tr;
use eframe::egui::{self, Color32, RichText};
use serde_json::Value;

use crate::settings::{Kind, Preferences, Setting, SCHEMA, SECTIONS};
use crate::shortcuts::SHORTCUTS;

/// Buttons on the "Gelişmiş" page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advanced {
    /// Open the data folder.
    OpenData,
    /// Rebuild every index.
    Reindex,
    /// Look for a new version.
    CheckUpdates,
    /// Delete the lookup history.
    ClearHistory,
    /// Delete all favorites.
    ClearFavorites,
    /// Save settings to a file.
    Export,
    /// Load settings from a file.
    Import,
    /// Scan the dictionary folders now (with the draft's folders).
    Rescan,
    /// Open the Dictionaries window on its settings tab.
    DictionariesSettings,
}

/// Pages shown in the Dictionaries window (Sözlükler → Ayarlar) instead.
pub const PAGES_IN_DICTIONARIES_WINDOW: &[&str] = &["dictionaries", "center"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Section(usize),
    DictionariesLink,
    Shortcuts,
    Advanced,
}

/// What the window asks the app to do.
pub enum Event {
    /// Apply the draft (window stays open).
    Apply,
    /// Apply the draft and close.
    Ok,
    /// Close without applying.
    Cancel,
    /// A button on the advanced page.
    Action(Advanced),
}

/// Open preferences window.
pub struct PrefsWindow {
    /// Edited copy.
    pub draft: Preferences,
    page: usize,
    filter: String,
    pending: Vec<Event>,
}

/// Settings without their own row: `dictionaries.flat_folders` is the
/// "subfolders" box of the dictionary folder list.
pub const HIDDEN_KEYS: &[&str] = &["dictionaries.flat_folders"];

const PAGE_SHORTCUTS: &str = "Kısayollar";
const PAGE_ADVANCED: &str = "Gelişmiş";

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// `#rrggbb` → color.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn differs(a: Option<&Value>, b: &Value) -> bool {
    match (a, b) {
        (Some(Value::Number(x)), Value::Number(y)) => x.as_f64() != y.as_f64(),
        (Some(x), y) => x != y,
        (None, _) => true,
    }
}

impl PrefsWindow {
    /// Open on `prefs` (a copy is edited).
    pub fn new(prefs: &Preferences) -> Self {
        Self {
            draft: prefs.clone(),
            page: 0,
            filter: String::new(),
            pending: Vec::new(),
        }
    }

    fn pages() -> Vec<(&'static str, Page)> {
        let mut v = Vec::new();
        for (i, (sec, title)) in SECTIONS.iter().enumerate() {
            if PAGES_IN_DICTIONARIES_WINDOW.contains(sec) {
                if *sec == PAGES_IN_DICTIONARIES_WINDOW[0] {
                    v.push(("Sözlükler", Page::DictionariesLink));
                }
                continue;
            }
            v.push((*title, Page::Section(i)));
        }
        v.push((PAGE_SHORTCUTS, Page::Shortcuts));
        v.push((PAGE_ADVANCED, Page::Advanced));
        v
    }

    /// Draw; returns the requested events.
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) -> Vec<Event> {
        let mut events = Vec::new();
        egui::Window::new(tr("Tercihler"))
            .open(open)
            .collapsible(false)
            .default_size([860.0, 560.0])
            .min_width(640.0)
            .resizable([true, false])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text(tr("Ayarlarda ara…"))
                            .desired_width(260.0),
                    );
                    if !self.filter.is_empty() && ui.small_button("✕").clicked() {
                        self.filter.clear();
                    }
                });
                ui.separator();
                let body_h = (ui.ctx().screen_rect().height() - 250.0).clamp(200.0, 480.0);
                let w = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(w, body_h),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.set_max_height(body_h);
                        // page list
                        ui.vertical(|ui| {
                            ui.set_width(170.0);
                            ui.set_min_height(body_h);
                            for (i, (title, _)) in Self::pages().iter().enumerate() {
                                let sel = self.page == i && self.filter.is_empty();
                                if ui.selectable_label(sel, tr(title)).clicked() {
                                    self.page = i;
                                    self.filter.clear();
                                }
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("prefs-page")
                                .max_height(body_h)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.filter.trim().is_empty() {
                                        self.page_body(ui, &mut events);
                                    } else {
                                        self.search_body(ui);
                                    }
                                });
                        });
                    },
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Tümünü varsayılana döndür")).clicked() {
                        let _ = self.draft.reset(None);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr("İptal")).clicked() {
                            events.push(Event::Cancel);
                        }
                        if ui.button(tr("Tamam")).clicked() {
                            events.push(Event::Ok);
                        }
                        if ui.button(tr("Uygula")).clicked() {
                            events.push(Event::Apply);
                        }
                    });
                });
            });
        events.append(&mut self.pending);
        events
    }

    fn page_body(&mut self, ui: &mut egui::Ui, events: &mut Vec<Event>) {
        let pages = Self::pages();
        let (_, page) = pages[self.page.min(pages.len() - 1)];
        if page == Page::DictionariesLink {
            ui.heading(tr("Sözlükler"));
            ui.add_space(6.0);
            ui.label(tr("Sözlük klasörleri, sözlük ve Sözlük Merkezi ayarları Sözlükler penceresinde, listeniz, kataloğunuz ve indirmelerinizle bir arada."));
            ui.add_space(6.0);
            if ui
                .button(tr("Sözlükler penceresinde aç (Ctrl+M)"))
                .clicked()
            {
                events.push(Event::Action(Advanced::DictionariesSettings));
            }
            return;
        }
        if page == Page::Shortcuts {
            ui.heading(tr(PAGE_SHORTCUTS));
            ui.add_space(6.0);
            egui::Grid::new("shortcuts")
                .striped(true)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    for (k, d) in SHORTCUTS {
                        ui.monospace(*k);
                        ui.label(tr(d));
                        ui.end_row();
                    }
                });
            return;
        }
        if page == Page::Advanced {
            ui.heading(tr(PAGE_ADVANCED));
            ui.add_space(6.0);
            let buttons: [(&str, &str, Advanced); 7] = [
                (
                    &tr("Veri klasörünü aç"),
                    &tr("Ayarlar, indeks ve geçmiş veritabanının bulunduğu klasör."),
                    Advanced::OpenData,
                ),
                (
                    &tr("Tüm indeksleri yeniden oluştur"),
                    &tr("Öneriler eksik veya hatalıysa."),
                    Advanced::Reindex,
                ),
                (
                    &tr("Güncellemeleri denetle"),
                    &tr("crates.io'daki en yeni sürümü sorar."),
                    Advanced::CheckUpdates,
                ),
                (&tr("Arama geçmişini temizle"), "", Advanced::ClearHistory),
                (&tr("Favorileri temizle"), "", Advanced::ClearFavorites),
                (
                    &tr("Ayarları dışa aktar…"),
                    &tr("settings.json biçiminde; Python sürümü de okur."),
                    Advanced::Export,
                ),
                (&tr("Ayarları içe aktar…"), "", Advanced::Import),
            ];
            egui::Grid::new("advanced")
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    for (label, help, a) in buttons {
                        if ui.button(label).clicked() {
                            events.push(Event::Action(a));
                        }
                        ui.weak(help);
                        ui.end_row();
                    }
                });
            ui.add_space(12.0);
            ui.weak(format!(
                "{} {}",
                tr("Ayar dosyası:"),
                self.draft
                    .path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ));
            return;
        }
        let Page::Section(i) = page else { return };
        let (sec, sec_title) = SECTIONS[i];
        ui.horizontal(|ui| {
            ui.heading(tr(sec_title));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(tr("Bu sayfayı sıfırla")).clicked() {
                    for s in SCHEMA
                        .iter()
                        .filter(|s| s.key.split('.').next() == Some(sec))
                    {
                        let _ = self.draft.reset(Some(s.key));
                    }
                }
            });
        });
        ui.add_space(6.0);
        let items: Vec<&Setting> = SCHEMA
            .iter()
            .filter(|s| s.key.split('.').next() == Some(sec))
            .filter(|s| !HIDDEN_KEYS.contains(&s.key))
            .collect();
        self.grid(ui, sec, &items);
    }

    fn search_body(&mut self, ui: &mut egui::Ui) {
        let q = self.filter.trim().to_lowercase();
        let mut any = false;
        for (sec, title) in SECTIONS {
            if PAGES_IN_DICTIONARIES_WINDOW.contains(sec) {
                continue; // in the Dictionaries window
            }
            let items: Vec<&Setting> = SCHEMA
                .iter()
                .filter(|s| s.key.split('.').next() == Some(*sec))
                .filter(|s| !HIDDEN_KEYS.contains(&s.key))
                .filter(|s| {
                    s.label.to_lowercase().contains(&q)
                        || s.key.to_lowercase().contains(&q)
                        || s.help.to_lowercase().contains(&q)
                        || tr(s.label).to_lowercase().contains(&q)
                        || tr(s.help).to_lowercase().contains(&q)
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            any = true;
            ui.label(RichText::new(tr(title)).strong());
            self.grid(ui, &format!("search-{}", sec), &items);
            ui.add_space(8.0);
        }
        if !any {
            ui.weak(tr("Eşleşen ayar yok."));
        }
    }

    /// Some pages as one form (the Dictionaries window's "Ayarlar" tab):
    /// a heading and the rows of each section, without the `skip` keys.
    pub fn form(&mut self, ui: &mut egui::Ui, sections: &[&str], skip: &[&str]) {
        for (sec, title) in SECTIONS {
            if !sections.contains(sec) {
                continue;
            }
            ui.label(RichText::new(tr(title)).strong().size(15.0));
            let items: Vec<&Setting> = SCHEMA
                .iter()
                .filter(|s| s.key.split('.').next() == Some(*sec))
                .filter(|s| !HIDDEN_KEYS.contains(&s.key) && !skip.contains(&s.key))
                .collect();
            self.grid(ui, &format!("form-{}", sec), &items);
            ui.add_space(10.0);
        }
    }

    /// Reset the rows of `sections` (but `skip`).
    pub fn reset_sections(&mut self, sections: &[&str], skip: &[&str]) {
        for s in SCHEMA.iter().filter(|s| {
            sections.contains(&s.key.split('.').next().unwrap_or("")) && !skip.contains(&s.key)
        }) {
            let _ = self.draft.reset(Some(s.key));
        }
    }

    /// Events asked by an embedded editor (e.g. "Şimdi tara").
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }

    fn grid(&mut self, ui: &mut egui::Ui, id: &str, items: &[&Setting]) {
        egui::Grid::new(("prefs-grid", id))
            .num_columns(3)
            .spacing([14.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                for s in items {
                    let r = ui.label(tr(s.label));
                    if !s.help.is_empty() {
                        r.on_hover_text(tr(s.help));
                    }
                    ui.horizontal(|ui| self.editor(ui, s));
                    let changed = differs(self.draft.get(s.key), &s.default.value());
                    if ui
                        .add_enabled(changed, egui::Button::new("↺").small())
                        .on_hover_text(tr("Varsayılana döndür"))
                        .clicked()
                    {
                        let _ = self.draft.reset(Some(s.key));
                        if s.key == "dictionaries.folders" {
                            let _ = self.draft.reset(Some("dictionaries.flat_folders"));
                        }
                    }
                    ui.end_row();
                }
            });
    }

    fn set(&mut self, key: &str, v: Value) {
        let _ = self.draft.set(key, v);
    }

    fn editor(&mut self, ui: &mut egui::Ui, s: &Setting) {
        let help_suffix = |ui: &mut egui::Ui| {
            if !s.help.is_empty() && s.help.chars().count() <= 24 {
                ui.weak(tr(s.help));
            }
        };
        if s.key == "dictionaries.folders" {
            self.folder_sources(ui);
            return;
        }
        match s.kind {
            Kind::Bool => {
                let mut v = self.draft.bool(s.key);
                if ui.checkbox(&mut v, "").changed() {
                    self.set(s.key, Value::Bool(v));
                }
                if s.help.chars().count() > 24 {
                    ui.weak(tr(s.help));
                }
            }
            Kind::Int => {
                let mut v = self.draft.int(s.key);
                let lo = s.min.unwrap_or(i64::MIN as f64) as i64;
                let hi = s.max.unwrap_or(i64::MAX as f64) as i64;
                if ui
                    .add(egui::DragValue::new(&mut v).range(lo..=hi))
                    .changed()
                {
                    self.set(s.key, Value::from(v));
                }
                help_suffix(ui);
            }
            Kind::Float => {
                let mut v = self.draft.float(s.key);
                let lo = s.min.unwrap_or(f64::MIN);
                let hi = s.max.unwrap_or(f64::MAX);
                let speed = if hi - lo > 20.0 { 0.5 } else { 0.01 };
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .range(lo..=hi)
                            .speed(speed)
                            .max_decimals(2),
                    )
                    .changed()
                {
                    self.set(s.key, Value::from((v * 100.0).round() / 100.0));
                }
                help_suffix(ui);
            }
            Kind::Str => {
                let mut v = self.draft.str(s.key);
                let font = s.key.ends_with("font_family");
                let hint = if font {
                    tr("varsayılan")
                } else {
                    String::new()
                };
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut v)
                            .hint_text(hint)
                            .desired_width(240.0),
                    )
                    .changed()
                {
                    self.set(s.key, Value::from(v));
                }
                if font && ui.button(tr("Dosya…")).clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter(tr("Yazı tipi"), &["ttf", "otf", "ttc"])
                        .pick_file()
                    {
                        self.set(s.key, Value::from(p.display().to_string()));
                    }
                }
            }
            Kind::Choice => {
                let cur = self.draft.str(s.key);
                let label = s
                    .choices
                    .iter()
                    .find(|c| c.0 == cur)
                    .map(|c| tr(c.1))
                    .unwrap_or_default();
                let mut picked = None;
                egui::ComboBox::from_id_salt(("choice", s.key))
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (v, l) in s.choices {
                            if ui.selectable_label(*v == cur, tr(l)).clicked() {
                                picked = Some(*v);
                            }
                        }
                    });
                if let Some(v) = picked {
                    self.set(s.key, Value::from(v));
                }
            }
            Kind::Color => {
                let cur = self.draft.str(s.key);
                let mut c = parse_hex(&cur).unwrap_or(Color32::GRAY);
                if egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut c,
                    egui::color_picker::Alpha::Opaque,
                )
                .changed()
                {
                    self.set(s.key, Value::from(hex(c)));
                }
                let mut t = cur.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut t).desired_width(80.0))
                    .changed()
                    && parse_hex(&t).is_some()
                {
                    self.set(s.key, Value::from(t));
                }
            }
            Kind::List => {
                let mut items = self.draft.list(s.key);
                let mut changed = false;
                ui.vertical(|ui| {
                    let mut remove = None;
                    for (i, it) in items.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::TextEdit::singleline(it).desired_width(300.0))
                                .changed()
                            {
                                changed = true;
                            }
                            if ui.small_button("✕").on_hover_text(tr("Kaldır")).clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        items.remove(i);
                        changed = true;
                    }
                    if items.is_empty() {
                        ui.weak(tr("(klasör yok)"));
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(tr("Klasör ekle…"))
                            .on_hover_text(tr(s.help))
                            .clicked()
                        {
                            if let Some(p) = rfd::FileDialog::new()
                                .set_title(tr("Sözlük klasörü"))
                                .pick_folder()
                            {
                                items.push(p.display().to_string());
                                changed = true;
                            }
                        }
                        if ui.button(tr("Satır ekle")).clicked() {
                            items.push(String::new());
                            changed = true;
                        }
                    });
                });
                if changed {
                    self.set(
                        s.key,
                        Value::Array(items.into_iter().map(Value::from).collect()),
                    );
                }
            }
        }
    }
}

impl PrefsWindow {
    /// The dictionary folders, each with its own "subfolders" box (like
    /// GoldenDict's Sources → Files).
    pub fn folder_sources(&mut self, ui: &mut egui::Ui) {
        let mut items = crate::query::folder_sources(&self.draft);
        let mut changed = false;
        ui.vertical(|ui| {
            let mut remove = None;
            egui::Grid::new("folder-sources")
                .num_columns(3)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    if !items.is_empty() {
                        ui.strong(tr("Klasör"));
                        ui.strong(tr("Alt klasörler"));
                        ui.label("");
                        ui.end_row();
                    }
                    for (i, (f, rec)) in items.iter_mut().enumerate() {
                        ui.label(f.as_str()).on_hover_text(f.as_str());
                        if ui.checkbox(rec, "").changed() {
                            changed = true;
                        }
                        if ui.small_button("✕").on_hover_text(tr("Kaldır")).clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
            if let Some(i) = remove {
                items.remove(i);
                changed = true;
            }
            if items.is_empty() {
                ui.weak(tr("(klasör yok)"));
            }
            ui.horizontal(|ui| {
                if ui.button(tr("Klasör ekle…")).clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title(tr("Sözlük klasörü"))
                        .pick_folder()
                    {
                        let p = crate::registry::norm_path(&p);
                        if !items.iter().any(|(f, _)| *f == p) {
                            items.push((p, self.draft.bool("dictionaries.recursive")));
                            changed = true;
                        }
                    }
                }
                if ui.button(tr("Şimdi tara")).clicked() {
                    self.pending.push(Event::Action(Advanced::Rescan));
                }
            });
        });
        if changed {
            crate::query::set_folder_sources(&mut self.draft, &items);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(
            parse_hex("#2563eb"),
            Some(Color32::from_rgb(0x25, 0x63, 0xeb))
        );
        assert_eq!(hex(Color32::from_rgb(1, 2, 255)), "#0102ff");
        assert!(parse_hex("red").is_none());
    }
}
