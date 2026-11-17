//! Program entry shared by the `dictz` binary and its short name `dct`.

use std::path::PathBuf;

/// Run dictz: a CLI command, or the window (with dictionary files).
pub fn main() {
    #[cfg(feature = "update")]
    crate::updater::cleanup_old_binary();
    let all: Vec<String> = std::env::args().skip(1).collect();
    // started at log-in: quietly, in the tray
    let autostart = all.iter().any(|a| a == crate::autostart::FLAG);
    let args: Vec<String> = all
        .into_iter()
        .filter(|a| a != crate::autostart::FLAG)
        .collect();
    let (files, popup): (Vec<PathBuf>, Option<String>) = if crate::cli::is_cli_invocation(&args) {
        attach_console();
        match crate::cli::run(args) {
            crate::cli::Outcome::Exit(code) => std::process::exit(code),
            crate::cli::Outcome::Gui { files, popup } => (files, popup),
        }
    } else {
        (args.iter().map(PathBuf::from).collect(), None)
    };
    run_gui(crate::paths::user_data_dir(), files, popup, autostart);
}

/// The GUI build is a Windows-subsystem program; CLI output goes to the
/// parent console only when one is attached. Nothing to do elsewhere.
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
fn run_gui(data_dir: PathBuf, files: Vec<PathBuf>, popup: Option<String>, autostart: bool) {
    if let Err(e) = crate::gui::run(data_dir, files, popup, autostart) {
        eprintln!("dictz: {}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "gui"))]
fn run_gui(_data_dir: PathBuf, _files: Vec<PathBuf>, _popup: Option<String>, _autostart: bool) {
    eprintln!("dictz was built without the `gui` feature; use `dictz help` for the command line.");
    std::process::exit(2);
}
