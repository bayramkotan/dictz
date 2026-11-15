//! Fonts: egui's built-in fonts plus system fonts that cover more scripts
//! (Turkish, Cyrillic, Greek, Arabic/Persian, CJK…), and a bold face.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// Name of the bold font family registered by [`install`].
pub const BOLD: &str = "dictz-bold";

fn candidates() -> (Vec<&'static str>, Vec<&'static str>) {
    if cfg!(windows) {
        (
            vec![
                "C:\\Windows\\Fonts\\segoeui.ttf",
                "C:\\Windows\\Fonts\\arial.ttf",
                "C:\\Windows\\Fonts\\tahoma.ttf",
                "C:\\Windows\\Fonts\\seguisym.ttf",
                "C:\\Windows\\Fonts\\msyh.ttc",
                "C:\\Windows\\Fonts\\malgun.ttf",
                "C:\\Windows\\Fonts\\YuGothR.ttc",
            ],
            vec![
                "C:\\Windows\\Fonts\\segoeuib.ttf",
                "C:\\Windows\\Fonts\\arialbd.ttf",
            ],
        )
    } else if cfg!(target_os = "macos") {
        (
            vec![
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                "/System/Library/Fonts/Geneva.ttf",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
            ],
            vec!["/System/Library/Fonts/Supplemental/Arial Bold.ttf"],
        )
    } else {
        (
            vec![
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            ],
            vec![
                "/usr/share/fonts/noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
            ],
        )
    }
}

fn load(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok().filter(|b| b.len() > 1024)
}

/// Register fonts. `user_font` (a .ttf/.otf path from the preferences)
/// becomes the primary face when it can be read.
pub fn install(ctx: &egui::Context, user_font: &str) {
    let mut defs = FontDefinitions::default();
    let (regular, bold) = candidates();
    let mut primary: Vec<String> = Vec::new();
    if !user_font.trim().is_empty() {
        if let Some(bytes) = load(user_font.trim()) {
            defs.font_data
                .insert("user".into(), Arc::new(FontData::from_owned(bytes)));
            primary.push("user".into());
        }
    }
    let mut fallbacks: Vec<String> = Vec::new();
    for (i, p) in regular.iter().enumerate() {
        if let Some(bytes) = load(p) {
            let name = format!("sys{}", i);
            defs.font_data
                .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
            fallbacks.push(name);
        }
    }
    let prop = defs.families.entry(FontFamily::Proportional).or_default();
    // user font → first system font (nicer than the default) → egui defaults → other system fonts
    let mut order: Vec<String> = primary.clone();
    if let Some(first) = fallbacks.first() {
        order.push(first.clone());
    }
    order.extend(prop.iter().cloned());
    order.extend(fallbacks.iter().skip(1).cloned());
    order.dedup();
    *prop = order.clone();
    let mono = defs.families.entry(FontFamily::Monospace).or_default();
    mono.extend(fallbacks.iter().cloned());

    let mut bold_family: Vec<String> = Vec::new();
    if primary.is_empty() {
        if let Some(bytes) = bold.iter().find_map(|p| load(p)) {
            defs.font_data
                .insert("sys-bold".into(), Arc::new(FontData::from_owned(bytes)));
            bold_family.push("sys-bold".into());
        }
    }
    bold_family.extend(order);
    defs.families
        .insert(FontFamily::Name(BOLD.into()), bold_family);
    ctx.set_fonts(defs);
}
