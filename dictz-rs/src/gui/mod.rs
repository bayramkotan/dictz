//! The dictz desktop application (egui).
//!
//! Same layout and features as the Python edition's Qt window: menu bar,
//! toolbar with back/forward, live search with suggestions, tabbed articles,
//! a sidebar with dictionaries / favorites / history, dictionary manager,
//! dictionary bar, “found in” panel, find in page, detailed preferences,
//! background loading, light/dark themes, zoom and keyboard shortcuts.

mod app;
mod audio;
mod center;
mod dictbadge;
mod fonts;
mod langflags;
mod prefs_window;
mod render;
mod services;
pub mod system;
mod themes;
mod uiicons;
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
pub fn run(
    data_dir: PathBuf,
    files: Vec<PathBuf>,
    popup: Option<String>,
    autostart: bool,
) -> Result<(), String> {
    let _ = std::fs::create_dir_all(&data_dir);
    let mut prefs = Preferences::load(&data_dir.join(SETTINGS_FILE));
    // held until the window closes (this function blocks until then)
    let mut _lock = None;
    if prefs.bool("general.single_instance") {
        match crate::instance::InstanceLock::acquire(&data_dir) {
            Some(l) => _lock = Some(l),
            None => {
                // another dictz (either edition) runs — maybe still starting or
                // busy: hand the request over when it answers, never open a
                // second window
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                while std::time::Instant::now() < deadline {
                    let ping = crate::instance::send(
                        &data_dir,
                        &serde_json::json!({"cmd": "ping"}),
                        std::time::Duration::from_secs(1),
                    );
                    if ping.is_some() && (autostart || hand_over(&data_dir, &files, &popup)) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
                return Ok(());
            }
        }
        // an older dictz without the lock may still run
        if autostart {
            // already running: a log-in start changes nothing
            if crate::instance::send(
                &data_dir,
                &serde_json::json!({"cmd": "ping"}),
                std::time::Duration::from_secs(1),
            )
            .is_some()
            {
                return Ok(());
            }
        } else if hand_over(&data_dir, &files, &popup) {
            return Ok(());
        }
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
            let mut a = app::DictzApp::new(cc, data_dir, prefs, files, popup);
            if autostart {
                a.start_in_tray();
            }
            Ok(Box::new(a))
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
