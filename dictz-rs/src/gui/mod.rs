//! The dictz desktop application (egui).
//!
//! Same layout and features as the Python edition's Qt window: menu bar,
//! toolbar with back/forward, live search with suggestions, tabbed articles,
//! a sidebar with dictionaries / favorites / history, dictionary manager,
//! preferences, light/dark themes, zoom and keyboard shortcuts.

mod app;
mod audio;
mod fonts;
mod render;
pub mod system;

use std::path::PathBuf;

/// Start the GUI. Blocks until the window is closed.
pub fn run(data_dir: PathBuf) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("dictz")
            .with_app_id("dictz")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([640.0, 420.0])
            .with_icon(app_icon()),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "dictz",
        options,
        Box::new(move |cc| Ok(Box::new(app::DictzApp::new(cc, data_dir)))),
    )
    .map_err(|e| e.to_string())
}

/// 64×64 window icon drawn in code: a blue rounded square with an open book.
fn app_icon() -> eframe::egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let inside_rounded = |x: f32, y: f32| {
        let r = 12.0;
        let cx = x.clamp(r, N as f32 - r);
        let cy = y.clamp(r, N as f32 - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let i = (y * N + x) * 4;
            if !inside_rounded(fx, fy) {
                continue;
            }
            let mut c = [0x25u8, 0x63, 0xEB, 0xFF];
            // two pages of an open book
            let page = (14..=30).contains(&x) || (33..=49).contains(&x);
            let top = 18
                + if x < 32 {
                    (30 - x.min(30)) / 6
                } else {
                    (x.max(33) - 33) / 6
                };
            if page && (top..=46).contains(&y) {
                c = [0xFF, 0xFF, 0xFF, 0xFF];
                let line_row = y > top + 4 && (y - top) % 6 == 0 && y < 44;
                let line_col = (17..=27).contains(&x) || (36..=46).contains(&x);
                if line_row && line_col {
                    c = [0x93, 0xB4, 0xF5, 0xFF];
                }
            }
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: N as u32,
        height: N as u32,
    }
}
