//! `dictz` — the desktop dictionary (Rust edition). `dct` is the same program.
//!
//! ```text
//! dictz                 open the window
//! dictz FILE...         add these dictionary files and open the window
//! dictz <command> ...   command line (`dictz help`) — same as the Python edition
//! dictz -V              version (also -v, --version, --Version, -version)
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

fn main() {
    dictz::entry::main();
}
