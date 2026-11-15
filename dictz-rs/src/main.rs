//! `dictz` — the desktop dictionary (Rust edition).
//!
//! ```text
//! dictz              launch the GUI
//! dictz --version    print the version and exit
//! dictz --selftest   headless check that dictionaries can be read; exit 0/1
//! dictz FILE...      launch the GUI and add these dictionary files
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("dictz {}", dictz::VERSION);
        return;
    }
    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(dictz::selftest::run());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("dictz {}\n\nUsage:\n  dictz              launch the GUI\n  dictz --version    print the version\n  dictz --selftest   headless self-test (exit 0 = OK)\n  dictz FILE...      add dictionary files and launch the GUI", dictz::VERSION);
        return;
    }
    let data_dir = dictz::paths::user_data_dir();
    let files: Vec<std::path::PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .collect();
    if !files.is_empty() {
        // register them in the shared index; the GUI restores them on start
        if let Ok(index) = dictz::index::IndexStore::open(&data_dir.join(dictz::paths::INDEX_DB)) {
            let mut qm = dictz::query::QueryManager::new(Some(index));
            qm.restore_from_index();
            for f in &files {
                if let Err(e) = qm.add_dictionary(f, false) {
                    eprintln!("dictz: {}: {}", f.display(), e);
                }
            }
        }
    }
    run_gui(data_dir);
}

#[cfg(feature = "gui")]
fn run_gui(data_dir: std::path::PathBuf) {
    if let Err(e) = dictz::gui::run(data_dir) {
        eprintln!("dictz: {}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "gui"))]
fn run_gui(_data_dir: std::path::PathBuf) {
    eprintln!(
        "dictz was built without the `gui` feature; only --version and --selftest are available."
    );
    std::process::exit(2);
}
