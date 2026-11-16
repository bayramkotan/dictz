//! `dictz` — the desktop dictionary (Rust edition).
//!
//! ```text
//! dictz                 open the window
//! dictz FILE...         add these dictionary files and open the window
//! dictz <command> ...   command line (`dictz help`) — same as the Python edition
//! ```

#![deny(unsafe_code)]
#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    #[cfg(feature = "update")]
    dictz::updater::cleanup_old_binary();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (files, popup): (Vec<PathBuf>, Option<String>) = if dictz::cli::is_cli_invocation(&args) {
        attach_console();
        match dictz::cli::run(args) {
            dictz::cli::Outcome::Exit(code) => std::process::exit(code),
            dictz::cli::Outcome::Gui { files, popup } => (files, popup),
        }
    } else {
        (args.iter().map(PathBuf::from).collect(), None)
    };
    run_gui(dictz::paths::user_data_dir(), files, popup);
}

/// The GUI build is a Windows-subsystem program; CLI output goes to the
/// parent console only when one is attached (Windows prints nothing
/// otherwise). Nothing to do on other systems.
#[cfg(windows)]
#[allow(unsafe_code)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call without pointers; failure (no parent console) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

#[cfg(feature = "gui")]
fn run_gui(data_dir: PathBuf, files: Vec<PathBuf>, popup: Option<String>) {
    if let Err(e) = dictz::gui::run(data_dir, files, popup) {
        eprintln!("dictz: {}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "gui"))]
fn run_gui(_data_dir: PathBuf, _files: Vec<PathBuf>, _popup: Option<String>) {
    eprintln!("dictz was built without the `gui` feature; use `dictz help` for the command line.");
    std::process::exit(2);
}
