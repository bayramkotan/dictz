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

fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut v: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(windows) {
        v.push("C:\\Windows\\Fonts".into());
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            v.push(std::path::PathBuf::from(l).join("Microsoft\\Windows\\Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        v.extend(["/System/Library/Fonts".into(), "/Library/Fonts".into()]);
        if let Some(h) = std::env::var_os("HOME") {
            v.push(std::path::PathBuf::from(h).join("Library/Fonts"));
        }
    } else {
        v.extend(["/usr/share/fonts".into(), "/usr/local/share/fonts".into()]);
        if let Some(h) = std::env::var_os("HOME") {
            let h = std::path::PathBuf::from(h);
            v.push(h.join(".local/share/fonts"));
            v.push(h.join(".fonts"));
        }
    }
    v
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A font given as a file path or as a family name ("DejaVu Serif"):
/// the name is matched against font file names in the system font folders,
/// preferring the regular face.
pub fn resolve(spec: &str) -> Option<Vec<u8>> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    if std::path::Path::new(spec).is_file() {
        return load(spec);
    }
    let want = norm(spec);
    let mut best: Option<(usize, std::path::PathBuf)> = None;
    let mut stack = font_dirs();
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            seen += 1;
            if seen > 20_000 {
                break;
            }
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                continue;
            }
            let stem = norm(
                &p.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            if !stem.starts_with(&want) {
                continue;
            }
            let rest = &stem[want.len()..];
            let score = if rest.is_empty() || rest == "regular" || rest == "book" {
                0
            } else {
                rest.len() + 1
            };
            if best.as_ref().map(|b| score < b.0).unwrap_or(true) {
                best = Some((score, p));
            }
        }
    }
    best.and_then(|(_, p)| load(&p.to_string_lossy()))
}

/// Name of the article font family registered by [`install`].
pub const ARTICLE: &str = "dictz-article";

/// Register fonts. `ui_font` / `article_font` (a family name or a
/// .ttf/.otf path from the preferences) become the primary faces when found.
pub fn install(ctx: &egui::Context, ui_font: &str, article_font: &str) {
    let mut defs = FontDefinitions::default();
    let (regular, bold) = candidates();
    let mut primary: Vec<String> = Vec::new();
    if let Some(bytes) = resolve(ui_font) {
        defs.font_data
            .insert("user".into(), Arc::new(FontData::from_owned(bytes)));
        primary.push("user".into());
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

    let mut article = order.clone();
    if let Some(bytes) = resolve(article_font) {
        defs.font_data
            .insert("article".into(), Arc::new(FontData::from_owned(bytes)));
        article.insert(0, "article".into());
    }
    defs.families
        .insert(FontFamily::Name(ARTICLE.into()), article);

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
