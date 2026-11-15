//! Preferences of the Rust edition (`dictz-rs.ini` in the data folder).
//! Same keys and defaults as the Python edition's `Preferences`.

use std::path::{Path, PathBuf};

/// User preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    /// "auto", "light" or "dark".
    pub theme: String,
    /// Optional font file (.ttf/.otf) used for the interface and articles.
    pub font_family: String,
    /// Base font size in points.
    pub font_size: u32,
    /// Article zoom, 0.5–3.0.
    pub zoom_factor: f32,
    /// Delay before live search, in milliseconds.
    pub search_debounce_ms: u64,
    /// Sidebar visible.
    pub show_sidebar: bool,
    /// Sidebar width in pixels.
    pub sidebar_width: f32,
    /// Last search text.
    pub last_search: String,
    path: Option<PathBuf>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            font_family: String::new(),
            font_size: 14,
            zoom_factor: 1.0,
            search_debounce_ms: 150,
            show_sidebar: true,
            sidebar_width: 280.0,
            last_search: String::new(),
            path: None,
        }
    }
}

impl Preferences {
    /// Load from `path`; missing file or keys fall back to the defaults.
    pub fn load(path: &Path) -> Self {
        let mut p = Self {
            path: Some(path.to_path_buf()),
            ..Self::default()
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return p;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "theme" if ["auto", "light", "dark"].contains(&v) => p.theme = v.into(),
                "font_family" => p.font_family = v.into(),
                "font_size" => p.font_size = v.parse().unwrap_or(p.font_size).clamp(8, 32),
                "zoom_factor" => p.zoom_factor = v.parse().unwrap_or(p.zoom_factor).clamp(0.5, 3.0),
                "search_debounce_ms" => {
                    p.search_debounce_ms = v.parse().unwrap_or(p.search_debounce_ms).min(1000)
                }
                "show_sidebar" => p.show_sidebar = v == "true",
                "sidebar_width" => p.sidebar_width = v.parse().unwrap_or(p.sidebar_width),
                "last_search" => p.last_search = v.into(),
                _ => {}
            }
        }
        p
    }

    /// Write to the file this was loaded from (no-op for defaults).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let one_line = |s: &str| s.replace(['\n', '\r'], " ");
        std::fs::write(
            path,
            format!(
                "theme={}\nfont_family={}\nfont_size={}\nzoom_factor={}\nsearch_debounce_ms={}\nshow_sidebar={}\nsidebar_width={}\nlast_search={}\n",
                self.theme,
                one_line(&self.font_family),
                self.font_size,
                self.zoom_factor,
                self.search_debounce_ms,
                self.show_sidebar,
                self.sidebar_width,
                one_line(&self.last_search)
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let f = std::env::temp_dir().join(format!("dictz-prefs-{}.ini", std::process::id()));
        let _ = std::fs::remove_file(&f);
        let mut p = Preferences::load(&f);
        assert_eq!(p.font_size, 14);
        p.theme = "dark".into();
        p.zoom_factor = 1.5;
        p.save().unwrap();
        let q = Preferences::load(&f);
        assert_eq!(q.theme, "dark");
        assert_eq!(q.zoom_factor, 1.5);
    }
}
