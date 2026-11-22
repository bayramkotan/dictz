//! The interface themes, shared with the Python edition.
//!
//! `themes.json` is a copy of `dictz/resources/themes.json` (made by
//! `tests/data/parity/make_samples.py`). Each theme has an id, a name (an
//! interface text, translated like the others), whether it is dark, its own
//! accent colour and the window colours; "auto" is not a theme, it follows
//! the system (light or dark). Mirrors `dictz/core/themes.py`.

use std::sync::OnceLock;

use eframe::egui::Color32;
use serde_json::Value;

/// The accent colour setting's default: while it is left there, the
/// theme's own accent is used.
pub const DEFAULT_ACCENT: &str = "#2563eb";

/// Window colours of a theme (the toolbar icons follow `text`; the
/// Python edition also reads `icon`).
#[derive(Debug, Clone)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub hover: Color32,
    pub header: Color32,
}

/// One theme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub accent: Color32,
    pub ui: Palette,
}

fn hex(v: &Value, k: &str) -> Color32 {
    v.get(k)
        .and_then(|x| x.as_str())
        .and_then(|s| Color32::from_hex(s).ok())
        .unwrap_or(Color32::GRAY)
}

/// Every theme, in the file's order.
pub fn all() -> &'static [Theme] {
    static T: OnceLock<Vec<Theme>> = OnceLock::new();
    T.get_or_init(|| {
        let raw: Value = serde_json::from_str(include_str!("themes.json")).unwrap_or(Value::Null);
        raw["themes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| {
                        let u = &t["ui"];
                        Theme {
                            id: t["id"].as_str().unwrap_or("").to_string(),
                            name: t["name"].as_str().unwrap_or("").to_string(),
                            dark: t["dark"].as_bool().unwrap_or(false),
                            accent: hex(t, "accent"),
                            ui: Palette {
                                bg: hex(u, "bg"),
                                panel: hex(u, "panel"),
                                border: hex(u, "border"),
                                text: hex(u, "text"),
                                muted: hex(u, "muted"),
                                hover: hex(u, "hover"),
                                header: hex(u, "header"),
                            },
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// "auto" → "light" / "dark"; an unknown id → "light".
pub fn resolve(id: &str, system_dark: bool) -> &'static Theme {
    let want = if id == "auto" {
        if system_dark {
            "dark"
        } else {
            "light"
        }
    } else {
        id
    };
    let ts = all();
    ts.iter()
        .find(|t| t.id == want)
        .or_else(|| ts.iter().find(|t| t.id == "light"))
        .unwrap_or(&ts[0])
}

/// The `appearance.theme` choices in menu order: auto, light themes, dark themes.
pub fn menu_order() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = vec![("auto", "Otomatik (sistem)")];
    for dark in [false, true] {
        v.extend(
            all()
                .iter()
                .filter(|t| t.dark == dark)
                .map(|t| (t.id.as_str(), t.name.as_str())),
        );
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_choices_as_the_settings_schema() {
        let schema = crate::settings::SCHEMA
            .iter()
            .find(|s| s.key == "appearance.theme")
            .unwrap();
        assert_eq!(menu_order(), schema.choices.to_vec());
        assert!(all().len() >= 20);
        assert_eq!(resolve("auto", true).id, "dark");
        assert_eq!(resolve("nope", false).id, "light");
        assert_eq!(
            resolve("nord", false).ui.bg,
            Color32::from_hex("#2e3440").unwrap()
        );
    }
}
