//! Flags for the interface-language picker (PNG, from flag-icons, MIT —
//! `src/flags/LICENSE-flag-icons.txt`). Same files as the Python edition's
//! `dictz/resources/flags/`.

use std::collections::HashMap;

use eframe::egui::{self, TextureHandle};

/// The PNG of the flag shown for language `code`.
pub fn flag_png(code: &str) -> &'static [u8] {
    match code {
        "tr" => include_bytes!("../flags/tr.png"),
        "en" => include_bytes!("../flags/en.png"),
        "az" => include_bytes!("../flags/az.png"),
        "de" => include_bytes!("../flags/de.png"),
        "fr" => include_bytes!("../flags/fr.png"),
        "es" => include_bytes!("../flags/es.png"),
        "it" => include_bytes!("../flags/it.png"),
        "pt" => include_bytes!("../flags/pt.png"),
        "nl" => include_bytes!("../flags/nl.png"),
        "pl" => include_bytes!("../flags/pl.png"),
        "ru" => include_bytes!("../flags/ru.png"),
        "uk" => include_bytes!("../flags/uk.png"),
        "ar" => include_bytes!("../flags/ar.png"),
        "fa" => include_bytes!("../flags/fa.png"),
        "ur" => include_bytes!("../flags/ur.png"),
        "kk" => include_bytes!("../flags/kk.png"),
        "ky" => include_bytes!("../flags/ky.png"),
        "uz" => include_bytes!("../flags/uz.png"),
        "tk" => include_bytes!("../flags/tk.png"),
        "zh" => include_bytes!("../flags/zh.png"),
        "ja" => include_bytes!("../flags/ja.png"),
        "ko" => include_bytes!("../flags/ko.png"),
        "id" => include_bytes!("../flags/id.png"),
        "ms" => include_bytes!("../flags/ms.png"),
        "hi" => include_bytes!("../flags/hi.png"),
        _ => include_bytes!("../flags/en.png"),
    }
}

/// Flag textures, loaded on first use.
#[derive(Default)]
pub struct Flags {
    map: HashMap<String, TextureHandle>,
}

impl Flags {
    /// The texture of `code`'s flag.
    pub fn get(&mut self, ctx: &egui::Context, code: &str) -> Option<TextureHandle> {
        if let Some(t) = self.map.get(code) {
            return Some(t.clone());
        }
        let img = image::load_from_memory(flag_png(code)).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let tex = ctx.load_texture(
            format!("flag:{}", code),
            color,
            egui::TextureOptions::LINEAR,
        );
        self.map.insert(code.to_string(), tex.clone());
        Some(tex)
    }
}
