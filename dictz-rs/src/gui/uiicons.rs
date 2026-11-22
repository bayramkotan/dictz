//! dictz's own interface icons (same pictures as the Python edition:
//! `dictz/resources/icons/*.svg`, drawn white at 48 px into `src/uiicons/` by
//! `tests/data/parity/make_samples.py`). They are tinted with the theme's
//! text colour when drawn.

use std::collections::HashMap;
use std::sync::Mutex;

use eframe::egui::{self, Color32, TextureHandle, TextureOptions};

const ICONS: &[(&str, &[u8])] = &[
    ("all", include_bytes!("../uiicons/all.png")),
    ("back", include_bytes!("../uiicons/back.png")),
    ("center", include_bytes!("../uiicons/center.png")),
    ("close", include_bytes!("../uiicons/close.png")),
    ("copy", include_bytes!("../uiicons/copy.png")),
    ("dict", include_bytes!("../uiicons/dict.png")),
    ("down", include_bytes!("../uiicons/down.png")),
    ("download", include_bytes!("../uiicons/download.png")),
    ("folder", include_bytes!("../uiicons/folder.png")),
    ("forward", include_bytes!("../uiicons/forward.png")),
    ("fullscreen", include_bytes!("../uiicons/fullscreen.png")),
    ("help", include_bytes!("../uiicons/help.png")),
    ("history", include_bytes!("../uiicons/history.png")),
    ("info", include_bytes!("../uiicons/info.png")),
    ("license", include_bytes!("../uiicons/license.png")),
    ("link", include_bytes!("../uiicons/link.png")),
    ("menu", include_bytes!("../uiicons/menu.png")),
    ("new", include_bytes!("../uiicons/new.png")),
    ("open", include_bytes!("../uiicons/open.png")),
    ("random", include_bytes!("../uiicons/random.png")),
    ("refresh", include_bytes!("../uiicons/refresh.png")),
    ("remove", include_bytes!("../uiicons/remove.png")),
    ("save", include_bytes!("../uiicons/save.png")),
    ("scan", include_bytes!("../uiicons/scan.png")),
    ("search", include_bytes!("../uiicons/search.png")),
    ("server", include_bytes!("../uiicons/server.png")),
    ("settings", include_bytes!("../uiicons/settings.png")),
    ("sound", include_bytes!("../uiicons/sound.png")),
    ("star", include_bytes!("../uiicons/star.png")),
    ("star-filled", include_bytes!("../uiicons/star-filled.png")),
    ("theme", include_bytes!("../uiicons/theme.png")),
    ("up", include_bytes!("../uiicons/up.png")),
    ("upload", include_bytes!("../uiicons/upload.png")),
    ("zoom-in", include_bytes!("../uiicons/zoom-in.png")),
    ("zoom-out", include_bytes!("../uiicons/zoom-out.png")),
    ("zoom-reset", include_bytes!("../uiicons/zoom-reset.png")),
];

static CACHE: Mutex<Option<HashMap<&'static str, TextureHandle>>> = Mutex::new(None);

/// Names of all icons.
#[cfg(test)]
pub fn names() -> Vec<&'static str> {
    ICONS.iter().map(|(n, _)| *n).collect()
}

/// The icon's texture (loaded once).
pub fn texture(ctx: &egui::Context, name: &str) -> Option<TextureHandle> {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(t) = map.get(name) {
        return Some(t.clone());
    }
    let (key, bytes) = ICONS.iter().find(|(n, _)| *n == name)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let tex = ctx.load_texture(format!("uiicon-{}", key), color, TextureOptions::LINEAR);
    map.insert(key, tex.clone());
    Some(tex)
}

/// The icon as an `egui::Image` of `size` points in `tint`.
pub fn image(
    ctx: &egui::Context,
    name: &str,
    size: f32,
    tint: Color32,
) -> Option<egui::Image<'static>> {
    let tex = texture(ctx, name)?;
    Some(
        egui::Image::new(egui::load::SizedTexture::new(
            tex.id(),
            egui::vec2(size, size),
        ))
        .fit_to_exact_size(egui::vec2(size, size))
        .tint(tint),
    )
}

/// Make the buttons in `ui` flat (no fill or border until hovered).
pub fn flat(ui: &mut egui::Ui) {
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = egui::Stroke::NONE;
}

/// A toolbar button showing only the icon (the text is the tooltip); call
/// [`flat`] on the toolbar first.
pub fn button(
    ui: &mut egui::Ui,
    name: &str,
    tip: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let tint = if enabled {
        ui.visuals().widgets.inactive.fg_stroke.color
    } else {
        ui.visuals().weak_text_color().gamma_multiply(0.5)
    };
    let size = ui.text_style_height(&egui::TextStyle::Body).max(14.0) + 3.0;
    let r = match image(ui.ctx(), name, size, tint) {
        Some(img) => ui.add_enabled(
            enabled,
            egui::Button::image(img)
                .selected(selected)
                .min_size(egui::vec2(size + 8.0, size + 8.0)),
        ),
        None => ui.add_enabled(enabled, egui::Button::new(name).selected(selected)),
    };
    r.on_hover_text(tip)
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_icons_as_the_python_edition() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dictz/resources/icons");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return; // the crate alone (crates.io) has no Python tree
        };
        let mut svg: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .collect();
        svg.sort();
        let mut png: Vec<String> = super::names().into_iter().map(str::to_string).collect();
        png.sort();
        assert_eq!(png, svg, "run tests/data/parity/make_samples.py");
    }
}
