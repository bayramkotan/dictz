//! The dictz desktop application (egui).
//!
//! Same layout and features as the Python edition's Qt window: menu bar,
//! toolbar with back/forward, live search with suggestions, tabbed articles,
//! a sidebar with dictionaries / favorites / history, dictionary manager,
//! dictionary bar, “found in” panel, find in page, detailed preferences,
//! background loading, light/dark themes, zoom and keyboard shortcuts.

mod app;
mod audio;
mod dictbadge;
mod fonts;
mod prefs_window;
mod render;
mod services;
pub mod system;
mod window;

use std::path::PathBuf;

use crate::paths::PREFS_FILE;
use crate::settings::{Preferences, SETTINGS_FILE};

/// Single instance: pass the request to the running dictz. `true` = done.
fn hand_over(data_dir: &std::path::Path, files: &[PathBuf], popup: &Option<String>) -> bool {
    use crate::instance::send;
    use serde_json::json;
    use std::time::Duration;
    let t = Duration::from_secs(2);
    if send(data_dir, &json!({"cmd": "ping"}), Duration::from_secs(1)).is_none() {
        return false;
    }
    if !files.is_empty() {
        let files: Vec<String> = files
            .iter()
            .map(|f| {
                std::path::absolute(f)
                    .unwrap_or_else(|_| f.clone())
                    .display()
                    .to_string()
            })
            .collect();
        send(data_dir, &json!({"cmd": "open", "files": files}), t);
    }
    match popup {
        Some(w) => send(data_dir, &json!({"cmd": "popup", "word": w}), t),
        None => send(data_dir, &json!({"cmd": "show"}), t),
    };
    true
}

/// Start the GUI, adding `files` (dictionaries) first; `popup` opens the
/// scan popup with that word (empty = clipboard). Blocks until the window is
/// closed.
pub fn run(data_dir: PathBuf, files: Vec<PathBuf>, popup: Option<String>) -> Result<(), String> {
    let _ = std::fs::create_dir_all(&data_dir);
    let mut prefs = Preferences::load(&data_dir.join(SETTINGS_FILE));
    if prefs.bool("general.single_instance") && hand_over(&data_dir, &files, &popup) {
        return Ok(());
    }
    prefs.migrate_from_ini(&data_dir.join(PREFS_FILE));
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("dictz")
        .with_app_id("dictz")
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([640.0, 420.0])
        .with_drag_and_drop(true)
        .with_icon(app_icon());
    if prefs.bool("general.always_on_top") {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions {
        viewport,
        persist_window: prefs.bool("general.remember_window"),
        ..Default::default()
    };
    eframe::run_native(
        "dictz",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::DictzApp::new(
                cc, data_dir, prefs, files, popup,
            )))
        }),
    )
    .map_err(|e| e.to_string())
}

/// 64×64 window icon (the same pixels as the Python edition's).
fn app_icon() -> eframe::egui::IconData {
    eframe::egui::IconData {
        rgba: crate::icon::app_icon_rgba(64),
        width: 64,
        height: 64,
    }
}
