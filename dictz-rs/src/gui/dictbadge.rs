//! Dictionary icons and badges (egui side of [`crate::dictident`]).
//!
//! Every place that names a dictionary (article headers, the dictionary bar,
//! "found in", the sidebar list) shows the same picture: the dictionary's own
//! icon when it has one, otherwise its monogram on its own colour.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use eframe::egui::{self, Color32, FontFamily, FontId, Sense, Stroke, TextureHandle, Vec2};

use crate::dictident;
use crate::query::QueryManager;

use super::fonts;

/// How one dictionary is drawn.
pub struct DictLook {
    /// Dictionary name.
    pub name: String,
    /// The dictionary's own colour.
    pub color: Color32,
    /// One or two letters for the generated badge.
    pub monogram: String,
    /// Language pair (`EN`, `TR`), when known.
    pub langs: Option<(String, String)>,
    /// `StarDict`, `DSL`, `BGL`.
    pub fmt: String,
    /// Headword count.
    pub words: usize,
    /// The dictionary's real icon.
    pub texture: Option<TextureHandle>,
}

impl DictLook {
    /// Name, languages, format and size, one per line.
    pub fn tooltip(&self) -> String {
        let mut s = self.name.clone();
        if let Some((a, b)) = &self.langs {
            s.push_str(&format!("\n{} → {}", a, b));
        }
        s.push_str(&format!(
            "\n{} · {}",
            self.fmt,
            crate::i18n::tr_args("{n} kelime", &[("n", &super::app::group(self.words))])
        ));
        s
    }

    /// Text colour for chips: the colour itself, lightened on a dark theme.
    pub fn ink(&self, dark: bool) -> Color32 {
        if !dark {
            return self.color;
        }
        let mix = |a: u8| (a as f32 + (255.0 - a as f32) * 0.40).round() as u8;
        Color32::from_rgb(
            mix(self.color.r()),
            mix(self.color.g()),
            mix(self.color.b()),
        )
    }

    /// The header background: the colour faded into the panel.
    pub fn tint(&self, dark: bool) -> Color32 {
        let base = if dark {
            Color32::from_rgb(0x1b, 0x1f, 0x26)
        } else {
            Color32::WHITE
        };
        let t = if dark { 0.80 } else { 0.88 };
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color32::from_rgb(
            mix(self.color.r(), base.r()),
            mix(self.color.g(), base.g()),
            mix(self.color.b(), base.b()),
        )
    }
}

/// Cache of [`DictLook`]s, keyed by name and file.
#[derive(Default)]
pub struct DictLooks {
    map: HashMap<(String, PathBuf), Rc<DictLook>>,
}

impl DictLooks {
    /// How `name` is drawn (loads its icon the first time).
    pub fn get(&mut self, ctx: &egui::Context, qm: &QueryManager, name: &str) -> Rc<DictLook> {
        let meta = qm.dictionary(name).map(|d| d.metadata().clone());
        let path = meta
            .as_ref()
            .map(|m| m.file_path.clone())
            .unwrap_or_default();
        let key = (name.to_string(), path.clone());
        if let Some(l) = self.map.get(&key) {
            return l.clone();
        }
        let (src, dst) = meta
            .as_ref()
            .map(|m| (m.source_language.as_str(), m.target_language.as_str()))
            .unwrap_or(("", ""));
        let [r, g, b] = dictident::color_rgb(name);
        let texture = if path.as_os_str().is_empty() {
            None
        } else {
            // looked for once, then kept in dictionaries.json + DATA/icons
            let cached = qm
                .index
                .as_ref()
                .filter(|i| i.get_dictionary(name).ok().flatten().is_some());
            match cached {
                Some(i) => i
                    .icon_path(name)
                    .and_then(|p| std::fs::read(p).ok())
                    .and_then(|b| texture_from(ctx, name, &b)),
                None => load_texture(ctx, name, &path),
            }
        };
        let look = Rc::new(DictLook {
            name: name.to_string(),
            color: Color32::from_rgb(r, g, b),
            monogram: dictident::monogram(name),
            langs: dictident::language_pair(src, dst, name),
            fmt: meta
                .as_ref()
                .map(|m| dictident::format_label(&m.format))
                .unwrap_or_default(),
            words: meta.as_ref().map(|m| m.word_count).unwrap_or(0),
            texture,
        });
        self.map.insert(key, look.clone());
        look
    }
}

fn load_texture(ctx: &egui::Context, name: &str, path: &std::path::Path) -> Option<TextureHandle> {
    let (bytes, _ext) = dictident::icon_bytes(path)?;
    texture_from(ctx, name, &bytes)
}

fn texture_from(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?;
    // a 1-pixel "icon" is a placeholder, not a picture
    if img.width() < 8 || img.height() < 8 {
        return None;
    }
    let img = img
        .resize(48, 48, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(
        format!("dict-icon:{}", name),
        color,
        egui::TextureOptions::LINEAR,
    ))
}

/// Draw the badge (`size` × `size` points).
pub fn badge(ui: &mut egui::Ui, look: &DictLook, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter();
    if let Some(tex) = &look.texture {
        let ts = tex.size_vec2();
        let scale = (size / ts.x).min(size / ts.y);
        let r = egui::Rect::from_center_size(rect.center(), ts * scale);
        painter.image(
            tex.id(),
            r,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.rect_filled(rect, size * 0.24, look.color);
        painter.rect_stroke(
            rect,
            size * 0.24,
            Stroke::new(1.0_f32, Color32::from_black_alpha(30)),
            egui::StrokeKind::Inside,
        );
        let px = size
            * if look.monogram.chars().count() > 1 {
                0.46
            } else {
                0.56
            };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &look.monogram,
            FontId::new(px, FontFamily::Name(fonts::BOLD.into())),
            Color32::WHITE,
        );
    }
    resp
}

/// A small rounded label (`EN → TR`, `DSL`).
pub fn chip(ui: &mut egui::Ui, text: &str, color: Color32, size: f32) {
    egui::Frame::NONE
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.8)))
        .corner_radius(9.0)
        .inner_margin(egui::Margin::symmetric(7, 0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(size)
                    .color(color)
                    .family(FontFamily::Name(fonts::BOLD.into())),
            );
        });
}
